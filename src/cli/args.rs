use crate::*;

pub(crate) fn pagination(args: &mut Args) -> Result<Value> {
    let page = option_u64(args, "--page", 1, 1, u64::MAX)?;
    let size = option_u64(args, "--size", 100, 1, 1000)?;
    Ok(json!({"pageNum": page, "pageSize": size}))
}

pub(crate) fn project_scope(args: &mut Args) -> Result<String> {
    let scope = args
        .take_option("--scope")?
        .unwrap_or_else(|| "mine".to_owned())
        .to_ascii_lowercase();
    if !matches!(scope.as_str(), "mine" | "shared" | "public" | "all") {
        bail!("--scope 只接受 mine、shared、public 或 all");
    }
    Ok(scope)
}

pub(crate) fn apply_project_scope(query: &mut Value, scope: &str) -> Result<()> {
    let access_type = match scope {
        "mine" => Some(1),
        "shared" => Some(2),
        "public" => Some(3),
        "all" => None,
        _ => bail!("未知项目范围: {scope}"),
    };
    if let Some(access_type) = access_type {
        insert_value(query, "accessType", Value::from(access_type))?;
    }
    Ok(())
}

pub(crate) fn command_concurrency(args: &mut Args, default: usize) -> Result<usize> {
    match args.take_option("--concurrency")? {
        Some(value) => parse_usize("concurrency", &value, 1, 1024),
        None => Ok(default),
    }
}

pub(crate) fn option_u64(
    args: &mut Args,
    name: &str,
    default: u64,
    min: u64,
    max: u64,
) -> Result<u64> {
    let Some(value) = args.take_option(name)? else {
        return Ok(default);
    };
    let parsed: u64 = value
        .parse()
        .with_context(|| format!("{name} 必须是整数"))?;
    if parsed < min || parsed > max {
        bail!("{name} 必须在 {min}..={max} 范围内");
    }
    Ok(parsed)
}

pub(crate) fn parse_usize(name: &str, value: &str, min: usize, max: usize) -> Result<usize> {
    let parsed: usize = value
        .parse()
        .with_context(|| format!("{name} 必须是整数"))?;
    if parsed < min || parsed > max {
        bail!("{name} 必须在 {min}..={max} 范围内");
    }
    Ok(parsed)
}

pub(crate) fn required_id(args: &mut Args, label: &str) -> Result<String> {
    let value = args.pop().with_context(|| format!("缺少 {label}"))?;
    validate_id(&value, label)?;
    Ok(value)
}

pub(crate) fn remaining_ids(args: &mut Args, label: &str) -> Result<Vec<String>> {
    let ids = args.drain();
    if ids.is_empty() {
        bail!("至少需要一个 {label}");
    }
    for id in &ids {
        validate_id(id, label)?;
    }
    Ok(ids)
}

pub(crate) fn validate_id(value: &str, label: &str) -> Result<()> {
    if value.is_empty()
        || !value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        bail!("{label} 含非法字符: {value}");
    }
    Ok(())
}

pub(crate) struct Args {
    values: VecDeque<String>,
}

impl Args {
    pub(crate) fn new(values: impl Iterator<Item = String>) -> Self {
        Self {
            values: values.collect(),
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    pub(crate) fn pop(&mut self) -> Option<String> {
        self.values.pop_front()
    }

    pub(crate) fn push_front(&mut self, value: impl Into<String>) {
        self.values.push_front(value.into());
    }

    pub(crate) fn peek(&self) -> Option<&str> {
        self.values.front().map(String::as_str)
    }

    pub(crate) fn take_flag(&mut self, name: &str) -> bool {
        if let Some(index) = self.values.iter().position(|value| value == name) {
            self.values.remove(index);
            true
        } else {
            false
        }
    }

    pub(crate) fn take_flag_any(&mut self, names: &[&str]) -> bool {
        names.iter().any(|name| self.take_flag(name))
    }

    pub(crate) fn take_option(&mut self, name: &str) -> Result<Option<String>> {
        let Some(index) = self
            .values
            .iter()
            .position(|value| value == name || value.starts_with(&format!("{name}=")))
        else {
            return Ok(None);
        };
        let item = self.values.remove(index).expect("index already checked");
        if let Some((_, value)) = item.split_once('=') {
            if value.is_empty() {
                bail!("{name} 缺少值");
            }
            return Ok(Some(value.to_owned()));
        }
        let value = self
            .values
            .remove(index)
            .with_context(|| format!("{name} 缺少值"))?;
        Ok(Some(value))
    }

    pub(crate) fn take_option_any(&mut self, names: &[&str]) -> Result<Option<String>> {
        for name in names {
            if let Some(value) = self.take_option(name)? {
                return Ok(Some(value));
            }
        }
        Ok(None)
    }

    pub(crate) fn drain(&mut self) -> Vec<String> {
        self.values.drain(..).collect()
    }

    pub(crate) fn ensure_empty(&self) -> Result<()> {
        if self.values.is_empty() {
            Ok(())
        } else {
            bail!(
                "无法识别的参数: {}",
                self.values.iter().cloned().collect::<Vec<_>>().join(" ")
            )
        }
    }
}
