use crate::*;

use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};

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
}

impl WebState {
    pub(crate) fn new(config: Config) -> Self {
        let api = ApiClient::from_saved_session(config.clone()).ok();
        Self {
            config: tokio::sync::RwLock::new(config),
            api: tokio::sync::RwLock::new(api),
            spaces: tokio::sync::RwLock::new(None),
        }
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
        let spaces = fetched.unwrap_or_else(|| {
            vec![SpaceInfo {
                id: default_space.clone(),
                name: default_space,
            }]
        });
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
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> Response {
    let origin = request
        .headers()
        .get(axum::http::header::ORIGIN)
        .and_then(|value| value.to_str().ok());
    let read_only = matches!(
        request.method(),
        &axum::http::Method::GET | &axum::http::Method::HEAD | &axum::http::Method::OPTIONS
    );
    if !origin.is_some_and(is_local_origin) && !(origin.is_none() && read_only) {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"ok": false, "error": "写操作必须来自本机 qd Web 页面"})),
        )
            .into_response();
    }
    next.run(request).await
}

pub(crate) fn query_u64(
    query: &std::collections::HashMap<String, String>,
    key: &str,
    default: u64,
    min: u64,
    max: u64,
) -> Result<u64, WebError> {
    let Some(value) = query.get(key).filter(|value| !value.is_empty()) else {
        return Ok(default);
    };
    let parsed: u64 = value
        .parse()
        .map_err(|_| WebError::bad_request(format!("{key} 必须是整数")))?;
    if parsed < min || parsed > max {
        return Err(WebError::bad_request(format!(
            "{key} 必须在 {min}..={max} 范围内"
        )));
    }
    Ok(parsed)
}

pub(crate) fn query_scope(
    query: &std::collections::HashMap<String, String>,
) -> Result<String, WebError> {
    let scope = query
        .get("scope")
        .filter(|value| !value.is_empty())
        .map(String::as_str)
        .unwrap_or("mine")
        .to_ascii_lowercase();
    if !matches!(scope.as_str(), "mine" | "shared" | "public" | "all") {
        return Err(WebError::bad_request("scope 只接受 mine、shared、public 或 all"));
    }
    Ok(scope)
}
