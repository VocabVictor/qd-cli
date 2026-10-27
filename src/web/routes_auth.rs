//! 本机账号的登录/注册/登出/改密：只在 `qd web --host` 绑定非 127.0.0.1 时启用，
//! 和 GPU 平台账号（routes_core.rs 里的 /api/login）完全独立。

use super::accounts::{self, Role};
use super::local_session::{SESSION_COOKIE, constant_time_eq, cookie_value, current_user};
use super::login_policy::{self, ClientInfo, SessionMode};
use super::ownership;
use super::policy_store;
use super::state::{WebError, WebResult, WebState, reply};
use crate::*;

use axum::Json;
use axum::extract::{ConnectInfo, State};
use axum::http::HeaderMap;
use axum::http::header::SET_COOKIE;
use axum::response::{IntoResponse, Response};
use std::net::SocketAddr;
use std::sync::Arc;

const SESSION_AGE: u64 = 7 * 24 * 60 * 60;

fn set_cookie_header(token: &str) -> String {
    format!("{SESSION_COOKIE}={token}; HttpOnly; Path=/; Max-Age={SESSION_AGE}; SameSite=Lax")
}

fn clear_cookie_header() -> String {
    format!("{SESSION_COOKIE}=; HttpOnly; Path=/; Max-Age=0; SameSite=Lax")
}

pub(crate) fn field<'a>(body: &'a Value, key: &str) -> Result<&'a str, WebError> {
    body.get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| WebError::bad_request(format!("缺少 {key}")))
}

/// argon2 一次几十毫秒；放进阻塞线程池，免得卡住单线程运行时上的其他请求。
pub(crate) async fn off_thread<T: Send + 'static>(
    job: impl FnOnce() -> Result<T, WebError> + Send + 'static,
) -> Result<T, WebError> {
    tokio::task::spawn_blocking(job)
        .await
        .map_err(|_| WebError::bad_request("后台任务异常退出"))?
}

fn session_response(token: &str, username: &str, role: Role) -> Response {
    (
        [(SET_COOKIE, set_cookie_header(token))],
        Json(json!({"ok": true, "data": {"username": username, "role": role}})),
    )
        .into_response()
}

fn token_of(request: &axum::extract::Request) -> Option<String> {
    cookie_value(request.headers(), SESSION_COOKIE).map(str::to_owned)
}

/// 签发 session 前过一遍登录限制（管理员不受限）：白名单、单点登录。
async fn start_session(state: &WebState, username: &str, client: ClientInfo) -> Result<(String, Role), WebError> {
    let name = username.to_owned();
    let (account, policy) = off_thread(move || {
        let account = accounts::get(&name)?.ok_or_else(WebError::unauthorized)?;
        let policy = policy_store::effective(&account)?;
        Ok((account, policy))
    })
    .await?;
    if let (Role::User, Some(policy)) = (account.role, policy) {
        login_policy::check_login(&policy, &client).map_err(WebError::forbidden)?;
        match policy.session_mode {
            SessionMode::Off => {}
            SessionMode::Kick => {
                let reason = format!("账号已在别处登录（IP {}），这里已退出", client.ip);
                state.sessions.revoke_user(&account.username, Some(&reason), None).await;
            }
            SessionMode::Deny => {
                if let Some(other) = state.sessions.online_of(&account.username).await {
                    return Err(WebError::forbidden(format!(
                        "账号正在 IP {} 上使用，同一时间只能在一处登录；请先在那边退出，或等它闲置 30 分钟后再试",
                        other.client.ip
                    )));
                }
            }
        }
    }
    let token = state.sessions.create(&account.username, client).await;
    Ok((token, account.role))
}

/// GET /api/auth/state —— 前端启动时先问这个：要不要显示登录墙、我是谁、我的配额。
pub(crate) async fn get_state(State(state): State<Arc<WebState>>, request: axum::extract::Request) -> WebResult {
    let client = login_policy::client_of(&request);
    if !state.public {
        return reply(json!({
            "required": false, "hasAccount": true, "loggedIn": true,
            "username": "local", "role": Role::Admin,
        }));
    }
    let has_account = off_thread(accounts::has_any).await?;
    let checked = match token_of(&request) {
        Some(token) => Some(current_user(&state, &token, &client, true).await),
        None => None,
    };
    let user = match checked {
        Some(Ok(user)) => user,
        // notice 告诉前端为什么被退出（在别处登录、环境变化、被管理员踢下线）
        Some(Err(notice)) => {
            return reply(json!({"required": true, "hasAccount": has_account, "loggedIn": false, "notice": notice}));
        }
        None => return reply(json!({"required": true, "hasAccount": has_account, "loggedIn": false})),
    };
    let name = user.name.clone();
    let account = off_thread(move || accounts::get(&name)).await?;
    let usage = ownership::current_usage(&state, &user.name).await.unwrap_or_default();
    reply(json!({
        "required": true,
        "hasAccount": has_account,
        "loggedIn": true,
        "username": user.name,
        "role": user.role,
        "quota": account.map(|a| a.quota).unwrap_or_default(),
        "usage": usage,
    }))
}

