use crate::*;
use crate::web::state::*;

use axum::{Json, extract::State};
use std::sync::Arc;

pub(crate) async fn get_state(State(state): State<Arc<WebState>>) -> WebResult {
    let api = state.api.read().await.clone();
    let config = state.config().await;
    let username = match &api {
        Some(api) => Some(api.username().await),
        None => None,
    };
    let spaces = if api.is_some() {
        state
            .spaces()
            .await
            .into_iter()
            .map(|space| json!({"id": space.id, "name": space.name}))
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    reply(json!({
        "version": env!("CARGO_PKG_VERSION"),
        "loggedIn": api.is_some(),
        "username": username,
        "baseUrl": config.base_url,
        "spaceId": config.space_id,
        "spaces": spaces,
        "concurrency": config.concurrency,
        "insecureTls": config.insecure_tls,
    }))
}

pub(crate) async fn post_login(
    State(state): State<Arc<WebState>>,
    Json(body): Json<Value>,
) -> WebResult {
    let username = body
        .get("username")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| WebError::bad_request("缺少 username"))?
        .to_owned();
    let mut password = body
        .get("password")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| WebError::bad_request("缺少 password"))?
        .to_owned();
    let ldap = body.get("ldap").and_then(Value::as_bool).unwrap_or(false);
    let remember = body
        .get("remember")
        .and_then(Value::as_bool)
        .unwrap_or(true);

    let config = state.config().await;
    let login_result = ApiClient::login(&config, &username, &password, ldap, None, None).await;
    let session = match login_result {
        Ok(session) => session,
        Err(error) => {
            clear_string(&mut password);
            return Err(error.into());
        }
    };
    session.save()?;
    if remember {
        SavedCredentials::new(username.clone(), password, ldap).save()?;
    } else {
        clear_string(&mut password);
        SavedCredentials::remove()?;
    }
    let api = ApiClient::new(config, session)?;
    *state.api.write().await = Some(api);
    *state.spaces.write().await = None;
    reply(json!({"username": username, "autoLogin": remember}))
}

pub(crate) async fn post_logout(State(state): State<Arc<WebState>>) -> WebResult {
    let session_removed = Session::remove()?;
    let credential_removed = SavedCredentials::remove()?;
    *state.api.write().await = None;
    *state.spaces.write().await = None;
    reply(json!({
        "sessionRemoved": session_removed,
        "credentialRemoved": credential_removed
    }))
}

/// 通用平台 API 透传（等价 `qd api METHOD PATH`）。
pub(crate) async fn post_call(
    State(state): State<Arc<WebState>>,
    Json(body): Json<Value>,
) -> WebResult {
    let api = state
        .api_for(body.get("spaceId").and_then(Value::as_str))
        .await?;
    let method = body
        .get("method")
        .and_then(Value::as_str)
        .unwrap_or("GET")
        .to_ascii_uppercase();
    let method = Method::from_bytes(method.as_bytes())
        .map_err(|_| WebError::bad_request("无效的 HTTP 方法"))?;
    let service = Service::parse(
        body.get("service")
            .and_then(Value::as_str)
            .unwrap_or("core"),
    )?;
    let path = body
        .get("path")
        .and_then(Value::as_str)
        .filter(|value| value.starts_with('/'))
        .ok_or_else(|| WebError::bad_request("path 必须以 / 开头"))?
        .to_owned();
    let data = body.get("data").cloned().unwrap_or_else(empty_object);
    reply(api.send(method, service, &path, data).await?)
}

pub(crate) async fn post_config(Json(body): Json<Value>) -> WebResult {
    let key = body
        .get("key")
        .and_then(Value::as_str)
        .ok_or_else(|| WebError::bad_request("缺少 key"))?;
    let value = body
        .get("value")
        .and_then(Value::as_str)
        .ok_or_else(|| WebError::bad_request("缺少 value"))?;
    let mut config = Config::load()?;
    config.set(key, value)?;
    reply(json!({"key": key, "value": value, "note": "重启 qd web 后对本服务生效"}))
}

pub(crate) async fn post_keepalive(
    State(state): State<Arc<WebState>>,
    Json(body): Json<Value>,
) -> WebResult {
    let action = body.get("action").and_then(Value::as_str).unwrap_or("");
    let value = match action {
        "once" => {
            let api = state.api().await?;
            keepalive_once(&api).await?
        }
        "install" => {
            let minutes = body
                .get("minutes")
                .and_then(Value::as_u64)
                .unwrap_or(30)
                .clamp(5, 1440);
            install_keepalive_task(minutes)?
        }
        "status" => keepalive_task_status()?,
        "remove" => remove_keepalive_task()?,
        _ => return Err(WebError::bad_request("action 只接受 once/install/status/remove")),
    };
    reply(value)
}
