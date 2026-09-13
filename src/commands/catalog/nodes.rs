use crate::*;

const NODE_RESOURCE_PATH: &str = "/jobQueue/node/job/list";

pub(crate) async fn resource_nodes_command(
    api: &ApiClient,
    args: &mut Args,
    compact: bool,
) -> Result<()> {
    let include_jobs = args.take_flag("--jobs");
    let query = node_resource_query(args)?;
    args.ensure_empty()?;
    let response = api.get(Service::Core, NODE_RESOURCE_PATH, query).await?;
    if compact {
        print_json(&response, true)
    } else {
        print_node_resource_list(&response, include_jobs)
    }
}

pub(crate) async fn resource_node_command(
    api: &ApiClient,
    args: &mut Args,
    compact: bool,
) -> Result<()> {
    let needle = args.pop().context("缺少 NODE_OR_IP")?;
    let group = args.take_option("--group")?;
    args.ensure_empty()?;
    let mut query = json!({
        "pageNum": 1,
        "pageSize": 1000,
        "sortField": "left_gpu_count",
        "sortOrder": "desc"
    });
    if let Some(group) = group {
        insert_value(&mut query, "rsgroup", Value::String(group))?;
    }
    let response = api.get(Service::Core, NODE_RESOURCE_PATH, query).await?;
    let node = response
        .pointer("/data/nodes")
        .and_then(Value::as_array)
        .and_then(|nodes| {
            nodes
                .iter()
                .find(|node| node_matches(node, &needle))
                .cloned()
        })
        .with_context(|| format!("没有找到节点: {needle}"))?;
    if compact {
        print_json(&node, true)
    } else {
        print_node_resource_detail(&node)
    }
}

pub(crate) fn node_resource_query(args: &mut Args) -> Result<Value> {
    let mut query = pagination(args)?;
    let group = args.take_option("--group")?;
    let gpu_model = args.take_option("--gpu-model")?;
    let gpu_type = args
        .take_option("--gpu-type")?
        .unwrap_or_else(|| "all".to_owned())
        .to_ascii_lowercase();
    let sort = args
        .take_option("--sort")?
        .unwrap_or_else(|| "gpu".to_owned())
        .to_ascii_lowercase();
    let ascending = args.take_flag("--asc");
    let descending = args.take_flag("--desc");
    if ascending && descending {
        bail!("--asc 和 --desc 不能同时使用");
    }

    if let Some(group) = group {
        insert_value(&mut query, "rsgroup", Value::String(group))?;
    }
    if let Some(model) = gpu_model {
        insert_value(&mut query, "gpuModel", Value::String(model))?;
    }
    match gpu_type.as_str() {
        "all" => {}
        "physical" | "gpu" | "1" => insert_value(&mut query, "gpuVStat", Value::from(1))?,
        "virtual" | "vgpu" | "2" => insert_value(&mut query, "gpuVStat", Value::from(2))?,
        _ => bail!("--gpu-type 只接受 physical、virtual 或 all"),
    }

    if let Some(value) = optional_u64(args, "--min-gpu", 0, u64::MAX)? {
        insert_value(&mut query, "leftGpuCount", Value::from(value))?;
    }
    if let Some(value) = optional_u64(args, "--min-cpu", 0, u64::MAX / 1000)? {
        insert_value(&mut query, "leftCpu", Value::from(value * 1000))?;
    }
    if let Some(value) = optional_u64(args, "--min-memory-gb", 0, u64::MAX / 1024)? {
        insert_value(&mut query, "leftMemory", Value::from(value * 1024))?;
    }

    let sort_field = match sort.as_str() {
        "gpu" => "left_gpu_count",
        "cpu" => "left_cpu",
        "memory" => "left_memory",
        _ => bail!("--sort 只接受 gpu、cpu 或 memory"),
    };
    insert_value(
        &mut query,
        "sortField",
        Value::String(sort_field.to_owned()),
    )?;
    insert_value(
        &mut query,
        "sortOrder",
        Value::String(if ascending { "asc" } else { "desc" }.to_owned()),
    )?;
    Ok(query)
}

fn optional_u64(args: &mut Args, name: &str, min: u64, max: u64) -> Result<Option<u64>> {
    let Some(value) = args.take_option(name)? else {
        return Ok(None);
    };
    let parsed = value
        .parse::<u64>()
        .with_context(|| format!("{name} 必须是非负整数"))?;
    if parsed < min || parsed > max {
        bail!("{name} 必须在 {min}..={max} 范围内");
    }
    Ok(Some(parsed))
}

fn node_matches(node: &Value, needle: &str) -> bool {
    ["name", "ip", "accessIp"].iter().any(|key| {
        node.get(*key)
            .and_then(Value::as_str)
            .is_some_and(|value| value.eq_ignore_ascii_case(needle))
    })
}
