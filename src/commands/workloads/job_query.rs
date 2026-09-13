use crate::*;

/// 提交任务：补 spaceId、POST /job/new、必要时回查 jobId 并塞回 data.jobId。
pub(crate) async fn submit_job(api: &ApiClient, config: &Config, mut body: Value) -> Result<Value> {
    insert_if_missing(
        &mut body,
        "spaceId",
        config.space_id.clone().map(Value::String),
    )?;
    let project_id = scalar_string(&body, "projectId");
    let job_name = scalar_string(&body, "jobName");
    let submitted_at = unix_millis();
    let mut result = api
        .send(Method::POST, Service::Core, "/job/new", body)
        .await?;
    let mut id = find_id(&result, &["jobId", "id"]);
    if id.is_none()
        && let (Some(project_id), Some(job_name)) = (project_id, job_name)
    {
        id = resolve_submitted_job(api, &project_id, &job_name, submitted_at).await;
    }
    if let Some(id) = &id {
        attach_job_id(&mut result, id)?;
    }
    Ok(result)
}

/// 作业级监控（与网页"监控"页同源）：/user/job/chart/:metric，
/// tool 服务，时间戳用【秒】——毫秒会静默返回空 series。
pub(crate) async fn job_metrics(api: &ApiClient, id: &str, minutes: u64) -> Result<Value> {
    let end = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    let start = end.saturating_sub(minutes * 60);
    let mut metrics = Vec::new();
    for (metric, label, unit) in [
        ("gpu_util", "GPU利用率", "%"),
        ("gpu_memory", "GPU显存", "%"),
        ("cpu", "CPU", "%"),
        ("memory", "内存", "%"),
    ] {
        let value = api
            .get(
                Service::Tool,
                &format!("/user/job/chart/{metric}"),
                json!({
                    "jobId": id,
                    "startTime": start,
                    "endTime": end,
                }),
            )
            .await?;
        let points: Vec<f64> = value
            .pointer("/option/series")
            .and_then(|s| s.as_array())
            .map(|series| {
                series
                    .iter()
                    .flat_map(|line| {
                        line.pointer("/data")
                            .and_then(|d| d.as_array())
                            .into_iter()
                            .flatten()
                    })
                    .filter_map(|p| {
                        p.pointer("/value")
                            .and_then(|v| v.as_str())
                            .and_then(|v| v.parse::<f64>().ok())
                    })
                    .collect()
            })
            .unwrap_or_default();
        let last = points.last().copied();
        let avg = (!points.is_empty()).then(|| points.iter().sum::<f64>() / points.len() as f64);
        let max = points.iter().cloned().fold(None::<f64>, |acc, v| {
            Some(acc.map_or(v, |a| a.max(v)))
        });
        metrics.push(json!({
            "metric": metric,
            "label": label,
            "unit": unit,
            "points": points,
            "last": last,
            "avg": avg,
            "max": max,
        }));
    }
    Ok(json!({"jobId": id, "minutes": minutes, "metrics": metrics}))
}

/// 解析任务实例的网页终端地址（要求 RUNNING）。
pub(crate) async fn job_terminal_url(api: &ApiClient, id: &str, instance: usize) -> Result<String> {
    let detail = job_show(api, id).await?;
    let data = detail.get("data").unwrap_or(&detail);
    let status = data.get("status").and_then(Value::as_str).unwrap_or("");
    if status != "RUNNING" {
        bail!("任务 {id} 当前状态为 {status}，需 RUNNING 才能连接终端");
    }
    let ptr = format!("/taskroleList/0/instanceList/{instance}/terminalUrl");
    data.pointer(&ptr)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty() && *value != "-")
        .context("任务未返回网页终端地址（实例可能未就绪）")
        .map(str::to_owned)
}

/// 拉取任务实例的运行日志（tool 服务 /user/job/log/newList）。
/// 从任务详情解析 instanceId，按时间正序分页收集每行 message。
pub(crate) async fn collect_job_logs(api: &ApiClient, id: &str, limit: u64) -> Result<Vec<String>> {
    let detail = job_show(api, id).await?;
    let data = detail.get("data").unwrap_or(&detail);
    let instance_id = data
        .pointer("/taskroleList/0/instanceList/0/instanceId")
        .and_then(Value::as_i64)
        .context("无法解析 instanceId（任务可能尚未调度或没有实例）")?;
    let mut start = data
        .get("startTime")
        .and_then(Value::as_i64)
        .filter(|v| *v > 0)
        .or_else(|| data.get("createTime").and_then(Value::as_i64))
        .unwrap_or(0);
    let end = unix_millis() as i64 + 60_000;
    let page = limit.clamp(1, 1000);
    let mut lines = Vec::new();
    for _ in 0..500 {
        let query = json!({
            "instanceId": instance_id,
            "startTime": start,
            "endTime": end,
            "limit": page,
            "sort": "FORWARD"
        });
        let resp = api
            .get(Service::Tool, "/user/job/log/newList", query)
            .await?;
        let list = resp
            .pointer("/data/logList")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        if list.is_empty() {
            break;
        }
        let mut last_ts_ns: i64 = 0;
        for item in &list {
            if let Some(msg) = item.get("message").and_then(Value::as_str) {
                lines.push(msg.to_owned());
            }
            if let Some(ts) = item.get("timestamp").and_then(Value::as_i64) {
                last_ts_ns = last_ts_ns.max(ts);
            }
        }
        if (list.len() as u64) < page {
            break;
        }
        let next = last_ts_ns / 1_000_000 + 1;
        if next <= start {
            break;
        }
        start = next;
    }
    Ok(lines)
}

pub(crate) async fn job_logs(api: &ApiClient, id: &str, limit: u64) -> Result<()> {
    let lines = collect_job_logs(api, id, limit).await?;
    if lines.is_empty() {
        eprintln!("(无日志)");
    }
    for line in &lines {
        println!("{line}");
    }
    Ok(())
}

/// 打印任务各实例的 SSH 连接信息（绕过默认脱敏，供交互调试用）。
pub(crate) async fn job_ssh(api: &ApiClient, id: &str) -> Result<()> {
    let detail = job_show(api, id).await?;
    let data = detail.get("data").unwrap_or(&detail);
    let taskroles = data
        .get("taskroleList")
        .and_then(Value::as_array)
        .context("任务详情缺少 taskroleList（任务可能尚未运行）")?;
    let mut any = false;
    for tr in taskroles {
        let Some(list) = tr.get("instanceList").and_then(Value::as_array) else {
            continue;
        };
        for inst in list {
            any = true;
            let name = inst
                .get("instanceName")
                .and_then(Value::as_str)
                .unwrap_or("");
            let node = inst.get("nodeIp").and_then(Value::as_str).unwrap_or("");
            let status = inst.get("status").and_then(Value::as_str).unwrap_or("");
            let port = inst.get("sshPort").and_then(Value::as_i64).unwrap_or(0);
            let conn = inst
                .get("sshConnection")
                .and_then(Value::as_str)
                .unwrap_or("");
            let term = inst.get("terminalUrl").and_then(Value::as_str).unwrap_or("");
            println!("instance {name}  status={status}  node={node}  sshPort={port}");
            println!("  sshConnection: {conn}");
            if !term.is_empty() {
                println!("  terminalUrl:   {term}");
            }
        }
    }
    if !any {
        eprintln!("(没有实例信息)");
    }
    Ok(())
}
