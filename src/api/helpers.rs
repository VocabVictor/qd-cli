use super::*;

pub(crate) fn resolve_api_path(service: Service, path: &str) -> Result<String> {
    if !path.starts_with('/') {
        bail!("API 路径必须以 / 开头");
    }
    let resolved = match service {
        Service::Auth => format!("{AUTH_PREFIX}{path}"),
        Service::Tool => format!("{TOOL_PREFIX}{path}"),
        Service::Absolute => path.to_owned(),
        Service::Core => {
            if path.starts_with("/user")
                || path.starts_with("/admin")
                || path.starts_with("/notify")
            {
                format!("{CORE_PREFIX}{path}")
            } else {
                format!("{CORE_PREFIX}/user{path}")
            }
        }
    };
    Ok(resolved)
}

pub fn ensure_api_success(value: &Value) -> Result<()> {
    if let Some(code) = value.get("code").and_then(Value::as_i64)
        && code != 0
    {
        let message = value
            .get("msg")
            .or_else(|| value.get("message"))
            .and_then(Value::as_str)
            .unwrap_or("未知错误");
        bail!("平台错误 {code}: {message}");
    }
    Ok(())
}

pub(crate) fn string_field(value: &Value, key: &str) -> Result<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| anyhow!("响应缺少字段 {key}"))
}

pub(crate) fn build_http_client(config: &Config) -> Result<Client> {
    config.require_base_url()?;
    Client::builder()
        .connect_timeout(Duration::from_secs(config.connect_timeout_secs))
        .timeout(Duration::from_secs(config.request_timeout_secs))
        .pool_idle_timeout(Duration::from_secs(60))
        .pool_max_idle_per_host(config.concurrency.min(64))
        .danger_accept_invalid_certs(config.insecure_tls)
        .redirect(reqwest::redirect::Policy::limited(5))
        .user_agent(concat!("qd/", env!("CARGO_PKG_VERSION")))
        .build()
        .context("创建 HTTP 客户端失败")
}

pub(crate) async fn backoff(attempt: u32) {
    let millis = 200u64.saturating_mul(1u64 << attempt.min(5));
    tokio::time::sleep(Duration::from_millis(millis)).await;
}

pub(crate) async fn send_connect_retry<F>(mut request: F) -> reqwest::Result<reqwest::Response>
where
    F: FnMut() -> reqwest::RequestBuilder,
{
    let mut attempt = 0u32;
    loop {
        match request().send().await {
            Ok(response) => return Ok(response),
            Err(error) if attempt < 3 && (error.is_timeout() || error.is_connect()) => {
                backoff(attempt).await;
                attempt += 1;
            }
            Err(error) => return Err(error),
        }
    }
}

pub(crate) fn safe_error_message(text: &str) -> String {
    if let Ok(value) = serde_json::from_str::<Value>(text) {
        return value
            .get("msg")
            .or_else(|| value.get("message"))
            .and_then(Value::as_str)
            .unwrap_or("未提供错误信息")
            .chars()
            .take(300)
            .collect();
    }
    if text.trim().is_empty() {
        "空响应".into()
    } else {
        "非 JSON 错误响应（内容已隐藏）".into()
    }
}

pub fn empty_object() -> Value {
    json!({})
}
