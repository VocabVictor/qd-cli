use super::*;

impl ApiClient {
    pub fn insecure_tls(&self) -> bool {
        self.config.insecure_tls
    }

    pub async fn username(&self) -> String {
        self.session.lock().await.username.clone()
    }

    /// 复用同一登录会话（含 token 自动刷新）与 HTTP 连接池，只换 spaceid 头。
    pub fn with_space(&self, space_id: Option<String>) -> Self {
        let mut config = self.config.clone();
        config.space_id = space_id;
        Self {
            http: self.http.clone(),
            config,
            session: self.session.clone(),
        }
    }

    pub fn from_saved_session(config: Config) -> Result<Self> {
        let session = if let Ok(token) = env::var("QD_TOKEN") {
            Session {
                username: env::var("QD_USERNAME").unwrap_or_else(|_| "environment".into()),
                access_token: token,
                refresh_token: env::var("QD_REFRESH_TOKEN").unwrap_or_default(),
            }
        } else {
            Session::load()?
        };
        Self::new(config, session)
    }

    pub fn new(config: Config, session: Session) -> Result<Self> {
        let http = build_http_client(&config)?;
        Ok(Self {
            http,
            config,
            session: Arc::new(Mutex::new(session)),
        })
    }

    pub async fn login(
        config: &Config,
        username: &str,
        password: &str,
        ldap: bool,
        captcha_id: Option<&str>,
        captcha_code: Option<&str>,
    ) -> Result<Session> {
        let http = build_http_client(config)?;
        let public_key_url = format!("{}{}", config.base_url, PUBLIC_KEY_PATH);
        let pem = send_connect_retry(|| http.get(&public_key_url))
            .await
            .context("获取登录公钥失败")?
            .error_for_status()
            .context("登录公钥接口返回错误")?
            .text()
            .await?;
        let public_key = RsaPublicKey::from_public_key_pem(&pem).context("登录公钥格式无效")?;
        let encrypted = public_key
            .encrypt(&mut OsRng, Pkcs1v15Encrypt, password.as_bytes())
            .context("密码 RSA 加密失败")?;
        let body = LoginBody {
            username,
            password: STANDARD.encode(encrypted),
            captcha_id,
            captcha_code,
        };
        let path = if ldap { "/ldap/login" } else { "/user/login" };
        let url = format!("{}{}{}", config.base_url, AUTH_PREFIX, path);
        let response = send_connect_retry(|| http.post(&url).json(&body))
            .await
            .context("账号密码登录请求失败")?;
        let status = response.status();
        let value: Value = response.json().await.context("登录响应不是有效 JSON")?;
        if !status.is_success() {
            bail!("登录失败: HTTP {status}");
        }
        ensure_api_success(&value)?;
        let data = value.get("data").context("登录响应缺少 data")?;
        let access_token = string_field(data, "token")?;
        let refresh_token = data
            .get("refreshToken")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        Ok(Session {
            username: username.to_owned(),
            access_token,
            refresh_token,
        })
    }

    pub async fn get(&self, service: Service, path: &str, query: Value) -> Result<Value> {
        self.request(Method::GET, service, path, query, true).await
    }

    pub async fn send(
        &self,
        method: Method,
        service: Service,
        path: &str,
        data: Value,
    ) -> Result<Value> {
        let safe_retry = matches!(method, Method::GET | Method::HEAD | Method::OPTIONS);
        self.request(method, service, path, data, safe_retry).await
    }

    pub async fn refresh_session(&self) -> Result<()> {
        let access_token = self.session.lock().await.access_token.clone();
        self.refresh_if_needed(&access_token).await
    }

    pub async fn get_absolute_text(&self, url: &str) -> Result<AbsoluteTextResponse> {
        let token = self.session.lock().await.access_token.clone();
        let mut request = self
            .http
            .get(url)
            .bearer_auth(token)
            .header("Accept-Language", "zh-Hans");
        if let Some(space_id) = &self.config.space_id {
            request = request.header("spaceid", space_id);
        }
        let response = request.send().await.context("读取网页终端入口失败")?;
        let status = response.status();
        let final_url = response.url().clone();
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default()
            .to_owned();
        let text = response.text().await.context("读取网页终端内容失败")?;
        Ok(AbsoluteTextResponse {
            status,
            content_type,
            final_url,
            text,
        })
    }

