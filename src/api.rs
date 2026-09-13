mod client;
mod helpers;
#[cfg(test)]
mod tests;

pub(crate) use helpers::*;

use crate::{config::Config, credentials::SavedCredentials, session::Session};
use anyhow::{Context, Result, anyhow, bail};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use rand::rngs::OsRng;
use reqwest::{Client, Method, StatusCode};
use rsa::{Pkcs1v15Encrypt, RsaPublicKey, pkcs8::DecodePublicKey};
use serde::Serialize;
use serde_json::{Value, json};
use std::{env, sync::Arc, time::Duration};
use tokio::sync::Mutex;

const CORE_PREFIX: &str = "/gemini/v1/gemini_api/gemini_api";
const AUTH_PREFIX: &str = "/gemini/v1/gemini_userauth";
const TOOL_PREFIX: &str = "/gemini/v1/geminitool_api/geminitool_api";
const PUBLIC_KEY_PATH: &str = "/gemini_web/gemini_auth_web/keys/public.pem";

#[derive(Debug, Clone, Copy)]
pub enum Service {
    Core,
    Auth,
    Tool,
    Absolute,
}

impl Service {
    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "core" => Ok(Self::Core),
            "auth" => Ok(Self::Auth),
            "tool" => Ok(Self::Tool),
            "absolute" | "raw" => Ok(Self::Absolute),
            _ => bail!("service 只接受 core、auth、tool 或 absolute"),
        }
    }
}

#[derive(Clone)]
pub struct ApiClient {
    http: Client,
    config: Config,
    session: Arc<Mutex<Session>>,
}

pub struct AbsoluteTextResponse {
    pub status: StatusCode,
    pub content_type: String,
    pub final_url: reqwest::Url,
    pub text: String,
}

#[derive(Serialize)]
struct LoginBody<'a> {
    #[serde(rename = "userName")]
    username: &'a str,
    password: String,
    #[serde(rename = "captchaId", skip_serializing_if = "Option::is_none")]
    captcha_id: Option<&'a str>,
    #[serde(rename = "captchaCode", skip_serializing_if = "Option::is_none")]
    captcha_code: Option<&'a str>,
}
