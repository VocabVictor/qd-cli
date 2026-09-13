use crate::*;

pub(crate) async fn idle_resource_groups(
    api: &ApiClient,
    ids: Vec<String>,
    concurrency: usize,
) -> Vec<Value> {
    let mut pending: VecDeque<String> = ids.into();
    let mut set = JoinSet::new();
    let mut results = Vec::with_capacity(pending.len());
    let limit = concurrency.max(1);

    while !pending.is_empty() || !set.is_empty() {
        while set.len() < limit {
            let Some(id) = pending.pop_front() else {
                break;
            };
            let client = api.clone();
            set.spawn(async move {
                let result = idle_resource_group(&client, &id).await;
                (id, result)
            });
        }
        if let Some(joined) = set.join_next().await {
            match joined {
                Ok((_, Ok(group))) => results.push(group),
                Ok((id, Err(error))) => results.push(json!({
                    "rsgroupId": id,
                    "ok": false,
                    "available": false,
                    "error": format!("{error:#}")
                })),
                Err(error) => results.push(json!({
                    "ok": false,
                    "available": false,
                    "error": error.to_string()
                })),
            }
        }
    }
    results
}

pub(crate) async fn idle_resource_group(api: &ApiClient, id: &str) -> Result<Value> {
    let path = format!("/rsgroup/detail/{id}");
    let detail_response = api.get(Service::Core, &path, empty_object()).await?;
    let detail = detail_response
        .get("data")
        .and_then(Value::as_object)
        .context("资源组详情响应缺少 data")?;

    let cpu_stock = api
        .send(
            Method::POST,
            Service::Core,
            "/job/jobResourceStock",
            stock_query(id, 2, 1, ""),
        )
        .await?;
    let idle_cpu = cpu_stock
        .pointer("/data/cpu")
        .cloned()
        .unwrap_or(Value::Null);
    let idle_memory = cpu_stock
        .pointer("/data/memory")
        .cloned()
        .unwrap_or(Value::Null);
    let schedule_rule = cpu_stock
        .pointer("/data/orionScheduleRule")
        .cloned()
        .unwrap_or(Value::Null);

    let graphics_cards = parse_graphics_cards(
        detail
            .get("graphicsCards")
            .and_then(Value::as_str)
            .unwrap_or_default(),
    );
    let mut idle_gpus = Vec::with_capacity(graphics_cards.len());
    for graphics_card in &graphics_cards {
        let stock = api
            .send(
                Method::POST,
                Service::Core,
                "/job/jobResourceStock",
                stock_query(id, 1, 2, graphics_card),
            )
            .await?;
        idle_gpus.push(json!({
            "graphicsCard": graphics_card,
            "count": stock.pointer("/data/gpuCount").cloned().unwrap_or(Value::Null)
        }));
    }

    // jobResourceStock answers "how many GPUs can ONE instance ask for", which is
    // capped by the best single node -- not the group total. Walk the node list too
    // so callers can tell "8 free, but 4+4 on two nodes" from "8 free on one node".
    // Best-effort: if the node listing is unavailable we still report the
    // per-instance ceiling rather than failing the whole group.
    let node_gpus = if graphics_cards.is_empty() {
        NodeGpuSummary::default()
    } else {
        node_gpu_summary(api, id).await.unwrap_or_default()
    };

    let gpu_available = idle_gpus
        .iter()
        .any(|gpu| gpu.get("count").and_then(Value::as_f64).unwrap_or(0.0) > 0.0)
        || node_gpus.total > 0;
    let available = idle_cpu.as_f64().unwrap_or(0.0) > 0.0 || gpu_available;
    let cpu_millicores = detail.get("cpuTotal").cloned().unwrap_or(Value::Null);
    let total_cpu_cores = cpu_millicores.as_f64().map(|value| value / 1000.0);

    Ok(json!({
        "ok": true,
        "available": available,
        "gpuAvailable": gpu_available,
        "rsgroupId": id,
        "rsgroupName": detail.get("rsgroupName").cloned().unwrap_or(Value::Null),
        "capacity": {
            "nodes": detail.get("nodeCount").cloned().unwrap_or(Value::Null),
            "cpuMillicores": cpu_millicores,
            "cpuCores": total_cpu_cores,
            "memoryMiB": detail.get("memoryTotal").cloned().unwrap_or(Value::Null),
            "gpuCount": detail.get("gpuTotal").cloned().unwrap_or(Value::Null),
            "gpuTypes": graphics_cards
        },
        "idle": {
            "cpuCores": idle_cpu,
            "memoryMiB": idle_memory,
            // gpus[].count is the PER-INSTANCE ceiling, not the group total.
            "gpus": idle_gpus,
            "gpuWholeCardsFree": node_gpus.total,
            "gpuMaxOnOneNode": node_gpus.max_per_node,
            "nodesWithFreeGpu": node_gpus.nodes_with_free
        },
        "scheduleRule": schedule_rule
    }))
}

#[derive(Default)]
pub(crate) struct NodeGpuSummary {
    pub(crate) total: u64,
    pub(crate) max_per_node: u64,
    pub(crate) nodes_with_free: u64,
}

/// Sum whole free cards across the group's nodes. `rsgroup` takes the group ID.
pub(crate) async fn node_gpu_summary(api: &ApiClient, id: &str) -> Result<NodeGpuSummary> {
    let query = json!({ "pageNum": 1, "pageSize": 500, "rsgroup": id });
    let response = api.get(Service::Core, "/jobQueue/node/job/list", query).await?;
    let mut summary = NodeGpuSummary::default();
    let Some(nodes) = response.pointer("/data/nodes").and_then(Value::as_array) else {
        return Ok(summary);
    };
    for node in nodes {
        let left = node.get("gpuLeft").and_then(Value::as_u64).unwrap_or(0);
        if left == 0 {
            continue;
        }
        summary.total += left;
        summary.max_per_node = summary.max_per_node.max(left);
        summary.nodes_with_free += 1;
    }
    Ok(summary)
}

pub(crate) fn stock_query(
    rsgroup_id: &str,
    main_resource_limit: u64,
    resource_type: u64,
    graphics_card: &str,
) -> Value {
    json!({
        "mainResourceLimit": main_resource_limit,
        "currentRoleIndex": 0,
        "rsgroupId": rsgroup_id,
        "taskRoleList": [{
            "index": 0,
            "instanceCount": 1,
            "resourceType": resource_type,
            "GPU": graphics_card
        }]
    })
}

pub(crate) fn parse_graphics_cards(value: &str) -> Vec<String> {
    value
        .split([',', ';', '|', '、'])
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .collect()
}
