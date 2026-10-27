//! 管理后台的登录安全：全局登录策略、单个账号的策略覆盖、在线会话列表与踢下线。
//! session_guard 已经按 `/api/admin/*` 拦掉了非管理员。

use super::accounts;
use super::login_override::{self, LoginOverride};
use super::login_policy::{self, LoginPolicy};
use super::policy_store;
use super::routes_auth::off_thread;
use super::state::{WebError, WebResult, WebState, reply};
use crate::*;

use axum::Json;
use axum::extract::{Path, State};
use std::sync::Arc;

fn parse_policy(body: Value) -> Result<LoginPolicy, WebError> {
    let policy: LoginPolicy = serde_json::from_value(body)
        .map_err(|err| WebError::bad_request(format!("登录策略格式不对: {err}")))?;
    login_policy::normalize(policy).map_err(WebError::bad_request)
}

/// GET /api/admin/login-policy
pub(crate) async fn get_policy() -> WebResult {
    reply(json!({"policy": off_thread(policy_store::global).await?}))
}

/// POST /api/admin/login-policy —— 改全局策略；跟随全局的成员下一个请求就按新策略检查。
pub(crate) async fn post_policy(Json(body): Json<Value>) -> WebResult {
    let policy = parse_policy(body)?;
    let saved = policy.clone();
    off_thread(move || policy_store::save_global(&policy)).await?;
    reply(json!({"policy": saved}))
}

/// POST /api/admin/users/{name}/login  body: {login: LoginOverride | null}，null 表示全部跟随全局。
/// 返回保存后的覆盖和合并后实际生效的策略（effective 为 null 表示不受限）。
pub(crate) async fn post_user_policy(Path(name): Path<String>, Json(body): Json<Value>) -> WebResult {
    let own = match body.get("login") {
        None | Some(Value::Null) => None,
        Some(value) => {
            let own: LoginOverride = serde_json::from_value(value.clone())
                .map_err(|err| WebError::bad_request(format!("登录限制格式不对: {err}")))?;
            Some(login_override::normalize(own).map_err(WebError::bad_request)?).filter(|o| !o.is_noop())
        }
    };
    let saved = own.clone();
    let effective = off_thread(move || {
        let effective = login_override::resolve(&policy_store::global()?, own.as_ref());
        accounts::set_login(&name, own)?;
        Ok(effective)
    })
    .await?;
    reply(json!({"login": saved, "effective": effective}))
}

/// GET /api/admin/sessions —— 所有在线会话（不含 token 本身）。
pub(crate) async fn list_sessions(State(state): State<Arc<WebState>>) -> WebResult {
    let sessions: Vec<Value> = state
        .sessions
        .list()
        .await
        .into_iter()
        .map(|s| {
            json!({
                "id": s.id, "username": s.user, "ip": s.client.ip.to_string(),
                "agent": s.client.agent, "device": s.client.device,
                "createdAt": s.created, "seenAt": s.seen_unix, "online": s.online(),
            })
        })
        .collect();
    reply(json!({"sessions": sessions}))
}

/// DELETE /api/admin/sessions/{id} —— 把某个会话踢下线。
pub(crate) async fn delete_session(State(state): State<Arc<WebState>>, Path(id): Path<String>) -> WebResult {
    let user = state
        .sessions
        .revoke_id(&id, "已被管理员强制下线")
        .await
        .ok_or_else(|| WebError::bad_request("这个会话已经不在线"))?;
    state.terminal_grants.write().await.remove(&user);
    reply(json!({"username": user}))
}
