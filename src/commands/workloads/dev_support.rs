use crate::*;

pub(crate) fn parse_dev_tools(value: &str) -> Result<Vec<Value>> {
    if value.eq_ignore_ascii_case("none") || value.trim().is_empty() {
        return Ok(Vec::new());
    }
    let mut tools: Vec<Value> = Vec::new();
    for tool in value
        .split(',')
        .map(str::trim)
        .filter(|tool| !tool.is_empty())
    {
        if !matches!(tool, "jupyterlab" | "tensorboard") {
            bail!("开发者工具只接受 jupyterlab、tensorboard 或 none");
        }
        if !tools.iter().any(|value| value.as_str() == Some(tool)) {
            tools.push(Value::String(tool.to_owned()));
        }
    }
    Ok(tools)
}

pub(crate) fn display_dev_tools(tools: &[Value]) -> String {
    let text = tools
        .iter()
        .filter_map(Value::as_str)
        .collect::<Vec<_>>()
        .join(", ");
    if text.is_empty() {
        "无".to_owned()
    } else {
        text
    }
}

pub(crate) fn extract_html_attribute(html: &str, tag: &str, attribute: &str) -> Vec<String> {
    let mut values = Vec::new();
    let lower = html.to_ascii_lowercase();
    let tag_start = format!("<{tag}");
    let attribute_start = format!("{attribute}=");
    let mut cursor = 0;
    while let Some(relative_tag) = lower[cursor..].find(&tag_start) {
        let start = cursor + relative_tag;
        let Some(relative_end) = lower[start..].find('>') else {
            break;
        };
        let end = start + relative_end;
        let fragment = &html[start..end];
        let fragment_lower = &lower[start..end];
        if let Some(relative_attribute) = fragment_lower.find(&attribute_start) {
            let value_start = relative_attribute + attribute_start.len();
            let remaining = fragment[value_start..].trim_start();
            if let Some(quote) = remaining
                .chars()
                .next()
                .filter(|value| matches!(value, '"' | '\''))
                && let Some(value_end) = remaining[1..].find(quote)
            {
                values.push(remaining[1..1 + value_end].to_owned());
            }
        }
        cursor = end + 1;
    }
    values
}

/// 解析开发环境的网页终端地址（要求 RUNNING）。
pub(crate) async fn dev_terminal_url(api: &ApiClient, id: &str) -> Result<String> {
    let path = format!("/jobenv/refresh/{id}");
    let response = api.get(Service::Core, &path, empty_object()).await?;
    let data = response_data(&response);
    let status = pointer_text(data, &["/status"]);
    if status != "RUNNING" {
        bail!("开发环境 {id} 当前状态为 {status}，请先启动");
    }
    data.pointer("/devJobDetail/taskroleList/0/instanceList/0/terminalUrl")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty() && *value != "-")
        .context("平台未返回网页终端地址")
        .map(str::to_owned)
}

pub(crate) async fn list_scoped_dev_environments(api: &ApiClient, scope: &str) -> Result<Value> {
    let projects = scoped_projects(api, scope).await?;
    let dev_list = projects
        .into_iter()
        .filter(|project| numeric_field(project, &["jobenvId"]).unwrap_or(0.0) > 0.0)
        .collect::<Vec<_>>();
    Ok(json!({
        "code": 0,
        "msg": "操作成功",
        "data": {"listCount": dev_list.len(), "devList": dev_list, "scope": scope}
    }))
}
