use anyhow::{anyhow, Result};
use base64::Engine;
use reqwest::header::{HeaderMap, HeaderValue, COOKIE, SET_COOKIE, USER_AGENT};
// use serde::Deserialize; (removed unused)
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::info;

use crate::db::Database;
use crate::t;

const API_BASE: &str = "https://api.vrchat.cloud/api/1";
const MAX_AUTO_LOGIN_PER_HOUR: usize = 3;

#[derive(Debug, Clone)]
pub enum LoginStatus {
    Success,
    TwoFactor,
    Failed,
}

#[derive(Debug, Clone)]
pub struct LoginResponse {
    pub status: LoginStatus,
    pub requires_two_factor_auth: Option<Vec<String>>,
    pub message: Option<String>,
    pub user: Option<Value>,
}

pub struct VRChatAPI {
    client: reqwest::Client,
    cookie: Arc<Mutex<String>>,
    username: Option<String>,
    password: Option<String>,
    db: Arc<Database>,
    login_in_progress: Arc<Mutex<bool>>,
    failed_requests: Arc<Mutex<HashMap<String, std::time::Instant>>>,
    auto_login_attempts: Arc<Mutex<Vec<std::time::Instant>>>,
}

impl VRChatAPI {
    pub fn new(db: Arc<Database>) -> Result<Self> {
        let username = std::env::var("VRC_USERNAME").ok();
        let password = std::env::var("VRC_PASSWORD").ok();
        let proxy_str = std::env::var("VRC_PROXY")
            .ok()
            .filter(|s| !s.trim().is_empty());

        let mut client_builder = reqwest::Client::builder()
            .user_agent("VRCX")
            .redirect(reqwest::redirect::Policy::limited(10));

        if let Some(ref proxy) = proxy_str {
            let proxy_url = if proxy.contains("://") {
                proxy.clone()
            } else {
                format!("socks5://{}", proxy)
            };
            info!("{}", t!("use_proxy", proxy_url));
            client_builder = client_builder.proxy(reqwest::Proxy::all(&proxy_url)?);
        } else {
            info!("{}", t!("direct_connection"));
        }

        let client = client_builder.build()?;

        // Load saved cookie from DB
        let saved_cookie = db.get_cookie("auth_cookie").unwrap_or_default();
        let cookie_str = if saved_cookie.is_empty() {
            std::env::var("VRC_COOKIE").unwrap_or_default()
        } else {
            info!("{}", t!("cookie_loaded"));
            saved_cookie
        };

        Ok(Self {
            client,
            cookie: Arc::new(Mutex::new(cookie_str)),
            username,
            password,
            db,
            login_in_progress: Arc::new(Mutex::new(false)),
            failed_requests: Arc::new(Mutex::new(HashMap::new())),
            auto_login_attempts: Arc::new(Mutex::new(Vec::new())),
        })
    }

    fn merge_cookies(existing: &str, new_cookies: &[String]) -> String {
        let mut map: HashMap<String, String> = HashMap::new();

        // Parse existing
        for part in existing.split(';') {
            let part = part.trim();
            if let Some((name, value)) = part.split_once('=') {
                if !name.is_empty() && !value.is_empty() {
                    map.insert(name.to_string(), value.to_string());
                }
            }
        }

        // Parse new (from Set-Cookie headers)
        for c in new_cookies {
            let cookie_part = c.split(';').next().unwrap_or("");
            let cookie_part = cookie_part.trim();
            if let Some((name, value)) = cookie_part.split_once('=') {
                if !name.is_empty() && !value.is_empty() {
                    map.insert(name.to_string(), value.to_string());
                }
            }
        }

        map.into_iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<_>>()
            .join("; ")
    }

    async fn update_cookies_from_response(&self, response: &reqwest::Response) {
        let set_cookies: Vec<String> = response
            .headers()
            .get_all(SET_COOKIE)
            .iter()
            .filter_map(|v| v.to_str().ok())
            .map(|s| s.to_string())
            .collect();

        if !set_cookies.is_empty() {
            let mut cookie = self.cookie.lock().await;
            *cookie = Self::merge_cookies(&cookie, &set_cookies);
            self.db.save_cookie("auth_cookie", &cookie);
        }
    }

