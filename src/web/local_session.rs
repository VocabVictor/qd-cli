//! 本机账号 session：公开绑定（--host 非 127.0.0.1）时唯一的访问门槛。
//! 纯内存 token -> (username, 过期时间)，`qd web` 进程重启即全部失效，不落盘。

use super::state::WebState;
use crate::*;

use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use rand::RngCore;

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};

const SESSION_TTL: Duration = Duration::from_secs(7 * 24 * 60 * 60);
pub(crate) const SESSION_COOKIE: &str = "qd_session";

impl WebState {
    /// 签发一个新 session token，供登录/注册成功后写进 Set-Cookie。
    pub(crate) async fn create_session(&self, username: &str) -> String {
        let mut bytes = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut bytes);
        let token = URL_SAFE_NO_PAD.encode(bytes);
        self.insert_session(token.clone(), username.to_owned(), Instant::now() + SESSION_TTL)
            .await;
        token
    }

    /// token 有效且未过期则返回对应用户名；顺手清掉已过期的记录。
    pub(crate) async fn session_username(&self, token: &str) -> Option<String> {
        self.lookup_session(token).await
    }

    pub(crate) async fn remove_session(&self, token: &str) {
        self.drop_session(token).await;
    }
}

pub(crate) fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

pub(crate) fn cookie_value<'a>(headers: &'a axum::http::HeaderMap, name: &str) -> Option<&'a str> {
    headers
        .get(axum::http::header::COOKIE)
        .and_then(|value| value.to_str().ok())
        .and_then(|cookies| {
            cookies.split(';').find_map(|pair| {
                let (key, value) = pair.trim().split_once('=')?;
                (key == name).then_some(value)
            })
        })
}

fn unauthenticated_response() -> Response {
    (
        StatusCode::UNAUTHORIZED,
        Json(json!({"ok": false, "error": "未登录，请先在网页里登录"})),
    )
        .into_response()
}

/// 公开绑定（--host 非 127.0.0.1）时的唯一防线：没有它，任何能连到这个端口的人
/// 都能用本机已保存的登录态操作 GPU 平台（看/删作业、进 pod 执行命令等）。
/// `/api/auth/*`（登录/注册本身）和静态页面放行，其余路由必须带有效的
/// qd_session Cookie。
pub(crate) async fn session_guard(
    axum::extract::State(state): axum::extract::State<Arc<WebState>>,
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> Response {
    if !state.public {
        return next.run(request).await;
    }
    let path = request.uri().path();
    if path == "/" || path.starts_with("/static/") || path.starts_with("/api/auth/") {
        return next.run(request).await;
    }
    let token = cookie_value(request.headers(), SESSION_COOKIE).map(str::to_owned);
    let authenticated = match token {
        Some(token) => state.session_username(&token).await.is_some(),
        None => false,
    };
    if !authenticated {
        return unauthenticated_response();
    }
    next.run(request).await
}
