use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::{env, fs, path::PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub base_url: String,
    pub space_id: Option<String>,
    pub concurrency: usize,
    pub connect_timeout_secs: u64,
    pub request_timeout_secs: u64,
    pub insecure_tls: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            base_url: String::new(),
            space_id: None,
            concurrency: 32,
            connect_timeout_secs: 5,
            request_timeout_secs: 30,
            insecure_tls: false,
        }
    }
}

impl Config {
    pub fn load() -> Result<Self> {
        let path = config_path()?;
        if !path.exists() {
            let config = Self::default();
            config.save()?;
            return Ok(config);
        }
        let bytes =
            fs::read(&path).with_context(|| format!("无法读取配置文件 {}", path.display()))?;
        let mut config: Self = serde_json::from_slice(&bytes)
            .with_context(|| format!("配置文件格式错误 {}", path.display()))?;
        config.normalize()?;
        Ok(config)
    }

    pub fn save(&self) -> Result<()> {
        let path = config_path()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let bytes = serde_json::to_vec_pretty(self)?;
        fs::write(&path, bytes).with_context(|| format!("无法写入配置文件 {}", path.display()))
    }

    pub fn set(&mut self, key: &str, value: &str) -> Result<()> {
        match key {
            "base-url" | "base_url" => self.base_url = value.to_owned(),
            "space-id" | "space_id" => {
                self.space_id = if value.is_empty() || value == "none" {
                    None
                } else {
                    Some(value.to_owned())
                }
            }
            "concurrency" => {
                self.concurrency = value.parse().context("concurrency 必须是正整数")?
            }
            "connect-timeout" | "connect_timeout_secs" => {
                self.connect_timeout_secs = value.parse().context("连接超时必须是正整数秒")?
            }
            "request-timeout" | "request_timeout_secs" => {
                self.request_timeout_secs = value.parse().context("请求超时必须是正整数秒")?
            }
            "insecure-tls" | "insecure_tls" => {
                self.insecure_tls = parse_bool(value)?;
            }
            _ => bail!("未知配置项: {key}"),
        }
        self.normalize()?;
        self.save()
    }

    fn normalize(&mut self) -> Result<()> {
        self.base_url = self.base_url.trim_end_matches('/').to_owned();
        if !self.base_url.is_empty()
            && !(self.base_url.starts_with("https://") || self.base_url.starts_with("http://"))
        {
            bail!("base_url 必须以 http:// 或 https:// 开头");
        }
        if self.concurrency == 0 || self.concurrency > 1024 {
            bail!("concurrency 必须在 1..=1024 范围内");
        }
        Ok(())
    }

    pub fn require_base_url(&self) -> Result<()> {
        if self.base_url.is_empty() {
            bail!(
                "未配置平台地址；请运行 `qd config set base-url URL`，或本次使用 `--base-url URL`"
            );
        }
        Ok(())
    }
}

fn parse_bool(value: &str) -> Result<bool> {
    match value.to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Ok(true),
        "0" | "false" | "no" | "off" => Ok(false),
        _ => bail!("布尔值只接受 true/false、yes/no、on/off 或 1/0"),
    }
}

fn home_join(parts: &[&str]) -> Option<PathBuf> {
    env::var_os("HOME").map(|home| {
        let mut path = PathBuf::from(home);
        for part in parts {
            path.push(part);
        }
        path
    })
}

pub fn config_path() -> Result<PathBuf> {
    let root = env::var_os("APPDATA")
        .or_else(|| env::var_os("XDG_CONFIG_HOME"))
        .map(PathBuf::from)
        .or_else(|| home_join(&[".config"]))
        .context("找不到 APPDATA/XDG_CONFIG_HOME/HOME")?;
    Ok(root.join("qd").join("config.json"))
}

pub fn session_path() -> Result<PathBuf> {
    let root = env::var_os("LOCALAPPDATA")
        .or_else(|| env::var_os("XDG_STATE_HOME"))
        .map(PathBuf::from)
        .or_else(|| home_join(&[".local", "state"]))
        .context("找不到 LOCALAPPDATA/XDG_STATE_HOME/HOME")?;
    Ok(root.join("qd").join("session.bin"))
}

pub fn credentials_path() -> Result<PathBuf> {
    let root = env::var_os("LOCALAPPDATA")
        .or_else(|| env::var_os("XDG_STATE_HOME"))
        .map(PathBuf::from)
        .or_else(|| home_join(&[".local", "state"]))
        .context("找不到 LOCALAPPDATA/XDG_STATE_HOME/HOME")?;
    Ok(root.join("qd").join("credentials.bin"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_is_bounded_and_normalized() {
        let mut config = Config::default();
        config.base_url.push('/');
        config.normalize().unwrap();
        assert!(!config.base_url.ends_with('/'));
        assert!((1..=1024).contains(&config.concurrency));
        assert!(config.base_url.is_empty());
        assert!(config.space_id.is_none());
        assert!(!config.insecure_tls);
        assert!(config.require_base_url().is_err());
    }

    #[test]
    fn rejects_invalid_config() {
        let mut config = Config {
            base_url: "not-a-url".into(),
            ..Config::default()
        };
        assert!(config.normalize().is_err());

        let mut config = Config {
            concurrency: 0,
            ..Config::default()
        };
        assert!(config.normalize().is_err());
    }

    #[test]
    fn parses_boolean_variants() {
        assert!(parse_bool("yes").unwrap());
        assert!(!parse_bool("OFF").unwrap());
        assert!(parse_bool("maybe").is_err());
    }
}
