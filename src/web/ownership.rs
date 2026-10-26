//! 作业归属：所有作业都用同一个平台账号提交，平台分不清是哪个本机用户提的，
//! 所以由 qd web 自己记在 web-jobs.json 里，用于"只能管自己的作业"和 GPU 配额。

use super::quota::{self, Usage};
use super::state::{WebError, WebState};
use crate::*;

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;

static LOCK: Mutex<()> = Mutex::new(());

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub(crate) struct JobRecord {
    pub(crate) job_id: String,
    pub(crate) user: String,
    pub(crate) space_id: String,
    pub(crate) rsgroup_id: String,
    pub(crate) gpu: u32,
    pub(crate) created_at: i64,
    /// 已到终态或已删除：不再占配额，但归属记录保留（日志/详情仍按归属判断）。
    #[serde(default)]
    pub(crate) released: bool,
}

fn path() -> Result<PathBuf, WebError> {
    let path = crate::config::config_path()
        .map_err(|err| WebError::bad_request(format!("定位配置目录失败: {err}")))?;
    Ok(path.with_file_name("web-jobs.json"))
}

fn edit<T>(job: impl FnOnce(&mut Vec<JobRecord>) -> (T, bool)) -> Result<T, WebError> {
    let _guard = LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let path = path()?;
    let mut list: Vec<JobRecord> = if path.exists() {
        let text = fs::read_to_string(&path)
            .map_err(|err| WebError::bad_request(format!("读取作业归属失败: {err}")))?;
        serde_json::from_str(&text)
            .map_err(|err| WebError::bad_request(format!("作业归属文件损坏，拒绝覆盖: {err}")))?
    } else {
        Vec::new()
    };
    let (value, dirty) = job(&mut list);
    if dirty {
        if let Some(dir) = path.parent() {
            let _ = fs::create_dir_all(dir);
        }
        let text = serde_json::to_string_pretty(&list)
            .map_err(|err| WebError::bad_request(format!("序列化作业归属失败: {err}")))?;
        fs::write(&path, text)
            .map_err(|err| WebError::bad_request(format!("写入作业归属失败: {err}")))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
        }
    }
    Ok(value)
}

pub(crate) fn record(entry: JobRecord) -> Result<(), WebError> {
    edit(|list| {
        list.retain(|existing| existing.job_id != entry.job_id);
        list.push(entry);
        ((), true)
    })
}

pub(crate) fn owner_of(job_id: &str) -> Result<Option<String>, WebError> {
    edit(|list| {
        let owner = list
            .iter()
            .find(|entry| entry.job_id == job_id)
            .map(|entry| entry.user.clone());
        (owner, false)
    })
}

/// 所有作业 ID → 提交者，给前端标注"我的作业"用。
pub(crate) fn owners() -> Result<Vec<(String, String)>, WebError> {
    edit(|list| {
        let pairs = list
            .iter()
            .map(|entry| (entry.job_id.clone(), entry.user.clone()))
            .collect();
        (pairs, false)
    })
}

fn active_of(user: &str) -> Result<Vec<JobRecord>, WebError> {
    edit(|list| {
        let active = list
            .iter()
            .filter(|entry| !entry.released && entry.user.eq_ignore_ascii_case(user))
            .cloned()
            .collect();
        (active, false)
    })
}

pub(crate) fn mark_released(job_ids: &[String]) -> Result<(), WebError> {
    edit(|list| {
        let mut dirty = false;
        for entry in list.iter_mut().filter(|entry| job_ids.contains(&entry.job_id)) {
            dirty |= !entry.released;
            entry.released = true;
        }
        ((), dirty)
    })
}

/// 用户当前占用：逐个问平台还没释放的作业状态，到终态的顺手标记释放。
/// 平台查询失败的作业按"仍在占用"算（宁可少给卡，不让配额被绕过）。
pub(crate) async fn current_usage(state: &WebState, user: &str) -> Result<Usage, WebError> {
    let records = active_of(user)?;
    if records.is_empty() {
        return Ok(Usage::default());
    }
    state.api().await?;
    let checks = records.iter().map(|entry| async move {
        let Ok(api) = state.api_for(Some(&entry.space_id)).await else {
            return false;
        };
        match job_show(&api, &entry.job_id).await {
            Ok(detail) => {
                let data = detail.get("data").unwrap_or(&detail);
                let status = data.get("status").and_then(Value::as_str).unwrap_or("");
                quota::is_terminal_status(status)
            }
            Err(_) => false,
        }
    });
    let finished = futures_util::future::join_all(checks).await;
    let released: Vec<String> = records
        .iter()
        .zip(&finished)
        .filter(|(_, done)| **done)
        .map(|(entry, _)| entry.job_id.clone())
        .collect();
    if !released.is_empty() {
        mark_released(&released)?;
    }
    let active: Vec<JobRecord> = records
        .into_iter()
        .zip(finished)
        .filter(|(_, done)| !done)
        .map(|(entry, _)| entry)
        .collect();
    Ok(quota::usage_of(&active))
}
