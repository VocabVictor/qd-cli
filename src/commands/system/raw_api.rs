use crate::*;

pub(crate) async fn raw_api_command(api: &ApiClient, args: &mut Args, compact: bool) -> Result<()> {
    let method: Method = args
        .pop()
        .context("api 缺少 METHOD")?
        .to_ascii_uppercase()
        .parse()
        .context("无效 HTTP 方法")?;
    let mut path = un_mangle_remote_path(&args.pop().context("api 缺少 PATH")?);
    if !path.starts_with('/') {
        path.insert(0, '/');
    }
    let service = Service::parse(
        &args
            .take_option("--service")?
            .unwrap_or_else(|| "core".into()),
    )?;
    let body = match args.take_option("--file")? {
        Some(path) => read_json(&path)?,
        None => empty_object(),
    };
    args.ensure_empty()?;
    print_json(&api.send(method, service, &path, body).await?, compact)
}
