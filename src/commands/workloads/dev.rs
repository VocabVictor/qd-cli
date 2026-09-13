use crate::*;

pub(crate) async fn dev_command(
    api: &ApiClient,
    config: &Config,
    args: &mut Args,
    compact: bool,
) -> Result<()> {
    match args.pop().as_deref() {
        Some("list") => {
            let scope = project_scope(args)?;
            args.ensure_empty()?;
            let response = list_scoped_dev_environments(api, &scope).await?;
            if compact {
                print_json(&response, true)
            } else {
                print_dev_list(&response)
            }
        }
        Some("show") => {
            let id = required_id(args, "JOBENV_ID")?;
            let full = args.take_flag("--full");
            args.ensure_empty()?;
            let path = format!("/jobenv/detail/{id}");
            let response = api.get(Service::Core, &path, empty_object()).await?;
            print_detail_response(&response, compact, full, dev_summary, "开发环境详情")
        }
        Some("create") => {
            let file = args
                .take_option("--file")?
                .context("dev create 需要 --file")?;
            args.ensure_empty()?;
            let mut body = read_json(&file)?;
            insert_if_missing(
                &mut body,
                "spaceId",
                config.space_id.clone().map(Value::String),
            )?;
            print_json(
                &api.send(Method::POST, Service::Core, "/jobenv/devJob/new", body)
                    .await?,
                compact,
            )
        }
        Some("start") => {
            let concurrency = command_concurrency(args, config.concurrency)?;
            let ids = remaining_ids(args, "JOBENV_ID")?;
            print_json(
                &Value::Array(bulk_action(api, ids, concurrency, BulkAction::DevStart).await),
                compact,
            )
        }
        Some("stop") => {
            let concurrency = command_concurrency(args, config.concurrency)?;
            let ids = remaining_ids(args, "JOBENV_ID")?;
            print_json(
                &Value::Array(bulk_action(api, ids, concurrency, BulkAction::DevStop).await),
                compact,
            )
        }
        Some("tools") => {
            let id = required_id(args, "JOBENV_ID")?;
            let tools = args.take_option("--set")?;
            args.ensure_empty()?;
            if let Some(tools) = tools {
                let tools = parse_dev_tools(&tools)?;
                let path = format!("/user/jobenv/updateTools/{id}");
                let response = api
                    .send(
                        Method::PUT,
                        Service::Core,
                        &path,
                        json!({"jobenvId": id.parse::<u64>()?, "tools": tools}),
                    )
                    .await?;
                if compact {
                    print_json(&response, true)
                } else {
                    print_pretty_table(
                        "开发者工具已更新",
                        &["环境 ID", "开发者工具"],
                        &[vec![id, display_dev_tools(&tools)]],
                        &[16, 48],
                    );
                    Ok(())
                }
            } else {
                let path = format!("/jobenv/detail/{id}");
                let response = api.get(Service::Core, &path, empty_object()).await?;
                let tools = response_data(&response)
                    .pointer("/tools")
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default();
                if compact {
                    print_json(&json!({"id": id, "tools": tools}), true)
                } else {
                    print_pretty_table(
                        "开发者工具",
                        &["环境 ID", "开发者工具"],
                        &[vec![id, display_dev_tools(&tools)]],
                        &[16, 48],
                    );
                    Ok(())
                }
            }
        }
        Some("ssh-config") => {
            let id = required_id(args, "JOBENV_ID")?;
            let enable = args.take_flag("--enable");
            let disable = args.take_flag("--disable");
            args.ensure_empty()?;
            if enable == disable {
                bail!("dev ssh-config 必须且只能指定 --enable 或 --disable");
            }
            let path = format!("/jobenv/updateSsh/{id}");
            api.send(
                Method::PUT,
                Service::Core,
                &path,
                json!({"jobenvId": id.parse::<u64>()?, "ssh": {"enabled": enable}}),
            )
            .await?;
            if compact {
                print_json(&json!({"id": id, "sshEnabled": enable}), true)
            } else {
                print_pretty_table(
                    "SSH 配置已更新",
                    &["环境 ID", "SSH"],
                    &[vec![
                        id,
                        if enable { "已启用" } else { "已禁用" }.to_owned(),
                    ]],
                    &[16, 16],
                );
                Ok(())
            }
        }
        Some("shell") => {
            let id = required_id(args, "JOBENV_ID")?;
            args.ensure_empty()?;
            dev_terminal_transport(api, &id, &[]).await
        }
        Some("refresh") => {
            let id = required_id(args, "JOBENV_ID")?;
            args.ensure_empty()?;
            let path = format!("/jobenv/refresh/{id}");
            let response = api.get(Service::Core, &path, empty_object()).await?;
            if compact {
                let data = response_data(&response);
                print_json(
                    &json!({
                        "id": id,
                        "status": pointer_text(data, &["/status"]),
                        "sshReady": data
                            .pointer("/devJobDetail/taskroleList/0/instanceList/0/sshConnection")
                            .and_then(Value::as_str)
                            .is_some_and(|value| !value.trim().is_empty() && value != "-"),
                        "terminalReady": data
                            .pointer("/devJobDetail/taskroleList/0/instanceList/0/terminalUrl")
                            .and_then(Value::as_str)
                            .is_some_and(|value| !value.trim().is_empty() && value != "-"),
                    }),
                    true,
                )
            } else {
                print_detail_response(&response, false, false, dev_summary, "开发环境详情")
            }
        }
        Some("terminal-info") => {
            let id = required_id(args, "JOBENV_ID")?;
            args.ensure_empty()?;
            let path = format!("/jobenv/refresh/{id}");
            let response = api.get(Service::Core, &path, empty_object()).await?;
            let terminal_url = response_data(&response)
                .pointer("/devJobDetail/taskroleList/0/instanceList/0/terminalUrl")
                .and_then(Value::as_str)
                .filter(|value| !value.trim().is_empty() && *value != "-")
                .context("平台未返回网页终端地址")?;
            let page = api.get_absolute_text(terminal_url).await?;
            let lower = page.text.to_ascii_lowercase();
            let framework = if lower.contains("ttyd") {
                "ttyd"
            } else if lower.contains("wetty") {
                "wetty"
            } else if lower.contains("xterm") {
                "xterm"
            } else if lower.contains("jupyter") {
                "jupyter"
            } else {
                "unknown"
            };
            let mut scripts = Vec::new();
            for source in extract_html_attribute(&page.text, "script", "src") {
                let Ok(url) = page.final_url.join(&source) else {
                    continue;
                };
                let script = api.get_absolute_text(url.as_str()).await?;
                let script_lower = script.text.to_ascii_lowercase();
                scripts.push(json!({
                    "name": url
                        .path_segments()
                        .and_then(|mut parts| parts.next_back())
                        .unwrap_or("script"),
                    "bytes": script.text.len(),
                    "websocket": script_lower.contains("websocket"),
                    "xterm": script_lower.contains("xterm"),
                    "ttyd": script_lower.contains("ttyd"),
                    "wetty": script_lower.contains("wetty"),
                    "socketIo": script_lower.contains("socket.io"),
                }));
            }
            let info = json!({
                "id": id,
                "httpStatus": page.status.as_u16(),
                "contentType": page.content_type,
                "host": page.final_url.host_str().unwrap_or("-"),
                "scheme": page.final_url.scheme(),
                "pathSegments": page.final_url.path_segments().map(|parts| parts.count()).unwrap_or(0),
                "htmlBytes": page.text.len(),
                "framework": framework,
                "hasWebSocketCode": lower.contains("websocket"),
                "hasXtermCode": lower.contains("xterm"),
                "scripts": scripts,
            });
            if compact {
                print_json(&info, true)
            } else {
                print_pretty_table(
                    "网页终端",
                    &["环境 ID", "状态", "协议", "框架", "页面大小"],
                    &[vec![
                        id,
                        page.status.as_u16().to_string(),
                        if lower.contains("websocket")
                            || scripts.iter().any(|script| {
                                script.get("websocket").and_then(Value::as_bool) == Some(true)
                            })
                        {
                            "WebSocket".to_owned()
                        } else {
                            "-".to_owned()
                        },
                        framework.to_owned(),
                        format_bytes(page.text.len() as f64),
                    ]],
                    &[16, 10, 14, 14, 14],
                );
                Ok(())
            }
        }
        Some("exec") => {
            let id = required_id(args, "JOBENV_ID")?;
            let mut command = args.drain();
            if command.first().map(String::as_str) == Some("--") {
                command.remove(0);
            }
            if command.is_empty() {
                bail!("dev exec 需要 COMMAND");
            }
            dev_terminal_transport(api, &id, &command).await
        }
        _ => bail!("用法: qd dev list|show|create|start|stop ..."),
    }
}
