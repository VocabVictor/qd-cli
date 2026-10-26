//! `qd web` 公开绑定（--host 非 127.0.0.1）时的本机账号体系：谁能打开这台机器上的
//! 控制面板、能用多少 GPU 卡，与 GPU 平台账号（`qd login`）完全无关。存在配置目录下的
//! web-accounts.json，密码只存 argon2 hash。

use crate::web::state::WebError;
use crate::*;

use argon2::{
    Argon2,
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString, rand_core::OsRng},
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Mutex;

/// 读改写整个文件期间持有，防止并发请求互相覆盖。
static LOCK: Mutex<()> = Mutex::new(());

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Role {
    Admin,
    #[default]
    User,
}

/// GPU 卡数上限：总数、按空间 ID、按资源组 ID；表里没有的空间/资源组按 0 处理。
#[derive(Serialize, Deserialize, Clone, Default, Debug, PartialEq)]
#[serde(default)]
pub(crate) struct Quota {
    pub(crate) total: u32,
    pub(crate) spaces: BTreeMap<String, u32>,
    pub(crate) groups: BTreeMap<String, u32>,
}

#[derive(Serialize, Deserialize, Clone)]
pub(crate) struct WebAccount {
    pub(crate) username: String,
    password_hash: String,
    #[serde(default)]
    pub(crate) role: Role,
    #[serde(default)]
    pub(crate) quota: Quota,
    #[serde(default)]
    pub(crate) created_at: i64,
}

fn accounts_path() -> Result<PathBuf, WebError> {
    let path = crate::config::config_path()
        .map_err(|err| WebError::bad_request(format!("定位配置目录失败: {err}")))?;
    Ok(path.with_file_name("web-accounts.json"))
}

fn load() -> Result<Vec<WebAccount>, WebError> {
    let path = accounts_path()?;
    if !path.exists() {
        return Ok(Vec::new());
    }
    let text = fs::read_to_string(&path)
        .map_err(|err| WebError::bad_request(format!("读取本机账号失败: {err}")))?;
    serde_json::from_str(&text)
        .map_err(|err| WebError::bad_request(format!("本机账号文件损坏，拒绝覆盖: {err}")))
}

fn save(list: &[WebAccount]) -> Result<(), WebError> {
    let path = accounts_path()?;
    if let Some(dir) = path.parent() {
        let _ = fs::create_dir_all(dir);
    }
    let text = serde_json::to_string_pretty(list)
        .map_err(|err| WebError::bad_request(format!("序列化本机账号失败: {err}")))?;
    fs::write(&path, text)
        .map_err(|err| WebError::bad_request(format!("写入本机账号失败: {err}")))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

/// 持锁执行一次读改写；闭包返回 (结果, 是否需要落盘)。
fn edit<T>(job: impl FnOnce(&mut Vec<WebAccount>) -> Result<(T, bool), WebError>) -> Result<T, WebError> {
    let _guard = LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut list = load()?;
    let (value, dirty) = job(&mut list)?;
    if dirty {
        save(&list)?;
    }
    Ok(value)
}

fn position(list: &[WebAccount], username: &str) -> Result<usize, WebError> {
    list.iter()
        .position(|account| account.username.eq_ignore_ascii_case(username.trim()))
        .ok_or_else(|| WebError::bad_request("没有这个用户"))
}

fn hash(password: &str) -> Result<String, WebError> {
    if password.len() < 8 {
        return Err(WebError::bad_request("密码至少 8 位"));
    }
    let salt = SaltString::generate(&mut OsRng);
    Ok(Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map_err(|_| WebError::bad_request("密码加密失败"))?
        .to_string())
}

fn other_admins(list: &[WebAccount], index: usize) -> usize {
    list.iter()
        .enumerate()
        .filter(|(i, account)| *i != index && account.role == Role::Admin)
        .count()
}

pub(crate) fn has_any() -> Result<bool, WebError> {
    edit(|list| Ok((!list.is_empty(), false)))
}

pub(crate) fn list() -> Result<Vec<WebAccount>, WebError> {
    edit(|list| Ok((list.clone(), false)))
}

pub(crate) fn get(username: &str) -> Result<Option<WebAccount>, WebError> {
    edit(|list| Ok((position(list, username).ok().map(|i| list[i].clone()), false)))
}

pub(crate) fn register(username: &str, password: &str, role: Role) -> Result<(), WebError> {
    let username = username.trim();
    let valid_chars = username
        .chars()
        .all(|c| c.is_alphanumeric() || matches!(c, '_' | '-' | '.'));
    if username.is_empty() || username.chars().count() > 32 || !valid_chars {
        return Err(WebError::bad_request("用户名 1~32 位，只能用字母、数字、下划线、短横线和点"));
    }
    let password_hash = hash(password)?;
    edit(|list| {
        if position(list, username).is_ok() {
            return Err(WebError::bad_request("用户名已存在"));
        }
        list.push(WebAccount {
            username: username.to_owned(),
            password_hash,
            role,
            quota: Quota::default(),
            created_at: unix_millis() as i64 / 1000,
        });
        Ok(((), true))
    })
}

/// 校验通过返回规范化的用户名（注册时的大小写）。
pub(crate) fn verify(username: &str, password: &str) -> Result<Option<String>, WebError> {
    let Some(account) = get(username)? else {
        // 不存在的用户也跑一次 hash，避免靠响应耗时探测用户名是否存在。
        let _ = hash("timing-equalizer-password");
        return Ok(None);
    };
    let Ok(parsed) = PasswordHash::new(&account.password_hash) else {
        return Ok(None);
    };
    let ok = Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok();
    Ok(ok.then_some(account.username))
}

pub(crate) fn set_password(username: &str, password: &str) -> Result<(), WebError> {
    let password_hash = hash(password)?;
    edit(|list| {
        let index = position(list, username)?;
        list[index].password_hash = password_hash;
        Ok(((), true))
    })
}

pub(crate) fn set_role(username: &str, role: Role) -> Result<(), WebError> {
    edit(|list| {
        let index = position(list, username)?;
        if role == Role::User && list[index].role == Role::Admin && other_admins(list, index) == 0 {
            return Err(WebError::bad_request("不能降级最后一个管理员"));
        }
        list[index].role = role;
        Ok(((), true))
    })
}

pub(crate) fn set_quota(username: &str, quota: Quota) -> Result<(), WebError> {
    edit(|list| {
        let index = position(list, username)?;
        list[index].quota = quota;
        Ok(((), true))
    })
}

pub(crate) fn delete(username: &str) -> Result<String, WebError> {
    edit(|list| {
        let index = position(list, username)?;
        if list[index].role == Role::Admin && other_admins(list, index) == 0 {
            return Err(WebError::bad_request("不能删除最后一个管理员"));
        }
        Ok((list.remove(index).username, true))
    })
}
