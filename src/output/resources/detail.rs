use super::*;

pub(crate) fn print_node_resource_detail(node: &Value) -> Result<()> {
    let rows = vec![
        vec!["节点".to_owned(), field_text(node, &["name"])],
        vec!["IP".to_owned(), field_text(node, &["ip", "accessIp"])],
        vec![
            "集群".to_owned(),
            field_text(node, &["clusterName", "clusterId"]),
        ],
        vec!["可用区".to_owned(), field_text(node, &["azName", "azId"])],
        vec![
            "状态".to_owned(),
            joined_fields(node, &["stat", "statMsg"], " / "),
        ],
        vec!["GPU 型号".to_owned(), joined_value(node.get("gpuModels"))],
        vec![
            "物理卡".to_owned(),
            numeric_pair(node, "gpuUsed", "gpuTotal", "卡", 1.0),
        ],
        vec![
            "虚拟卡".to_owned(),
            numeric_pair(node, "orionRatioUsed", "orionRatioTotal", "卡", 100.0),
        ],
        vec!["剩余整卡".to_owned(), free_gpu_text(node)],
        vec![
            "CPU".to_owned(),
            numeric_pair(node, "cpuRequest", "cpuTotal", "核", 1.0),
        ],
        vec![
            "内存".to_owned(),
            memory_pair(node, "memoryRequest", "memoryTotal"),
        ],
        vec![
            "临时存储".to_owned(),
            memory_pair(node, "storageRequest", "storageTotal"),
        ],
        vec![
            "资源组".to_owned(),
            joined_value(node.get("resourceGroups")),
        ],
        vec!["操作系统".to_owned(), field_text(node, &["osVersion"])],
        vec!["内核".to_owned(), field_text(node, &["kernelVersion"])],
    ];
    print_pretty_table("节点详情", &["字段", "值"], &rows, &[18, 86]);
    print_gpu_devices(node);
    super::jobs::print_node_jobs(node);
    Ok(())
}

fn print_gpu_devices(node: &Value) {
    let devices = node
        .get("gpuList")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let rows = devices
        .iter()
        .map(|gpu| {
            vec![
                field_text(gpu, &["index"]),
                field_text(gpu, &["model"]),
                field_text(gpu, &["vStat"]),
                numeric_pair(gpu, "gpuUsed", "gpuTotal", "卡", 1.0),
                numeric_pair(gpu, "orionRatioUsed", "orionRatioTotal", "卡", 100.0),
                field_text(gpu, &["gpuUtil"]),
                field_text(gpu, &["uuid"]),
            ]
        })
        .collect::<Vec<_>>();
    print_pretty_table(
        &format!("GPU 明细（{} 张）", rows.len()),
        &[
            "编号",
            "型号",
            "形态",
            "物理占用",
            "虚拟占用",
            "利用率",
            "UUID",
        ],
        &rows,
        &[8, 24, 12, 14, 14, 10, 28],
    );
}
