use crate::*;

pub(crate) fn response_array<'a>(response: &'a Value, paths: &[&str]) -> &'a [Value] {
    paths
        .iter()
        .find_map(|path| response.pointer(path).and_then(Value::as_array))
        .map(Vec::as_slice)
        .unwrap_or(&[])
}

pub(crate) fn response_count(response: &Value, paths: &[&str], fallback: usize) -> u64 {
    paths
        .iter()
        .find_map(|path| response.pointer(path).and_then(Value::as_u64))
        .unwrap_or(fallback as u64)
}

pub(crate) fn field_text(value: &Value, keys: &[&str]) -> String {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(display_value))
        .unwrap_or_else(|| "-".to_owned())
}

pub(crate) fn joined_fields(value: &Value, keys: &[&str], separator: &str) -> String {
    let values = keys
        .iter()
        .filter_map(|key| value.get(*key).and_then(display_value))
        .collect::<Vec<_>>();
    if values.is_empty() {
        "-".to_owned()
    } else {
        values.join(separator)
    }
}

pub(crate) fn display_value(value: &Value) -> Option<String> {
    match value {
        Value::String(value) if !value.trim().is_empty() => Some(value.trim().to_owned()),
        Value::Number(value) => Some(value.to_string()),
        Value::Bool(value) => Some(if *value { "是" } else { "否" }.to_owned()),
        _ => None,
    }
}

pub(crate) fn numeric_field(value: &Value, keys: &[&str]) -> Option<f64> {
    keys.iter().find_map(|key| {
        value.get(*key).and_then(|field| {
            field
                .as_f64()
                .or_else(|| field.as_str().and_then(|value| value.parse().ok()))
        })
    })
}

pub(crate) fn format_cpu_field(job: &Value) -> String {
    let Some(cpu) = numeric_field(job, &["statCpu", "cpu", "cpuCount"]) else {
        return "-".to_owned();
    };
    let cores = if cpu >= 1000.0 { cpu / 1000.0 } else { cpu };
    format!("{} 核", format_number(cores))
}

pub(crate) fn format_bytes(value: f64) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = KIB * 1024.0;
    const GIB: f64 = MIB * 1024.0;
    const TIB: f64 = GIB * 1024.0;
    if value >= TIB {
        format!("{:.2} TiB", value / TIB)
    } else if value >= GIB {
        format!("{:.2} GiB", value / GIB)
    } else if value >= MIB {
        format!("{:.2} MiB", value / MIB)
    } else if value >= KIB {
        format!("{:.2} KiB", value / KIB)
    } else {
        format!("{} B", format_number(value))
    }
}
