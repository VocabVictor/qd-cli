use crate::*;

pub(crate) async fn image_command(
    api: &ApiClient,
    config: &Config,
    args: &mut Args,
    compact: bool,
) -> Result<()> {
    match args.pop().as_deref() {
        Some("list") => {
            let source = args
                .take_option("--source")?
                .unwrap_or_else(|| "mine".to_owned());
            if !matches!(source.as_str(), "current" | "mine") {
                bail!("--source 只接受 current 或 mine");
            }
            let key_words = args
                .take_option_any(&["--keywords", "--key-words"])?
                .unwrap_or_default();
            let mut query = pagination(args)?;
            query["source"] = Value::String(source);
            query["keyWords"] = Value::String(key_words);
            if let Some(space_id) = config.space_id.clone() {
                query["spaceId"] = Value::String(space_id);
            }
            args.ensure_empty()?;
            let response = api.get(Service::Core, "/jobenv/image/list", query).await?;
            if compact {
                print_json(&response, true)
            } else {
                print_image_list(&response)
            }
        }
        Some("repositories") => {
            // 平台要求 spaceId + source 才放行；source=current 列空间仓库
            let source = args
                .take_option("--source")?
                .unwrap_or_else(|| "current".to_owned());
            let key_words = args
                .take_option_any(&["--keywords", "--key-words"])?
                .unwrap_or_default();
            let space_id = args.take_option("--space")?.or_else(|| config.space_id.clone());
            let mut query = pagination(args)?;
            query["source"] = Value::String(source);
            query["keyWords"] = Value::String(key_words);
            if let Some(space_id) = space_id {
                query["spaceId"] = Value::String(space_id);
            }
            args.ensure_empty()?;
            let response = api
                .get(Service::Core, "/imageRepository/list", query)
                .await?;
            if compact {
                print_json(&response, true)
            } else {
                print_image_repository_list(&response)
            }
        }
        _ => bail!("用法: qd image list|repositories"),
    }
}
