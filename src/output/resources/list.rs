use super::*;

pub(crate) fn print_node_resource_list(response: &Value, include_jobs: bool) -> Result<()> {
    print_cluster_resource_summary(response);
    let nodes = response_array(response, &["/data/nodes"]);
    let rows = nodes.iter().map(node_resource_row).collect::<Vec<_>>();
    let total = response_count(response, &["/data/total"], rows.len());
    print_pretty_table(
        &format!("节点资源（本页 {} 个，共 {total} 个）", rows.len()),
        &[
            "节点",
            "GPU 型号",
            "物理卡",
            "虚拟卡",
            "剩余整卡",
            "CPU 申请/总量",
            "内存申请/总量",
            "临时存储",
            "资源组",
            "任务",
        ],
        &rows,
        &[18, 24, 13, 13, 16, 18, 22, 22, 22, 8],
    );
    if include_jobs {
        for node in nodes {
            super::jobs::print_node_jobs(node);
        }
    } else if !nodes.is_empty() {
        println!("提示：加 `--jobs` 展开本页任务，或运行 `qd resource node 节点名` 查看详情。");
    }
    Ok(())
}

fn print_cluster_resource_summary(response: &Value) {
    let data = response.get("data").unwrap_or(response);
    let p_total = numeric_field(data, &["pGpuTotal"]).unwrap_or(0.0);
    let p_used = numeric_field(data, &["pGpuUsed"]).unwrap_or(0.0);
    let v_total = numeric_field(data, &["orionRatioTotal"]).unwrap_or(0.0) / 100.0;
    let v_used = numeric_field(data, &["orionRatioUsed"]).unwrap_or(0.0) / 100.0;
    let cpu_total = numeric_field(data, &["cpuTotal"]).unwrap_or(0.0) / 1000.0;
    let cpu_used = numeric_field(data, &["cpuUsed"]).unwrap_or(0.0) / 1000.0;
    let memory_total = numeric_field(data, &["memoryTotal"]).unwrap_or(0.0);
    let memory_used = numeric_field(data, &["memoryUsed"]).unwrap_or(0.0);
    let rows = vec![
        summary_row("物理 GPU", p_used, p_total, "卡", false),
        summary_row("虚拟 GPU", v_used, v_total, "卡", false),
        summary_row("CPU", cpu_used, cpu_total, "核", false),
        summary_row("内存", memory_used, memory_total, "", true),
    ];
    print_pretty_table(
        "系统资源汇总",
        &["资源", "可用", "已用", "总量"],
        &rows,
        &[14, 20, 20, 20],
    );
}

fn summary_row(name: &str, used: f64, total: f64, unit: &str, memory: bool) -> Vec<String> {
    let format = |value| {
        if memory {
            format_memory_mib(value)
        } else {
            format!("{} {unit}", format_number(value.max(0.0)))
        }
    };
    vec![
        name.to_owned(),
        format((total - used).max(0.0)),
        format(used),
        format(total),
    ]
}

fn node_resource_row(node: &Value) -> Vec<String> {
    vec![
        node_name(node),
        joined_value(node.get("gpuModels")),
        numeric_pair(node, "gpuUsed", "gpuTotal", "卡", 1.0),
        numeric_pair(node, "orionRatioUsed", "orionRatioTotal", "卡", 100.0),
        free_gpu_text(node),
        numeric_pair(node, "cpuRequest", "cpuTotal", "核", 1.0),
        memory_pair(node, "memoryRequest", "memoryTotal"),
        memory_pair(node, "storageRequest", "storageTotal"),
        joined_value(node.get("resourceGroups")),
        node.get("jobList")
            .and_then(Value::as_array)
            .map(|jobs| jobs.len().to_string())
            .unwrap_or_else(|| "0".to_owned()),
    ]
}
