use crate::config::session_path;
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub username: String,
    pub access_token: String,
    pub refresh_token: String,
}

impl Session {
    pub fn load() -> Result<Self> {
        let path = session_path()?;
        if !path.exists() {
            bail!("尚未登录，请先运行 qd login --username <账号>");
        }
        let encrypted =
            fs::read(&path).with_context(|| format!("无法读取登录状态 {}", path.display()))?;
        let plain = unprotect(&encrypted)?;
        serde_json::from_slice(&plain).context("登录状态损坏，请重新登录")
    }

    pub fn save(&self) -> Result<()> {
        let path = session_path()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let plain = serde_json::to_vec(self)?;
        let encrypted = protect(&plain)?;
        fs::write(&path, encrypted)
            .with_context(|| format!("无法保存登录状态 {}", path.display()))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
        }
        Ok(())
    }

    pub fn remove() -> Result<bool> {
        let path = session_path()?;
        if path.exists() {
            fs::remove_file(path)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

#[cfg(windows)]
pub(crate) fn protect(data: &[u8]) -> Result<Vec<u8>> {
    use std::{ptr, slice};
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Cryptography::{
        CRYPT_INTEGER_BLOB, CRYPTPROTECT_UI_FORBIDDEN, CryptProtectData,
    };

    let input = CRYPT_INTEGER_BLOB {
        cbData: data.len().try_into().context("登录状态过大")?,
        pbData: data.as_ptr() as *mut u8,
    };
    let mut output = CRYPT_INTEGER_BLOB::default();
    let ok = unsafe {
        CryptProtectData(
            &input,
            ptr::null(),
            ptr::null(),
            ptr::null(),
            ptr::null(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )
    };
    if ok == 0 {
        return Err(std::io::Error::last_os_error()).context("Windows DPAPI 加密失败");
    }
    let result = unsafe { slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec() };
    unsafe { LocalFree(output.pbData as *mut core::ffi::c_void) };
    Ok(result)
}

#[cfg(windows)]
pub(crate) fn unprotect(data: &[u8]) -> Result<Vec<u8>> {
    use std::{ptr, slice};
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Cryptography::{
        CRYPT_INTEGER_BLOB, CRYPTPROTECT_UI_FORBIDDEN, CryptUnprotectData,
    };

    let input = CRYPT_INTEGER_BLOB {
        cbData: data.len().try_into().context("登录状态过大")?,
        pbData: data.as_ptr() as *mut u8,
    };
    let mut output = CRYPT_INTEGER_BLOB::default();
    let ok = unsafe {
        CryptUnprotectData(
            &input,
            ptr::null_mut(),
            ptr::null(),
            ptr::null(),
            ptr::null(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )
    };
    if ok == 0 {
        return Err(std::io::Error::last_os_error()).context("Windows DPAPI 解密失败");
    }
    let result = unsafe { slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec() };
    unsafe { LocalFree(output.pbData as *mut core::ffi::c_void) };
    Ok(result)
}

// Linux: AES-256-GCM，密钥由 machine-id + 用户名派生——语义对齐 DPAPI 的
// “绑定本机与当前用户”：密文拷到其他机器或其他用户下无法解开。
#[cfg(not(windows))]
fn derive_key() -> [u8; 32] {
    use sha2::{Digest, Sha256};
    let machine_id = fs::read_to_string("/etc/machine-id")
        .or_else(|_| fs::read_to_string("/var/lib/dbus/machine-id"))
        .unwrap_or_default();
    let user = std::env::var("USER")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_default();
    let mut hasher = Sha256::new();
    hasher.update(b"qd-linux-session-v1\0");
    hasher.update(machine_id.trim().as_bytes());
    hasher.update(b"\0");
    hasher.update(user.as_bytes());
    hasher.finalize().into()
}

#[cfg(not(windows))]
pub(crate) fn protect(data: &[u8]) -> Result<Vec<u8>> {
    use aes_gcm::{Aes256Gcm, KeyInit, Nonce, aead::Aead};
    use rand::RngCore;
    let cipher = Aes256Gcm::new((&derive_key()).into());
    let mut nonce = [0u8; 12];
    rand::thread_rng().fill_bytes(&mut nonce);
    let sealed = cipher
        .encrypt(Nonce::from_slice(&nonce), data)
        .map_err(|_| anyhow::anyhow!("会话加密失败"))?;
    let mut out = Vec::with_capacity(16 + sealed.len());
    out.extend_from_slice(b"QDL1");
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&sealed);
    Ok(out)
}

#[cfg(not(windows))]
pub(crate) fn unprotect(data: &[u8]) -> Result<Vec<u8>> {
    use aes_gcm::{Aes256Gcm, KeyInit, Nonce, aead::Aead};
    if data.len() < 16 || &data[..4] != b"QDL1" {
        bail!("会话文件格式无法识别（可能来自其他平台或已损坏），请重新登录");
    }
    let cipher = Aes256Gcm::new((&derive_key()).into());
    cipher
        .decrypt(Nonce::from_slice(&data[4..16]), &data[16..])
        .map_err(|_| anyhow::anyhow!("会话解密失败（机器或用户不匹配），请重新登录"))
}

#[cfg(all(test, not(windows)))]
mod linux_tests {
    use super::*;

    #[test]
    fn linux_protect_round_trip() {
        let plain = b"unit-test-session-material";
        let encrypted = protect(plain).unwrap();
        assert_ne!(&encrypted[..], &plain[..]);
        assert_eq!(&encrypted[..4], b"QDL1");
        assert_eq!(unprotect(&encrypted).unwrap(), plain);
    }

    #[test]
    fn linux_unprotect_rejects_garbage() {
        assert!(unprotect(b"not-a-session").is_err());
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn dpapi_round_trip() {
        let plain = b"unit-test-session-material";
        let encrypted = protect(plain).unwrap();
        assert_ne!(encrypted, plain);
        assert_eq!(unprotect(&encrypted).unwrap(), plain);
    }
}
