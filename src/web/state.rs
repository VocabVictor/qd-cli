use crate::*;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};

#[derive(Clone, Debug)]
pub(crate) struct TerminalGrant {
    /// 上游是否 https/wss（真实平台是；本地模拟平台是 http/ws）。
    pub(crate) secure: bool,
    pub(crate) authority: String,
    pub(crate) ws_prefix: Option<String>,
}

#[derive(Clone)]
pub(crate) struct SpaceInfo {
    pub(crate) id: String,
    pub(crate) name: String,
}

pub(crate) struct WebState {
    pub(crate) config: tokio::sync::RwLock<Config>,
    pub(crate) api: tokio::sync::RwLock<Option<ApiClient>>,
    /// 用户可见空间列表缓存（登录/登出时清空）。
    pub(crate) spaces: tokio::sync::RwLock<Option<Vec<SpaceInfo>>>,
    /// 用户名 -> 最近一次解析出的终端授权（上游 host:port + 该 pod 的 /ws 路径前缀）。
    /// 反向代理只按这里的记录转发，避免普通用户靠拼路径连到别人的 pod。
    pub(crate) terminal_grants: tokio::sync::RwLock<HashMap<String, TerminalGrant>>,
    /// 绑定非回环地址时为 true，跳过 Origin 同源检查，改用本机账号 session 把关。
    pub(crate) public: bool,
    /// 公开绑定时必须设置：普通邀请码注册普通用户（配额 0），管理员邀请码注册管理员。
    pub(crate) invite_user: Option<String>,
    pub(crate) invite_admin: Option<String>,
    /// token -> (username, 过期时间)。重启进程即丢失，不持久化。
    local_sessions: tokio::sync::RwLock<HashMap<String, (String, Instant)>>,
    pub(crate) auth_limiter: super::rate_limit::AuthLimiter,
}

impl WebState {
    pub(crate) fn new(config: Config, public: bool, invites: (Option<String>, Option<String>)) -> Self {
        let api = ApiClient::from_saved_session(config.clone()).ok();
        Self {
            config: tokio::sync::RwLock::new(config),
            api: tokio::sync::RwLock::new(api),
            spaces: tokio::sync::RwLock::new(None),
            terminal_grants: tokio::sync::RwLock::new(HashMap::new()),
            public,
            invite_user: invites.0,
            invite_admin: invites.1,
            local_sessions: tokio::sync::RwLock::new(HashMap::new()),
            auth_limiter: Default::default(),
        }
    }

    /// 底层存取给 local_session.rs 用：这里才摸得到私有的 local_sessions 字段。
    pub(crate) async fn insert_session(&self, token: String, username: String, expires: Instant) {
        self.local_sessions.write().await.insert(token, (username, expires));
    }

    /// token 有效且未过期则返回对应用户名；顺手清掉已过期的记录。
    pub(crate) async fn lookup_session(&self, token: &str) -> Option<String> {
        let mut sessions = self.local_sessions.write().await;
        sessions.retain(|_, (_, expires)| *expires > Instant::now());
        sessions.get(token).map(|(username, _)| username.clone())
    }

    pub(crate) async fn drop_session(&self, token: &str) {
        self.local_sessions.write().await.remove(token);
    }

    /// 删除/降级账号后踢掉该用户的全部 session 和终端授权。
    pub(crate) async fn drop_user_sessions(&self, username: &str) {
        self.local_sessions
            .write()
            .await
            .retain(|_, (owner, _)| !owner.eq_ignore_ascii_case(username));
        self.terminal_grants.write().await.remove(username);
    }

    pub(crate) async fn config(&self) -> Config {
        self.config.read().await.clone()
    }

    /// 指定空间的客户端；space 为空则用默认空间。
    pub(crate) async fn api_for(&self, space: Option<&str>) -> Result<ApiClient, WebError> {
        let api = self.api().await?;
        Ok(match space.filter(|value| !value.is_empty()) {
            Some(space) => api.with_space(Some(space.to_owned())),
            None => api,
        })
    }

