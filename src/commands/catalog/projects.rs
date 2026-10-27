use crate::*;

pub(crate) async fn project_command(
    api: &ApiClient,
    config: &Config,
    args: &mut Args,
    compact: bool,
) -> Result<()> {
    match args.pop().as_deref() {
        Some("list") => {
            let scope = project_scope(args)?;
            let mut query = pagination(args)?;
            apply_project_scope(&mut query, &scope)?;
            args.ensure_empty()?;
            let response = api.get(Service::Core, "/project/list", query).await?;
            if compact {
                print_json(&response, true)
            } else {
                print_project_list(&response)
            }
        }
        Some("show") => {
            let id = required_id(args, "PROJECT_ID")?;
            let full = args.take_flag("--full");
            args.ensure_empty()?;
            let path = format!("/project/detail/{id}");
            let response = api.get(Service::Core, &path, empty_object()).await?;
            print_detail_response(&response, compact, full, project_summary, "项目详情")
        }
        Some("create") => {
            let file = args
                .take_option("--file")?
                .context("project create 需要 --file")?;
            args.ensure_empty()?;
            let mut body = read_json(&file)?;
            insert_if_missing(
                &mut body,
                "spaceId",
                config.space_id.clone().map(Value::String),
            )?;
            print_json(
                &api.send(Method::POST, Service::Core, "/project/new", body)
                    .await?,
                compact,
            )
        }
        _ => bail!("用法: qd project list|show|create ..."),
    }
}

/// 作用域内的全部项目。按平台原生分页逐页取，直到拿满 listCount（全空间约千个项目，
/// 单页 1000 会漏掉超出的部分）。
pub(crate) async fn scoped_projects(api: &ApiClient, scope: &str) -> Result<Vec<Value>> {
    const PAGE: u64 = 500;
    let mut out = Vec::new();
    for page in 1..=200u64 {
        let mut query = json!({"pageNum": page, "pageSize": PAGE});
        apply_project_scope(&mut query, scope)?;
        let response = api.get(Service::Core, "/project/list", query).await?;
        let batch = response_array(&response, &["/data/projectList", "/data/list"]);
        let total = response.pointer("/data/listCount").and_then(value_u64);
        let got = batch.len() as u64;
        out.extend(batch.iter().cloned());
        if got < PAGE || total.is_some_and(|t| out.len() as u64 >= t) {
            break;
        }
    }
    Ok(out)
}
