use crate::*;
use crate::web::ownership::{self, JobRecord};
use crate::web::quota;
use crate::web::roles::{CurrentUser, require_owner};
use crate::web::state::*;

use axum::{
    Extension, Json,
    extract::{Path, Query, State},
};
use std::{collections::HashMap, sync::Arc};

fn space_of(query: &HashMap<String, String>) -> Option<&str> {
    // 前端历史上两种参数名都用过（space / spaceId），都认
    query
        .get("space")
        .or_else(|| query.get("spaceId"))
        .map(String::as_str)
        .filter(|value| !value.is_empty())
}

/// 作业列表：跨空间、跨项目汇总，按创建时间倒序，走平台原生分页。
/// 参数：page/size 分页，status 逗号分隔的平台状态，q 作业名关键字（平台 keyWords），
/// space 只看某个空间，project 只看某个项目。
pub(crate) async fn list_jobs(
    State(state): State<Arc<WebState>>,
    Query(query): Query<HashMap<String, String>>,
) -> WebResult {
    let scope = query_scope(&query)?;
    let page = query_u64(&query, "page", 1, 1, 100_000)?;
    let size = query_u64(&query, "size", 50, 1, 500)?;
    let statuses = parse_statuses(query.get("status").map(String::as_str)).map_err(|err| WebError::bad_request(format!("{err:#}")))?;
    let keyword = query.get("q").map(|v| v.trim().to_owned()).filter(|v| !v.is_empty());
    if keyword.as_ref().is_some_and(|k| k.chars().count() > 64) {
        return Err(WebError::bad_request("关键字最多 64 个字"));
    }
    let only_space = space_of(&query);
    let only_project = query.get("project").filter(|v| !v.is_empty());
    if let Some(id) = only_project {
        validate_id(id, "PROJECT_ID")?;
    }

    // 同一个项目可能在多个空间的列表里都出现：每个（项目, 状态）只保留一份，
    // 并优先用项目所属空间的客户端去查（平台按 spaceid 请求头路由）。
    let mut picked: HashMap<(String, Option<String>), JobSource> = HashMap::new();
    let mut failed_spaces = Vec::new();
    for space in state.spaces().await {
        if only_space.is_some_and(|id| id != space.id) {
            continue;
        }
        let api = state.api_for(Some(&space.id)).await?;
        let mut tag = serde_json::Map::new();
        tag.insert("spaceId".to_owned(), Value::String(space.id.clone()));
        tag.insert("spaceName".to_owned(), Value::String(space.name.clone()));
        match job_sources(&api, &scope, &statuses, keyword.as_deref(), tag).await {
            Ok(list) => {
                for source in list {
                    if only_project.is_some_and(|id| *id != source.project_id) {
                        continue;
                    }
                    let home = source.tag.get("spaceId").and_then(Value::as_str) == source.api.space_id();
                    let key = (source.project_id.clone(), source.status.clone());
                    let keep = picked.get(&key).is_some_and(|old| {
                        old.tag.get("spaceId").and_then(Value::as_str) == old.api.space_id() || !home
                    });
                    if !keep {
                        picked.insert(key, source);
                    }
                }
            }
            Err(_) => failed_spaces.push(space.name.clone()),
        }
    }
    let config = state.config().await;
    let mut result = merged_job_page(picked.into_values().collect(), page, size, config.concurrency).await;

    // 标注提交者，前端据此决定"我的作业"能不能取消/删除/进终端。
    let owners: HashMap<String, String> = ownership::owners()?.into_iter().collect();
    for job in &mut result.jobs {
        let id = job.get("jobId").or_else(|| job.get("id")).and_then(value_string);
        if let (Some(id), Some(object)) = (id, job.as_object_mut()) {
            let owner = owners.get(&id).cloned().map(Value::String).unwrap_or(Value::Null);
            object.insert("qdOwner".to_owned(), owner);
        }
    }
    reply(json!({
        "code": 0,
        "data": {
            "listCount": result.total,
            "jobList": result.jobs,
            "page": page,
            "size": size,
            "scope": scope,
            "skippedProjects": result.skipped,
            "failedSpaces": failed_spaces,
        }
    }))
}

pub(crate) async fn get_job(
    State(state): State<Arc<WebState>>,
    Path(id): Path<String>,
    Query(query): Query<HashMap<String, String>>,
) -> WebResult {
    let api = state.api_for(space_of(&query)).await?;
    validate_id(&id, "JOB_ID")?;
    reply(job_show(&api, &id).await?)
}

