//! 查询字符串参数解析。

use super::state::WebError;

pub(crate) fn query_u64(
    query: &std::collections::HashMap<String, String>,
    key: &str,
    default: u64,
    min: u64,
    max: u64,
) -> Result<u64, WebError> {
    let Some(value) = query.get(key).filter(|value| !value.is_empty()) else {
        return Ok(default);
    };
    let parsed: u64 = value
        .parse()
        .map_err(|_| WebError::bad_request(format!("{key} 必须是整数")))?;
    if parsed < min || parsed > max {
        return Err(WebError::bad_request(format!(
            "{key} 必须在 {min}..={max} 范围内"
        )));
    }
    Ok(parsed)
}

pub(crate) fn query_scope(
    query: &std::collections::HashMap<String, String>,
) -> Result<String, WebError> {
    let scope = query
        .get("scope")
        .filter(|value| !value.is_empty())
        .map(String::as_str)
        .unwrap_or("mine")
        .to_ascii_lowercase();
    if !matches!(scope.as_str(), "mine" | "shared" | "public" | "all") {
        return Err(WebError::bad_request("scope 只接受 mine、shared、public 或 all"));
    }
    Ok(scope)
}
