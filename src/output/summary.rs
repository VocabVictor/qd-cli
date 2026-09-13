use crate::*;

pub(crate) fn format_memory_mib(value: f64) -> String {
    if value >= 1024.0 * 1024.0 {
        format!("{:.2} TiB", value / 1024.0 / 1024.0)
    } else if value >= 1024.0 {
        format!("{:.2} GiB", value / 1024.0)
    } else {
        format!("{} MiB", format_number(value))
    }
}

pub(crate) fn format_number(value: f64) -> String {
    if value.fract() == 0.0 {
        format!("{value:.0}")
    } else {
        format!("{value:.2}")
    }
}

pub(crate) fn print_detail_response(
    response: &Value,
    compact: bool,
    full: bool,
    summarize: fn(&Value) -> Value,
    title: &str,
) -> Result<()> {
    if full {
        return print_json(response, compact);
    }
    let summary = summarize(response);
    if compact {
        print_json(&summary, true)
    } else {
        print_summary_table(title, &summary)
    }
}

pub(crate) fn print_summary_table(title: &str, summary: &Value) -> Result<()> {
    let object = summary.as_object().context("摘要必须是 JSON 对象")?;
    let rows = object
        .iter()
        .map(|(key, value)| {
            vec![
                summary_label(key).to_owned(),
                value_string(value).unwrap_or_else(|| "-".to_owned()),
            ]
        })
        .collect::<Vec<_>>();
    print_pretty_table(title, &["字段", "值"], &rows, &[18, 86]);
    Ok(())
}

pub(crate) fn summary_label(key: &str) -> &str {
    match key {
        "id" => "ID",
        "name" => "名称",
        "project" => "项目",
        "space" => "空间",
        "status" => "状态",
        "resourceGroup" => "资源组",
        "image" => "镜像",
        "resources" => "资源配置",
        "jobId" => "当前任务 ID",
        "node" => "节点",
        "tools" => "开发者工具",
        "services" => "服务数",
        "owner" => "负责人",
        "jobs" => "任务数",
        "nodes" => "节点数",
        "cpu" => "CPU 总量",
        "memory" => "内存总量",
        "gpu" => "GPU 总量",
        "network" => "网络模式",
        _ => key,
    }
}

pub(crate) fn response_data(response: &Value) -> &Value {
    response.get("data").unwrap_or(response)
}

pub(crate) fn pointer_text(value: &Value, pointers: &[&str]) -> String {
    pointers
        .iter()
        .find_map(|pointer| value.pointer(pointer).and_then(value_string))
        .filter(|text| !text.is_empty())
        .unwrap_or_else(|| "-".to_owned())
}

pub(crate) fn joined_pointer_text(value: &Value, pointers: &[&str]) -> String {
    let values = pointers
        .iter()
        .filter_map(|pointer| value.pointer(pointer).and_then(value_string))
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>();
    if values.is_empty() {
        "-".to_owned()
    } else {
        values.join(" / ")
    }
}

pub(crate) fn array_text(value: &Value, pointer: &str) -> String {
    value
        .pointer(pointer)
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(value_string)
                .collect::<Vec<_>>()
                .join(", ")
        })
        .filter(|text| !text.is_empty())
        .unwrap_or_else(|| "-".to_owned())
}

pub(crate) fn spec_summary(value: &Value) -> String {
    let spec = [
        "/specInstanceInfo",
        "/devJobDetail/taskroleList/0/specInstanceInfo",
        "/taskroleList/0/specInstanceInfo",
    ]
    .iter()
    .find_map(|pointer| value.pointer(pointer).filter(|item| item.is_object()));
    let Some(spec) = spec else {
        return "-".to_owned();
    };
    let mut parts = Vec::new();
    if let Some(cpu) = numeric_field(spec, &["cpu"]) {
        parts.push(format!("CPU {}", format_number(cpu)));
    }
    if let Some(memory) = numeric_field(spec, &["memory"]) {
        parts.push(format!("内存 {}", format_memory_mib(memory)));
    }
    if let Some(gpu) = numeric_field(spec, &["gpu"])
        && gpu > 0.0
    {
        let gpu_type = field_text(spec, &["gpuType", "graphicsCard"]);
        if gpu_type == "-" {
            parts.push(format!("GPU {}", format_number(gpu)));
        } else {
            parts.push(format!("{gpu_type} × {}", format_number(gpu)));
        }
    }
    if parts.is_empty() {
        "-".to_owned()
    } else {
        parts.join("，")
    }
}

