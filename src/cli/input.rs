use crate::*;

pub(crate) fn read_json(path: &str) -> Result<Value> {
    let bytes = if path == "-" {
        let mut bytes = Vec::new();
        io::stdin().read_to_end(&mut bytes)?;
        bytes
    } else {
        fs::read(Path::new(path)).with_context(|| format!("无法读取 {path}"))?
    };
    let value: Value =
        serde_json::from_slice(&bytes).with_context(|| format!("{path} 不是有效 JSON"))?;
    if !value.is_object() {
        bail!("请求 JSON 顶层必须是对象");
    }
    Ok(value)
}

pub(crate) fn read_stdin_trimmed() -> Result<String> {
    let mut value = String::new();
    io::stdin().read_to_string(&mut value)?;
    // 剥掉 PowerShell 管道等来源可能加在开头的 UTF-8 BOM，否则密码会多出 U+FEFF
    let value = value.strip_prefix('\u{feff}').unwrap_or(value.as_str());
    Ok(value.trim_end_matches(['\r', '\n']).to_owned())
}

pub(crate) fn insert_if_missing(target: &mut Value, key: &str, value: Option<Value>) -> Result<()> {
    let object = target.as_object_mut().context("JSON 顶层必须是对象")?;
    if !object.contains_key(key)
        && let Some(value) = value
    {
        object.insert(key.to_owned(), value);
    }
    Ok(())
}

pub(crate) fn insert_value(target: &mut Value, key: &str, value: Value) -> Result<()> {
    target
        .as_object_mut()
        .context("JSON 顶层必须是对象")?
        .insert(key.to_owned(), value);
    Ok(())
}

pub(crate) fn find_id(value: &Value, keys: &[&str]) -> Option<String> {
    for key in keys {
        if let Some(candidate) = value.pointer(&format!("/data/{key}")) {
            if let Some(text) = candidate.as_str() {
                return Some(text.to_owned());
            }
            if let Some(number) = candidate.as_u64() {
                return Some(number.to_string());
            }
        }
    }
    None
}

pub(crate) fn print_json(value: &Value, compact: bool) -> Result<()> {
    let mut value = value.clone();
    redact_sensitive_fields(&mut value);
    if compact {
        println!("{}", serde_json::to_string(&value)?);
    } else {
        println!("{}", serde_json::to_string_pretty(&value)?);
    }
    Ok(())
}

pub(crate) fn redact_sensitive_fields(value: &mut Value) {
    match value {
        Value::Object(object) => {
            for (key, value) in object {
                let normalized = key.to_ascii_lowercase();
                let sensitive = normalized.contains("token")
                    || normalized.contains("password")
                    || normalized.contains("secret")
                    || normalized.contains("cookie")
                    || normalized.contains("authorization")
                    || matches!(
                        normalized.as_str(),
                        "terminalurl" | "jupyterurl" | "sshconnection"
                    );
                if sensitive {
                    *value = Value::String("***REDACTED***".to_owned());
                } else {
                    redact_sensitive_fields(value);
                }
            }
        }
        Value::Array(values) => {
            for value in values {
                redact_sensitive_fields(value);
            }
        }
        _ => {}
    }
}
