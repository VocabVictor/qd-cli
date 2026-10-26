//! 本机账号 session：公开绑定（--host 非 127.0.0.1）时唯一的访问门槛。
//! 纯内存 token -> (username, 过期时间)，`qd web` 进程重启即全部失效，不落盘。

use super::accounts;
use super::roles::{CurrentUser, admin_only_route};
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

fn same_origin(headers: &axum::http::HeaderMap) -> bool {
    let Some(origin) = headers.get(axum::http::header::ORIGIN).and_then(|v| v.to_str().ok()) else {
        return true;
    };
    let Some(host) = headers.get(axum::http::header::HOST).and_then(|v| v.to_str().ok()) else {
        return false;
    };
    origin
        .split_once("://")
        .map(|(_, rest)| rest.trim_end_matches('/'))
        .is_some_and(|origin_host| origin_host.eq_ignore_ascii_case(host))
}

fn error_response(status: StatusCode, message: &str) -> Response {
    (status, Json(json!({"ok": false, "error": message}))).into_response()
}

/// 公开绑定（--host 非 127.0.0.1）时的唯一防线：没有它，任何能连到这个端口的人
/// 都能用本机已保存的登录态操作 GPU 平台（看/删作业、进 pod 执行命令等）。
/// 只放行登录页本身；其余路由必须带有效 session，并把 CurrentUser 放进 extensions
/// 给各 handler 做细粒度权限判断。整条只给管理员的路由在这里直接拦。
pub(crate) async fn session_guard(
    axum::extract::State(state): axum::extract::State<Arc<WebState>>,
    mut request: axum::extract::Request,
    next: axum::middleware::Next,
) -> Response {
    if !state.public {
        request.extensions_mut().insert(CurrentUser::local());
        return next.run(request).await;
    }
    // 跨站写请求：Origin 存在且与 Host 不一致就拒（SameSite=Lax 之外的第二道 CSRF 防线）。
    let is_write = !matches!(
        request.method(),
        &axum::http::Method::GET | &axum::http::Method::HEAD | &axum::http::Method::OPTIONS
    );
    if is_write && !same_origin(request.headers()) {
        return error_response(StatusCode::FORBIDDEN, "跨站请求被拒绝");
    }
    // `/` 是内嵌的单文件前端，`/api/auth/*` 是登录注册接口。`/static/*`、`/terminal`、
    // `/ws/*` 是终端反向代理，必须登录后才能用。
    let path = request.uri().path().to_owned();
    if path == "/" || path.starts_with("/api/auth/") {
        return next.run(request).await;
    }
    let Some(token) = cookie_value(request.headers(), SESSION_COOKIE).map(str::to_owned) else {
        return error_response(StatusCode::UNAUTHORIZED, "未登录，请先在网页里登录");
    };
    let Some(user) = current_user(&state, &token).await else {
        return error_response(StatusCode::UNAUTHORIZED, "未登录，请先在网页里登录");
    };
    if admin_only_route(request.method(), &path) && !user.is_admin() {
        return error_response(StatusCode::FORBIDDEN, "需要管理员权限");
    }
    request.extensions_mut().insert(user);
    next.run(request).await
}

/// session 有效且账号仍存在才算登录；每次都重新读账号，角色变更/删除立即生效。
pub(crate) async fn current_user(state: &WebState, token: &str) -> Option<CurrentUser> {
    let name = state.session_username(token).await?;
    let account = tokio::task::spawn_blocking(move || accounts::get(&name))
        .await
        .ok()?
        .ok()??;
    Some(CurrentUser {
        name: account.username,
        role: account.role,
    })
}