pub(crate) async fn get_job_logs(
    State(state): State<Arc<WebState>>,
    Path(id): Path<String>,
    Query(query): Query<HashMap<String, String>>,
) -> WebResult {
    let api = state.api_for(space_of(&query)).await?;
    validate_id(&id, "JOB_ID")?;
    let limit = query_u64(&query, "limit", 500, 1, 1000)?;
    let lines = collect_job_logs(&api, &id, limit).await?;
    reply(json!({"jobId": id, "lines": lines}))
}

pub(crate) async fn get_job_metrics(
    State(state): State<Arc<WebState>>,
    Path(id): Path<String>,
    Query(query): Query<HashMap<String, String>>,
) -> WebResult {
    let api = state.api_for(space_of(&query)).await?;
    validate_id(&id, "JOB_ID")?;
    let minutes = query_u64(&query, "minutes", 15, 1, 1440)?;
    reply(job_metrics(&api, &id, minutes).await?)
}

pub(crate) async fn post_job_submit(
    State(state): State<Arc<WebState>>,
    Extension(user): Extension<CurrentUser>,
    Json(mut body): Json<Value>,
) -> WebResult {
    if !body.is_object() {
        return Err(WebError::bad_request("任务定义必须是 JSON 对象"));
    }
    let config = state.config().await;
    // spaceid 请求头要与 body 里的 spaceId 一致，平台按头路由
    let space = scalar_string(&body, "spaceId")
        .or_else(|| config.space_id.clone())
        .ok_or_else(|| WebError::bad_request("缺少 spaceId"))?;
    let group = scalar_string(&body, "rsgroupId").unwrap_or_default();
    let gpu = quota::job_gpu(&body);
    if !user.is_admin() {
        let name = user.name.clone();
        let account = tokio::task::spawn_blocking(move || crate::web::accounts::get(&name))
            .await
            .map_err(|_| WebError::bad_request("后台任务异常退出"))??
            .ok_or_else(WebError::unauthorized)?;
        let usage = ownership::current_usage(&state, &user.name).await?;
        quota::check(&account.quota, &usage, &space, &group, gpu).map_err(WebError::forbidden)?;
    }
    let api = state.api_for(Some(&space)).await?;
    // 作业名自动加平台用户名前缀，避免与其他用户的作业混淆
    let prefix = format!("{}-", api.username().await);
    if let Some(obj) = body.as_object_mut() {
        let name = obj.get("jobName").and_then(Value::as_str).unwrap_or_default().to_owned();
        if !name.starts_with(&prefix) {
            obj.insert("jobName".to_owned(), Value::String(format!("{prefix}{name}")));
        }
    }
    let result = submit_job(&api, &config, body).await?;
    let job_id = result
        .pointer("/data/jobId")
        .and_then(value_string)
        .ok_or_else(|| {
            WebError::bad_request(
                "作业已提交，但平台没返回 jobId，无法记录归属和配额；请管理员在平台上核对并处理这个作业",
            )
        })?;
    ownership::record(JobRecord {
        job_id,
        user: user.name.clone(),
        space_id: space,
        rsgroup_id: group,
        gpu,
        created_at: (unix_millis() / 1000) as i64,
        released: false,
    })?;
    reply(result)
}

pub(crate) async fn post_bulk(
    State(state): State<Arc<WebState>>,
    Extension(user): Extension<CurrentUser>,
    Json(body): Json<Value>,
) -> WebResult {
    let action = match body.get("action").and_then(Value::as_str) {
        Some("job-cancel") => BulkAction::JobCancel,
        Some("job-delete") => BulkAction::JobDelete,
        // 开发环境的启停只走 CLI（qd dev start/stop），网页端不提供写操作
        _ => {
            return Err(WebError::bad_request("action 只接受 job-cancel/job-delete"));
        }
    };
    let ids = body
        .get("ids")
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(value_string)
                .collect::<Vec<String>>()
        })
        .filter(|ids| !ids.is_empty())
        .ok_or_else(|| WebError::bad_request("缺少 ids"))?;
    for id in &ids {
        validate_id(id, "ID")?;
        require_owner(&user, id)?;
    }
    let api = state
        .api_for(body.get("spaceId").and_then(Value::as_str))
        .await?;
    let deleting = matches!(action, BulkAction::JobDelete);
    let results = bulk_action(&api, ids, state.config().await.concurrency, action).await;
    if deleting {
        let deleted: Vec<String> = results
            .iter()
            .filter(|r| r.get("ok").and_then(Value::as_bool) == Some(true))
            .filter_map(|r| r.get("id").and_then(value_string))
            .collect();
        ownership::mark_released(&deleted)?;
    }
    reply(Value::Array(results))
}
