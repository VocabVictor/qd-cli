use crate::*;

pub(crate) async fn resource_command(
    api: &ApiClient,
    config: &Config,
    args: &mut Args,
    compact: bool,
) -> Result<()> {
    match args.pop().as_deref() {
        Some("groups") => {
            let query = pagination(args)?;
            args.ensure_empty()?;
            let response = api
                .get(Service::Core, "/space/rsgroup/bindlist", query)
                .await?;
            if compact {
                print_json(&response, true)
            } else {
                print_resource_group_list(&response)
            }
        }
        Some("group") => {
            let id = required_id(args, "RSGROUP_ID")?;
            let full = args.take_flag("--full");
            args.ensure_empty()?;
            let path = format!("/rsgroup/detail/{id}");
            let response = api.get(Service::Core, &path, empty_object()).await?;
            print_detail_response(
                &response,
                compact,
                full,
                resource_group_summary,
                "资源组详情",
            )
        }
        Some("idle") => {
            let available_only = args.take_flag("--available-only");
            let gpu_only = args.take_flag("--gpu-only");
            let concurrency = command_concurrency(args, config.concurrency)?;
            let ids = if args.is_empty() {
                resource_group_ids(api).await?
            } else {
                remaining_ids(args, "RSGROUP_ID")?
            };
            let mut groups = idle_resource_groups(api, ids, concurrency).await;
            groups.sort_by(|left, right| {
                left.get("rsgroupName")
                    .and_then(Value::as_str)
                    .cmp(&right.get("rsgroupName").and_then(Value::as_str))
            });
            if available_only {
                groups
                    .retain(|group| group.get("available").and_then(Value::as_bool) == Some(true));
            }
            if gpu_only {
                groups.retain(|group| {
                    group.get("gpuAvailable").and_then(Value::as_bool) == Some(true)
                });
            }
            if compact {
                print_json(
                    &json!({
                        "code": 0,
                        "data": {"listCount": groups.len(), "groups": groups}
                    }),
                    true,
                )
            } else {
                print_idle_resources(&groups);
                Ok(())
            }
        }
        Some("nodes") => resource_nodes_command(api, args, compact).await,
        Some("node") => resource_node_command(api, args, compact).await,
        _ => bail!(
            "用法: qd resource groups | group RSGROUP_ID | idle [RSGROUP_ID...] | nodes | node NODE"
        ),
    }
}

pub(crate) async fn resource_group_ids(api: &ApiClient) -> Result<Vec<String>> {
    let response = api
        .get(
            Service::Core,
            "/space/rsgroup/bindlist",
            json!({"pageNum": 1, "pageSize": 1000}),
        )
        .await?;
    let groups = response
        .pointer("/data/rsgroupList")
        .and_then(Value::as_array)
        .context("资源组列表响应缺少 rsgroupList")?;
    groups
        .iter()
        .map(|group| {
            let id = group
                .get("rsgroupId")
                .and_then(value_string)
                .context("资源组缺少 rsgroupId")?;
            validate_id(&id, "RSGROUP_ID")?;
            Ok(id)
        })
        .collect()
}
