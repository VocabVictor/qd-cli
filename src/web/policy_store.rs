//! 全局登录策略存在 web-login.json（0600）；单个账号可以在 web-accounts.json 里覆盖。

use super::accounts::WebAccount;
use super::login_policy::LoginPolicy;
use super::state::WebError;
use crate::*;

use std::path::PathBuf;

fn policy_path() -> Result<PathBuf, WebError> {
    let path = crate::config::config_path()
        .map_err(|err| WebError::bad_request(format!("定位配置目录失败: {err}")))?;
    Ok(path.with_file_name("web-login.json"))
}

pub(crate) fn global() -> Result<LoginPolicy, WebError> {
    let path = policy_path()?;
    if !path.exists() {
        return Ok(LoginPolicy::default());
    }
    let text = fs::read_to_string(&path)
        .map_err(|err| WebError::bad_request(format!("读取登录策略失败: {err}")))?;
    serde_json::from_str(&text)
        .map_err(|err| WebError::bad_request(format!("登录策略文件损坏: {err}")))
}

pub(crate) fn save_global(policy: &LoginPolicy) -> Result<(), WebError> {
    let path = policy_path()?;
    if let Some(dir) = path.parent() {
        let _ = fs::create_dir_all(dir);
    }
    let text = serde_json::to_string_pretty(policy)
        .map_err(|err| WebError::bad_request(format!("序列化登录策略失败: {err}")))?;
    fs::write(&path, text).map_err(|err| WebError::bad_request(format!("写入登录策略失败: {err}")))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

/// 全局策略叠加账号自己的逐项覆盖；None 表示这个账号不受登录限制。
pub(crate) fn effective(account: &WebAccount) -> Result<Option<LoginPolicy>, WebError> {
    Ok(super::login_override::resolve(&global()?, account.login.as_ref()))
}
