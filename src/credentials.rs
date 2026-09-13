use crate::{
    config::credentials_path,
    session::{protect, unprotect},
};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Serialize, Deserialize)]
pub struct SavedCredentials {
    username: String,
    password: String,
    #[serde(default)]
    ldap: bool,
}

impl SavedCredentials {
    pub fn new(username: String, password: String, ldap: bool) -> Self {
        Self {
            username,
            password,
            ldap,
        }
    }

    pub fn username(&self) -> &str {
        &self.username
    }

    pub fn password(&self) -> &str {
        &self.password
    }

    pub fn ldap(&self) -> bool {
        self.ldap
    }

    pub fn load_optional() -> Result<Option<Self>> {
        let path = credentials_path()?;
        if !path.exists() {
            return Ok(None);
        }
        let encrypted =
            fs::read(&path).with_context(|| format!("无法读取自动登录凭据 {}", path.display()))?;
        let mut plain = unprotect(&encrypted)?;
        let result = serde_json::from_slice(&plain).context("自动登录凭据损坏，请重新登录");
        plain.fill(0);
        Ok(Some(result?))
    }

    pub fn save(&self) -> Result<()> {
        let path = credentials_path()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut plain = serde_json::to_vec(self)?;
        let encrypted = protect(&plain);
        plain.fill(0);
        let encrypted = encrypted?;
        fs::write(&path, encrypted)
            .with_context(|| format!("无法保存自动登录凭据 {}", path.display()))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
        }
        Ok(())
    }

    pub fn remove() -> Result<bool> {
        let path = credentials_path()?;
        if path.exists() {
            fs::remove_file(path)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

impl Drop for SavedCredentials {
    fn drop(&mut self) {
        unsafe {
            self.password.as_bytes_mut().fill(0);
        }
    }
}

pub fn clear_string(value: &mut String) {
    unsafe {
        value.as_bytes_mut().fill(0);
    }
    value.clear();
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn credential_payload_is_dpapi_protected() {
        let credential = SavedCredentials::new("unit-user".into(), "unit-password".into(), false);
        let mut plain = serde_json::to_vec(&credential).unwrap();
        let encrypted = protect(&plain).unwrap();
        assert_ne!(encrypted, plain);
        let mut decrypted = unprotect(&encrypted).unwrap();
        let restored: SavedCredentials = serde_json::from_slice(&decrypted).unwrap();
        assert_eq!(restored.username(), "unit-user");
        assert_eq!(restored.password(), "unit-password");
        assert!(!restored.ldap());
        plain.fill(0);
        decrypted.fill(0);
    }
}
