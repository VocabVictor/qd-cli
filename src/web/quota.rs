//! GPU 配额的纯计算：从作业定义算卡数、汇总占用、判定能否再提交。
//! 不碰网络和文件，方便单测。

use super::accounts::Quota;
use super::ownership::JobRecord;
use crate::*;

use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Serialize, Default, Debug, PartialEq)]
pub(crate) struct Usage {
    pub(crate) total: u32,
    pub(crate) spaces: BTreeMap<String, u32>,
    pub(crate) groups: BTreeMap<String, u32>,
}

fn number(value: Option<&Value>) -> Option<u64> {
    value.and_then(|v| value_u64(v).or_else(|| v.as_f64().map(|f| f.max(0.0) as u64)))
}

/// 一个作业要占的卡数 = Σ taskroles[i].gpu × instance（instance 缺省 1）。
/// 只看请求体本身，不信前端另传的汇总值。
pub(crate) fn job_gpu(body: &Value) -> u32 {
    let roles = body.get("taskroles").and_then(Value::as_array);
    let total: u64 = roles
        .into_iter()
        .flatten()
        .map(|role| {
            let gpu = number(role.get("gpu")).unwrap_or(0);
            let instance = number(role.get("instance")).unwrap_or(1).max(1);
            gpu.saturating_mul(instance)
        })
        .fold(0u64, u64::saturating_add);
    total.min(u32::MAX as u64) as u32
}

pub(crate) fn usage_of(active: &[JobRecord]) -> Usage {
    let mut usage = Usage::default();
    for job in active.iter().filter(|job| job.gpu > 0) {
        usage.total = usage.total.saturating_add(job.gpu);
        let space = usage.spaces.entry(job.space_id.clone()).or_default();
        *space = space.saturating_add(job.gpu);
        let group = usage.groups.entry(job.rsgroup_id.clone()).or_default();
        *group = group.saturating_add(job.gpu);
    }
    usage
}

/// 三个维度都满足"已用 + 本次 ≤ 上限"才放行；CPU 作业（0 卡）永远放行。
pub(crate) fn check(quota: &Quota, usage: &Usage, space: &str, group: &str, gpu: u32) -> Result<(), String> {
    if gpu == 0 {
        return Ok(());
    }
    let limits = [
        ("总配额".to_owned(), quota.total, usage.total),
        (
            format!("空间 {space}"),
            quota.spaces.get(space).copied().unwrap_or(0),
            usage.spaces.get(space).copied().unwrap_or(0),
        ),
        (
            format!("资源组 {group}"),
            quota.groups.get(group).copied().unwrap_or(0),
            usage.groups.get(group).copied().unwrap_or(0),
        ),
    ];
    for (label, limit, used) in limits {
        if used.saturating_add(gpu) > limit {
            return Err(format!(
                "GPU 配额不足：{label} 已用 {used}/{limit} 张，本次需要 {gpu} 张。请联系管理员调整配额"
            ));
        }
    }
    Ok(())
}

pub(crate) fn is_terminal_status(status: &str) -> bool {
    matches!(
        status.trim().to_ascii_uppercase().as_str(),
        "STOPPED" | "FAILED" | "SUCCEEDED" | "SUCCESS" | "CANCELLED" | "CANCELED" | "DELETED"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(space: &str, group: &str, gpu: u32) -> JobRecord {
        JobRecord {
            job_id: format!("{space}-{group}-{gpu}"),
            user: "alice".into(),
            space_id: space.into(),
            rsgroup_id: group.into(),
            gpu,
            created_at: 0,
            released: false,
        }
    }

    fn quota(total: u32, space: u32, group: u32) -> Quota {
        Quota {
            total,
            spaces: BTreeMap::from([("s1".to_owned(), space)]),
            groups: BTreeMap::from([("g1".to_owned(), group)]),
        }
    }

    #[test]
    fn job_gpu_multiplies_instances_and_sums_roles() {
        let body = json!({"taskroles": [
            {"gpu": 2, "instance": 3},
            {"gpu": "1"},
            {"gpu": 0, "instance": 8},
        ]});
        assert_eq!(job_gpu(&body), 7);
        assert_eq!(job_gpu(&json!({})), 0);
    }

    #[test]
    fn default_quota_blocks_any_gpu_but_allows_cpu() {
        let usage = Usage::default();
        assert!(check(&Quota::default(), &usage, "s1", "g1", 1).is_err());
        assert!(check(&Quota::default(), &usage, "s1", "g1", 0).is_ok());
    }

    #[test]
    fn every_dimension_is_enforced_at_the_boundary() {
        let usage = usage_of(&[record("s1", "g1", 2)]);
        assert!(check(&quota(4, 4, 4), &usage, "s1", "g1", 2).is_ok());
        let total = check(&quota(3, 4, 4), &usage, "s1", "g1", 2).unwrap_err();
        assert!(total.contains("总配额"));
        let space = check(&quota(8, 3, 8), &usage, "s1", "g1", 2).unwrap_err();
        assert!(space.contains("空间 s1"));
        let group = check(&quota(8, 8, 3), &usage, "s1", "g1", 2).unwrap_err();
        assert!(group.contains("资源组 g1"));
    }

    #[test]
    fn unlisted_space_or_group_means_zero() {
        let usage = Usage::default();
        assert!(check(&quota(8, 8, 8), &usage, "other", "g1", 1).is_err());
        assert!(check(&quota(8, 8, 8), &usage, "s1", "other", 1).is_err());
    }

    #[test]
    fn usage_ignores_cpu_jobs_and_groups_by_dimension() {
        let usage = usage_of(&[record("s1", "g1", 2), record("s1", "g2", 1), record("s2", "g1", 0)]);
        assert_eq!(usage.total, 3);
        assert_eq!(usage.spaces.get("s1"), Some(&3));
        assert_eq!(usage.spaces.get("s2"), None);
        assert_eq!(usage.groups.get("g1"), Some(&2));
    }

    #[test]
    fn terminal_statuses_release_quota() {
        assert!(is_terminal_status("succeeded"));
        assert!(is_terminal_status("STOPPED"));
        assert!(!is_terminal_status("RUNNING"));
        assert!(!is_terminal_status("WAITING"));
    }
}
