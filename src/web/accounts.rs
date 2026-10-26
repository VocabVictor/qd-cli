//! `qd web` 公开绑定（--host 非 127.0.0.1）时的本机账号体系：谁能打开这台机器上的
//! 控制面板，与 GPU 平台账号（`qd login`）完全无关。存在配置目录下的
//! web-accounts.json，密码只存 argon2 hash，不落地明文/可逆密文。

use crate::*;
use crate::web::state::WebError;

use argon2::{
    Argon2,
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString, rand_core::OsRng},
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone)]
pub(crate) struct WebAccount {
    pub(crate) username: String,
    password_hash: String,
    #[serde(default)]
    #[allow(dead_code)]
    created_at: i64,
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
    Ok(serde_json::from_str(&text).unwrap_or_default())
}

fn save(list: &[WebAccount]) -> Result<(), WebError> {
    let path = accounts_path()?;
    if let Some(dir) = path.parent() {
        let _ = fs::create_dir_all(dir);
    }
    let text = serde_json::to_string_pretty(list)
        .map_err(|err| WebError::bad_request(format!("序列化本机账号失败: {err}")))?;
    fs::write(&path, text).map_err(|err| WebError::bad_request(format!("写入本机账号失败: {err}")))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

pub(crate) fn has_any() -> Result<bool, WebError> {
    Ok(!load()?.is_empty())
}

pub(crate) fn register(username: &str, password: &str) -> Result<(), WebError> {
    let username = username.trim();
    if username.is_empty() || username.chars().count() > 60 {
        return Err(WebError::bad_request("用户名长度必须在 1~60 字符之间"));
    }
    if password.len() < 8 {
        return Err(WebError::bad_request("密码至少 8 位"));
    }
    let mut list = load()?;
    if list
        .iter()
        .any(|account| account.username.eq_ignore_ascii_case(username))
    {
        return Err(WebError::bad_request("用户名已存在"));
    }
    let salt = SaltString::generate(&mut OsRng);
    let password_hash = Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map_err(|_| WebError::bad_request("密码加密失败"))?
        .to_string();
    list.push(WebAccount {
        username: username.to_owned(),
        password_hash,
        created_at: now_unix(),
    });
    save(&list)
}

pub(crate) fn verify(username: &str, password: &str) -> Result<bool, WebError> {
    let list = load()?;
    let Some(account) = list
        .iter()
        .find(|account| account.username.eq_ignore_ascii_case(username))
    else {
        return Ok(false);
    };
    let Ok(parsed_hash) = PasswordHash::new(&account.password_hash) else {
        return Ok(false);
    };
    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_ok())
}

fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}
