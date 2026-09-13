//! 终端反向代理：平台的网页终端用自签证书、且地址常指向公网 IP，
//! 浏览器无法在 iframe 里加载。这里由 qd 自己转发页面、静态资源和 WebSocket，
//! 让终端与 qd web 同源（http://127.0.0.1:port），既没有证书问题也不受浏览器代理影响。

use super::state::WebState;

use axum::{
    body::Body,
    extract::{
        State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    http::{StatusCode, Uri, header},
    response::{IntoResponse, Response},
};
use futures_util::{SinkExt, StreamExt};
use std::sync::Arc;

/// 记录终端上游 `host:port`（由 /api/terminal-url 解析出真实地址时写入）。
pub(crate) async fn remember_upstream(state: &WebState, terminal_url: &str) {
    if let Ok(url) = reqwest::Url::parse(terminal_url)
        && let Some(host) = url.host_str()
    {
        let authority = match url.port() {
            Some(port) => format!("{host}:{port}"),
            None => host.to_owned(),
        };
        *state.terminal_upstream.write().await = Some(authority);
    }
}

async fn upstream_of(state: &WebState) -> Option<String> {
    state.terminal_upstream.read().await.clone()
}

fn deny(msg: &str) -> Response {
    (StatusCode::BAD_GATEWAY, msg.to_owned()).into_response()
}

/// 转发终端页面与静态资源（GET）。
pub(crate) async fn proxy_http(State(state): State<Arc<WebState>>, uri: Uri) -> Response {
    let Some(authority) = upstream_of(&state).await else {
        return deny("尚未解析终端地址，请先在页面上打开某个作业的终端");
    };
    let path_and_query = uri
        .path_and_query()
        .map(|value| value.as_str())
        .unwrap_or("/");
    let target = format!("https://{authority}{path_and_query}");

    let insecure = state.config().await.insecure_tls;
    let client = match reqwest::Client::builder()
        .danger_accept_invalid_certs(insecure)
        .build()
    {
        Ok(client) => client,
        Err(err) => return deny(&format!("构建代理客户端失败: {err}")),
    };
    let response = match client.get(&target).send().await {
        Ok(response) => response,
        Err(err) => return deny(&format!("终端上游请求失败: {err}")),
    };

    let status = response.status();
    let content_type = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("application/octet-stream")
        .to_owned();
    let bytes = match response.bytes().await {
        Ok(bytes) => bytes,
        Err(err) => return deny(&format!("读取终端上游响应失败: {err}")),
    };

    // terminal.js 把 WebSocket 协议硬编码成 wss://，而 qd web 是 http，
    // 改成按当前页面协议选择，其余内容原样透传。
    let body = if path_and_query.starts_with("/static/terminal.js") {
        Body::from(
            String::from_utf8_lossy(&bytes).replace(
                "\"wss://\"",
                "(document.location.protocol === \"https:\" ? \"wss://\" : \"ws://\")",
            ),
        )
    } else {
        Body::from(bytes)
    };

    (
        status,
        [
            (header::CONTENT_TYPE, content_type),
            (header::CACHE_CONTROL, "no-store".to_owned()),
        ],
        body,
    )
        .into_response()
}

/// 转发终端 WebSocket（/ws/...）。
pub(crate) async fn proxy_ws(
    upgrade: WebSocketUpgrade,
    State(state): State<Arc<WebState>>,
    uri: Uri,
) -> Response {
    let Some(authority) = upstream_of(&state).await else {
        return deny("尚未解析终端地址");
    };
    let path_and_query = uri
        .path_and_query()
        .map(|value| value.as_str())
        .unwrap_or("/")
        .to_owned();
    let target = format!("wss://{authority}{path_and_query}");
    let insecure = state.config().await.insecure_tls;
    upgrade.on_upgrade(move |socket| pump(socket, target, insecure))
}

async fn pump(client: WebSocket, target: String, insecure: bool) {
    let connector = match native_tls::TlsConnector::builder()
        .danger_accept_invalid_certs(insecure)
        .danger_accept_invalid_hostnames(insecure)
        .build()
    {
        Ok(connector) => connector,
        Err(_) => return,
    };
    let upstream = tokio_tungstenite::connect_async_tls_with_config(
        &target,
        None,
        false,
        Some(tokio_tungstenite::Connector::NativeTls(connector)),
    )
    .await;
    let Ok((upstream, _)) = upstream else { return };

    let (mut up_tx, mut up_rx) = upstream.split();
    let (mut cl_tx, mut cl_rx) = client.split();

    // 浏览器 -> 平台
    let to_upstream = async {
        while let Some(Ok(message)) = cl_rx.next().await {
            let forwarded = match message {
                Message::Text(text) => {
                    tokio_tungstenite::tungstenite::Message::Text(text.as_str().into())
                }
                Message::Binary(bytes) => {
                    tokio_tungstenite::tungstenite::Message::Binary(bytes)
                }
                Message::Close(_) => break,
                Message::Ping(bytes) => {
                    tokio_tungstenite::tungstenite::Message::Ping(bytes)
                }
                Message::Pong(bytes) => {
                    tokio_tungstenite::tungstenite::Message::Pong(bytes)
                }
            };
            if up_tx.send(forwarded).await.is_err() {
                break;
            }
        }
        let _ = up_tx.close().await;
    };

    // 平台 -> 浏览器
    let to_client = async {
        while let Some(Ok(message)) = up_rx.next().await {
            let forwarded = match message {
                tokio_tungstenite::tungstenite::Message::Text(text) => {
                    Message::Text(text.as_str().into())
                }
                tokio_tungstenite::tungstenite::Message::Binary(bytes) => {
                    Message::Binary(bytes)
                }
                tokio_tungstenite::tungstenite::Message::Close(_) => break,
                tokio_tungstenite::tungstenite::Message::Ping(bytes) => Message::Ping(bytes),
                tokio_tungstenite::tungstenite::Message::Pong(bytes) => Message::Pong(bytes),
                tokio_tungstenite::tungstenite::Message::Frame(_) => continue,
            };
            if cl_tx.send(forwarded).await.is_err() {
                break;
            }
        }
        let _ = cl_tx.close().await;
    };

    tokio::join!(to_upstream, to_client);
}
