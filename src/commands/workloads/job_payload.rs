use crate::*;

pub(crate) fn result_is_terminal(result: &Value) -> bool {
    if result.get("ok").and_then(Value::as_bool) != Some(true) {
        return true;
    }
    let response = &result["response"];
    let status = response
        .pointer("/data/status")
        .or_else(|| response.pointer("/data/jobStatus"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_ascii_uppercase();
    matches!(
        status.as_str(),
        "STOPPED" | "FAILED" | "SUCCEEDED" | "SUCCESS" | "CANCELLED" | "CANCELED" | "DELETED"
    )
}

pub(crate) fn job_template(config: &Config) -> Value {
    json!({
        "jobName": "gpu-demo",
        "trainType": 1,
        "spaceId": config.space_id,
        "projectId": "PROJECT_ID",
        "rsgroupId": "请用 qd resource groups 查询",
        "description": "submitted by qd",
        "imageId": 0,
        "maxRunHour": 1,
        "maxRetryCount": 0,
        "datasetInData": [],
        "preInData": {"dataType": 0, "dataPath": "", "dataBucket": ""},
        "services": [],
        "taskroles": [{
            "instance": 1,
            "runScript": "python /gemini/code/infer.py",
            "customEnv": "",
            "cpu": 4,
        "memory": 8192,
        "storage": 20480,
        "gpu": 1,
        "gpuType": "NVIDIA H800 80GB"
        }]
    })
}

pub(crate) async fn resolve_submitted_job(
    api: &ApiClient,
    project_id: &str,
    job_name: &str,
    submitted_at: u64,
) -> Option<String> {
    let path = format!("/project/job/list/{project_id}");
    for attempt in 0..20 {
        if attempt > 0 {
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
        let Ok(response) = api
            .get(Service::Core, &path, json!({"pageNum": 1, "pageSize": 100}))
            .await
        else {
            continue;
        };
        let Some(jobs) = response.pointer("/data/jobList").and_then(Value::as_array) else {
            continue;
        };
        let candidate = jobs
            .iter()
            .filter(|job| job.get("jobName").and_then(Value::as_str) == Some(job_name))
            .filter_map(|job| {
                let created = job.get("createTime").and_then(value_u64)?;
                if created.saturating_add(60_000) < submitted_at {
                    return None;
                }
                let id = job.get("jobId").and_then(value_string)?;
                Some((created, id))
            })
            .max_by_key(|(created, _)| *created);
        if let Some((_, id)) = candidate {
            return Some(id);
        }
    }
    None
}

pub(crate) fn attach_job_id(result: &mut Value, id: &str) -> Result<()> {
    let object = result.as_object_mut().context("平台响应顶层不是对象")?;
    let data = object.entry("data").or_insert_with(|| json!({}));
    if !data.is_object() {
        *data = json!({});
    }
    data.as_object_mut()
        .expect("data was normalized to an object")
        .insert("jobId".to_owned(), Value::String(id.to_owned()));
    Ok(())
}
