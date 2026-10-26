//! 本机账号的登录/注册/登出：只在 `qd web --host` 绑定非 127.0.0.1 时启用，
//! 和 GPU 平台账号（routes_core.rs 里的 /api/login）完全独立——这一层只回答
//! "你能不能打开这台机器上的控制面板"。

use super::accounts;
use super::local_session::{SESSION_COOKIE, constant_time_eq, cookie_value};
use super::state::{WebError, WebResult, WebState, reply};
use crate::*;

use axum::Json;
use axum::extract::State;
use axum::http::header::SET_COOKIE;
use axum::response::{IntoResponse, Response};
use std::sync::Arc;

const SESSION_MAX_AGE_SECS: u64 = 7 * 24 * 60 * 60;

fn set_cookie_header(token: &str) -> String {
    format!("{SESSION_COOKIE}={token}; HttpOnly; Path=/; Max-Age={SESSION_MAX_AGE_SECS}; SameSite=Lax")
}

fn clear_cookie_header() -> String {
    format!("{SESSION_COOKIE}=; HttpOnly; Path=/; Max-Age=0; SameSite=Lax")
}

fn field<'a>(body: &'a Value, key: &str) -> Result<&'a str, WebError> {
    body.get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| WebError::bad_request(format!("缺少 {key}")))
}

/// GET /api/auth/state —— 前端启动时先问这个：要不要显示登录/注册墙。
pub(crate) async fn get_state(State(state): State<Arc<WebState>>, request: axum::extract::Request) -> WebResult {
    let required = state.public;
    let has_account = if required { accounts::has_any()? } else { true };
    let token = cookie_value(request.headers(), SESSION_COOKIE).map(str::to_owned);
    let username = match token {
        Some(token) => state.session_username(&token).await,
        None => None,
    };
    reply(json!({
        "required": required,
        "hasAccount": has_account,
        "loggedIn": !required || username.is_some(),
        "username": username,
    }))
}

/// POST /api/auth/register —— 必须带与启动时 --password/QD_WEB_PASSWORD 一致的邀请码。
pub(crate) async fn post_register(
    State(state): State<Arc<WebState>>,
    Json(body): Json<Value>,
) -> Result<Response, WebError> {
    if !state.public {
        return Err(WebError::bad_request("本机回环绑定不需要注册"));
    }
    let username = field(&body, "username")?.to_owned();
    let password = field(&body, "password")?.to_owned();
    let invite_code = field(&body, "inviteCode")?;
    let expected = state
        .web_password
        .as_deref()
        .ok_or_else(|| WebError::bad_request("服务未设置邀请码，无法注册"))?;
    if !constant_time_eq(invite_code.as_bytes(), expected.as_bytes()) {
        return Err(WebError::unauthorized_message("邀请码不对"));
    }
    accounts::register(&username, &password)?;
    let token = state.create_session(&username).await;
    Ok((
        [(SET_COOKIE, set_cookie_header(&token))],
        Json(json!({"ok": true, "data": {"username": username}})),
    )
        .into_response())
}

/// POST /api/auth/login
pub(crate) async fn post_login(
    State(state): State<Arc<WebState>>,
    Json(body): Json<Value>,
) -> Result<Response, WebError> {
    let username = field(&body, "username")?.to_owned();
    let password = field(&body, "password")?;
    if !accounts::verify(&username, password)? {
        return Err(WebError::unauthorized_message("用户名或密码不对"));
    }
    let token = state.create_session(&username).await;
    Ok((
        [(SET_COOKIE, set_cookie_header(&token))],
        Json(json!({"ok": true, "data": {"username": username}})),
    )
        .into_response())
}

/// POST /api/auth/logout
pub(crate) async fn post_logout(
    State(state): State<Arc<WebState>>,
    request: axum::extract::Request,
) -> Response {
    let token = cookie_value(request.headers(), SESSION_COOKIE).map(str::to_owned);
    if let Some(token) = token {
        state.remove_session(&token).await;
    }
    (
        [(SET_COOKIE, clear_cookie_header())],
        Json(json!({"ok": true, "data": {}})),
    )
        .into_response()
}
