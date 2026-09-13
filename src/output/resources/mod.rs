use crate::*;

mod detail;
mod jobs;
mod list;

pub(crate) use detail::*;
pub(crate) use list::*;

fn node_name(node: &Value) -> String {
    let name = field_text(node, &["name"]);
    let ip = field_text(node, &["ip"]);
    if ip == "-" || ip == name {
        name
    } else {
        format!("{name} / {ip}")
    }
}

fn free_gpu_text(node: &Value) -> String {
    let p_total = numeric_field(node, &["gpuTotal"]).unwrap_or(0.0);
    let p_used = numeric_field(node, &["gpuUsed"]).unwrap_or(0.0);
    let v_total = numeric_field(node, &["orionRatioTotal"]).unwrap_or(0.0) / 100.0;
    let v_used = numeric_field(node, &["orionRatioUsed"]).unwrap_or(0.0) / 100.0;
    let physical = (p_total - p_used).max(0.0).floor();
    let virtual_gpu = (v_total - v_used).max(0.0).floor();
    format!(
        "{} 卡（{}/{})",
        format_number((physical + virtual_gpu).floor()),
        format_number(physical),
        format_number(virtual_gpu)
    )
}

fn numeric_pair(node: &Value, used: &str, total: &str, unit: &str, divisor: f64) -> String {
    let used = numeric_field(node, &[used]).unwrap_or(0.0) / divisor;
    let total = numeric_field(node, &[total]).unwrap_or(0.0) / divisor;
    format!("{} / {} {unit}", format_number(used), format_number(total))
}

fn memory_pair(node: &Value, used: &str, total: &str) -> String {
    let used = numeric_field(node, &[used]).unwrap_or(0.0);
    let total = numeric_field(node, &[total]).unwrap_or(0.0);
    format!("{} / {}", format_memory_mib(used), format_memory_mib(total))
}

fn joined_value(value: Option<&Value>) -> String {
    match value {
        Some(Value::Array(values)) => {
            let text = values.iter().filter_map(display_value).collect::<Vec<_>>();
            if text.is_empty() {
                "-".to_owned()
            } else {
                text.join("，")
            }
        }
        Some(value) => display_value(value).unwrap_or_else(|| "-".to_owned()),
        None => "-".to_owned(),
    }
}

fn format_duration(seconds: f64) -> String {
    let seconds = seconds.max(0.0) as u64;
    let days = seconds / 86_400;
    let hours = seconds % 86_400 / 3_600;
    let minutes = seconds % 3_600 / 60;
    let seconds = seconds % 60;
    if days > 0 {
        format!("{days}天 {hours:02}:{minutes:02}:{seconds:02}")
    } else {
        format!("{hours:02}:{minutes:02}:{seconds:02}")
    }
}
