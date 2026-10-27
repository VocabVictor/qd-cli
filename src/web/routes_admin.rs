//! 管理后台：用户列表、角色、GPU 配额、重置密码、删除。session_guard 已经按
//! `/api/admin/*` 拦掉了非管理员，这里不再重复判断。

use super::accounts::{self, Quota, Role};
use super::ownership;
use super::routes_auth::{field, off_thread};
use super::state::{WebError, WebResult, WebState, reply};
use crate::*;

use axum::Json;
use axum::extract::{Path, State};
use std::collections::BTreeMap;
use std::sync::Arc;

/// GET /api/admin/users
pub(crate) async fn list_users(State(state): State<Arc<WebState>>) -> WebResult {
    let (accounts, global) = off_thread(|| Ok((accounts::list()?, super::policy_store::global()?))).await?;
    let mut users = Vec::with_capacity(accounts.len());
    for account in accounts {
        let usage = ownership::current_usage(&state, &account.username)
            .await
            .unwrap_or_default();
        users.push(json!({
            "username": account.username,
            "role": account.role,
            "quota": account.quota,
            "usage": usage,
            "createdAt": account.created_at,
            "effective": super::login_override::resolve(&global, account.login.as_ref()),
            "login": account.login,
        }));
    }
    reply(json!({"users": users}))
}

fn limits(value: Option<&Value>) -> Result<BTreeMap<String, u32>, WebError> {
    let Some(object) = value.and_then(Value::as_object) else {
        return Ok(BTreeMap::new());
    };
    let mut map = BTreeMap::new();
    for (key, value) in object {
        validate_id(key, "空间/资源组 ID").map_err(|err| WebError::bad_request(format!("{err:#}")))?;
        let limit = value_u64(value)
            .filter(|n| *n <= 4096)
            .ok_or_else(|| WebError::bad_request(format!("{key} 的上限必须是 0~4096 的整数")))?;
        if limit > 0 {
            map.insert(key.clone(), limit as u32);
        }
    }
    Ok(map)
}

/// POST /api/admin/users/{name}/quota  body: {total, spaces: {id: n}, groups: {id: n}}
pub(crate) async fn post_quota(Path(name): Path<String>, Json(body): Json<Value>) -> WebResult {
    let total = body
        .get("total")
        .and_then(value_u64)
        .filter(|n| *n <= 4096)
        .ok_or_else(|| WebError::bad_request("total 必须是 0~4096 的整数"))? as u32;
    let quota = Quota {
        total,
        spaces: limits(body.get("spaces"))?,
        groups: limits(body.get("groups"))?,
    };
    let saved = quota.clone();
    off_thread(move || accounts::set_quota(&name, quota)).await?;
    reply(json!({"quota": saved}))
}

/// POST /api/admin/users/{name}/role  body: {role: "admin" | "user"}
pub(crate) async fn post_role(
    State(state): State<Arc<WebState>>,
    Path(name): Path<String>,
    Json(body): Json<Value>,
) -> WebResult {
    let role = match field(&body, "role")? {
        "admin" => Role::Admin,
        "user" => Role::User,
        _ => return Err(WebError::bad_request("role 只接受 admin 或 user")),
    };
    let target = name.clone();
    off_thread(move || accounts::set_role(&target, role)).await?;
    // 降级立即生效：session_guard 每次都重新读角色，这里再清一次终端授权。
    if role == Role::User {
        state.terminal_grants.write().await.remove(&name);
    }
    reply(json!({"username": name, "role": role}))
}

/// POST /api/admin/users/{name}/password  body: {password}
pub(crate) async fn post_password(
    State(state): State<Arc<WebState>>,
    Path(name): Path<String>,
    Json(body): Json<Value>,
) -> WebResult {
    let password = field(&body, "password")?.to_owned();
    let target = name.clone();
    off_thread(move || accounts::set_password(&target, &password)).await?;
    state.drop_user_sessions(&name, Some("管理员重置了你的密码，请用新密码登录")).await;
    reply(json!({"username": name}))
}

/// DELETE /api/admin/users/{name}
pub(crate) async fn delete_user(
    State(state): State<Arc<WebState>>,
    Path(name): Path<String>,
) -> WebResult {
    let removed = off_thread(move || accounts::delete(&name)).await?;
    state.drop_user_sessions(&removed, Some("账号已被管理员删除")).await;
    reply(json!({"removed": removed}))
}
