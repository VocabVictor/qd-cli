//! 作业 / 开发机的提交模板：存在配置目录下的 templates.json，
//! 与浏览器无关，换浏览器或清缓存都不会丢。

use super::state::{WebError, WebResult, reply};
use crate::*;

use axum::{Json, extract::Path as UrlPath};
use std::path::PathBuf;

fn templates_path() -> Result<PathBuf, WebError> {
    let path = crate::config::config_path().map_err(|err| WebError::bad_request(format!("定位配置目录失败: {err}")))?;
    Ok(path.with_file_name("templates.json"))
}

fn load() -> Result<Vec<Value>, WebError> {
    let path = templates_path()?;
    if !path.exists() {
        return Ok(Vec::new());
    }
    let text = fs::read_to_string(&path)
        .map_err(|err| WebError::bad_request(format!("读取模板失败: {err}")))?;
    Ok(serde_json::from_str(&text).unwrap_or_default())
}

fn save(list: &[Value]) -> Result<(), WebError> {
    let path = templates_path()?;
    if let Some(dir) = path.parent() {
        let _ = fs::create_dir_all(dir);
    }
    let text = serde_json::to_string_pretty(list)
        .map_err(|err| WebError::bad_request(format!("序列化模板失败: {err}")))?;
    fs::write(&path, text).map_err(|err| WebError::bad_request(format!("写入模板失败: {err}")))
}

fn name_of(item: &Value) -> &str {
    item.get("name").and_then(Value::as_str).unwrap_or_default()
}

/// GET /api/templates —— 全部模板。
pub(crate) async fn list_templates() -> WebResult {
    reply(json!({ "templates": load()? }))
}

/// POST /api/templates —— 新增或按名覆盖。
pub(crate) async fn post_template(Json(body): Json<Value>) -> WebResult {
    let name = body
        .get("name")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .ok_or_else(|| WebError::bad_request("模板名不能为空"))?
        .to_owned();
    if name.chars().count() > 60 {
        return Err(WebError::bad_request("模板名过长（最多 60 字）"));
    }
    let kind = body.get("kind").and_then(Value::as_str).unwrap_or("job");
    if !matches!(kind, "job" | "dev") {
        return Err(WebError::bad_request("模板类型只能是 job 或 dev"));
    }
    let payload = body
        .get("payload")
        .cloned()
        .ok_or_else(|| WebError::bad_request("缺少 payload"))?;

    let mut list = load()?;
    list.retain(|item| name_of(item) != name);
    list.push(json!({ "name": name, "kind": kind, "payload": payload }));
    if list.len() > 100 {
        return Err(WebError::bad_request("模板数量已达上限（100 个）"));
    }
    save(&list)?;
    reply(json!({ "saved": name, "count": list.len() }))
}

/// DELETE /api/templates/{name} —— 按名删除。
pub(crate) async fn delete_template(UrlPath(name): UrlPath<String>) -> WebResult {
    let mut list = load()?;
    let before = list.len();
    list.retain(|item| name_of(item) != name);
    if list.len() == before {
        return Err(WebError::bad_request("没有这个模板"));
    }
    save(&list)?;
    reply(json!({ "removed": name, "count": list.len() }))
}