    async fn request(
        &self,
        method: Method,
        service: Service,
        path: &str,
        data: Value,
        safe_retry: bool,
    ) -> Result<Value> {
        let url = self.resolve_url(service, path)?;
        let mut attempt = 0u32;
        let mut refreshed = false;
        loop {
            let token = self.session.lock().await.access_token.clone();
            let mut request = self
                .http
                .request(method.clone(), &url)
                .bearer_auth(&token)
                .header("Accept-Language", "zh-Hans");
            if let Some(space_id) = &self.config.space_id {
                request = request.header("spaceid", space_id);
            }
            request = if matches!(method, Method::GET | Method::DELETE | Method::HEAD) {
                request.query(&data)
            } else {
                request.json(&data)
            };

            let response = match request.send().await {
                Ok(response) => response,
                Err(error)
                    if safe_retry && attempt < 3 && (error.is_timeout() || error.is_connect()) =>
                {
                    backoff(attempt).await;
                    attempt += 1;
                    continue;
                }
                Err(error) => return Err(error).context("平台 API 请求失败"),
            };
            let status = response.status();

            if status == StatusCode::UNAUTHORIZED && !refreshed {
                self.refresh_if_needed(&token).await?;
                refreshed = true;
                continue;
            }
            if safe_retry
                && attempt < 3
                && (status == StatusCode::TOO_MANY_REQUESTS || status.is_server_error())
            {
                backoff(attempt).await;
                attempt += 1;
                continue;
            }

            let text = response.text().await.context("读取 API 响应失败")?;
            if !status.is_success() {
                let detail = safe_error_message(&text);
                bail!("平台 API 返回 HTTP {status}: {detail}");
            }
            let value: Value = serde_json::from_str(&text).context("平台 API 响应不是有效 JSON")?;
            ensure_api_success(&value)?;
            return Ok(value);
        }
    }

    async fn refresh_if_needed(&self, rejected_token: &str) -> Result<()> {
        let mut session = self.session.lock().await;
        if session.access_token != rejected_token {
            return Ok(());
        }
        if session.refresh_token.is_empty() {
            bail!("登录已过期且没有 refresh token，请重新运行 qd login");
        }
        let url = format!(
            "{}{}{}",
            self.config.base_url, AUTH_PREFIX, "/token/refresh"
        );
        let response = self
            .http
            .get(url)
            .query(&[("refreshToken", session.refresh_token.as_str())])
            .send()
            .await
            .context("刷新登录状态失败")?;
        let status = response.status();
        let value: Value = response.json().await.context("刷新响应不是有效 JSON")?;
        if !status.is_success() {
            if status == StatusCode::UNAUTHORIZED
                && let Some(credentials) = SavedCredentials::load_optional()?
            {
                let renewed = Self::login(
                    &self.config,
                    credentials.username(),
                    credentials.password(),
                    credentials.ldap(),
                    None,
                    None,
                )
                .await
                .context("refresh token 已失效，自动重新登录失败")?;
                *session = renewed;
                session.save()?;
                return Ok(());
            }
            bail!("刷新登录状态失败: HTTP {status}；请重新运行 qd login --username <账号>");
        }
        ensure_api_success(&value)?;
        let token = value
            .pointer("/data/token")
            .and_then(Value::as_str)
            .context("刷新响应缺少 token")?;
        session.access_token = token.to_owned();
        if let Some(refresh_token) = value
            .pointer("/data/refreshToken")
            .and_then(Value::as_str)
            .filter(|token| !token.is_empty())
        {
            session.refresh_token = refresh_token.to_owned();
        }
        session.save()?;
        Ok(())
    }

    fn resolve_url(&self, service: Service, path: &str) -> Result<String> {
        Ok(format!(
            "{}{}",
            self.config.base_url,
            resolve_api_path(service, path)?
        ))
    }
}
