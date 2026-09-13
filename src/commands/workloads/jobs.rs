use crate::*;

pub(crate) async fn job_command(
    api: &ApiClient,
    config: &Config,
    args: &mut Args,
    compact: bool,
) -> Result<()> {
    match args.pop().as_deref() {
        Some("list") => {
            let scope = project_scope(args)?;
            let query = pagination(args)?;
            let status = args.take_option("--status")?;
            let project_id = args.pop();
            args.ensure_empty()?;
            let response = if let Some(project_id) = project_id {
                validate_id(&project_id, "PROJECT_ID")?;
                let mut query = query;
                if let Some(status) = status {
                    insert_value(&mut query, "status", Value::String(status))?;
                }
                let path = format!("/project/job/list/{project_id}");
                api.get(Service::Core, &path, query).await?
            } else {
                list_scoped_jobs(api, config, &scope, &query, status.as_deref()).await?
            };
            if compact {
                print_json(&response, true)
            } else {
                print_job_list(&response)
            }
        }
        Some("show") => {
            let id = required_id(args, "JOB_ID")?;
            let full = args.take_flag("--full");
            args.ensure_empty()?;
            let response = job_show(api, &id).await?;
            print_detail_response(&response, compact, full, job_summary, "任务详情")
        }
        Some("submit") => {
            let file = args
                .take_option("--file")?
                .context("job submit 需要 --file")?;
            let wait = args.take_flag("--wait");
            let interval = option_u64(args, "--interval", 3, 1, 3600)?;
            let timeout = option_u64(args, "--timeout", 0, 0, u64::MAX)?;
            args.ensure_empty()?;
            let body = read_json(&file)?;
            let result = submit_job(api, config, body).await?;
            let id = result
                .pointer("/data/jobId")
                .and_then(value_string)
                .or_else(|| find_id(&result, &["jobId", "id"]));
            print_json(&result, compact)?;
            if wait {
                let id = id.context(
                    "任务已提交，但平台响应未返回 jobId，且无法从项目任务列表中定位；请勿直接重复提交",
                )?;
                let waited =
                    wait_jobs(api, vec![id], interval, timeout, config.concurrency).await?;
                print_json(&waited, compact)?;
            }
            Ok(())
        }
        Some("cancel") => {
            let concurrency = command_concurrency(args, config.concurrency)?;
            let ids = remaining_ids(args, "JOB_ID")?;
            let result = bulk_action(api, ids, concurrency, BulkAction::JobCancel).await;
            print_json(&Value::Array(result), compact)
        }
        Some("delete") => {
            let concurrency = command_concurrency(args, config.concurrency)?;
            let ids = remaining_ids(args, "JOB_ID")?;
            let result = bulk_action(api, ids, concurrency, BulkAction::JobDelete).await;
            print_json(&Value::Array(result), compact)
        }
        Some("wait") => {
            let interval = option_u64(args, "--interval", 3, 1, 3600)?;
            let timeout = option_u64(args, "--timeout", 0, 0, u64::MAX)?;
            let concurrency = command_concurrency(args, config.concurrency)?;
            let ids = remaining_ids(args, "JOB_ID")?;
            let value = wait_jobs(api, ids, interval, timeout, concurrency).await?;
            print_json(&value, compact)
        }
        Some("template") => {
            args.ensure_empty()?;
            print_json(&job_template(config), compact)
        }
        Some("logs") => {
            let id = required_id(args, "JOB_ID")?;
            let limit = option_u64(args, "--limit", 500, 1, 1000)?;
            args.ensure_empty()?;
            job_logs(api, &id, limit).await
        }
        Some("top") => {
            let id = required_id(args, "JOB_ID")?;
            let minutes = option_u64(args, "--minutes", 15, 1, 1440)?;
            args.ensure_empty()?;
            let report = job_metrics(api, &id, minutes).await?;
            if compact {
                return print_json(&report, true);
            }
            println!("最近 {minutes} 分钟（作业 {id}）:");
            for entry in report
                .pointer("/metrics")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                let label = entry.get("label").and_then(Value::as_str).unwrap_or("-");
                let unit = entry.get("unit").and_then(Value::as_str).unwrap_or("");
                let count = entry
                    .get("points")
                    .and_then(Value::as_array)
                    .map_or(0, Vec::len);
                match (
                    entry.get("last").and_then(Value::as_f64),
                    entry.get("avg").and_then(Value::as_f64),
                    entry.get("max").and_then(Value::as_f64),
                ) {
                    (Some(last), Some(avg), Some(max)) => println!(
                        "  {label:<10} 当前 {last:6.1}{unit}  均值 {avg:6.1}{unit}  峰值 {max:6.1}{unit}  ({count} 点)"
                    ),
                    _ => println!("  {label:<10} 无数据"),
                }
            }
            Ok(())
        }
        Some("ssh") => {
            let id = required_id(args, "JOB_ID")?;
            args.ensure_empty()?;
            job_ssh(api, &id).await
        }
        Some("exec") => {
            let id = required_id(args, "JOB_ID")?;
            let instance = option_u64(args, "--instance", 0, 0, 64)? as usize;
            let mut command = args.drain();
            if command.first().map(String::as_str) == Some("--") {
                command.remove(0);
            }
            if command.is_empty() {
                bail!("job exec 需要 COMMAND");
            }
            job_exec(api, &id, instance, &command).await
        }
        _ => bail!("用法: qd job list|show|submit|cancel|delete|wait|template|logs|top|ssh|exec ..."),
    }
}
