use crate::*;
use crate::web::state::*;

use axum::{
    Json,
    extract::{Query, State},
};
use std::{collections::HashMap, sync::Arc, time::Duration};

/// 开发环境列表：跨全部空间聚合。
pub(crate) async fn list_devs(
    State(state): State<Arc<WebState>>,
    Query(query): Query<HashMap<String, String>>,
) -> WebResult {
    let scope = query_scope(&query)?;
    let mut devs = Vec::new();
    let mut failed_spaces = Vec::new();
    for space in state.spaces().await {
        let api = state.api_for(Some(&space.id)).await?;
        match list_scoped_dev_environments(&api, &scope).await {
            Ok(response) => {
                for mut dev in response_array(&response, &["/data/devList"]).to_vec() {
                    attach_space(&mut dev, &space);
                    devs.push(dev);
                }
            }
            Err(_) => failed_spaces.push(space.name.clone()),
        }
    }
    reply(json!({
        "code": 0,
        "data": {"listCount": devs.len(), "devList": devs, "scope": scope, "failedSpaces": failed_spaces}
    }))
}

/// 空闲资源：跨全部空间聚合所有资源组。
pub(crate) async fn get_idle(
    State(state): State<Arc<WebState>>,
    Query(query): Query<HashMap<String, String>>,
) -> WebResult {
    let concurrency = state.config().await.concurrency;
    let mut groups = Vec::new();
    let mut failed_spaces = Vec::new();
    for space in state.spaces().await {
        let api = state.api_for(Some(&space.id)).await?;
        match resource_group_ids(&api).await {
            Ok(ids) => {
                for mut group in idle_resource_groups(&api, ids, concurrency).await {
                    attach_space(&mut group, &space);
                    groups.push(group);
                }
            }
            Err(_) => failed_spaces.push(space.name.clone()),
        }
    }
    groups.sort_by(|left, right| {
        let key = |group: &Value| {
            (
                group
                    .get("spaceName")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
                group
                    .get("rsgroupName")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
            )
        };
        key(left).cmp(&key(right))
    });
    if query.get("availableOnly").map(String::as_str) == Some("1") {
        groups.retain(|group| group.get("available").and_then(Value::as_bool) == Some(true));
    }
    if query.get("gpuOnly").map(String::as_str) == Some("1") {
        groups.retain(|group| group.get("gpuAvailable").and_then(Value::as_bool) == Some(true));
    }
    reply(json!({"listCount": groups.len(), "groups": groups, "failedSpaces": failed_spaces}))
}

async fn resolve_terminal_url(
    api: &ApiClient,
    query: &HashMap<String, String>,
) -> Result<String, WebError> {
    let id = query
        .get("id")
        .filter(|value| !value.is_empty())
        .ok_or_else(|| WebError::bad_request("缺少 id"))?;
    validate_id(id, "ID")?;
    let url = match query.get("kind").map(String::as_str) {
        Some("job") => {
            let instance = query_u64(query, "instance", 0, 0, 64)? as usize;
            job_terminal_url(api, id, instance).await?
        }
        Some("dev") => dev_terminal_url(api, id).await?,
        _ => return Err(WebError::bad_request("kind 只接受 job 或 dev")),
    };
    Ok(url)
}

pub(crate) async fn get_terminal_url(
    State(state): State<Arc<WebState>>,
    Query(query): Query<HashMap<String, String>>,
) -> WebResult {
    let api = state
        .api_for(query.get("space").map(String::as_str))
        .await?;
    let url = resolve_terminal_url(&api, &query).await?;
    // 记录真实上游，并把地址换成 qd 自己的同源代理路径，
    // 这样 iframe 不受自签证书与浏览器代理影响。
    super::proxy::remember_upstream(&state, &url).await;
    let local = reqwest::Url::parse(&url)
        .ok()
        .map(|parsed| match parsed.query() {
            Some(query) => format!("{}?{}", parsed.path(), query),
            None => parsed.path().to_owned(),
        })
        .unwrap_or(url);
    reply(json!({"terminalUrl": local}))
}

/// 一次性远程命令：连平台网页终端执行并捕获输出（等价 job/dev exec）。
pub(crate) async fn post_exec(
    State(state): State<Arc<WebState>>,
    Json(body): Json<Value>,
) -> WebResult {
    let api = state
        .api_for(body.get("space").and_then(Value::as_str))
        .await?;
    let command = body
        .get("command")
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(|value| value.as_str().map(str::to_owned))
                .collect::<Vec<String>>()
        })
        .filter(|command| !command.is_empty())
        .ok_or_else(|| WebError::bad_request("缺少 command（字符串数组）"))?;
    let mut query = HashMap::new();
    for key in ["kind", "id", "instance"] {
        if let Some(value) = body.get(key).and_then(value_string) {
            query.insert(key.to_owned(), value);
        }
    }
    let timeout_secs = body
        .get("timeoutSecs")
        .and_then(Value::as_u64)
        .unwrap_or(110)
        .clamp(5, 3600);
    let url = resolve_terminal_url(&api, &query).await?;
    let mut websocket = connect_web_terminal(&api, &url).await?;
    let outcome = tokio::time::timeout(
        Duration::from_secs(timeout_secs),
        run_web_terminal_command(&mut websocket, &command),
    )
    .await
    .map_err(|_| {
        WebError::bad_request(format!(
            "远程命令超过 {timeout_secs} 秒未返回（平台网关约 2 分钟会断开长连接，长任务请用 nohup 后台跑再查日志）"
        ))
    })??;
    let (output, exit_code) = outcome;
    reply(json!({"output": output, "exitCode": exit_code}))
}