    /// 用户全部空间；接口失败时退化为配置里的默认空间。
    pub(crate) async fn spaces(&self) -> Vec<SpaceInfo> {
        if let Some(cached) = self.spaces.read().await.clone() {
            return cached;
        }
        let default_space = self.config().await.space_id.unwrap_or_default();
        let fetched = match self.api().await {
            Ok(api) => api
                .get(Service::Core, "/space/list", empty_object())
                .await
                .ok()
                .and_then(|value| {
                    let list = value.pointer("/data/userSpaceList")?.as_array()?.clone();
                    let spaces: Vec<SpaceInfo> = list
                        .iter()
                        .filter_map(|space| {
                            Some(SpaceInfo {
                                id: space.get("spaceId")?.as_str()?.to_owned(),
                                name: space
                                    .get("spaceName")
                                    .and_then(Value::as_str)
                                    .unwrap_or_default()
                                    .to_owned(),
                            })
                        })
                        .collect();
                    (!spaces.is_empty()).then_some(spaces)
                }),
            Err(_) => None,
        };
        // 取到了才缓存：隧道没通/接口抖动时兜底只用这一次，
        // 否则一次失败就会把「只有默认空间」钉死到进程退出，跨空间凭空消失。
        let Some(spaces) = fetched else {
            return vec![SpaceInfo {
                id: default_space.clone(),
                name: default_space,
            }];
        };
        *self.spaces.write().await = Some(spaces.clone());
        spaces
    }

    /// 取当前登录态的 API 客户端；没有则返回 401，前端据此切到登录页。
    pub(crate) async fn api(&self) -> Result<ApiClient, WebError> {
        self.api
            .read()
            .await
            .clone()
            .ok_or_else(WebError::unauthorized)
    }
}

/// 给聚合结果里的对象补上来源空间标签（已有字段不覆盖）。
pub(crate) fn attach_space(value: &mut Value, space: &SpaceInfo) {
    if let Some(object) = value.as_object_mut() {
        object
            .entry("spaceId")
            .or_insert_with(|| Value::String(space.id.clone()));
        object
            .entry("spaceName")
            .or_insert_with(|| Value::String(space.name.clone()));
    }
}

pub(crate) struct WebError {
    status: StatusCode,
    message: String,
}

impl WebError {
    pub(crate) fn unauthorized() -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            message: "未登录，请先在网页里登录".to_owned(),
        }
    }

    pub(crate) fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: message.into(),
        }
    }

    pub(crate) fn too_many_requests(retry_after_secs: u64) -> Self {
        Self {
            status: StatusCode::TOO_MANY_REQUESTS,
            message: format!("失败次数过多，请 {} 分钟后再试", retry_after_secs.div_ceil(60)),
        }
    }

    pub(crate) fn forbidden(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::FORBIDDEN,
            message: message.into(),
        }
    }

    pub(crate) fn unauthorized_message(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            message: message.into(),
        }
    }
}

impl From<anyhow::Error> for WebError {
    fn from(error: anyhow::Error) -> Self {
        let message = format!("{error:#}");
        let status = if message.contains("重新运行 qd login") || message.contains("登录已过期")
        {
            StatusCode::UNAUTHORIZED
        } else {
            StatusCode::BAD_GATEWAY
        };
        Self { status, message }
    }
}

impl IntoResponse for WebError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(json!({"ok": false, "error": self.message})),
        )
            .into_response()
    }
}

pub(crate) type WebResult = std::result::Result<Json<Value>, WebError>;

pub(crate) fn reply(data: Value) -> WebResult {
    Ok(Json(json!({"ok": true, "data": data})))
}

fn is_local_origin(origin: &str) -> bool {
    for prefix in ["http://127.0.0.1", "http://localhost"] {
        if origin == prefix {
            return true;
        }
        if let Some(port) = origin.strip_prefix(&format!("{prefix}:")) {
            return port.parse::<u16>().is_ok();
        }
    }
    false
}

/// Web 服务只绑定回环地址。写操作还必须来自其本机页面，防止跨站请求触发
/// 提交、删除或远程执行；只读请求保留给本机排障工具使用。
pub(crate) async fn origin_guard(
    axum::extract::State(state): axum::extract::State<Arc<WebState>>,
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> Response {
    if state.public {
        return next.run(request).await;
    }
    let origin = request
        .headers()
        .get(axum::http::header::ORIGIN)
        .and_then(|value| value.to_str().ok());
    let read_only = matches!(
        request.method(),
        &axum::http::Method::GET | &axum::http::Method::HEAD | &axum::http::Method::OPTIONS
    );
    let from_local_page = origin.is_some_and(is_local_origin);
    // 无 Origin 头的读请求（curl、浏览器直接开页）放行，写请求一律要求本机页面
    let anonymous_read = origin.is_none() && read_only;
    if !from_local_page && !anonymous_read {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"ok": false, "error": "写操作必须来自本机 qd Web 页面"})),
        )
            .into_response();
    }
    next.run(request).await
}

pub(crate) use super::query::{query_scope, query_u64};
