use super::*;

pub(super) fn print_node_jobs(node: &Value) {
    let jobs = node
        .get("jobList")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let rows = jobs.iter().map(job_row).collect::<Vec<_>>();
    print_pretty_table(
        &format!(
            "节点 {} 上的任务（{} 个）",
            field_text(node, &["name"]),
            rows.len()
        ),
        &[
            "任务名称",
            "任务 ID",
            "类型",
            "状态",
            "实例",
            "GPU",
            "CPU",
            "内存",
            "资源组",
            "运行时长",
            "项目/空间",
            "创建者",
        ],
        &rows,
        &[26, 20, 12, 12, 10, 30, 10, 14, 18, 14, 28, 14],
    );
}

fn job_row(job: &Value) -> Vec<String> {
    let gpu = field_text(job, &["gpuModel"]);
    let indexes = field_text(job, &["gpuIndexes"]);
    let gpu = if gpu == "-" {
        "-".to_owned()
    } else if indexes == "-" {
        gpu
    } else {
        format!("{gpu} [{indexes}]")
    };
    job_row_with_gpu(job, gpu)
}

fn job_row_with_gpu(job: &Value, gpu: String) -> Vec<String> {
    let kind = match numeric_field(job, &["jobType"]).unwrap_or(0.0) as u64 {
        1 => "离线任务".to_owned(),
        2 => "开发环境".to_owned(),
        3 => "可视化".to_owned(),
        5 => "推理服务".to_owned(),
        value => value.to_string(),
    };
    let uptime = numeric_field(job, &["uptime"])
        .map(format_duration)
        .unwrap_or_else(|| "-".to_owned());
    vec![
        field_text(job, &["jobName"]),
        field_text(job, &["jobDisplayId", "jobId"]),
        kind,
        field_text(job, &["status"]),
        format!(
            "{} / {}",
            field_text(job, &["instanceOn"]),
            field_text(job, &["instanceTotal"])
        ),
        gpu,
        numeric_field(job, &["cpu"])
            .map(|value| format!("{} 核", format_number(value)))
            .unwrap_or_else(|| "-".to_owned()),
        numeric_field(job, &["memory"])
            .map(format_memory_mib)
            .unwrap_or_else(|| "-".to_owned()),
        field_text(job, &["rsgroupName"]),
        uptime,
        joined_fields(job, &["projectName", "spaceName"], " / "),
        field_text(job, &["displayName", "userName"]),
    ]
}
