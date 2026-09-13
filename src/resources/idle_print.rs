use crate::*;

pub(crate) fn print_idle_resources(groups: &[Value]) {
    let mut ordered: Vec<&Value> = groups.iter().collect();
    ordered.sort_by(|left, right| {
        idle_rank(right).cmp(&idle_rank(left)).then_with(|| {
            left.get("rsgroupName")
                .and_then(Value::as_str)
                .cmp(&right.get("rsgroupName").and_then(Value::as_str))
        })
    });

    let mut rows = Vec::with_capacity(ordered.len());
    for group in ordered {
        let name = group
            .get("rsgroupName")
            .and_then(Value::as_str)
            .or_else(|| group.get("rsgroupId").and_then(Value::as_str))
            .unwrap_or("未知资源组");
        if group.get("ok").and_then(Value::as_bool) != Some(true) {
            let error = group
                .get("error")
                .and_then(Value::as_str)
                .unwrap_or("查询失败");
            rows.push(vec![
                "查询失败".to_owned(),
                name.to_owned(),
                error.to_owned(),
                "-".to_owned(),
                "-".to_owned(),
                "-".to_owned(),
            ]);
            continue;
        }

        let label = match idle_rank(group) {
            2 => "GPU 可用",
            1 => "CPU 可用",
            _ => "无可用资源",
        };
        let cpu = group
            .pointer("/idle/cpuCores")
            .and_then(Value::as_f64)
            .unwrap_or(0.0);
        let memory = group
            .pointer("/idle/memoryMiB")
            .and_then(Value::as_f64)
            .unwrap_or(0.0);
        let nodes = group
            .pointer("/capacity/nodes")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        rows.push(vec![
            label.to_owned(),
            name.to_owned(),
            idle_gpu_text(group),
            format!("{} 核", format_number(cpu)),
            format_memory_mib(memory),
            nodes.to_string(),
        ]);
    }
    print_pretty_table(
        &format!("当前可调度闲置资源（{} 个资源组）", rows.len()),
        &[
            "状态",
            "资源组",
            "空闲 GPU（整组 / 单实例上限）",
            "空闲 CPU",
            "空闲内存",
            "节点",
        ],
        &rows,
        &[12, 22, 66, 12, 14, 8],
    );
    println!(
        "说明：整组 = 全部节点空闲整卡之和；单实例上限 = 一个实例最多能申请的卡数，受最好的\
         单节点限制。两者不等时要跨节点提多实例作业才能吃满，例如整组 8 / 单实例 4 → 2 实例 × 4 卡。"
    );
    println!("提示：`qd idle --gpu-only` 只显示有空闲 GPU 的资源组；`--compact` 输出 JSON。");
}

pub(crate) fn idle_rank(group: &Value) -> u8 {
    if group.get("gpuAvailable").and_then(Value::as_bool) == Some(true) {
        2
    } else if group.get("available").and_then(Value::as_bool) == Some(true) {
        1
    } else {
        0
    }
}

pub(crate) fn idle_gpu_text(group: &Value) -> String {
    let total = group
        .pointer("/capacity/gpuCount")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let Some(gpus) = group.pointer("/idle/gpus").and_then(Value::as_array) else {
        return "无 GPU".to_owned();
    };
    if gpus.is_empty() {
        return "无 GPU".to_owned();
    }
    let whole = group
        .pointer("/idle/gpuWholeCardsFree")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let nodes_with_free = group
        .pointer("/idle/nodesWithFreeGpu")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let values = gpus
        .iter()
        .map(|gpu| {
            let name = gpu
                .get("graphicsCard")
                .and_then(Value::as_str)
                .unwrap_or("未知型号");
            let per_instance = gpu.get("count").and_then(Value::as_u64).unwrap_or(0);
            format!("{name} 整组 {whole} / 单实例 {per_instance}")
        })
        .collect::<Vec<_>>()
        .join("，");
    // Flag the split case explicitly -- reading the per-instance ceiling as the
    // group total is exactly the mistake this column used to invite.
    let spread = if whole > 0 && nodes_with_free > 1 {
        format!("，分布在 {nodes_with_free} 个节点")
    } else {
        String::new()
    };
    format!("{values}{spread}（总 {total}）")
}