pub(crate) fn project_summary(response: &Value) -> Value {
    let data = response_data(response);
    json!({
        "id": pointer_text(data, &["/projectId", "/id"]),
        "name": pointer_text(data, &["/projectName", "/name"]),
        "space": pointer_text(data, &["/spaceName", "/spaceId"]),
        "status": joined_pointer_text(data, &["/jobenvStatus", "/jobenvSubStatus"]),
        "jobs": pointer_text(data, &["/jobCount"]),
        "owner": pointer_text(data, &["/ownerDisplayName", "/createDisplayName", "/createUserName"])
    })
}

pub(crate) fn job_summary(response: &Value) -> Value {
    let data = response_data(response);
    json!({
        "id": pointer_text(data, &["/jobId", "/id"]),
        "name": pointer_text(data, &["/jobName", "/name"]),
        "project": pointer_text(data, &["/projectName", "/projectId"]),
        "space": pointer_text(data, &["/spaceName", "/spaceId"]),
        "status": joined_pointer_text(data, &["/status", "/subStatus", "/statusMsg"]),
        "resourceGroup": pointer_text(data, &["/rsgroupName", "/rsgroupId"]),
        "image": pointer_text(data, &["/imageDesc", "/imageId"]),
        "resources": spec_summary(data),
        "owner": pointer_text(data, &["/createDisplayName", "/createUserName"])
    })
}

pub(crate) fn dev_summary(response: &Value) -> Value {
    let data = response_data(response);
    json!({
        "id": pointer_text(data, &["/jobenvId", "/id"]),
        "name": pointer_text(data, &["/jobenvName", "/name"]),
        "project": pointer_text(data, &["/projectName", "/projectId"]),
        "space": pointer_text(data, &["/spaceName", "/spaceId"]),
        "status": joined_pointer_text(data, &["/status", "/subStatus", "/statusMsg"]),
        "resourceGroup": pointer_text(data, &["/rsgroupName", "/rsgroupId"]),
        "image": pointer_text(data, &["/imageDesc", "/imageId"]),
        "resources": spec_summary(data),
        "jobId": pointer_text(data, &["/devJobDetail/jobId"]),
        "node": pointer_text(data, &["/nodeName", "/nodeIp"]),
        "tools": array_text(data, "/tools"),
        "services": data.pointer("/services").and_then(Value::as_array).map_or(0, Vec::len)
    })
}

pub(crate) fn resource_group_summary(response: &Value) -> Value {
    let data = response_data(response);
    let cpu = numeric_field(data, &["cpuTotal"])
        .map(|value| format!("{} 核", format_number(value / 1000.0)))
        .unwrap_or_else(|| "-".to_owned());
    let memory = numeric_field(data, &["memoryTotal"])
        .map(format_memory_mib)
        .unwrap_or_else(|| "-".to_owned());
    let gpu_count = numeric_field(data, &["gpuTotal"]).unwrap_or(0.0);
    let gpu_type = field_text(data, &["graphicsCards"]);
    let gpu = if gpu_count <= 0.0 {
        "无 GPU".to_owned()
    } else if gpu_type == "-" {
        format!("{} 张", format_number(gpu_count))
    } else {
        format!("{gpu_type} × {}", format_number(gpu_count))
    };
    json!({
        "id": pointer_text(data, &["/rsgroupId", "/id"]),
        "name": pointer_text(data, &["/rsgroupName", "/name"]),
        "nodes": pointer_text(data, &["/nodeCount"]),
        "cpu": cpu,
        "memory": memory,
        "gpu": gpu,
        "network": pointer_text(data, &["/networkMode"])
    })
}
