//! 本机账号 session 的请求守卫：公开绑定（--host 非 127.0.0.1）时唯一的访问门槛。
//! session 本身存在 sessions.rs；普通用户每个请求还要过一遍 login_policy（白名单、环境绑定）。

use super::accounts::{self, Role};
use super::login_policy::{self, ClientInfo};
use super::policy_store;
use super::roles::{CurrentUser, admin_only_route};
use super::state::WebState;
use crate::*;

use std::sync::Arc;

use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};

pub(crate) const SESSION_COOKIE: &str = "qd_session";

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

/// 终端 iframe、静态资源和 WebSocket 由浏览器直接发起，带不上前端加的设备头。
fn browser_direct(path: &str) -> bool {
    path == "/terminal" || path.starts_with("/static/") || path.starts_with("/ws/")
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
    let client = login_policy::client_of(&request);
    let user = match current_user(&state, &token, &client, !browser_direct(&path)).await {
        Ok(user) => user,
        Err(message) => return error_response(StatusCode::UNAUTHORIZED, &message),
    };
    if admin_only_route(request.method(), &path) && !user.is_admin() {
        return error_response(StatusCode::FORBIDDEN, "需要管理员权限");
    }
    request.extensions_mut().insert(user);
    next.run(request).await
}

/// session 有效、账号仍存在、（普通用户）登录限制仍满足，才算登录；
/// 每次都重新读账号和策略，角色变更、删号、改白名单立即生效。
/// 失败时返回给前端显示的原因；违反策略的 session 会被直接作废。
pub(crate) async fn current_user(
    state: &WebState,
    token: &str,
    client: &ClientInfo,
    with_device: bool,
) -> Result<CurrentUser, String> {
    let expired = || "未登录，请先在网页里登录".to_owned();
    let Some(session) = state.sessions.touch(token).await else {
        return Err(state.sessions.notice(token).await.unwrap_or_else(expired));
    };
    let name = session.user.clone();
    let loaded = tokio::task::spawn_blocking(move || -> Result<_, super::state::WebError> {
        let Some(account) = accounts::get(&name)? else { return Ok(None) };
        let policy = policy_store::effective(&account)?;
        Ok(Some((account, policy)))
    })
    .await;
    let Ok(Ok(Some((account, policy)))) = loaded else {
        return Err(expired());
    };
    if let (Role::User, Some(policy)) = (account.role, policy) {
        if let Err(reason) = login_policy::check_request(&policy, &session.client, client, with_device) {
            state.sessions.revoke(token, Some(&reason)).await;
            return Err(reason);
        }
    }
    Ok(CurrentUser {
        name: account.username,
        role: account.role,
    })
}
