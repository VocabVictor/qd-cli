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

pub(crate) async fn scoped_projects(api: &ApiClient, scope: &str) -> Result<Vec<Value>> {
    let mut query = json!({"pageNum": 1, "pageSize": 1000});
    apply_project_scope(&mut query, scope)?;
    let response = api.get(Service::Core, "/project/list", query).await?;
    Ok(response_array(&response, &["/data/projectList", "/data/list"]).to_vec())
}