/// POST /api/auth/register —— 邀请码决定角色：管理员码 → 管理员，普通码 → 普通用户（配额 0）。
pub(crate) async fn post_register(
    State(state): State<Arc<WebState>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<Response, WebError> {
    if !state.public {
        return Err(WebError::bad_request("本机回环绑定不需要注册"));
    }
    let ip_key = format!("ip:{}", peer.ip());
    if let Some(secs) = state.auth_limiter.locked_for(&[ip_key.as_str()]).await {
        return Err(WebError::too_many_requests(secs));
    }
    let username = field(&body, "username")?.trim().to_owned();
    let password = field(&body, "password")?.to_owned();
    let code = field(&body, "inviteCode")?.as_bytes();
    let matches = |expected: &Option<String>| {
        expected
            .as_deref()
            .is_some_and(|expected| constant_time_eq(code, expected.as_bytes()))
    };
    let role = if matches(&state.invite_admin) {
        Role::Admin
    } else if matches(&state.invite_user) {
        Role::User
    } else {
        state.auth_limiter.record_failure(&[ip_key.as_str()]).await;
        return Err(WebError::unauthorized_message("邀请码不对"));
    };
    let name = username.clone();
    // 普通用户先过全局白名单，不在白名单里的人拿到邀请码也注册不了
    if role == Role::User {
        let client = login_policy::client_from(Some(peer.ip()), &headers);
        let policy = off_thread(policy_store::global).await?;
        login_policy::check_login(&policy, &client).map_err(WebError::forbidden)?;
    }
    off_thread(move || accounts::register(&name, &password, role)).await?;
    let client = login_policy::client_from(Some(peer.ip()), &headers);
    let (token, role) = start_session(&state, &username, client).await?;
    Ok(session_response(&token, &username, role))
}

/// POST /api/auth/login
pub(crate) async fn post_login(
    State(state): State<Arc<WebState>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<Response, WebError> {
    if !state.public {
        return Err(WebError::bad_request("本机回环绑定不需要登录"));
    }
    let username = field(&body, "username")?.trim().to_owned();
    let password = field(&body, "password")?.to_owned();
    let ip_key = format!("ip:{}", peer.ip());
    let user_key = format!("user:{}", username.to_lowercase());
    let keys = [ip_key.as_str(), user_key.as_str()];
    if let Some(secs) = state.auth_limiter.locked_for(&keys).await {
        return Err(WebError::too_many_requests(secs));
    }
    let name = username.clone();
    let Some(canonical) = off_thread(move || accounts::verify(&name, &password)).await? else {
        state.auth_limiter.record_failure(&keys).await;
        return Err(WebError::unauthorized_message("用户名或密码不对"));
    };
    // 只清用户名计数：IP 计数保留，免得手里有一个号的人靠穿插成功登录去爆破别的号。
    state.auth_limiter.clear(&user_key).await;
    let client = login_policy::client_from(Some(peer.ip()), &headers);
    let (token, role) = start_session(&state, &canonical, client).await?;
    Ok(session_response(&token, &canonical, role))
}

/// POST /api/auth/password —— 改自己的密码，必须给出旧密码；成功后其他 session 全部失效。
pub(crate) async fn post_password(
    State(state): State<Arc<WebState>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    request: axum::extract::Request,
) -> Result<Response, WebError> {
    let token = token_of(&request).ok_or_else(WebError::unauthorized)?;
    let client = login_policy::client_of(&request);
    let user = current_user(&state, &token, &client, true)
        .await
        .map_err(WebError::unauthorized_message)?;
    let bytes = axum::body::to_bytes(request.into_body(), 64 * 1024)
        .await
        .map_err(|_| WebError::bad_request("请求体过大"))?;
    let body: Value = serde_json::from_slice(&bytes).map_err(|_| WebError::bad_request("请求体不是 JSON"))?;
    let old = field(&body, "oldPassword")?.to_owned();
    let new = field(&body, "newPassword")?.to_owned();
    let ip_key = format!("ip:{}", peer.ip());
    let user_key = format!("user:{}", user.name.to_lowercase());
    let keys = [ip_key.as_str(), user_key.as_str()];
    if let Some(secs) = state.auth_limiter.locked_for(&keys).await {
        return Err(WebError::too_many_requests(secs));
    }
    let name = user.name.clone();
    if off_thread(move || accounts::verify(&name, &old)).await?.is_none() {
        state.auth_limiter.record_failure(&keys).await;
        return Err(WebError::unauthorized_message("旧密码不对"));
    }
    let name = user.name.clone();
    off_thread(move || accounts::set_password(&name, &new)).await?;
    state.drop_user_sessions(&user.name, Some("密码已修改，请用新密码重新登录")).await;
    let token = state.sessions.create(&user.name, client).await;
    Ok(session_response(&token, &user.name, user.role))
}

/// POST /api/auth/logout
pub(crate) async fn post_logout(
    State(state): State<Arc<WebState>>,
    request: axum::extract::Request,
) -> Response {
    if let Some(token) = token_of(&request) {
        state.sessions.revoke(&token, None).await;
    }
    (
        [(SET_COOKIE, clear_cookie_header())],
        Json(json!({"ok": true, "data": {}})),
    )
        .into_response()
}