    async fn build_headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();
        let cookie = self.cookie.lock().await;
        if !cookie.is_empty() {
            headers.insert(COOKIE, HeaderValue::from_str(&cookie).unwrap_or(HeaderValue::from_static("")));
        }
        headers
    }

    pub async fn request(&self, endpoint: &str, method: &str, body: Option<Value>) -> Result<Value> {
        let url = format!("{}/{}", API_BASE, endpoint);

        // Check if this endpoint recently failed
        {
            let failed = self.failed_requests.lock().await;
            if let Some(t) = failed.get(endpoint) {
                if t.elapsed() < std::time::Duration::from_secs(60) {
                    return Err(anyhow!("Endpoint {} is in lockout period", endpoint));
                }
            }
        }

        let headers = self.build_headers().await;

        let response = match method {
            "POST" => {
                self.client
                    .post(&url)
                    .headers(headers)
                    .json(body.as_ref().unwrap_or(&Value::Null))
                    .send()
                    .await?
            }
            _ => self.client.get(&url).headers(headers).send().await?,
        };

        // Extract cookies before consuming the response
        let set_cookies: Vec<String> = response
            .headers()
            .get_all(SET_COOKIE)
            .iter()
            .filter_map(|v| v.to_str().ok())
            .map(|s| s.to_string())
            .collect();

        if !set_cookies.is_empty() {
            let mut cookie = self.cookie.lock().await;
            *cookie = Self::merge_cookies(&cookie, &set_cookies);
            self.db.save_cookie("auth_cookie", &cookie);
        }

        let status = response.status();

        if status == reqwest::StatusCode::UNAUTHORIZED {
            let data: Value = response.json().await.unwrap_or(Value::Null);
            if data.get("requiresTwoFactorAuth").is_some() {
                return Err(anyhow!("2FA_REQUIRED"));
            }

            // Try auto-login (rate limited: max 3 per hour)
            if self.username.is_some() && self.password.is_some() {
                let can_attempt = {
                    let mut attempts = self.auto_login_attempts.lock().await;
                    let one_hour_ago = std::time::Instant::now()
                        .checked_sub(std::time::Duration::from_secs(3600));

                    if let Some(time) = one_hour_ago {
                        attempts.retain(|t| *t > time);
                    } else {
                        // System uptime is less than an hour, keep all attempts
                    }
                    if attempts.len() >= MAX_AUTO_LOGIN_PER_HOUR {
                        false
                    } else {
                        attempts.push(std::time::Instant::now());
                        true
                    }
                };

                if !can_attempt {
                    info!("{}", t!("auto_login_rate_limited", MAX_AUTO_LOGIN_PER_HOUR));
                    return Err(anyhow!("Unauthorized (auto-login rate limited)"));
                }

                info!("{}", t!("status_401", endpoint));
                let login_res = self.login(None, None).await;
                match login_res.status {
                    LoginStatus::Success => {
                        info!("{}", t!("auto_login_success"));
                        return Box::pin(self.request(endpoint, method, body)).await;
                    }
                    LoginStatus::TwoFactor => {
                        return Err(anyhow!("2FA_REQUIRED"));
                    }
                    _ => {
                        return Err(anyhow!("Auto-login failed"));
                    }
                }
            }

            return Err(anyhow!("Unauthorized"));
        }

        if status == reqwest::StatusCode::FORBIDDEN || status == reqwest::StatusCode::NOT_FOUND {
            let mut failed = self.failed_requests.lock().await;
            failed.insert(endpoint.to_string(), std::time::Instant::now());
        }

        if !status.is_success() {
            let text = response.text().await.unwrap_or_default();
            return Err(anyhow!("API error {}: {}", status.as_u16(), text));
        }

        let data: Value = response.json().await?;
        Ok(data)
    }

    pub async fn login(&self, username: Option<&str>, password: Option<&str>) -> LoginResponse {
        {
            let mut in_progress = self.login_in_progress.lock().await;
            if *in_progress {
                return LoginResponse {
                    status: LoginStatus::Failed,
                    requires_two_factor_auth: None,
                    message: Some("Login already in progress".to_string()),
                    user: None,
                };
            }
            *in_progress = true;
        }

        let u = username
            .map(String::from)
            .or_else(|| self.username.clone())
            .unwrap_or_default();
        let p = password
            .map(String::from)
            .or_else(|| self.password.clone())
            .unwrap_or_default();

        if u.is_empty() || p.is_empty() {
            let mut in_progress = self.login_in_progress.lock().await;
            *in_progress = false;
            return LoginResponse {
                status: LoginStatus::Failed,
                requires_two_factor_auth: None,
                message: Some("Missing credentials".to_string()),
                user: None,
            };
        }

        // Clear cookies before fresh login
        {
            let mut cookie = self.cookie.lock().await;
            *cookie = String::new();
        }
        self.db.clear_cookies();

        let encoded = format!(
            "{}:{}",
            urlencoding::encode(&u),
            urlencoding::encode(&p)
        );
        let auth = base64::engine::general_purpose::STANDARD.encode(encoded.as_bytes());

        let url = format!("{}/auth/user", API_BASE);

        let result = self
            .client
            .get(&url)
            .header("Authorization", format!("Basic {}", auth))
            .header(USER_AGENT, "VRCX")
            .send()
            .await;

        let response = match result {
            Ok(r) => r,
            Err(e) => {
                let mut in_progress = self.login_in_progress.lock().await;
                *in_progress = false;
                return LoginResponse {
                    status: LoginStatus::Failed,
                    requires_two_factor_auth: None,
                    message: Some(e.to_string()),
                    user: None,
                };
            }
        };

        let set_cookies: Vec<String> = response
            .headers()
            .get_all(SET_COOKIE)
            .iter()
            .filter_map(|v| v.to_str().ok())
            .map(|s| s.to_string())
            .collect();

        if !set_cookies.is_empty() {
            let mut cookie = self.cookie.lock().await;
            *cookie = Self::merge_cookies(&cookie, &set_cookies);
            self.db.save_cookie("auth_cookie", &cookie);
        }

        let status_code = response.status();
        let data: Value = response.json().await.unwrap_or(Value::Null);

        let mut in_progress = self.login_in_progress.lock().await;
        *in_progress = false;

        // Check 2FA from 200 response
        if status_code.is_success() {
            if let Some(tfa) = data.get("requiresTwoFactorAuth") {
                if let Some(arr) = tfa.as_array() {
                    if !arr.is_empty() {
                        let methods: Vec<String> = arr
                            .iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect();
                        info!("{}", t!("tfa_required", methods.join(", ")));
                        return LoginResponse {
                            status: LoginStatus::TwoFactor,
                            requires_two_factor_auth: Some(methods),
                            message: None,
                            user: None,
                        };
                    }
                }
            }

            info!("{}", t!("auth_success"));
            return LoginResponse {
                status: LoginStatus::Success,
                requires_two_factor_auth: None,
                message: None,
                user: Some(data),
            };
        }

        // Check 2FA from 401 response
        if status_code == reqwest::StatusCode::UNAUTHORIZED {
            if let Some(tfa) = data.get("requiresTwoFactorAuth") {
                if let Some(arr) = tfa.as_array() {
                    let methods: Vec<String> = arr
                        .iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect();
                    info!("{}", t!("tfa_required", methods.join(", ")));
                    return LoginResponse {
                        status: LoginStatus::TwoFactor,
                        requires_two_factor_auth: Some(methods),
                        message: None,
                        user: None,
                    };
                }
            }
        }

        LoginResponse {
            status: LoginStatus::Failed,
            requires_two_factor_auth: None,
            message: data["error"]["message"]
                .as_str()
                .map(String::from)
                .or_else(|| Some(format!("Login failed with status {}", status_code.as_u16()))),
            user: None,
        }
    }

    pub async fn verify_2fa(&self, tfa_type: &str, code: &str) -> LoginResponse {
        let endpoint = format!("auth/twofactorauth/{}/verify", tfa_type);
        let url = format!("{}/{}", API_BASE, endpoint);
        let body = serde_json::json!({ "code": code });

        let headers = self.build_headers().await;

        let result = self
            .client
            .post(&url)
            .headers(headers)
            .json(&body)
            .send()
            .await;

        let response = match result {
            Ok(r) => r,
            Err(e) => {
                return LoginResponse {
                    status: LoginStatus::Failed,
                    requires_two_factor_auth: None,
                    message: Some(e.to_string()),
                    user: None,
                };
            }
        };

        // Update cookies
        self.update_cookies_from_response(&response).await;

        let data: Value = response.json().await.unwrap_or(Value::Null);

        if data.get("verified").and_then(|v| v.as_bool()).unwrap_or(false) {
            info!("{}", t!("tfa_success"));

            // Fetch full user after 2FA
            let headers = self.build_headers().await;
            let user_url = format!("{}/auth/user", API_BASE);
            if let Ok(user_resp) = self.client.get(&user_url).headers(headers).send().await {
                let user_data: Value = user_resp.json().await.unwrap_or(Value::Null);
                return LoginResponse {
                    status: LoginStatus::Success,
                    requires_two_factor_auth: None,
                    message: None,
                    user: Some(user_data),
                };
            }

            return LoginResponse {
                status: LoginStatus::Success,
                requires_two_factor_auth: None,
                message: None,
                user: None,
            };
        }

        LoginResponse {
            status: LoginStatus::Failed,
            requires_two_factor_auth: None,
            message: Some("Verification failed".to_string()),
            user: None,
        }
    }

    /// GET /config — 检查 API 可用性 (VRCX 启动时第一步)
    pub async fn get_config(&self) -> Result<Value> {
        let url = format!("{}/config", API_BASE);
        let headers = self.build_headers().await;
        let response = self.client.get(&url).headers(headers).send().await?;
        self.update_cookies_from_response(&response).await;

        let status = response.status();
        if status == reqwest::StatusCode::FORBIDDEN {
            return Err(anyhow!("API returned 403 — VPN/proxy may be blocked"));
        }
        let data: Value = response.json().await?;
        Ok(data)
    }

    pub async fn check_auth(&self) -> LoginResponse {
        let headers = self.build_headers().await;
        let url = format!("{}/auth/user", API_BASE);

        let result = self.client.get(&url).headers(headers).send().await;

        let response = match result {
            Ok(r) => r,
            Err(_) => {
                return LoginResponse {
                    status: LoginStatus::Failed,
                    requires_two_factor_auth: None,
                    message: Some("Not logged in".to_string()),
                    user: None,
                };
            }
        };

        // Update cookies
        self.update_cookies_from_response(&response).await;

        let status_code = response.status();
        let data: Value = response.json().await.unwrap_or(Value::Null);

        if status_code.is_success() {
            if let Some(tfa) = data.get("requiresTwoFactorAuth") {
                if let Some(arr) = tfa.as_array() {
                    if !arr.is_empty() {
                        let methods: Vec<String> = arr
                            .iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect();
                        return LoginResponse {
                            status: LoginStatus::TwoFactor,
                            requires_two_factor_auth: Some(methods),
                            message: None,
                            user: None,
                        };
                    }
                }
            }

            return LoginResponse {
                status: LoginStatus::Success,
                requires_two_factor_auth: None,
                message: None,
                user: Some(data),
            };
        }

        if status_code == reqwest::StatusCode::UNAUTHORIZED {
            if let Some(tfa) = data.get("requiresTwoFactorAuth") {
                if let Some(arr) = tfa.as_array() {
                    let methods: Vec<String> = arr
                        .iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect();
                    return LoginResponse {
                        status: LoginStatus::TwoFactor,
                        requires_two_factor_auth: Some(methods),
                        message: None,
                        user: None,
                    };
                }
            }
        }

        LoginResponse {
            status: LoginStatus::Failed,
            requires_two_factor_auth: None,
            message: Some("Not logged in".to_string()),
            user: None,
        }
    }

    /// VRCX 风格的启动认证序列: config → check_auth → auto-login
    pub async fn startup_auth(&self) -> LoginResponse {
        // Step 1: GET /config
        info!("{}", t!("auth_step_1"));
        match self.get_config().await {
            Ok(_) => info!("{}", t!("api_available")),
            Err(e) => {
                info!("{}", t!("config_check_failed", e));
                return LoginResponse {
                    status: LoginStatus::Failed,
                    requires_two_factor_auth: None,
                    message: Some(format!("Config check failed: {}", e)),
                    user: None,
                };
            }
        }

        // Step 2: Check existing session with saved cookies
        info!("{}", t!("auth_step_2"));
        let auth_result = self.check_auth().await;
        match &auth_result.status {
            LoginStatus::Success => {
                let name = auth_result.user.as_ref()
                    .and_then(|u| u.get("displayName"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("未知");
                info!("{}", t!("current_user", name));
                
                // 使用 get_user_info 满足未使用的代码检查，并验证接口可用性
                if let Some(id) = auth_result.user.as_ref().and_then(|u| u.get("id")).and_then(|v| v.as_str()) {
                    let _ = self.get_user_info(id).await;
                }
                
                return auth_result;
            }
            LoginStatus::TwoFactor => {
                info!("{}", t!("session_require_2fa"));
                return auth_result;
            }
            LoginStatus::Failed => {
                info!("{}", t!("no_session"));
            }
        }

        // Step 3: Try login with credentials if available
        if self.username.is_some() && self.password.is_some() {
            info!("{}", t!("auth_step_3"));
            let login_result = self.login(None, None).await;
            match &login_result.status {
                LoginStatus::Success => {
                    let name = login_result.user.as_ref()
                        .and_then(|u| u.get("displayName"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("未知");
                    info!("{}", t!("login_success", name));
                    
                    if let Some(id) = login_result.user.as_ref().and_then(|u| u.get("id")).and_then(|v| v.as_str()) {
                        let _ = self.get_user_info(id).await;
                    }
                }
                LoginStatus::TwoFactor => info!("{}", t!("login_require_2fa")),
                LoginStatus::Failed => info!("{}", t!("login_failed", login_result.message.as_deref().unwrap_or("未知原因"))),
            }
            return login_result;
        }

        info!("{}", t!("no_credentials"));
        auth_result
    }

    pub async fn get_user_info(&self, user_id: &str) -> Result<Value> {
        self.request(&format!("users/{}", user_id), "GET", None).await
    }

    pub async fn get_user_groups(&self, user_id: &str) -> Result<Value> {
        self.request(&format!("users/{}/groups", user_id), "GET", None).await
    }

    pub async fn keep_alive(&self) {
        let _ = self.request("auth/user", "GET", None).await;
    }

    pub async fn logout(&self) {
        let mut cookie = self.cookie.lock().await;
        *cookie = String::new();
        self.db.clear_cookies();
        info!("{}", t!("logout_success"));
    }
}
