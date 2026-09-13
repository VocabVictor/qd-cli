use crate::*;

pub(crate) async fn list_scoped_jobs(
    api: &ApiClient,
    config: &Config,
    scope: &str,
    pagination: &Value,
    status: Option<&str>,
) -> Result<Value> {
    let projects = scoped_projects(api, scope).await?;
    let mut pending = VecDeque::new();
    for project in projects {
        let project_id = field_text(&project, &["projectId", "id"]);
        if project_id == "-" {
            continue;
        }
        validate_id(&project_id, "PROJECT_ID")?;
        let project_name = field_text(&project, &["projectName", "name"]);
        pending.push_back((project_id, project_name));
    }

    let mut set = JoinSet::new();
    let mut jobs = Vec::new();
    // shared/public/all 范围下部分项目对当前账号无查询权限（平台错误 101000），
    // 单个项目失败只计数跳过，不让整个聚合报废。
    let mut skipped = 0u64;
    let limit = config.concurrency.max(1);
    while !pending.is_empty() || !set.is_empty() {
        while set.len() < limit {
            let Some((project_id, project_name)) = pending.pop_front() else {
                break;
            };
            let client = api.clone();
            let status = status.map(str::to_owned);
            set.spawn(async move {
                let path = format!("/project/job/list/{project_id}");
                let mut query = json!({"pageNum": 1, "pageSize": 1000});
                if let Some(status) = status {
                    query["status"] = Value::String(status);
                }
                let response = client.get(Service::Core, &path, query).await?;
                Ok::<_, anyhow::Error>((project_id, project_name, response))
            });
        }
        if let Some(joined) = set.join_next().await {
            match joined.context("查询我的任务失败")? {
                Ok((project_id, project_name, response)) => {
                    for mut job in
                        response_array(&response, &["/data/jobList", "/data/list"]).to_vec()
                    {
                        if let Some(object) = job.as_object_mut() {
                            object
                                .entry("projectId")
                                .or_insert_with(|| Value::String(project_id.clone()));
                            object
                                .entry("projectName")
                                .or_insert_with(|| Value::String(project_name.clone()));
                        }
                        jobs.push(job);
                    }
                }
                Err(_) => skipped += 1,
            }
        }
    }

    jobs.sort_by(|left, right| {
        value_u64(right.get("createTime").unwrap_or(&Value::Null))
            .cmp(&value_u64(left.get("createTime").unwrap_or(&Value::Null)))
    });
    let total = jobs.len();
    let page = pagination
        .get("pageNum")
        .and_then(Value::as_u64)
        .unwrap_or(1) as usize;
    let size = pagination
        .get("pageSize")
        .and_then(Value::as_u64)
        .unwrap_or(100) as usize;
    let start = page.saturating_sub(1).saturating_mul(size).min(total);
    let job_list = jobs.into_iter().skip(start).take(size).collect::<Vec<_>>();
    Ok(json!({
        "code": 0,
        "msg": "操作成功",
        "data": {"listCount": total, "jobList": job_list, "scope": scope, "skippedProjects": skipped}
    }))
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

