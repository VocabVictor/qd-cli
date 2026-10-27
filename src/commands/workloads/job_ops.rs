use crate::*;

/// 作用域内全部项目的作业，按创建时间倒序分页。走平台原生分页（见 job_paging），
/// 只取目标页需要的数据；status 可以是逗号分隔的多个平台状态（如 STOPPED,FAILED）。
pub(crate) async fn list_scoped_jobs(
    api: &ApiClient,
    config: &Config,
    scope: &str,
    pagination: &Value,
    status: Option<&str>,
) -> Result<Value> {
    let page = pagination.get("pageNum").and_then(value_u64).unwrap_or(1).max(1);
    let size = pagination.get("pageSize").and_then(value_u64).unwrap_or(100).clamp(1, 1000);
    let statuses = parse_statuses(status)?;
    let sources = job_sources(api, scope, &statuses, None, serde_json::Map::new()).await?;
    let result = merged_job_page(sources, page, size, config.concurrency).await;
    Ok(json!({
        "code": 0,
        "msg": "操作成功",
        "data": {"listCount": result.total, "jobList": result.jobs, "scope": scope, "skippedProjects": result.skipped}
    }))
}

/// 逗号分隔的平台状态名，统一大写；只允许字母和下划线，最多 8 个。
pub(crate) fn parse_statuses(raw: Option<&str>) -> Result<Vec<String>> {
    let Some(raw) = raw.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(Vec::new());
    };
    let mut out = Vec::new();
    for item in raw.split(',').map(str::trim).filter(|item| !item.is_empty()) {
        if !item.chars().all(|c| c.is_ascii_alphabetic() || c == '_') {
            bail!("状态只能是平台状态名（如 RUNNING、FAILED），用逗号分隔");
        }
        let item = item.to_ascii_uppercase();
        if !out.contains(&item) {
            out.push(item);
        }
    }
    if out.len() > 8 {
        bail!("一次最多筛选 8 个状态");
    }
    Ok(out)
}

pub(crate) async fn job_show(api: &ApiClient, id: &str) -> Result<Value> {
    let path = format!("/job/detail/{id}");
    api.get(Service::Core, &path, empty_object()).await
}

/// 在任务实例内执行命令（复用网页终端 websocket 传输，供交互调试）。
pub(crate) async fn job_exec(
    api: &ApiClient,
    id: &str,
    instance: usize,
    command: &[String],
) -> Result<()> {
    let terminal_url = job_terminal_url(api, id, instance).await?;
    dev_web_terminal(api, &terminal_url, command).await
}

#[derive(Clone, Copy)]
pub(crate) enum BulkAction {
    JobCancel,
    JobDelete,
    DevStart,
    DevStop,
    JobShow,
}

impl BulkAction {
    pub(crate) fn request(self, id: &str) -> (Method, String, Value) {
        match self {
            Self::JobCancel => (Method::POST, format!("/job/cancel/{id}"), empty_object()),
            Self::JobDelete => (Method::DELETE, format!("/job/delete/{id}"), empty_object()),
            Self::DevStart => (
                Method::POST,
                format!("/jobenv/devJob/start/{id}"),
                empty_object(),
            ),
            Self::DevStop => (
                Method::POST,
                format!("/jobenv/devJob/cancel/{id}"),
                json!({"isSaveImage": 2}),
            ),
            Self::JobShow => (Method::GET, format!("/job/detail/{id}"), empty_object()),
        }
    }
}

pub(crate) async fn bulk_action(
    api: &ApiClient,
    ids: Vec<String>,
    concurrency: usize,
    action: BulkAction,
) -> Vec<Value> {
    let mut pending: VecDeque<String> = ids.into();
    let mut set = JoinSet::new();
    let mut results = Vec::with_capacity(pending.len());
    let limit = concurrency.max(1);

    while !pending.is_empty() || !set.is_empty() {
        while set.len() < limit {
            let Some(id) = pending.pop_front() else { break };
            let client = api.clone();
            set.spawn(async move {
                let (method, path, body) = action.request(&id);
                let result = client.send(method, Service::Core, &path, body).await;
                (id, result)
            });
        }
        if let Some(joined) = set.join_next().await {
            match joined {
                Ok((id, Ok(response))) => {
                    results.push(json!({"id": id, "ok": true, "response": response}))
                }
                Ok((id, Err(error))) => {
                    results.push(json!({"id": id, "ok": false, "error": format!("{error:#}")}))
                }
                Err(error) => results.push(json!({"ok": false, "error": error.to_string()})),
            }
        }
    }
    results
}

pub(crate) async fn wait_jobs(
    api: &ApiClient,
    ids: Vec<String>,
    interval_secs: u64,
    timeout_secs: u64,
    concurrency: usize,
) -> Result<Value> {
    let started = Instant::now();
    loop {
        let results = bulk_action(api, ids.clone(), concurrency, BulkAction::JobShow).await;
        let all_terminal = results.iter().all(result_is_terminal);
        if all_terminal {
            return Ok(Value::Array(results));
        }
        if timeout_secs > 0 && started.elapsed() >= Duration::from_secs(timeout_secs) {
            bail!("等待任务超时（{timeout_secs} 秒）");
        }
        tokio::time::sleep(Duration::from_secs(interval_secs)).await;
    }
}

