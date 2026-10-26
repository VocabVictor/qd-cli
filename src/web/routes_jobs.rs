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

/// 作业列表：跨全部空间聚合（指定 project 时只查其所在空间）。
pub(crate) async fn list_jobs(
    State(state): State<Arc<WebState>>,
    Query(query): Query<HashMap<String, String>>,
) -> WebResult {
    let scope = query_scope(&query)?;
    let page = query_u64(&query, "page", 1, 1, u64::MAX)? as usize;
    let size = query_u64(&query, "size", 100, 1, 1000)? as usize;
    let status = query.get("status").filter(|value| !value.is_empty());

    if let Some(project_id) = query.get("project").filter(|value| !value.is_empty()) {
        validate_id(project_id, "PROJECT_ID")?;
        let api = state.api_for(space_of(&query)).await?;
        let mut request = json!({"pageNum": page, "pageSize": size});
        if let Some(status) = status {
            insert_value(&mut request, "status", Value::String(status.clone()))?;
        }
        let path = format!("/project/job/list/{project_id}");
        return reply(api.get(Service::Core, &path, request).await?);
    }

    let config = state.config().await;
    let pagination = json!({"pageNum": 1, "pageSize": 1000});
    let mut jobs = Vec::new();
    let mut skipped = 0u64;
    let mut failed_spaces = Vec::new();
    for space in state.spaces().await {
        let api = state.api_for(Some(&space.id)).await?;
        match list_scoped_jobs(&api, &config, &scope, &pagination, status.map(String::as_str))
            .await
        {
            Ok(response) => {
                skipped += response
                    .pointer("/data/skippedProjects")
                    .and_then(Value::as_u64)
                    .unwrap_or(0);
                for mut job in response_array(&response, &["/data/jobList"]).to_vec() {
                    attach_space(&mut job, &space);
                    jobs.push(job);
                }
            }
            Err(_) => failed_spaces.push(space.name.clone()),
        }
    }
    // 标注提交者，前端据此决定"我的作业"能不能取消/删除/进终端。
    let owners: HashMap<String, String> = ownership::owners()?.into_iter().collect();
    for job in &mut jobs {
        let id = job.get("jobId").or_else(|| job.get("id")).and_then(value_string);
        if let (Some(id), Some(object)) = (id, job.as_object_mut()) {
            let owner = owners.get(&id).cloned().map(Value::String).unwrap_or(Value::Null);
            object.insert("qdOwner".to_owned(), owner);
        }
    }
    jobs.sort_by(|left, right| {
        value_u64(right.get("createTime").unwrap_or(&Value::Null))
            .cmp(&value_u64(left.get("createTime").unwrap_or(&Value::Null)))
    });
    let total = jobs.len();
    let start = page.saturating_sub(1).saturating_mul(size).min(total);
    let job_list = jobs.into_iter().skip(start).take(size).collect::<Vec<_>>();
    reply(json!({
        "code": 0,
        "data": {
            "listCount": total,
            "jobList": job_list,
            "scope": scope,
            "skippedProjects": skipped,
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
    Json(body): Json<Value>,
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
