//! 终端反向代理：平台的网页终端用自签证书、且地址常指向公网 IP，
//! 浏览器无法在 iframe 里加载。这里由 qd 自己转发页面、静态资源和 WebSocket，
//! 让终端与 qd web 同源（http://127.0.0.1:port），既没有证书问题也不受浏览器代理影响。

use super::roles::CurrentUser;
use super::state::{TerminalGrant, WebState};

use axum::{
    body::Body,
    extract::{
        Extension, State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    http::{StatusCode, Uri, header},
    response::{IntoResponse, Response},
};
use futures_util::{SinkExt, StreamExt};
use std::sync::Arc;

/// 记录某个用户被授权的终端：上游 host:port，以及该 pod 的 WebSocket 路径前缀
/// （/ws/{namespace}/{pod}/{container}）。代理只按调用者自己的授权转发。
pub(crate) async fn grant_terminal(state: &WebState, user: &str, terminal_url: &str) {
    let Ok(url) = reqwest::Url::parse(terminal_url) else { return };
    let Some(host) = url.host_str() else { return };
    let authority = match url.port() {
        Some(port) => format!("{host}:{port}"),
        None => host.to_owned(),
    };
    let param = |name: &str| {
        url.query_pairs()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.into_owned())
    };
    let ws_prefix = match (param("namespace"), param("pod"), param("container")) {
        (Some(ns), Some(pod), Some(container)) => Some(format!("/ws/{ns}/{pod}/{container}/")),
        _ => None,
    };
    state
        .terminal_grants
        .write()
        .await
        .insert(user.to_owned(), TerminalGrant { secure: url.scheme() != "http", authority, ws_prefix });
}

async fn grant_of(state: &WebState, user: &str) -> Option<TerminalGrant> {
    state.terminal_grants.read().await.get(user).cloned()
}

fn forbid(msg: &str) -> Response {
    (StatusCode::FORBIDDEN, msg.to_owned()).into_response()
}

fn deny(msg: &str) -> Response {
    (StatusCode::BAD_GATEWAY, msg.to_owned()).into_response()
}

/// 转发终端页面与静态资源（GET）。
pub(crate) async fn proxy_http(
    State(state): State<Arc<WebState>>,
    Extension(user): Extension<CurrentUser>,
    uri: Uri,
) -> Response {
    let Some(TerminalGrant { authority, secure, .. }) = grant_of(&state, &user.name).await else {
        return deny("尚未解析终端地址，请先在页面上打开某个作业的终端");
    };
    let path_and_query = uri
        .path_and_query()
        .map(|value| value.as_str())
        .unwrap_or("/");
    let target = format!("{}://{authority}{path_and_query}", if secure { "https" } else { "http" });

    let config = state.config().await;
    let client = match crate::api::http_client_builder(&config).and_then(|builder| {
        builder
            .danger_accept_invalid_certs(config.insecure_tls)
            .build()
            .map_err(Into::into)
    }) {
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
    Extension(user): Extension<CurrentUser>,
    uri: Uri,
) -> Response {
    let Some(grant) = grant_of(&state, &user.name).await else {
        return deny("尚未解析终端地址");
    };
    let path_and_query = uri
        .path_and_query()
        .map(|value| value.as_str())
        .unwrap_or("/")
        .to_owned();
    // 普通用户只能连到刚被授权的那个 pod；拼别的 namespace/pod 路径直接拒绝。
    if !user.is_admin() {
        let allowed = grant
            .ws_prefix
            .as_deref()
            .is_some_and(|prefix| uri.path().starts_with(prefix) && !uri.path().contains(".."));
        if !allowed {
            return forbid("只能连接自己作业的终端");
        }
    }
    let authority = grant.authority;
    let ws_scheme = if grant.secure { "wss" } else { "ws" };
    let target = format!("{ws_scheme}://{authority}{path_and_query}");
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
