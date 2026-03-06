//! API 反向代理模块
//! 提供本地 HTTP 反向代理服务，将请求转发到上游 AI 服务 API
//! 参考 Antigravity-Manager 的 proxy 架构设计
//! Antigravity: Claude Messages → Google v1internal 协议转换

use axum::{
    body::Body,
    extract::{Path, State},
    http::{HeaderMap, Method, StatusCode},
    response::{IntoResponse, Response},
    routing::{any, get},
    Json, Router,
};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, OnceLock, RwLock};
use tokio::sync::watch;
use tower_http::cors::{Any, CorsLayer};

use super::config;
use super::logger;

// ============================================================================
// 配置类型
// ============================================================================

/// 默认 API 代理端口
pub const DEFAULT_PROXY_PORT: u16 = 19530;

/// 端口尝试范围
pub const PROXY_PORT_RANGE: u16 = 100;

/// API 代理配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiProxyConfig {
    /// 是否启用 API 代理服务
    #[serde(default)]
    pub enabled: bool,

    /// 代理监听端口
    #[serde(default = "default_proxy_port")]
    pub port: u16,

    /// 是否允许局域网访问
    #[serde(default)]
    pub allow_lan_access: bool,

    /// API 密钥（为空则不验证）
    #[serde(default)]
    pub api_key: String,

    /// 是否自动启动
    #[serde(default)]
    pub auto_start: bool,

    /// 请求超时时间(秒)
    #[serde(default = "default_request_timeout")]
    pub request_timeout: u64,

    /// 各 Provider 的代理设置
    #[serde(default)]
    pub providers: HashMap<String, ProviderProxyConfig>,

    /// 选定的账号邮箱（用于指定使用哪个账号的凭据，为空则使用所有可用账号）
    #[serde(default)]
    pub selected_account_email: String,

    /// Warp 会话池服务的 API 地址 (支持本地或远端代理地址)
    #[serde(default = "default_warp_api_url")]
    pub warp_api_url: String,
}

/// 单个 Provider 的代理配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderProxyConfig {
    /// 是否启用此 Provider 的代理
    #[serde(default)]
    pub enabled: bool,

    /// 负载均衡策略: "round_robin" | "random" | "single"
    #[serde(default = "default_strategy")]
    pub strategy: String,

    /// 参与轮询的账号 ID 列表（空 = 全部）
    #[serde(default)]
    pub account_ids: Vec<String>,
}

fn default_proxy_port() -> u16 {
    DEFAULT_PROXY_PORT
}

fn default_request_timeout() -> u64 {
    120
}

fn default_strategy() -> String {
    "round_robin".to_string()
}

fn default_warp_api_url() -> String {
    "http://127.0.0.1:8010".to_string()
}

impl Default for ApiProxyConfig {
    fn default() -> Self {
        let mut providers = HashMap::new();
        providers.insert(
            "antigravity".to_string(),
            ProviderProxyConfig {
                enabled: false,
                strategy: "round_robin".to_string(),
                account_ids: Vec::new(),
            },
        );
        providers.insert(
            "codex".to_string(),
            ProviderProxyConfig {
                enabled: false,
                strategy: "round_robin".to_string(),
                account_ids: Vec::new(),
            },
        );
        providers.insert(
            "kiro".to_string(),
            ProviderProxyConfig {
                enabled: false,
                strategy: "round_robin".to_string(),
                account_ids: Vec::new(),
            },
        );
        providers.insert(
            "windsurf".to_string(),
            ProviderProxyConfig {
                enabled: false,
                strategy: "round_robin".to_string(),
                account_ids: Vec::new(),
            },
        );
        providers.insert(
            "warp".to_string(),
            ProviderProxyConfig {
                enabled: false,
                strategy: "round_robin".to_string(),
                account_ids: Vec::new(),
            },
        );

        Self {
            enabled: false,
            port: DEFAULT_PROXY_PORT,
            allow_lan_access: false,
            api_key: String::new(),
            auto_start: false,
            request_timeout: default_request_timeout(),
            providers,
            selected_account_email: String::new(),
            warp_api_url: default_warp_api_url(),
        }
    }
}

impl Default for ProviderProxyConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            strategy: "round_robin".to_string(),
            account_ids: Vec::new(),
        }
    }
}

// ============================================================================
// Provider 定义
// ============================================================================

/// Provider 上游信息
struct ProviderUpstream {
    /// 上游 base URL
    base_url: &'static str,
    /// 认证头名称
    auth_header: &'static str,
    /// 认证头前缀
    auth_prefix: &'static str,
}



fn get_provider_upstream(provider: &str) -> Option<ProviderUpstream> {
    match provider {
        "antigravity" => Some(ProviderUpstream {
            // 实际会用 ANTIGRAVITY_ENDPOINTS 多端点降级
            base_url: "https://daily-cloudcode-pa.sandbox.googleapis.com/v1internal",
            auth_header: "Authorization",
            auth_prefix: "Bearer ",
        }),
        "codex" => Some(ProviderUpstream {
            base_url: "https://chatgpt.com/backend-api/codex",
            auth_header: "Authorization",
            auth_prefix: "Bearer ",
        }),
        "kiro" => Some(ProviderUpstream {
            base_url: "https://q.us-east-1.amazonaws.com",
            auth_header: "Authorization",
            auth_prefix: "Bearer ",
        }),
        "windsurf" => Some(ProviderUpstream {
            base_url: "https://api.githubcopilot.com",
            auth_header: "Authorization",
            auth_prefix: "Bearer ",
        }),
        "warp" => Some(ProviderUpstream {
            base_url: "https://app.warp.dev",
            auth_header: "Authorization",
            auth_prefix: "Bearer ",
        }),
        _ => None,
    }
}

// ============================================================================
// 代理服务状态
// ============================================================================

/// 代理服务运行状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyStatus {
    pub running: bool,
    pub port: u16,
    pub actual_port: Option<u16>,
    pub enabled_providers: Vec<String>,
}

/// 账号凭据（包含 token + project_id + 域名选择标记）
#[derive(Debug, Clone)]
struct AccountCredential {
    id: String,
    access_token: String,
    project_id: String,
    /// GCP ToS 账号走 prod 端点
    is_gcp_tos: bool,
    /// Kiro profileArn（用于 Kiro API 鉴权）
    profile_arn: Option<String>,
}

/// 代理服务器内部状态
struct ProxyServerState {
    config: RwLock<ApiProxyConfig>,
    http_client: Client,
    /// 各 Provider 的轮询计数器
    round_robin_counters: RwLock<HashMap<String, AtomicUsize>>,
}

impl ProxyServerState {
    fn new(config: ApiProxyConfig) -> Self {
        let timeout = config.request_timeout;
        Self {
            config: RwLock::new(config),
            http_client: Client::builder()
                .timeout(std::time::Duration::from_secs(timeout))
                .build()
                .unwrap_or_default(),
            round_robin_counters: RwLock::new(HashMap::new()),
        }
    }

    /// 获取下一个要使用的账号凭据
    async fn get_next_credential(&self, provider: &str) -> Result<AccountCredential, String> {
        let (enabled, strategy) = {
            let config = self.config.read().map_err(|e| format!("锁读取失败: {}", e))?;
            let provider_config = config
                .providers
                .get(provider)
                .ok_or_else(|| format!("Provider '{}' 未配置", provider))?;
            (provider_config.enabled, provider_config.strategy.clone())
        };

        if !enabled {
            return Err(format!("Provider '{}' 未启用", provider));
        }

        let creds = self.get_available_credentials(provider).await?;

        if creds.is_empty() {
            return Err(format!("Provider '{}' 没有可用的账号", provider));
        }

        // 根据策略选择账号
        let idx = match strategy.as_str() {
            "random" => rand::random::<usize>() % creds.len(),
            "single" => 0,
            _ => {
                // round_robin (默认)
                let mut counters = self
                    .round_robin_counters
                    .write()
                    .map_err(|e| format!("锁写入失败: {}", e))?;
                let counter = counters
                    .entry(provider.to_string())
                    .or_insert_with(|| AtomicUsize::new(0));
                counter.fetch_add(1, Ordering::Relaxed) % creds.len()
            }
        };

        Ok(creds[idx].clone())
    }

    /// 获取 Provider 的可用凭据列表
    async fn get_available_credentials(&self, provider: &str) -> Result<Vec<AccountCredential>, String> {
        match provider {
            "antigravity" => {
                let accounts = super::account::list_accounts()
                    .map_err(|e| format!("获取账号失败: {}", e))?;
                let selected_email = {
                    let config = self.config.read().map_err(|e| format!("锁读取失败: {}", e))?;
                    config.selected_account_email.clone()
                };
                let creds: Vec<AccountCredential> = accounts
                    .iter()
                    .filter(|a| !a.disabled && !a.token.access_token.is_empty())
                    .filter(|a| {
                        // 如果指定了账号邮箱，只使用该账号
                        if selected_email.is_empty() {
                            true
                        } else {
                            a.email == selected_email
                        }
                    })
                    .map(|a| AccountCredential {
                        id: a.id.clone(),
                        access_token: a.token.access_token.clone(),
                        project_id: a.token.project_id.clone().unwrap_or_default(),
                        is_gcp_tos: a.token.is_gcp_tos.unwrap_or(false),
                        profile_arn: None,
                    })
                    .collect();
                if creds.is_empty() && !selected_email.is_empty() {
                    return Err(format!("指定账号 {} 不可用（可能已禁用或 token 为空）", selected_email));
                }
                Ok(creds)
            }
            "codex" => {
                let accounts = super::codex_account::list_accounts();
                let creds: Vec<AccountCredential> = accounts
                    .iter()
                    .filter(|a| !a.tokens.access_token.is_empty())
                    .map(|a| AccountCredential {
                        id: a.id.clone(),
                        access_token: a.tokens.access_token.clone(),
                        project_id: String::new(),
                        is_gcp_tos: false,
                        profile_arn: None,
                    })
                    .collect();
                Ok(creds)
            }
            "kiro" => {
                let mut accounts = super::kiro_account::list_accounts();
                let now = chrono::Utc::now().timestamp();
                for account in accounts.iter_mut() {
                    let expires = account.expires_at.unwrap_or(now + 3600);
                    if expires < now + 600 {
                        if let Ok(refreshed) = super::kiro_account::refresh_account_token(&account.id).await {
                            *account = refreshed;
                        } else {
                            logger::log_warn(&format!("[ApiProxy] Kiro 账号 {} 刷新失败", account.email));
                        }
                    }
                }
                
                let creds: Vec<AccountCredential> = accounts
                    .iter()
                    .filter(|a| !a.access_token.is_empty())
                    .map(|a| {
                        // 从 kiro_auth_token_raw 或 kiro_profile_raw 提取 profileArn
                        let profile_arn = a.kiro_auth_token_raw.as_ref()
                            .and_then(|v| v.get("profileArn").and_then(|p| p.as_str()).map(|s| s.to_string()))
                            .or_else(|| a.kiro_profile_raw.as_ref()
                                .and_then(|v| v.get("profileArn").and_then(|p| p.as_str()).map(|s| s.to_string())));
                        AccountCredential {
                            id: a.id.clone(),
                            access_token: a.access_token.clone(),
                            project_id: a.idc_region.clone().unwrap_or_else(|| "us-east-1".to_string()),
                            is_gcp_tos: false,
                            profile_arn,
                        }
                    })
                    .collect();
                Ok(creds)
            }
            "windsurf" => {
                let mut accounts = super::windsurf_account::list_accounts();
                let now = chrono::Utc::now().timestamp();
                for account in accounts.iter_mut() {
                    let expires = account.copilot_expires_at.unwrap_or(now + 3600);
                    if expires < now + 600 {
                        if let Ok(refreshed) = super::windsurf_account::refresh_account_token(&account.id).await {
                            *account = refreshed;
                        } else {
                            logger::log_warn(&format!("[ApiProxy] Windsurf 账号 {} 刷新失败", account.github_login));
                        }
                    }
                }
                
                let creds: Vec<AccountCredential> = accounts
                    .iter()
                    .filter(|a| !a.copilot_token.is_empty())
                    .map(|a| AccountCredential {
                        id: a.id.clone(),
                        access_token: a.copilot_token.clone(),
                        project_id: String::new(),
                        is_gcp_tos: false,
                        profile_arn: None,
                    })
                    .collect();
                Ok(creds)
            }
            "warp" => {
                let accounts = super::warp_account::list_accounts();
                let creds: Vec<AccountCredential> = accounts
                    .iter()
                    .filter(|a| !a.auth_token.is_empty())
                    .map(|a| AccountCredential {
                        id: a.id.clone(),
                        access_token: a.auth_token.clone(),
                        project_id: String::new(),
                        is_gcp_tos: false,
                        profile_arn: None,
                    })
                    .collect();
                Ok(creds)
            }
            _ => Err(format!("不支持的 Provider: {}", provider)),
        }
    }
}

type SharedState = Arc<ProxyServerState>;

// ============================================================================
// 全局代理服务实例
// ============================================================================

static PROXY_RUNNING: OnceLock<AtomicBool> = OnceLock::new();
static PROXY_ACTUAL_PORT: OnceLock<RwLock<Option<u16>>> = OnceLock::new();
static PROXY_SHUTDOWN_TX: OnceLock<RwLock<Option<watch::Sender<()>>>> = OnceLock::new();
static PROXY_STATE: OnceLock<RwLock<Option<SharedState>>> = OnceLock::new();

fn is_proxy_running() -> bool {
    PROXY_RUNNING
        .get_or_init(|| AtomicBool::new(false))
        .load(Ordering::Relaxed)
}

fn set_proxy_running(running: bool) {
    PROXY_RUNNING
        .get_or_init(|| AtomicBool::new(false))
        .store(running, Ordering::Relaxed);
}

fn get_proxy_actual_port() -> Option<u16> {
    PROXY_ACTUAL_PORT
        .get_or_init(|| RwLock::new(None))
        .read()
        .ok()
        .and_then(|p| *p)
}

fn set_proxy_actual_port(port: Option<u16>) {
    if let Some(lock) = PROXY_ACTUAL_PORT.get() {
        if let Ok(mut p) = lock.write() {
            *p = port;
        }
    } else {
        let _ = PROXY_ACTUAL_PORT.set(RwLock::new(port));
    }
}

// ============================================================================
// 配置持久化
// ============================================================================

const PROXY_CONFIG_FILE: &str = "api_proxy_config.json";

/// 加载代理配置
pub fn load_proxy_config() -> ApiProxyConfig {
    let data_dir = match config::get_data_dir() {
        Ok(d) => d,
        Err(_) => return ApiProxyConfig::default(),
    };
    let config_path = data_dir.join(PROXY_CONFIG_FILE);
    if !config_path.exists() {
        return ApiProxyConfig::default();
    }
    match std::fs::read_to_string(&config_path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(_) => ApiProxyConfig::default(),
    }
}

/// 保存代理配置
pub fn save_proxy_config(proxy_config: &ApiProxyConfig) -> Result<(), String> {
    let data_dir = config::get_data_dir()?;
    if !data_dir.exists() {
        std::fs::create_dir_all(&data_dir).map_err(|e| format!("创建目录失败: {}", e))?;
    }
    let config_path = data_dir.join(PROXY_CONFIG_FILE);
    let json = serde_json::to_string_pretty(proxy_config)
        .map_err(|e| format!("序列化配置失败: {}", e))?;
    std::fs::write(&config_path, json).map_err(|e| format!("写入配置失败: {}", e))?;

    // 同步更新运行中代理的内存配置
    if let Some(lock) = PROXY_STATE.get() {
        if let Ok(guard) = lock.read() {
            if let Some(state) = guard.as_ref() {
                if let Ok(mut cfg) = state.config.write() {
                    *cfg = proxy_config.clone();
                    logger::log_info(&format!(
                        "[ApiProxy] 运行中配置已同步: selected_account={}",
                        proxy_config.selected_account_email
                    ));
                }
            }
        }
    }

    logger::log_info(&format!(
        "[ApiProxy] 配置已保存: enabled={}, port={}",
        proxy_config.enabled, proxy_config.port
    ));
    Ok(())
}

// ============================================================================
// HTTP 路由和处理器
// ============================================================================

/// 健康检查
async fn health_check() -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "ok",
        "service": "cockpit-tools-api-proxy",
        "version": env!("CARGO_PKG_VERSION")
    }))
}

// ============================================================================
// Claude → Google v1internal 协议转换
// ============================================================================

/// 将 Claude Messages 格式的请求体转换为 Google v1internal generateContent 格式
fn convert_claude_to_google(body: &Value, project_id: &str) -> Result<(Value, bool), String> {
    let messages = body.get("messages")
        .and_then(|v| v.as_array())
        .ok_or("请求体缺少 messages 字段")?;

    let model = body.get("model")
        .and_then(|v| v.as_str())
        .unwrap_or("claude-sonnet-4-20250514");

    let stream = body.get("stream")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    // 转换 messages → contents
    let mut contents: Vec<Value> = Vec::new();
    for msg in messages {
        let role = msg.get("role").and_then(|v| v.as_str()).unwrap_or("user");
        // Google API: user → user, assistant → model
        let gemini_role = if role == "assistant" { "model" } else { "user" };

        let parts = match msg.get("content") {
            Some(Value::String(s)) => {
                serde_json::json!([{"text": s}])
            }
            Some(Value::Array(arr)) => {
                let mut parts_vec: Vec<Value> = Vec::new();
                for block in arr {
                    match block.get("type").and_then(|v| v.as_str()) {
                        Some("text") => {
                            if let Some(text) = block.get("text").and_then(|v| v.as_str()) {
                                if !text.trim().is_empty() {
                                    parts_vec.push(serde_json::json!({"text": text}));
                                }
                            }
                        }
                        Some("image") => {
                            if let Some(source) = block.get("source") {
                                if let (Some(media_type), Some(data)) = (
                                    source.get("media_type").and_then(|v| v.as_str()),
                                    source.get("data").and_then(|v| v.as_str())
                                ) {
                                    parts_vec.push(serde_json::json!({
                                        "inlineData": {
                                            "mimeType": media_type,
                                            "data": data
                                        }
                                    }));
                                }
                            }
                        }
                        Some("thinking") => {
                            // 透传 thinking 块
                            let thinking = block.get("thinking")
                                .and_then(|v| v.as_str())
                                .unwrap_or("");
                            let mut part = serde_json::json!({
                                "text": thinking,
                                "thought": true
                            });
                            if let Some(sig) = block.get("signature").and_then(|v| v.as_str()) {
                                part["thoughtSignature"] = Value::String(sig.to_string());
                            }
                            parts_vec.push(part);
                        }
                        Some("tool_use") => {
                            let fc = serde_json::json!({
                                "functionCall": {
                                    "id": block.get("id").unwrap_or(&Value::Null),
                                    "name": block.get("name").unwrap_or(&Value::Null),
                                    "args": block.get("input").unwrap_or(&serde_json::json!({}))
                                }
                            });
                            parts_vec.push(fc);
                        }
                        Some("tool_result") => {
                            let output = block.get("content")
                                .map(|c| match c {
                                    Value::String(s) => s.clone(),
                                    Value::Array(arr) => arr.iter()
                                        .filter_map(|b| b.get("text").and_then(|v| v.as_str()))
                                        .collect::<Vec<_>>()
                                        .join("\n"),
                                    _ => c.to_string(),
                                })
                                .unwrap_or_default();
                            let fr = serde_json::json!({
                                "functionResponse": {
                                    "id": block.get("tool_use_id").unwrap_or(&Value::Null),
                                    "name": "tool_result",
                                    "response": {"output": output}
                                }
                            });
                            parts_vec.push(fr);
                        }
                        _ => {
                            // 其他类型转为 text
                            let text = serde_json::to_string(block).unwrap_or_default();
                            if !text.is_empty() {
                                parts_vec.push(serde_json::json!({"text": text}));
                            }
                        }
                    }
                }
                Value::Array(parts_vec)
            }
            _ => continue,
        };

        contents.push(serde_json::json!({
            "role": gemini_role,
            "parts": parts
        }));
    }

    // 构建 generationConfig
    let mut gen_config = serde_json::json!({
        "candidateCount": 1
    });
    if let Some(max_tokens) = body.get("max_tokens").and_then(|v| v.as_u64()) {
        gen_config["maxOutputTokens"] = Value::from(max_tokens);
    }
    if let Some(temp) = body.get("temperature").and_then(|v| v.as_f64()) {
        gen_config["temperature"] = Value::from(temp);
    } else {
        gen_config["temperature"] = Value::from(0.4);
    }
    if let Some(top_p) = body.get("top_p").and_then(|v| v.as_f64()) {
        gen_config["topP"] = Value::from(top_p);
    }

    // 处理 thinking
    if let Some(thinking) = body.get("thinking") {
        if let Some("enabled") = thinking.get("type").and_then(|v| v.as_str()) {
            let budget = thinking.get("budget_tokens")
                .and_then(|v| v.as_u64())
                .unwrap_or(48000);
            gen_config["thinkingConfig"] = serde_json::json!({
                "thinkingBudget": budget,
                "includeThoughts": true
            });
        }
    }

    // 构建内部请求体
    let mut request_body = serde_json::json!({
        "contents": contents,
        "generationConfig": gen_config,
        "safetySettings": [
            { "category": "HARM_CATEGORY_HARASSMENT", "threshold": "OFF" },
            { "category": "HARM_CATEGORY_HATE_SPEECH", "threshold": "OFF" },
            { "category": "HARM_CATEGORY_SEXUALLY_EXPLICIT", "threshold": "OFF" },
            { "category": "HARM_CATEGORY_DANGEROUS_CONTENT", "threshold": "OFF" },
            { "category": "HARM_CATEGORY_CIVIC_INTEGRITY", "threshold": "OFF" }
        ]
    });

    // 处理 system prompt
    if let Some(system) = body.get("system") {
        let sys_text = match system {
            Value::String(s) => s.clone(),
            Value::Array(arr) => arr.iter()
                .filter_map(|b| b.get("text").and_then(|v| v.as_str()))
                .collect::<Vec<_>>()
                .join("\n"),
            _ => system.to_string(),
        };
        if !sys_text.is_empty() {
            request_body["systemInstruction"] = serde_json::json!({
                "parts": [{"text": sys_text}]
            });
        }
    }

    // 处理 tools
    if let Some(tools) = body.get("tools").and_then(|v| v.as_array()) {
        let gemini_tools: Vec<Value> = tools.iter().map(|tool| {
            serde_json::json!({
                "functionDeclarations": [{
                    "name": tool.get("name").unwrap_or(&Value::Null),
                    "description": tool.get("description").unwrap_or(&Value::Null),
                    "parameters": tool.get("input_schema").unwrap_or(&serde_json::json!({}))
                }]
            })
        }).collect();
        request_body["tools"] = Value::Array(gemini_tools);
    }

    // 组装最终 payload — 对齐 gcli2api 已验证可用的格式
    // gcli2api 使用简单结构: {model, project, request: inner_body}
    let final_payload = serde_json::json!({
        "model": model,
        "project": project_id,
        "request": request_body
    });

    Ok((final_payload, stream))
}

/// 将 Google SSE 事件转换为 Claude SSE 事件
fn convert_google_sse_to_claude(data: &str, _model: &str, _msg_id: &str) -> Vec<String> {
    let mut events = Vec::new();

    logger::log_info(&format!("[ApiProxy DEBUG] Google SSE Data: {}", data));

    // 解析 Google 的 SSE data 行
    let json: Value = match serde_json::from_str(data) {
        Ok(v) => v,
        Err(e) => {
            logger::log_info(&format!("[ApiProxy DEBUG] Parse fail: {}", e));
            return events;
        }
    };

    // 提取 response.candidates[0].content.parts
    let candidates = json.get("response").and_then(|r| r.get("candidates")).or_else(|| json.get("candidates"));
    let parts = match candidates
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("content"))
        .and_then(|c| c.get("parts"))
        .and_then(|p| p.as_array())
    {
        Some(p) => p,
        None => return events,
    };

    for part in parts {
        let is_thought = part.get("thought").and_then(|v| v.as_bool()).unwrap_or(false);
        let text = part.get("text").and_then(|v| v.as_str());
        let thought_signature = part.get("thoughtSignature").and_then(|v| v.as_str());

        // 如果是 thought = true 或者包含 thoughtSignature，则作为 thinking 块发送
        if is_thought || thought_signature.is_some() {
            if let Some(content) = text.or(thought_signature) {
                let event = serde_json::json!({
                    "type": "content_block_delta",
                    "index": 0,
                    "delta": {
                        "type": "thinking_delta",
                        "thinking": content
                    }
                });
                events.push(format!("data: {}\n\n", event));
            }
        } else if let Some(content) = text {
            // 普通 text delta
            let event = serde_json::json!({
                "type": "content_block_delta",
                "index": 0,
                "delta": {
                    "type": "text_delta",
                    "text": content
                }
            });
            events.push(format!("data: {}\n\n", event));
        }
    }

    // 检查是否结束
    let finish_reason = json.get("candidates")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("finishReason"))
        .and_then(|v| v.as_str());

    if let Some(reason) = finish_reason {
        if reason == "STOP" || reason == "MAX_TOKENS" {
            // 提取 usage
            let usage = json.get("usageMetadata");
            let input_tokens = usage
                .and_then(|u| u.get("promptTokenCount"))
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            let output_tokens = usage
                .and_then(|u| u.get("candidatesTokenCount"))
                .and_then(|v| v.as_u64())
                .unwrap_or(0);

            let stop_reason = if reason == "MAX_TOKENS" { "max_tokens" } else { "end_turn" };

            let end_event = serde_json::json!({
                "type": "message_delta",
                "delta": {
                    "stop_reason": stop_reason,
                    "stop_sequence": null
                },
                "usage": {
                    "input_tokens": input_tokens,
                    "output_tokens": output_tokens
                }
            });
            events.push(format!("data: {}\n\n", end_event));
        }
    }

    events
}

/// 通过 loadCodeAssist API 获取 project_id
/// 参考: Antigravity-Manager/src-tauri/src/proxy/project_resolver.rs
async fn fetch_project_id_via_load_code_assist(
    client: &Client,
    access_token: &str,
) -> Result<String, String> {
    let url = "https://daily-cloudcode-pa.sandbox.googleapis.com/v1internal:loadCodeAssist";
    let body = serde_json::json!({
        "metadata": {
            "ideType": "ANTIGRAVITY"
        }
    });

    let resp = client
        .post(url)
        .header("Authorization", format!("Bearer {}", access_token))
        .header("Content-Type", "application/json")
        .header("User-Agent", "antigravity/2.15.8 (Windows; AMD64)")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("loadCodeAssist 请求失败: {}", e))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body_text = resp.text().await.unwrap_or_default();
        return Err(format!("loadCodeAssist 返回 {}: {}", status, &body_text[..body_text.len().min(200)]));
    }

    let data: Value = resp.json()
        .await
        .map_err(|e| format!("解析 loadCodeAssist 响应失败: {}", e))?;

    if let Some(project_id) = data.get("cloudaicompanionProject").and_then(|v| v.as_str()) {
        Ok(project_id.to_string())
    } else {
        Err("账号无资格获取 cloudaicompanionProject".to_string())
    }
}



#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuotaModelInfo {
    pub id: String,
    pub remaining_fraction: Option<f64>,
    pub reset_time: Option<String>,
}

/// 检查是否是有效的模型名称（过滤 fetchAvailableModels 返回的非模型 key）
/// 参考 sub2api/useModelWhitelist.ts antigravityModels 列表
fn is_valid_model_name(name: &str) -> bool {
    const VALID_PREFIXES: &[&str] = &[
        "gemini-",
        "claude-",
        "gpt-",
        "o1", "o3", "o4",
        "tab_",
    ];
    VALID_PREFIXES.iter().any(|prefix| name.starts_with(prefix))
}

/// 通过 fetchAvailableModels API 获取账号可用的模型列表及配额状态
/// 参考: Antigravity-Manager/src-tauri/src/modules/quota.rs
/// 参考: gcli2api/src/api/antigravity.py -> fetch_available_models
pub async fn fetch_available_models(access_token: &str, project_id: &str) -> Result<Vec<QuotaModelInfo>, String> {
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .unwrap_or_default();

    let url = "https://cloudcode-pa.googleapis.com/v1internal:fetchAvailableModels";
    let body = if project_id.is_empty() {
        serde_json::json!({})
    } else {
        serde_json::json!({ "project": project_id })
    };

    let resp = client
        .post(url)
        .header("Authorization", format!("Bearer {}", access_token))
        .header("Content-Type", "application/json")
        .header("User-Agent", "antigravity/2.15.8 (Windows; AMD64)")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("fetchAvailableModels 请求失败: {}", e))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body_text = resp.text().await.unwrap_or_default();
        return Err(format!("fetchAvailableModels 返回 {}: {}", status, &body_text[..body_text.len().min(300)]));
    }

    let data: Value = resp.json()
        .await
        .map_err(|e| format!("解析 fetchAvailableModels 响应失败: {}", e))?;

    let mut models = Vec::new();
    if let Some(models_map) = data.get("models").and_then(|v| v.as_object()) {
        for (model_id, info) in models_map {
            // 过滤非模型名称的 key（如 chat_20706 等是 project/session ID，不是真实模型）
            // 参考 sub2api/useModelWhitelist.ts antigravityModels 列表
            if !is_valid_model_name(model_id) {
                continue;
            }

            let mut remaining_fraction = None;
            let mut reset_time = None;

            if let Some(quota_info) = info.get("quotaInfo").and_then(|v| v.as_object()) {
                remaining_fraction = quota_info.get("remainingFraction").and_then(|v| v.as_f64());
                reset_time = quota_info.get("resetTime").and_then(|v| v.as_str()).map(|s| s.to_string());
            }

            models.push(QuotaModelInfo {
                id: model_id.clone(),
                remaining_fraction,
                reset_time,
            });
        }
    }

    logger::log_info(&format!(
        "[ApiProxy] fetchAvailableModels 返回 {} 个模型信息",
        models.len()
    ));

    Ok(models)
}

/// 解析 Kiro 账号的 profileArn 中的 region
fn parse_profile_arn_region(profile_arn: &str) -> Option<String> {
    let mut segments = profile_arn.split(':');
    let prefix = segments.next()?.trim();
    if !prefix.eq_ignore_ascii_case("arn") {
        return None;
    }
    let _partition = segments.next()?;
    let _service = segments.next()?;
    let region = segments.next()?.trim();
    if region.is_empty() {
        None
    } else {
        Some(region.to_string())
    }
}

/// 根据 region 获取 Kiro 请求基地址
fn kiro_runtime_endpoint_for_region(region: Option<&str>) -> String {
    let region = region.unwrap_or("us-east-1").trim().to_ascii_lowercase();
    match region.as_str() {
        "us-east-1" => "https://q.us-east-1.amazonaws.com".to_string(),
        "eu-central-1" => "https://q.eu-central-1.amazonaws.com".to_string(),
        "us-gov-east-1" => "https://q-fips.us-gov-east-1.amazonaws.com".to_string(),
        "us-gov-west-1" => "https://q-fips.us-gov-west-1.amazonaws.com".to_string(),
        "us-iso-east-1" => "https://q.us-iso-east-1.c2s.ic.gov".to_string(),
        "us-isob-east-1" => "https://q.us-isob-east-1.sc2s.sgov.gov".to_string(),
        "us-isof-south-1" => "https://q.us-isof-south-1.csp.hci.ic.gov".to_string(),
        "us-isof-east-1" => "https://q.us-isof-east-1.csp.hci.ic.gov".to_string(),
        _ => "https://q.us-east-1.amazonaws.com".to_string(),
    }
}

/// 请求 Kiro 取可用模型列表 (ListAvailableModels)
pub async fn fetch_kiro_models(
    access_token: &str,
    profile_arn: Option<&str>,
) -> Result<Vec<String>, String> {
    let region = profile_arn.and_then(parse_profile_arn_region);
    let endpoint = kiro_runtime_endpoint_for_region(region.as_deref());
    let mut url = format!("{}/ListAvailableModels?origin=AI_EDITOR", endpoint);
    
    // 如果有 profileArn 并且看起来像 KIRO Desktop 账号（简单判断有没有 arn 格式）
    if let Some(arn) = profile_arn {
        // AWS SSO OIDC (Builder ID) 用户不推荐传 profileArn
        if arn.starts_with("arn:") {
            url.push_str(&format!("&profileArn={}", urlencoding::encode(arn)));
        }
    }

    let client = Client::new();
    
    let started_at = tokio::time::Instant::now();
    let response = client
        .get(&url)
        .header("Authorization", format!("Bearer {}", access_token))
        .header("Content-Type", "application/json")
        .header("User-Agent", "aws-sdk-js/1.0.27 ua/2.1 os/win32#10.0.19044 lang/js md/nodejs#22.21.1 api/codewhispererstreaming#1.0.27 m/E KiroIDE-0.7.45-fetch")
        .header("x-amz-user-agent", "aws-sdk-js/1.0.27 KiroIDE-0.7.45-fetch")
        .header("x-amzn-codewhisperer-optout", "true")
        .header("x-amzn-kiro-agent-mode", "vibe")
        .timeout(std::time::Duration::from_secs(10)) // 稍微短一点的超时
        .send()
        .await
        .map_err(|e| format!("请求 Kiro 失败: {}", e))?;

    let status = response.status();
    let body = response
        .text()
        .await
        .unwrap_or_else(|_| "<no-body>".to_string());

    if !status.is_success() {
        return Err(format!(
            "Kiro 返回异常: status={}, body={}",
            status.as_u16(),
            body
        ));
    }

    let parsed: serde_json::Value = serde_json::from_str(&body)
        .map_err(|e| format!("解析 JSON 失败: {} (body: {})", e, body))?;

    let mut model_ids = Vec::new();
    if let Some(models_array) = parsed.get("models").and_then(|m| m.as_array()) {
        for m in models_array {
            if let Some(id) = m.get("modelId").and_then(|id| id.as_str()) {
                model_ids.push(id.to_string());
            }
        }
    }

    logger::log_info(&format!(
        "[ApiProxy] Kiro /ListAvailableModels 耗时 {}ms，返回 {} 个模型",
        started_at.elapsed().as_millis(),
        model_ids.len()
    ));

    // 合并隐藏模型（API 不返回但实际可用）
    for hidden in KIRO_HIDDEN_MODELS {
        if !model_ids.iter().any(|m| m == hidden) {
            model_ids.push(hidden.to_string());
        }
    }

    // 如果 API 返回空，使用 fallback
    if model_ids.is_empty() {
        logger::log_warn("[ApiProxy] Kiro 模型列表为空，使用 fallback");
        model_ids = KIRO_FALLBACK_MODELS.iter().map(|s| s.to_string()).collect();
    }

    Ok(model_ids)
}

// ============================================================================
// Kiro 消息转换辅助函数（参考 kiro-gateway converters_core.py）
// ============================================================================

/// Kiro 隐藏模型列表（API 不返回但实际可用）
const KIRO_HIDDEN_MODELS: &[&str] = &["claude-3.7-sonnet"];

/// Kiro Fallback 模型列表（API 不可达时的兜底）
const KIRO_FALLBACK_MODELS: &[&str] = &[
    "auto",
    "claude-sonnet-4",
    "claude-haiku-4.5",
    "claude-sonnet-4.5",
    "claude-opus-4.5",
];

/// 工具描述最大长度（超过此值移入 system prompt）
const TOOL_DESCRIPTION_MAX_LENGTH: usize = 10000;

/// 工具名最大长度（Kiro API 限制）
const TOOL_NAME_MAX_LENGTH: usize = 64;

/// Fake Reasoning 默认最大 thinking token 数
const FAKE_REASONING_MAX_TOKENS: usize = 4000;

/// 从消息内容中提取纯文本
fn kiro_extract_text(content: &Value) -> String {
    match content {
        Value::String(s) => s.clone(),
        Value::Array(arr) => {
            arr.iter()
                .filter_map(|item| {
                    let item_type = item.get("type").and_then(|v| v.as_str()).unwrap_or("");
                    if item_type == "image" || item_type == "image_url" {
                        return None;
                    }
                    item.get("text").and_then(|v| v.as_str()).map(|s| s.to_string())
                })
                .collect::<Vec<_>>()
                .join("")
        }
        Value::Null => String::new(),
        _ => content.to_string(),
    }
}

/// 从消息内容中提取图片（支持 OpenAI 和 Anthropic 格式）
fn kiro_extract_images(content: &Value) -> Vec<Value> {
    let arr = match content.as_array() {
        Some(a) => a,
        None => return vec![],
    };
    let mut images = Vec::new();
    for item in arr {
        let item_type = item.get("type").and_then(|v| v.as_str()).unwrap_or("");
        // OpenAI: {"type": "image_url", "image_url": {"url": "data:image/jpeg;base64,..."}}
        if item_type == "image_url" {
            if let Some(url) = item.get("image_url")
                .and_then(|iu| iu.get("url"))
                .and_then(|u| u.as_str())
            {
                if url.starts_with("data:") {
                    if let Some((header, data)) = url.split_once(',') {
                        let media_type = header.split(';').next()
                            .unwrap_or("data:image/jpeg")
                            .strip_prefix("data:")
                            .unwrap_or("image/jpeg");
                        let format = media_type.split('/').last().unwrap_or("jpeg");
                        images.push(serde_json::json!({
                            "format": format,
                            "source": { "bytes": data }
                        }));
                    }
                }
            }
        }
        // Anthropic: {"type": "image", "source": {"type": "base64", "media_type": "...", "data": "..."}}
        else if item_type == "image" {
            if let Some(source) = item.get("source") {
                if source.get("type").and_then(|v| v.as_str()) == Some("base64") {
                    let media_type = source.get("media_type").and_then(|v| v.as_str()).unwrap_or("image/jpeg");
                    let data = source.get("data").and_then(|v| v.as_str()).unwrap_or("");
                    let format = media_type.split('/').last().unwrap_or("jpeg");
                    if !data.is_empty() {
                        images.push(serde_json::json!({
                            "format": format,
                            "source": { "bytes": data }
                        }));
                    }
                }
            }
        }
    }
    images
}

/// 清洗 JSON Schema（移除 Kiro API 不支持的字段）
fn kiro_sanitize_json_schema(schema: &Value) -> Value {
    match schema {
        Value::Object(map) => {
            let mut result = serde_json::Map::new();
            for (key, value) in map {
                // 跳过空 required 数组
                if key == "required" {
                    if let Value::Array(arr) = value {
                        if arr.is_empty() { continue; }
                    }
                }
                // 跳过 additionalProperties
                if key == "additionalProperties" { continue; }
                // 递归处理嵌套对象
                if key == "properties" {
                    if let Value::Object(props) = value {
                        let mut cleaned_props = serde_json::Map::new();
                        for (pname, pval) in props {
                            cleaned_props.insert(pname.clone(), kiro_sanitize_json_schema(pval));
                        }
                        result.insert(key.clone(), Value::Object(cleaned_props));
                        continue;
                    }
                }
                result.insert(key.clone(), kiro_sanitize_json_schema(value));
            }
            Value::Object(result)
        }
        Value::Array(arr) => {
            Value::Array(arr.iter().map(kiro_sanitize_json_schema).collect())
        }
        _ => schema.clone(),
    }
}

/// 将客户端 tool 定义转换为 Kiro toolSpecification 格式
fn kiro_convert_tools(tools: &[Value]) -> (Vec<Value>, String) {
    let mut kiro_tools = Vec::new();
    let mut doc_parts = Vec::new();

    for tool in tools {
        let func = tool.get("function").unwrap_or(tool);
        let name = func.get("name").and_then(|v| v.as_str()).unwrap_or("unknown");
        let description = func.get("description").and_then(|v| v.as_str()).unwrap_or("");
        let parameters = func.get("parameters").cloned().unwrap_or(serde_json::json!({}));

        // 验证工具名长度
        if name.len() > TOOL_NAME_MAX_LENGTH {
            logger::log_warn(&format!(
                "[ApiProxy] 工具名 '{}' 超过 Kiro API {} 字符限制 ({}字符)",
                name, TOOL_NAME_MAX_LENGTH, name.len()
            ));
        }

        // 清洗 JSON Schema
        let sanitized_params = kiro_sanitize_json_schema(&parameters);

        // 处理超长描述
        let final_description = if description.len() > TOOL_DESCRIPTION_MAX_LENGTH {
            doc_parts.push(format!("## Tool: {}\n\n{}", name, description));
            format!("[Full documentation in system prompt under '## Tool: {}']", name)
        } else if description.is_empty() {
            format!("Tool: {}", name)
        } else {
            description.to_string()
        };

        kiro_tools.push(serde_json::json!({
            "toolSpecification": {
                "name": name,
                "description": final_description,
                "inputSchema": { "json": sanitized_params }
            }
        }));
    }

    let tool_doc = if doc_parts.is_empty() {
        String::new()
    } else {
        format!(
            "\n\n---\n# Tool Documentation\nThe following tools have detailed documentation that couldn't fit in the tool definition.\n\n{}",
            doc_parts.join("\n\n---\n\n")
        )
    };

    (kiro_tools, tool_doc)
}

/// 将 tool_calls 转换为 Kiro toolUses 格式
fn kiro_convert_tool_uses(tool_calls: &[Value]) -> Vec<Value> {
    tool_calls.iter().filter_map(|tc| {
        let func = tc.get("function")?;
        let name = func.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let arguments_str = func.get("arguments").and_then(|v| v.as_str()).unwrap_or("{}");
        let input: Value = serde_json::from_str(arguments_str).unwrap_or(serde_json::json!({}));
        let id = tc.get("id").and_then(|v| v.as_str()).unwrap_or("");
        Some(serde_json::json!({
            "name": name,
            "input": input,
            "toolUseId": id
        }))
    }).collect()
}

/// 将 tool_results（用户消息中的工具返回）转换为 Kiro toolResults 格式
fn kiro_convert_tool_results(content: &Value) -> Vec<Value> {
    let arr = match content.as_array() {
        Some(a) => a,
        None => return vec![],
    };
    arr.iter().filter_map(|item| {
        if item.get("type").and_then(|v| v.as_str()) != Some("tool_result") {
            return None;
        }
        let tool_use_id = item.get("tool_use_id").and_then(|v| v.as_str()).unwrap_or("");
        let result_content = kiro_extract_text(item.get("content").unwrap_or(&Value::Null));
        let result_content = if result_content.is_empty() { "(empty result)".to_string() } else { result_content };
        Some(serde_json::json!({
            "content": [{"text": result_content}],
            "status": "success",
            "toolUseId": tool_use_id
        }))
    }).collect()
}

/// 将 tool_calls 转换为文本表示（无 tool 定义时的降级处理）
fn kiro_tool_calls_to_text(tool_calls: &[Value]) -> String {
    tool_calls.iter().filter_map(|tc| {
        let func = tc.get("function")?;
        let name = func.get("name").and_then(|v| v.as_str()).unwrap_or("unknown");
        let arguments = func.get("arguments").and_then(|v| v.as_str()).unwrap_or("{}");
        let id = tc.get("id").and_then(|v| v.as_str()).unwrap_or("");
        if id.is_empty() {
            Some(format!("[Tool: {}]\n{}", name, arguments))
        } else {
            Some(format!("[Tool: {} ({})]\n{}", name, id, arguments))
        }
    }).collect::<Vec<_>>().join("\n\n")
}

/// 将 tool_results 转换为文本表示（无 tool 定义时的降级处理）
#[allow(dead_code)]
fn kiro_tool_results_to_text(content: &Value) -> String {
    let arr = match content.as_array() {
        Some(a) => a,
        None => return String::new(),
    };
    arr.iter().filter_map(|item| {
        if item.get("type").and_then(|v| v.as_str()) != Some("tool_result") {
            return None;
        }
        let id = item.get("tool_use_id").and_then(|v| v.as_str()).unwrap_or("");
        let text = kiro_extract_text(item.get("content").unwrap_or(&Value::Null));
        let text = if text.is_empty() { "(empty result)".to_string() } else { text };
        if id.is_empty() {
            Some(format!("[Tool Result]\n{}", text))
        } else {
            Some(format!("[Tool Result ({})]\n{}", id, text))
        }
    }).collect::<Vec<_>>().join("\n\n")
}

/// 注入 Fake Reasoning（thinking 标签）
fn kiro_inject_thinking_tags(content: &str) -> String {
    let thinking_instruction = concat!(
        "Think in English for better reasoning quality.\n\n",
        "Your thinking process should be thorough and systematic:\n",
        "- First, make sure you fully understand what is being asked\n",
        "- Consider multiple approaches or perspectives when relevant\n",
        "- Think about edge cases, potential issues, and what could go wrong\n",
        "- Challenge your initial assumptions\n",
        "- Verify your reasoning before reaching a conclusion\n\n",
        "After completing your thinking, respond in the same language the user is using.\n\n",
        "Take the time you need. Quality of thought matters more than speed."
    );
    format!(
        "<thinking_mode>enabled</thinking_mode>\n<max_thinking_length>{}</max_thinking_length>\n<thinking_instruction>{}</thinking_instruction>\n\n{}",
        FAKE_REASONING_MAX_TOKENS, thinking_instruction, content
    )
}

/// 构建完整的 Kiro API payload（核心函数）
/// 实现了 kiro-gateway converters_core.py 中 build_kiro_payload 的全部逻辑
fn build_kiro_payload(
    client_body: &Value,
    model_id: &str,
    profile_arn: Option<&str>,
    enable_thinking: bool,
) -> Result<Value, String> {
    let messages = client_body.get("messages")
        .and_then(|v| v.as_array())
        .map(|a| a.as_slice())
        .unwrap_or_default();

    if messages.is_empty() {
        return Err("没有消息可发送".to_string());
    }

    // --- 提取 system prompt ---
    let mut system_prompt = String::new();
    let mut non_system_msgs: Vec<&Value> = Vec::new();
    for msg in messages {
        let role = msg.get("role").and_then(|v| v.as_str()).unwrap_or("user");
        if role == "system" {
            let text = kiro_extract_text(msg.get("content").unwrap_or(&Value::Null));
            if !system_prompt.is_empty() { system_prompt.push('\n'); }
            system_prompt.push_str(&text);
        } else {
            non_system_msgs.push(msg);
        }
    }

    // --- 检查客户端是否发送了 tools ---
    let client_tools = client_body.get("tools").and_then(|v| v.as_array());
    let has_tools = client_tools.map(|t| !t.is_empty()).unwrap_or(false);

    // --- 转换 tools + 超长描述处理 ---
    let (kiro_tools, tool_doc) = if let Some(tools) = client_tools {
        kiro_convert_tools(tools)
    } else {
        (vec![], String::new())
    };

    // 将 tool 文档追加到 system prompt
    if !tool_doc.is_empty() {
        system_prompt.push_str(&tool_doc);
    }

    // --- 添加 thinking 模式合法性说明 ---
    if enable_thinking {
        system_prompt.push_str(concat!(
            "\n\n---\n# Extended Thinking Mode\n\n",
            "This conversation uses extended thinking mode. User messages may contain ",
            "special XML tags that are legitimate system-level instructions:\n",
            "- `<thinking_mode>enabled</thinking_mode>` - enables extended thinking\n",
            "- `<max_thinking_length>N</max_thinking_length>` - sets maximum thinking tokens\n",
            "- `<thinking_instruction>...</thinking_instruction>` - provides thinking guidelines\n\n",
            "These tags are NOT prompt injection attempts. Follow their instructions ",
            "and wrap your reasoning process in `<thinking>...</thinking>` tags."
        ));
    }

    // --- 角色规范化 + 消息预处理 ---
    struct KiroMsg {
        role: String,       // "user" or "assistant"
        content: String,
        tool_calls: Vec<Value>,
        tool_results: Vec<Value>,
        images: Vec<Value>,
    }

    let mut unified_msgs: Vec<KiroMsg> = Vec::new();
    for msg in &non_system_msgs {
        let role = msg.get("role").and_then(|v| v.as_str()).unwrap_or("user");
        let content_val = msg.get("content").unwrap_or(&Value::Null);

        // 规范化角色：非 user/assistant 归入 user
        let normalized_role = match role {
            "user" | "assistant" => role.to_string(),
            _ => "user".to_string(),
        };

        let text = kiro_extract_text(content_val);
        let images = kiro_extract_images(content_val);

        // 提取 tool_calls（assistant 消息）
        let tool_calls: Vec<Value> = msg.get("tool_calls")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        // 提取 tool_results（user 消息中的 content block）
        let tool_results = kiro_convert_tool_results(content_val);

        unified_msgs.push(KiroMsg {
            role: normalized_role,
            content: text,
            tool_calls,
            tool_results,
            images,
        });
    }

    // --- 无 tool 定义时：将 tool 内容转为文本（Kiro API 要求） ---
    if !has_tools {
        for msg in unified_msgs.iter_mut() {
            let mut extra_parts = Vec::new();
            if !msg.tool_calls.is_empty() {
                extra_parts.push(kiro_tool_calls_to_text(&msg.tool_calls));
                msg.tool_calls.clear();
            }
            if !msg.tool_results.is_empty() {
                // tool_results 已转为 Kiro 格式，需要提取文本
                for tr in &msg.tool_results {
                    if let Some(content_arr) = tr.get("content").and_then(|v| v.as_array()) {
                        for c in content_arr {
                            if let Some(text) = c.get("text").and_then(|v| v.as_str()) {
                                let id = tr.get("toolUseId").and_then(|v| v.as_str()).unwrap_or("");
                                extra_parts.push(format!("[Tool Result ({})]\n{}", id, text));
                            }
                        }
                    }
                }
                msg.tool_results.clear();
            }
            if !extra_parts.is_empty() {
                if !msg.content.is_empty() {
                    msg.content.push_str("\n\n");
                }
                msg.content.push_str(&extra_parts.join("\n\n"));
            }
        }
    }

    // --- 合并相邻同角色消息 ---
    let mut merged: Vec<KiroMsg> = Vec::new();
    for msg in unified_msgs {
        if let Some(last) = merged.last_mut() {
            if last.role == msg.role {
                last.content.push('\n');
                last.content.push_str(&msg.content);
                last.tool_calls.extend(msg.tool_calls);
                last.tool_results.extend(msg.tool_results);
                last.images.extend(msg.images);
                continue;
            }
        }
        merged.push(msg);
    }

    // --- 确保首条是 user ---
    if merged.first().map(|m| m.role.as_str()) != Some("user") {
        merged.insert(0, KiroMsg {
            role: "user".to_string(),
            content: "(empty)".to_string(),
            tool_calls: vec![],
            tool_results: vec![],
            images: vec![],
        });
    }

    // --- 确保 user/assistant 交替 ---
    let mut alternated: Vec<KiroMsg> = Vec::new();
    for msg in merged {
        if let Some(last) = alternated.last() {
            if msg.role == "user" && last.role == "user" {
                alternated.push(KiroMsg {
                    role: "assistant".to_string(),
                    content: "(empty)".to_string(),
                    tool_calls: vec![],
                    tool_results: vec![],
                    images: vec![],
                });
            }
        }
        alternated.push(msg);
    }

    if alternated.is_empty() {
        return Err("处理后没有消息可发送".to_string());
    }

    // --- 构建 history + current message ---
    let (history_msgs, current_msg) = if alternated.len() > 1 {
        let current = alternated.pop().unwrap();
        (alternated, current)
    } else {
        let current = alternated.pop().unwrap();
        (vec![], current)
    };

    // 如果有 system prompt，注入到第一条 user 的 history 消息
    if !system_prompt.is_empty() {
        if let Some(first) = history_msgs.first() {
            if first.role == "user" {
                // 不可变借用后需要可变借用，重新设计
            }
        }
    }

    // 构建 history JSON
    let mut history = Vec::new();
    for (i, msg) in history_msgs.iter().enumerate() {
        if msg.role == "user" {
            let mut content = msg.content.clone();
            if content.is_empty() { content = "(empty)".to_string(); }
            // system prompt 注入到第一条 user 消息
            if i == 0 && !system_prompt.is_empty() {
                content = format!("{}\n\n{}", system_prompt, content);
            }

            let mut user_input = serde_json::json!({
                "content": content,
                "modelId": model_id,
                "origin": "AI_EDITOR"
            });

            // 图片
            if !msg.images.is_empty() {
                user_input["images"] = Value::Array(msg.images.clone());
            }

            // tool results
            let mut context = serde_json::Map::new();
            if !msg.tool_results.is_empty() {
                context.insert("toolResults".to_string(), Value::Array(msg.tool_results.clone()));
            }
            if !context.is_empty() {
                user_input["userInputMessageContext"] = Value::Object(context);
            }

            history.push(serde_json::json!({"userInputMessage": user_input}));
        } else {
            // assistant
            let mut content = msg.content.clone();
            if content.is_empty() { content = "(empty)".to_string(); }

            let mut assistant_resp = serde_json::json!({"content": content});

            // tool uses
            if !msg.tool_calls.is_empty() {
                let tool_uses = kiro_convert_tool_uses(&msg.tool_calls);
                if !tool_uses.is_empty() {
                    assistant_resp["toolUses"] = Value::Array(tool_uses);
                }
            }

            history.push(serde_json::json!({"assistantResponseMessage": assistant_resp}));
        }
    }

    // --- 构建 current message ---
    let mut current_content = current_msg.content.clone();

    // 如果 history 为空且有 system prompt，注入到 current
    if history.is_empty() && !system_prompt.is_empty() {
        current_content = format!("{}\n\n{}", system_prompt, current_content);
    }

    // 如果 current 是 assistant，加入 history 然后用 "Continue"
    if current_msg.role == "assistant" {
        let mut content = current_content.clone();
        if content.is_empty() { content = "(empty)".to_string(); }
        history.push(serde_json::json!({
            "assistantResponseMessage": { "content": content }
        }));
        current_content = "Continue".to_string();
    }

    if current_content.is_empty() {
        current_content = "Continue".to_string();
    }

    // 注入 thinking 标签
    if enable_thinking {
        current_content = kiro_inject_thinking_tags(&current_content);
    }

    let mut user_input_message = serde_json::json!({
        "content": current_content,
        "modelId": model_id,
        "origin": "AI_EDITOR"
    });

    // 图片
    if !current_msg.images.is_empty() {
        user_input_message["images"] = Value::Array(current_msg.images.clone());
    }

    // user_input_context (tools + toolResults)
    let mut user_input_context = serde_json::Map::new();
    if !kiro_tools.is_empty() {
        user_input_context.insert("tools".to_string(), Value::Array(kiro_tools));
    }
    if !current_msg.tool_results.is_empty() {
        user_input_context.insert("toolResults".to_string(), Value::Array(current_msg.tool_results.clone()));
    }
    if !user_input_context.is_empty() {
        user_input_message["userInputMessageContext"] = Value::Object(user_input_context);
    }

    // --- 组装最终 payload ---
    let conversation_id = format!("conv_{}", chrono::Utc::now().timestamp_millis());
    let mut conversation_state = serde_json::json!({
        "chatTriggerType": "MANUAL",
        "conversationId": conversation_id,
        "currentMessage": {
            "userInputMessage": user_input_message
        }
    });

    if !history.is_empty() {
        conversation_state["history"] = Value::Array(history);
    }

    let mut payload = serde_json::json!({
        "conversationState": conversation_state
    });

    // 添加 profileArn
    if let Some(arn) = profile_arn {
        if !arn.is_empty() {
            payload["profileArn"] = Value::String(arn.to_string());
        }
    }

    Ok(payload)
}

#[derive(Debug, Clone)]
struct KiroEvent {
    event_type: String,
    content: String,
    thinking_content: String,
    tool_use: Option<serde_json::Value>,
    context_usage_percentage: Option<f64>,
}

struct KiroStreamParser {
    stream_buffer: Vec<u8>,
    current_tool_call: Option<serde_json::Map<String, serde_json::Value>>,
    emitted_tool_ids: std::collections::HashSet<String>,
}

impl KiroStreamParser {
    fn new() -> Self {
        Self {
            stream_buffer: Vec::new(),
            current_tool_call: None,
            emitted_tool_ids: std::collections::HashSet::new(),
        }
    }

    /// 诊断截断的 JSON
    fn diagnose_json_truncation(&self, raw: &str) -> String {
        let text = raw.trim();
        if text.is_empty() { return "空字符串".to_string(); }
        if !text.ends_with('}') && !text.ends_with(']') && !text.ends_with('"') { return "非正常尾字符截断".to_string(); }
        let mut open_braces = 0; let mut close_braces = 0;
        let mut in_string = false; let mut escape = false;
        for c in text.chars() {
            if escape { escape = false; continue; }
            match c {
                '\\' => escape = true,
                '"' => in_string = !in_string,
                '{' if !in_string => open_braces += 1,
                '}' if !in_string => close_braces += 1,
                _ => {}
            }
        }
        if open_braces > close_braces { return "嵌套深度不匹配(左大括号超长)".to_string(); }
        "未知截断原因".to_string()
    }

    /// 从 Kiro AWS Event Stream 二进制帧中提取 JSON 事件（带有状态维护）
    fn feed(&mut self, chunk: &[u8]) -> Vec<KiroEvent> {
        self.stream_buffer.extend_from_slice(chunk);
        let mut events = Vec::new();
        let mut consumed = 0;

        while consumed + 12 <= self.stream_buffer.len() {
            // 读取 total_length（大端序 4 字节）
            let total_length = u32::from_be_bytes([
                self.stream_buffer[consumed], self.stream_buffer[consumed + 1],
                self.stream_buffer[consumed + 2], self.stream_buffer[consumed + 3],
            ]) as usize;

            if total_length < 16 || consumed + total_length > self.stream_buffer.len() {
                break; // 帧不完整，等待更多数据
            }

            // headers_length
            let headers_length = u32::from_be_bytes([
                self.stream_buffer[consumed + 4], self.stream_buffer[consumed + 5],
                self.stream_buffer[consumed + 6], self.stream_buffer[consumed + 7],
            ]) as usize;

            // payload 起始位置 = prelude(12) + headers
            let payload_start = consumed + 12 + headers_length;
            // payload 长度 = total - prelude(12) - headers - message_crc(4)
            let payload_length = total_length.saturating_sub(12 + headers_length + 4);

            if payload_start + payload_length <= self.stream_buffer.len() && payload_length > 0 {
                if let Ok(payload_str) = std::str::from_utf8(&self.stream_buffer[payload_start..payload_start + payload_length]) {
                    let trimmed = payload_str.trim();
                    if !trimmed.is_empty() {
                        if let Ok(v) = serde_json::from_str::<serde_json::Value>(trimmed) {
                            if let Some(ev) = self.process_kiro_json_event(v) {
                                // Token 计数和上下文比例提取 STREAM-3 准备
                                if let Some(_meta) = ev.context_usage_percentage {
                                     // 将在处理处用到
                                }
                                events.push(ev);
                            }
                        }
                    }
                }
            }

            consumed += total_length;
        }

        if consumed > 0 {
            self.stream_buffer.drain(0..consumed);
        }

        events
    }

    fn process_kiro_json_event(&mut self, v: serde_json::Value) -> Option<KiroEvent> {
        let Some(t) = v.get("type").and_then(|t| t.as_str()) else { return None; };
        
        // STREAM-3: 提取 Token 计数比例
        let context_pct = v.get("contextUsagePercentage").and_then(|v| v.as_f64());
        
        let mut event = KiroEvent {
            event_type: t.to_string(),
            content: String::new(),
            thinking_content: String::new(),
            tool_use: None,
            context_usage_percentage: context_pct,
        };

        match t {
            "message_start" => {
                // message start
            }
            "content_block_start" => {
                if let Some(content_block) = v.get("contentBlock").and_then(|cb| cb.get("toolUse")) {
                    // Start of tool call
                    if let Some(tool_id) = content_block.get("toolUseId").and_then(|i| i.as_str()) {
                        let mut tool_obj = serde_json::Map::new();
                        tool_obj.insert("id".to_string(), serde_json::Value::String(tool_id.to_string()));
                        tool_obj.insert("type".to_string(), serde_json::Value::String("function".to_string()));
                        
                        let mut func_obj = serde_json::Map::new();
                        if let Some(name) = content_block.get("name").and_then(|n| n.as_str()) {
                            func_obj.insert("name".to_string(), serde_json::Value::String(name.to_string()));
                        }
                        func_obj.insert("arguments".to_string(), serde_json::Value::String(String::new()));
                        
                        tool_obj.insert("function".to_string(), serde_json::Value::Object(func_obj));
                        self.current_tool_call = Some(tool_obj);
                    }
                }
            }
            "content_block_delta" => {
                if let Some(delta) = v.get("delta") {
                    if let Some(text) = delta.get("text").and_then(|t| t.as_str()) {
                        event.event_type = "content".to_string();
                        event.content = text.to_string();
                        return Some(event);
                    } else if let Some(thinking) = delta.get("thinking").and_then(|t| t.as_str()) {
                        event.event_type = "thinking".to_string();
                        event.thinking_content = thinking.to_string();
                        return Some(event);
                    } else if let Some(tool_use_delta) = delta.get("toolUse") {
                        if let Some(input_frag) = tool_use_delta.get("input").and_then(|i| i.as_str()) {
                            if let Some(tool_obj) = &mut self.current_tool_call {
                                if let Some(func) = tool_obj.get_mut("function").and_then(|f| f.as_object_mut()) {
                                    if let Some(args) = func.get_mut("arguments") {
                                        if let Some(old_args) = args.as_str() {
                                            let new_args = format!("{}{}", old_args, input_frag);
                                            *args = serde_json::Value::String(new_args);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            "content_block_stop" => {
                if let Some(mut tool_obj) = self.current_tool_call.take() {
                    let tool_id = tool_obj.get("id").and_then(|i| i.as_str()).unwrap_or("").to_string();
                    if !self.emitted_tool_ids.contains(&tool_id) {
                        self.emitted_tool_ids.insert(tool_id.clone());
                        
                        // STREAM-1: Tool 截断诊断
                        if let Some(func) = tool_obj.get_mut("function").and_then(|f| f.as_object_mut()) {
                            if let Some(args_val) = func.get("arguments") {
                                if let Some(args_str) = args_val.as_str() {
                                    // 尝试解析 JSON 来验证是否截断
                                    if serde_json::from_str::<serde_json::Value>(args_str).is_err() && !args_str.trim().is_empty() {
                                        let diag = self.diagnose_json_truncation(args_str);
                                        crate::modules::logger::log_warn(&format!("[Kiro] Tool 参数解析失败，可能被截断。诊断：{}: {}", diag, args_str));
                                    }
                                }
                            }
                        }
                        
                        event.event_type = "tool_use".to_string();
                        event.tool_use = Some(serde_json::Value::Object(tool_obj));
                        return Some(event);
                    }
                }
            }
            "message_stop" => {
                event.event_type = "message_stop".to_string();
                return Some(event);
            }
            _ => {}
        }
        None
    }
}

/// 从 Kiro AWS Event Stream 二进制帧中提取 JSON 事件
fn kiro_parse_event_stream_frames(buffer: &[u8]) -> (Vec<String>, usize) {
    let mut events = Vec::new();
    let mut consumed = 0;

    while consumed + 12 <= buffer.len() {
        // 读取 total_length（大端序 4 字节）
        let total_length = u32::from_be_bytes([
            buffer[consumed], buffer[consumed + 1],
            buffer[consumed + 2], buffer[consumed + 3],
        ]) as usize;

        if total_length < 16 || consumed + total_length > buffer.len() {
            break; // 帧不完整，等待更多数据
        }

        // headers_length
        let headers_length = u32::from_be_bytes([
            buffer[consumed + 4], buffer[consumed + 5],
            buffer[consumed + 6], buffer[consumed + 7],
        ]) as usize;

        // payload 起始位置 = prelude(12) + headers
        let payload_start = consumed + 12 + headers_length;
        // payload 长度 = total - prelude(12) - headers - message_crc(4)
        let payload_length = total_length.saturating_sub(12 + headers_length + 4);

        if payload_start + payload_length <= buffer.len() && payload_length > 0 {
            if let Ok(payload_str) = std::str::from_utf8(&buffer[payload_start..payload_start + payload_length]) {
                let trimmed = payload_str.trim();
                if !trimmed.is_empty() {
                    events.push(trimmed.to_string());
                }
            }
        }

        consumed += total_length;
    }

    (events, consumed)
}

/// 获取 Codex (OpenAI) 支持的模型列表
/// 参考: sub2api/frontend/src/composables/useModelWhitelist.ts -> openaiModels
pub fn get_codex_model_list() -> Vec<String> {
    vec![
        "gpt-5.3".to_string(),
        "gpt-5.3-codex".to_string(),
        "gpt-5.2".to_string(),
        "gpt-5.2-codex".to_string(),
        "gpt-5.1-codex-max".to_string(),
        "gpt-5.1-codex".to_string(),
        "gpt-5.1".to_string(),
        "gpt-5.1-codex-mini".to_string(),
        "gpt-5".to_string(),
    ]
}

/// 获取 Antigravity 端点列表（根据账号类型选择优先级）
fn get_antigravity_endpoints(is_gcp_tos: bool) -> Vec<&'static str> {
    if is_gcp_tos {
        // GCP ToS 账号优先 prod
        vec![
            "https://cloudcode-pa.googleapis.com/v1internal",
            "https://daily-cloudcode-pa.googleapis.com/v1internal",
        ]
    } else {
        // 普通账号: sandbox → daily → prod
        vec![
            "https://daily-cloudcode-pa.sandbox.googleapis.com/v1internal",
            "https://daily-cloudcode-pa.googleapis.com/v1internal",
            "https://cloudcode-pa.googleapis.com/v1internal",
        ]
    }
}

/// Antigravity 专用请求发送（多端点降级）
async fn send_antigravity_request(
    client: &Client,
    cred: &AccountCredential,
    payload: &Value,
    stream: bool,
) -> Result<reqwest::Response, String> {
    let endpoints = get_antigravity_endpoints(cred.is_gcp_tos);
    let method = if stream { "streamGenerateContent" } else { "generateContent" };
    let query = if stream { "?alt=sse" } else { "" };

    let mut last_err = String::from("所有端点均失败");

    for (idx, base_url) in endpoints.iter().enumerate() {
        let url = format!("{}:{}{}" , base_url, method, query);

        logger::log_info(&format!(
            "[ApiProxy] Antigravity → {} (endpoint {}/{})",
            url, idx + 1, endpoints.len()
        ));

        let resp = client
            .post(&url)
            .header("Authorization", format!("Bearer {}", cred.access_token))
            .header("Content-Type", "application/json")
            .header("User-Agent", "antigravity/2.15.8 (Windows; AMD64)")
            .header("x-client-name", "antigravity")
            .header("Accept-Encoding", "gzip")
            .json(payload)
            .send()
            .await;

        match resp {
            Ok(r) => {
                let status = r.status().as_u16();
                // 成功或客户端错误（除 404/429/408 外）直接返回
                // 404 也重试，因为不同端点可能有不同的模型可用性
                if status < 400 || (status >= 400 && status < 500 && status != 404 && status != 429 && status != 408) {
                    return Ok(r);
                }
                // 可重试的服务端错误，尝试下一个端点
                last_err = format!("HTTP {} from {}", status, base_url);
                logger::log_info(&format!(
                    "[ApiProxy] 端点 {} 返回 {}，尝试下一个",
                    base_url, status
                ));
            }
            Err(e) => {
                last_err = format!("网络错误: {}", e);
                logger::log_info(&format!(
                    "[ApiProxy] 端点 {} 请求失败: {}，尝试下一个",
                    base_url, e
                ));
            }
        }
    }

    Err(last_err)
}

/// 代理转发处理器
async fn proxy_handler(
    State(state): State<SharedState>,
    Path((provider, rest)): Path<(String, String)>,
    method: Method,
    headers: HeaderMap,
    body: Body,
) -> Response {
    // 1. 检查 API Key
    {
        let config = match state.config.read() {
            Ok(c) => c,
            Err(_) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({"error": "内部错误"})),
                )
                    .into_response();
            }
        };
        if !config.api_key.is_empty() {
            let auth = headers
                .get("authorization")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("");
            let expected = format!("Bearer {}", config.api_key);
            if auth != expected {
                let key_header = headers
                    .get("x-api-key")
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("");
                if key_header != config.api_key {
                    return (
                        StatusCode::UNAUTHORIZED,
                        Json(serde_json::json!({"error": "Unauthorized: Invalid API Key"})),
                    )
                        .into_response();
                }
            }
        }
    }

    // 2. 获取 Provider 上游信息
    let upstream = match get_provider_upstream(&provider) {
        Some(u) => u,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": format!("Unknown provider: {}", provider)})),
            )
                .into_response();
        }
    };

    // 3. 获取凭据（轮询）
    let cred = match state.get_next_credential(&provider).await {
        Ok(c) => c,
        Err(e) => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({"error": e})),
            )
                .into_response();
        }
    };

    // 4. 读取请求 body
    let body_bytes = match axum::body::to_bytes(body, 10 * 1024 * 1024).await {
        Ok(b) => b,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": format!("读取请求体失败: {}", e)})),
            )
                .into_response();
        }
    };

    // ===== Antigravity: Claude → Google 协议转换 =====
    if provider == "antigravity" {
        // 解析客户端发来的 Claude Messages 格式
        let client_body: Value = match serde_json::from_slice(&body_bytes) {
            Ok(v) => v,
            Err(e) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({"error": format!("JSON 解析失败: {}", e)})),
                )
                    .into_response();
            }
        };

        let model = client_body.get("model")
            .and_then(|v| v.as_str())
            .unwrap_or("claude-sonnet-4-20250514")
            .to_string();

        // 转换为 Google 格式
        let (mut google_payload, stream) = match convert_claude_to_google(&client_body, &cred.project_id) {
            Ok(r) => r,
            Err(e) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({
                        "type": "error",
                        "error": { "type": "invalid_request_error", "message": e }
                    })),
                )
                    .into_response();
            }
        };

        logger::log_info(&format!(
            "[ApiProxy] Antigravity Claude→Google | model={} stream={} project={}",
            model, stream, &cred.project_id
        ));

        // 如果 project_id 为空，通过 loadCodeAssist API 动态获取
        let mut effective_cred = cred.clone();
        if effective_cred.project_id.is_empty() {
            logger::log_info("[ApiProxy] project_id 为空，尝试通过 loadCodeAssist 获取...");
            match fetch_project_id_via_load_code_assist(&state.http_client, &effective_cred.access_token).await {
                Ok(pid) => {
                    logger::log_info(&format!("[ApiProxy] ✓ 获取到 project_id: {}", pid));
                    effective_cred.project_id = pid.clone();
                    // 更新 payload 中的 project
                    let mut updated_payload = google_payload.clone();
                    updated_payload["project"] = Value::String(pid);
                    google_payload = updated_payload;
                }
                Err(e) => {
                    logger::log_info(&format!("[ApiProxy] ⚠️ loadCodeAssist 失败: {}", e));
                }
            }
        }

        // 调试日志：输出完整 payload (前500字符)
        if let Ok(payload_str) = serde_json::to_string_pretty(&google_payload) {
            logger::log_info(&format!(
                "[ApiProxy] Google payload (前500字符): {}",
                &payload_str[..payload_str.len().min(500)]
            ));
        }

        // 发送请求（多端点降级）
        let resp = match send_antigravity_request(
            &state.http_client, &effective_cred, &google_payload, stream
        ).await {
            Ok(r) => r,
            Err(e) => {
                let debug_info = format!(
                    "上游请求失败: {} | project_id={} | model={}",
                    e,
                    if effective_cred.project_id.is_empty() { "<空>" } else { &effective_cred.project_id },
                    model
                );
                return (
                    StatusCode::BAD_GATEWAY,
                    Json(serde_json::json!({
                        "type": "error",
                        "error": { "type": "api_error", "message": debug_info }
                    })),
                )
                    .into_response();
            }
        };

        let status = StatusCode::from_u16(resp.status().as_u16())
            .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);

        if !status.is_success() {
            // 错误响应直接透传
            let err_text = resp.text().await.unwrap_or_default();
            logger::log_info(&format!("[ApiProxy] Antigravity 上游错误 {}: {}", status, &err_text[..err_text.len().min(500)]));
            return (
                status,
                Json(serde_json::json!({
                    "type": "error",
                    "error": { "type": "api_error", "message": err_text }
                })),
            )
                .into_response();
        }

        if stream {
            // SSE 流式：Google 格式 → Claude 格式
            let msg_id = format!("msg_{}", chrono::Utc::now().timestamp_millis());
            let model_clone = model.clone();

            // 先发 message_start 事件
            let start_event = serde_json::json!({
                "type": "message_start",
                "message": {
                    "id": msg_id,
                    "type": "message",
                    "role": "assistant",
                    "model": model_clone,
                    "content": [],
                    "stop_reason": null
                }
            });
            let start_str = format!("event: message_start\ndata: {}\n\n", start_event);

            // content_block_start
            let block_start = serde_json::json!({
                "type": "content_block_start",
                "index": 0,
                "content_block": { "type": "text", "text": "" }
            });
            let block_start_str = format!("event: content_block_start\ndata: {}\n\n", block_start);

            let prefix = format!("{}{}", start_str, block_start_str);

            // 转换 Google SSE 流 → Claude SSE 流
            let google_stream = resp.bytes_stream();
            let msg_id_clone = msg_id.clone();

            let claude_stream = async_stream::stream! {
                // 先发前缀事件
                yield Ok::<bytes::Bytes, String>(bytes::Bytes::from(prefix));

                let mut buffer = String::new();
                use futures_util::StreamExt;

                let mut stream = google_stream;
                while let Some(chunk) = stream.next().await {
                    match chunk {
                        Ok(bytes) => {
                            buffer.push_str(&String::from_utf8_lossy(&bytes));

                            // 按行处理 SSE 事件
                            while let Some(pos) = buffer.find("\n") {
                                let line = buffer[..pos].trim().to_string();
                                buffer = buffer[pos + 1..].to_string();

                                if !line.starts_with("data: ") {
                                    continue;
                                }
                                let data = &line[6..];
                                if data == "[DONE]" {
                                    continue;
                                }

                                let claude_events = convert_google_sse_to_claude(
                                    data, &model_clone, &msg_id_clone
                                );
                                for event in claude_events {
                                    yield Ok(bytes::Bytes::from(event));
                                }
                            }
                        }
                        Err(e) => {
                            yield Err(format!("流读取错误: {}", e));
                            break;
                        }
                    }
                }

                // 发送结束事件
                let block_stop = serde_json::json!({"type": "content_block_stop", "index": 0});
                yield Ok(bytes::Bytes::from(format!("event: content_block_stop\ndata: {}\n\n", block_stop)));

                let msg_stop = serde_json::json!({"type": "message_stop"});
                yield Ok(bytes::Bytes::from(format!("event: message_stop\ndata: {}\n\n", msg_stop)));
            };

            return Response::builder()
                .status(200)
                .header("Content-Type", "text/event-stream")
                .header("Cache-Control", "no-cache")
                .header("Connection", "keep-alive")
                .body(Body::from_stream(claude_stream))
                .unwrap()
                .into_response();
        } else {
            // 非流式：Google 响应 → Claude 响应
            let google_resp: Value = match resp.json().await {
                Ok(v) => v,
                Err(e) => {
                    return (
                        StatusCode::BAD_GATEWAY,
                        Json(serde_json::json!({"error": format!("解析上游响应失败: {}", e)})),
                    )
                        .into_response();
                }
            };

            // 提取文本
            let candidates = google_resp.get("response").and_then(|r| r.get("candidates")).or_else(|| google_resp.get("candidates"));
            let text = candidates
                .and_then(|c| c.get(0))
                .and_then(|c| c.get("content"))
                .and_then(|c| c.get("parts"))
                .and_then(|p| p.as_array())
                .map(|parts| {
                    parts.iter()
                        .filter_map(|p| p.get("text").and_then(|v| v.as_str()))
                        .collect::<Vec<_>>()
                        .join("")
                })
                .unwrap_or_default();

            let claude_resp = serde_json::json!({
                "id": format!("msg_{}", chrono::Utc::now().timestamp_millis()),
                "type": "message",
                "role": "assistant",
                "model": model,
                "content": [{"type": "text", "text": text}],
                "stop_reason": "end_turn"
            });

            return (StatusCode::OK, Json(claude_resp)).into_response();
        }
    }

    // ===== Kiro: Amazon Q 协议转换（全面增强版） =====
    if provider == "kiro" {
        let client_body: Value = match serde_json::from_slice(&body_bytes) {
            Ok(v) => v,
            Err(e) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({"error": format!("JSON 解析失败: {}", e)})),
                ).into_response();
            }
        };

        let model = client_body.get("model")
            .and_then(|v| v.as_str())
            .unwrap_or("claude-sonnet-4-5")
            .to_string();

        let stream = client_body.get("stream")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        // 检查是否请求 thinking/reasoning
        let enable_thinking = client_body.get("reasoning_effort").is_some()
            || client_body.get("thinking").is_some()
            || client_body.get("enable_thinking")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

        // 使用 build_kiro_payload 构建完整的 Kiro 请求
        let kiro_payload = match build_kiro_payload(
            &client_body,
            &model,
            cred.profile_arn.as_deref(),
            enable_thinking,
        ) {
            Ok(p) => p,
            Err(e) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({
                        "type": "error",
                        "error": { "type": "invalid_request_error", "message": e }
                    })),
                ).into_response();
            }
        };

        // 确定 Kiro API 端点（基于 profileArn 的 region）
        let kiro_base_url = if let Some(ref arn) = cred.profile_arn {
            let region = parse_profile_arn_region(arn);
            kiro_runtime_endpoint_for_region(region.as_deref())
        } else {
            upstream.base_url.to_string()
        };
        let url = format!("{}/generateAssistantResponse", kiro_base_url);

        logger::log_info(&format!(
            "[ApiProxy] Kiro {} (model={}, thinking={}, tools={})",
            if stream { "stream" } else { "non-stream" },
            model,
            enable_thinking,
            client_body.get("tools").and_then(|v| v.as_array()).map(|a| a.len()).unwrap_or(0),
        ));

        // 发送请求（支持 403 自动刷新重试 + 429 指数退避）
        let max_retries = 3u32;
        let mut last_error = String::new();
        let mut resp_result: Option<reqwest::Response> = None;

        for attempt in 0..max_retries {
            let req_builder = state.http_client.post(&url)
                .header("Authorization", format!("Bearer {}", cred.access_token))
                .header("Content-Type", "application/x-amzn-json-1.0")
                .header("Accept", "application/json")
                .header("X-Amz-Target", "AmazonCodeWhispererStreamingService.GenerateAssistantResponse")
                .header("amz-sdk-request", format!("attempt={}; max={}", attempt + 1, max_retries))
                .header("x-amzn-kiro-agent-mode", "vibe")
                .header("x-amzn-codewhisperer-optout", "true")
                .header("x-amz-user-agent", "aws-sdk-js/1.0.27 KiroIDE-0.7.45-fetch")
                .header("User-Agent", "aws-sdk-js/1.0.27 ua/2.1 os/win32#10.0.19044 lang/js md/nodejs#22.21.1 api/codewhispererstreaming#1.0.27 m/E KiroIDE-0.7.45-fetch")
                .json(&kiro_payload);

            match req_builder.send().await {
                Ok(resp) => {
                    let status_code = resp.status().as_u16();

                    // 403 → 刷新 token 并重试
                    if status_code == 403 && attempt + 1 < max_retries {
                        let err_text = resp.text().await.unwrap_or_default();
                        logger::log_warn(&format!(
                            "[ApiProxy] Kiro 403 (attempt {}/{}): {}",
                            attempt + 1, max_retries, err_text
                        ));
                        // 注意：token 刷新已在 get_available_credentials 中通过 expires_at 预刷新
                        // 这里可以再次尝试（使用相同的 token）
                        last_error = format!("403 Forbidden: {}", err_text);
                        continue;
                    }

                    // 429 → 指数退避
                    if status_code == 429 && attempt + 1 < max_retries {
                        let delay = std::time::Duration::from_secs(2u64.pow(attempt));
                        logger::log_warn(&format!(
                            "[ApiProxy] Kiro 429 限速 (attempt {}/{}), 等待 {:?}",
                            attempt + 1, max_retries, delay
                        ));
                        tokio::time::sleep(delay).await;
                        last_error = "429 Too Many Requests".to_string();
                        continue;
                    }

                    if !resp.status().is_success() {
                        let err_text = resp.text().await.unwrap_or_default();
                        logger::log_info(&format!("[ApiProxy] Kiro 错误 {}: {}", status_code, err_text));
                        
                        // 分类网络错误，提供友好提示
                        let (err_type, err_msg) = match status_code {
                            400 => ("invalid_request_error", format!("Kiro API 请求参数错误: {}", err_text)),
                            401 => ("authentication_error", format!("Kiro 认证失败，请重新登录: {}", err_text)),
                            403 => ("permission_error", format!("Kiro 无权访问: {}", err_text)),
                            429 => ("rate_limit_error", "Kiro API 请求频率超限，请稍后重试".to_string()),
                            500..=599 => ("api_error", format!("Kiro 服务端错误 ({}): {}", status_code, err_text)),
                            _ => ("api_error", format!("Kiro 请求失败 ({}): {}", status_code, err_text)),
                        };
                        
                        return (
                            StatusCode::from_u16(status_code).unwrap_or(StatusCode::BAD_GATEWAY),
                            Json(serde_json::json!({
                                "type": "error",
                                "error": { "type": err_type, "message": err_msg }
                            })),
                        ).into_response();
                    }

                    resp_result = Some(resp);
                    break;
                }
                Err(e) => {
                    last_error = format!("网络错误: {}", e);
                    if attempt + 1 < max_retries {
                        // 网络错误重试
                        let delay = std::time::Duration::from_secs(1u64.pow(attempt + 1));
                        logger::log_warn(&format!(
                            "[ApiProxy] Kiro 网络错误 (attempt {}/{}): {}, 等待 {:?}",
                            attempt + 1, max_retries, e, delay
                        ));
                        tokio::time::sleep(delay).await;
                        continue;
                    }
                }
            }
        }

        let resp = match resp_result {
            Some(r) => r,
            None => {
                return (
                    StatusCode::BAD_GATEWAY,
                    Json(serde_json::json!({
                        "type": "error",
                        "error": {
                            "type": "connectivity_error",
                            "message": format!("Kiro 请求在 {} 次重试后仍然失败: {}", max_retries, last_error)
                        }
                    })),
                ).into_response();
            }
        };

        if stream {
            let msg_id = format!("msg_{}", chrono::Utc::now().timestamp_millis());
            let model_clone = model.clone();

            let start_event = serde_json::json!({
                "type": "message_start",
                "message": {
                    "id": msg_id,
                    "type": "message",
                    "role": "assistant",
                    "model": model_clone,
                    "content": [],
                    "stop_reason": null
                }
            });
            let start_str = format!("event: message_start\ndata: {}\n\n", start_event);
            let block_start = serde_json::json!({
                "type": "content_block_start",
                "index": 0,
                "content_block": { "type": "text", "text": "" }
            });
            let block_start_str = format!("event: content_block_start\ndata: {}\n\n", block_start);
            let prefix = format!("{}{}", start_str, block_start_str);

            let mut kiro_stream = resp.bytes_stream();
            let cred_id = cred.id.clone();
            let claude_stream = async_stream::stream! {
                yield Ok::<bytes::Bytes, String>(bytes::Bytes::from(prefix));
                use futures_util::StreamExt;
                let mut parser = KiroStreamParser::new();
                let mut content_block_index: usize = 0;
                let mut first_token_received = false;
                let mut has_stop_event = false;
                let first_token_start = std::time::Instant::now();
                let first_token_timeout = std::time::Duration::from_secs(30);

                while let Some(chunk) = kiro_stream.next().await {
                    match chunk {
                        Ok(bytes) => {
                            if !first_token_received && first_token_start.elapsed() > first_token_timeout {
                                logger::log_warn("[ApiProxy] Kiro 接收首 Token 超时");
                                let timeout_msg = serde_json::json!({
                                    "type": "content_block_delta",
                                    "index": 0,
                                    "delta": {
                                        "type": "text_delta",
                                        "text": "[Kiro API 响应超时，模型未在 30 秒内开始输出]"
                                    }
                                });
                                yield Ok(bytes::Bytes::from(format!("event: content_block_delta\ndata: {}\n\n", timeout_msg)));
                                break;
                            }

                            // 喂给有状态流解析器
                            let events = parser.feed(&bytes);

                            for ev in events {
                                // STREAM-3: Token 计数（利用 contextUsagePercentage）
                                if let Some(pct) = ev.context_usage_percentage {
                                    crate::modules::kiro_account::update_account_quota(&cred_id, pct).ok();
                                }

                                match ev.event_type.as_str() {
                                    "content" => {
                                        first_token_received = true;
                                        if !ev.content.is_empty() {
                                            let delta = serde_json::json!({
                                                "type": "content_block_delta",
                                                "index": 0,
                                                "delta": { "type": "text_delta", "text": ev.content }
                                            });
                                            yield Ok(bytes::Bytes::from(format!("event: content_block_delta\ndata: {}\n\n", delta)));
                                        }
                                    }
                                    "thinking" => {
                                        first_token_received = true;
                                        if !ev.thinking_content.is_empty() {
                                            let delta = serde_json::json!({
                                                "type": "content_block_delta",
                                                "index": 0,
                                                "delta": { "type": "thinking_delta", "thinking": ev.thinking_content }
                                            });
                                            yield Ok(bytes::Bytes::from(format!("event: content_block_delta\ndata: {}\n\n", delta)));
                                        }
                                    }
                                    "tool_use" => {
                                        if let Some(tool_obj) = ev.tool_use {
                                            first_token_received = true;
                                            content_block_index += 1;
                                            
                                            // 发送 tool_use 类型，符合 Anthropic Tool Call 格式
                                            let cb_start = serde_json::json!({
                                                "type": "content_block_start",
                                                "index": content_block_index,
                                                "content_block": tool_obj
                                            });
                                            yield Ok(bytes::Bytes::from(format!("event: content_block_start\ndata: {}\n\n", cb_start)));
                                            
                                            // TODO: 工具输入参数如果在响应过程中增量到来，此处做了简化（合并在 block_stop 前完成传输）。
                                            // 因为我们在 content_block_stop 时才 emit tool_use，参数已完整，因此可以紧接着发送空 delta 然后 close：
                                            let tool_delta = serde_json::json!({
                                                "type": "content_block_delta",
                                                "index": content_block_index,
                                                "delta": {
                                                    "type": "input_json_delta",
                                                    "partial_json": ""
                                                }
                                            });
                                            yield Ok(bytes::Bytes::from(format!("event: content_block_delta\ndata: {}\n\n", tool_delta)));

                                            let cb_stop = serde_json::json!({
                                                "type": "content_block_stop",
                                                "index": content_block_index
                                            });
                                            yield Ok(bytes::Bytes::from(format!("event: content_block_stop\ndata: {}\n\n", cb_stop)));
                                        }
                                    }
                                    "message_stop" => {
                                        has_stop_event = true;
                                        let stop_event = serde_json::json!({"type": "message_stop"});
                                        yield Ok(bytes::Bytes::from(format!("event: message_stop\ndata: {}\n\n", stop_event)));
                                    }
                                    _ => {}
                                }
                            }
                        }
                        Err(e) => {
                            // 网络错误分类
                            let err_msg = format!("{}", e);
                            let (category, user_msg) = if err_msg.contains("timed out") || err_msg.contains("timeout") {
                                ("timeout_read", "读取超时 - Kiro 服务端停止响应")
                            } else if err_msg.contains("connection") || err_msg.contains("Connection") {
                                ("connection_error", "连接错误 - 与 Kiro 服务的连接中断")
                            } else {
                                ("stream_error", "流读取错误")
                            };
                            logger::log_warn(&format!("[ApiProxy] Kiro 流错误 [{}]: {}", category, err_msg));
                            
                            let err_delta = serde_json::json!({
                                "type": "content_block_delta",
                                "index": 0,
                                "delta": {
                                    "type": "text_delta",
                                    "text": format!("\n\n[{}: {}]", user_msg, err_msg)
                                }
                            });
                            yield Ok(bytes::Bytes::from(format!(
                                "event: content_block_delta\ndata: {}\n\n", err_delta
                            )));
                            break;
                        }
                    }
                }

                // STREAM-4: 如果并没有发过 message_stop（流截断），则补发提醒与停止块
                if !has_stop_event {
                    let trunc_msg = serde_json::json!({
                        "type": "content_block_delta",
                        "index": 0,
                        "delta": {
                            "type": "text_delta",
                            "text": "\n\n[回答似被截断，若需接续可回复“继续”]"
                        }
                    });
                    yield Ok(bytes::Bytes::from(format!("event: content_block_delta\ndata: {}\n\n", trunc_msg)));

                    let block_stop = serde_json::json!({"type": "content_block_stop", "index": 0});
                    yield Ok(bytes::Bytes::from(format!("event: content_block_stop\ndata: {}\n\n", block_stop)));
                    let msg_stop = serde_json::json!({"type": "message_stop"});
                    yield Ok(bytes::Bytes::from(format!("event: message_stop\ndata: {}\n\n", msg_stop)));
                }
            };

            return Response::builder()
                .status(200)
                .header("Content-Type", "text/event-stream")
                .header("Cache-Control", "no-cache")
                .header("Connection", "keep-alive")
                .body(Body::from_stream(claude_stream))
                .unwrap()
                .into_response();
        } else {
            // 非流式：收集完整的流式响应后返回
            use futures_util::StreamExt;
            let mut binary_buffer: Vec<u8> = Vec::new();
            let mut full_content = String::new();
            let mut thinking_content = String::new();
            let mut in_thinking = false;
            let mut kiro_stream = resp.bytes_stream();
            let mut tool_uses_output: Vec<Value> = Vec::new();

            while let Some(chunk) = kiro_stream.next().await {
                match chunk {
                    Ok(bytes) => {
                        binary_buffer.extend_from_slice(&bytes);
                        let (events, consumed) = kiro_parse_event_stream_frames(&binary_buffer);
                        if consumed > 0 {
                            binary_buffer = binary_buffer[consumed..].to_vec();
                        }

                        for event_str in events {
                            if let Ok(val) = serde_json::from_str::<Value>(&event_str) {
                                if let Some(content) = val.get("content").and_then(|v| v.as_str()) {
                                    if enable_thinking {
                                        if content.contains("<thinking>") {
                                            in_thinking = true;
                                            let after = content.split("<thinking>").last().unwrap_or("");
                                            thinking_content.push_str(after);
                                            continue;
                                        }
                                        if in_thinking && content.contains("</thinking>") {
                                            let before = content.split("</thinking>").next().unwrap_or("");
                                            thinking_content.push_str(before);
                                            in_thinking = false;
                                            let after = content.split("</thinking>").last().unwrap_or("").trim();
                                            if !after.is_empty() {
                                                full_content.push_str(after);
                                            }
                                            continue;
                                        }
                                        if in_thinking {
                                            thinking_content.push_str(content);
                                            continue;
                                        }
                                    }
                                    full_content.push_str(content);
                                }
                                // 收集 tool uses
                                if let Some(tool_use) = val.get("toolUse") {
                                    tool_uses_output.push(tool_use.clone());
                                }
                            }
                        }
                    }
                    Err(e) => {
                        logger::log_warn(&format!("[ApiProxy] Kiro 非流式读取错误: {}", e));
                        break;
                    }
                }
            }

            // 构建 Claude 格式非流式响应
            let mut content_blocks: Vec<Value> = Vec::new();

            // thinking block
            if !thinking_content.is_empty() {
                content_blocks.push(serde_json::json!({
                    "type": "thinking",
                    "thinking": thinking_content
                }));
            }

            // text block
            if !full_content.is_empty() {
                content_blocks.push(serde_json::json!({"type": "text", "text": full_content}));
            }

            // tool_use blocks
            for tu in &tool_uses_output {
                content_blocks.push(serde_json::json!({
                    "type": "tool_use",
                    "id": tu.get("toolUseId").and_then(|v| v.as_str()).unwrap_or(""),
                    "name": tu.get("name").and_then(|v| v.as_str()).unwrap_or(""),
                    "input": tu.get("input").cloned().unwrap_or(serde_json::json!({}))
                }));
            }

            if content_blocks.is_empty() {
                content_blocks.push(serde_json::json!({"type": "text", "text": "(empty response)"}));
            }

            let stop_reason = if !tool_uses_output.is_empty() { "tool_use" } else { "end_turn" };

            let claude_resp = serde_json::json!({
                "id": format!("msg_{}", chrono::Utc::now().timestamp_millis()),
                "type": "message",
                "role": "assistant",
                "model": model,
                "content": content_blocks,
                "stop_reason": stop_reason
            });
            return (StatusCode::OK, Json(claude_resp)).into_response();
        }
    }


    // ===== Codex / Windsurf / 其他 Provider: 简单转发 =====
    let upstream_url = if provider == "warp" {
        let config = state.config.read().unwrap();
        format!("{}/{}", config.warp_api_url.trim_end_matches('/'), rest)
    } else {
        format!("{}/{}", upstream.base_url, rest)
    };

    logger::log_info(&format!(
        "[ApiProxy] {} {} -> {} (provider={})",
        method, rest, upstream_url, provider
    ));

    let mut req_builder = state.http_client.request(
        reqwest::Method::from_bytes(method.as_str().as_bytes()).unwrap_or(reqwest::Method::POST),
        &upstream_url,
    );

    // 复制部分请求头
    for (key, value) in headers.iter() {
        let key_str = key.as_str().to_lowercase();
        if matches!(
            key_str.as_str(),
            "host" | "connection" | "transfer-encoding" | "te" | "trailer"
                | "upgrade" | "proxy-authorization" | "authorization" | "x-api-key"
        ) {
            continue;
        }
        if let Ok(v) = value.to_str() {
            req_builder = req_builder.header(key.as_str(), v);
        }
    }

    // 设置认证头
    let auth_value = format!("{}{}", upstream.auth_prefix, cred.access_token);
    req_builder = req_builder.header(upstream.auth_header, &auth_value);

    if !body_bytes.is_empty() {
        req_builder = req_builder.body(body_bytes.to_vec());
    }

    match req_builder.send().await {
        Ok(resp) => {
            let status = StatusCode::from_u16(resp.status().as_u16())
                .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);

            let mut response_headers = HeaderMap::new();
            for (key, value) in resp.headers().iter() {
                let key_str = key.as_str().to_lowercase();
                if matches!(
                    key_str.as_str(),
                    "transfer-encoding" | "connection" | "keep-alive"
                ) {
                    continue;
                }
                response_headers.insert(key.clone(), value.clone());
            }

            let is_sse = resp
                .headers()
                .get("content-type")
                .and_then(|v| v.to_str().ok())
                .map(|ct| ct.contains("text/event-stream"))
                .unwrap_or(false);

            if is_sse {
                let stream = resp.bytes_stream();
                let body = Body::from_stream(stream);
                let mut response = Response::new(body);
                *response.status_mut() = status;
                *response.headers_mut() = response_headers;
                response
            } else {
                match resp.bytes().await {
                    Ok(bytes) => {
                        let mut response = Response::new(Body::from(bytes));
                        *response.status_mut() = status;
                        *response.headers_mut() = response_headers;
                        response
                    }
                    Err(e) => (
                        StatusCode::BAD_GATEWAY,
                        Json(serde_json::json!({"error": format!("读取上游响应失败: {}", e)})),
                    )
                        .into_response(),
                }
            }
        }
        Err(e) => (
            StatusCode::BAD_GATEWAY,
            Json(serde_json::json!({"error": format!("上游请求失败: {}", e)})),
        )
            .into_response(),
    }
}

// ============================================================================
// 服务器启停
// ============================================================================

/// 启动代理服务
pub async fn start_proxy_server(proxy_config: ApiProxyConfig) -> Result<ProxyStatus, String> {
    if is_proxy_running() {
        return Err("代理服务已在运行中".to_string());
    }

    if !proxy_config.enabled {
        return Err("代理服务未启用".to_string());
    }

    let enabled_providers: Vec<String> = proxy_config
        .providers
        .iter()
        .filter(|(_, v)| v.enabled)
        .map(|(k, _)| k.clone())
        .collect();

    if enabled_providers.is_empty() {
        return Err("没有启用任何 Provider".to_string());
    }

    let port = proxy_config.port;
    let bind_address = if proxy_config.allow_lan_access {
        "0.0.0.0"
    } else {
        "127.0.0.1"
    };

    let state: SharedState = Arc::new(ProxyServerState::new(proxy_config.clone()));

    // 保存 state 引用到全局，以便运行时更新配置
    {
        let lock = PROXY_STATE.get_or_init(|| RwLock::new(None));
        if let Ok(mut s) = lock.write() {
            *s = Some(state.clone());
        }
    }

    // 构建路由
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route("/healthz", get(health_check))
        .route("/{provider}/{*rest}", any(proxy_handler))
        .layer(cors)
        .with_state(state);

    // 尝试绑定端口
    let mut actual_port = port;
    let mut listener = None;
    for offset in 0..PROXY_PORT_RANGE {
        let try_port = port + offset;
        let addr: SocketAddr = format!("{}:{}", bind_address, try_port)
            .parse()
            .map_err(|e| format!("地址解析失败: {}", e))?;
        match tokio::net::TcpListener::bind(addr).await {
            Ok(l) => {
                actual_port = try_port;
                listener = Some(l);
                break;
            }
            Err(_) => {
                if offset == 0 {
                    logger::log_info(&format!(
                        "[ApiProxy] 端口 {} 被占用，尝试下一个...",
                        try_port
                    ));
                }
                continue;
            }
        }
    }

    let listener = listener.ok_or(format!(
        "无法绑定端口 {}-{}",
        port,
        port + PROXY_PORT_RANGE - 1
    ))?;

    logger::log_info(&format!(
        "[ApiProxy] 反向代理启动在 {}:{} (providers: {:?})",
        bind_address, actual_port, enabled_providers
    ));

    set_proxy_running(true);
    set_proxy_actual_port(Some(actual_port));

    // 创建 shutdown channel
    let (shutdown_tx, mut shutdown_rx) = watch::channel(());
    {
        let lock = PROXY_SHUTDOWN_TX.get_or_init(|| RwLock::new(None));
        if let Ok(mut tx) = lock.write() {
            *tx = Some(shutdown_tx);
        }
    }

    // 启动服务
    tokio::spawn(async move {
        let server = axum::serve(listener, app).with_graceful_shutdown(async move {
            let _ = shutdown_rx.changed().await;
        });

        if let Err(e) = server.await {
            logger::log_error(&format!("[ApiProxy] 服务异常退出: {}", e));
        }

        set_proxy_running(false);
        set_proxy_actual_port(None);
        logger::log_info("[ApiProxy] 反向代理已停止");
    });

    Ok(ProxyStatus {
        running: true,
        port: proxy_config.port,
        actual_port: Some(actual_port),
        enabled_providers,
    })
}

/// 停止代理服务
pub fn stop_proxy_server() -> Result<(), String> {
    if !is_proxy_running() {
        return Ok(());
    }

    let lock = PROXY_SHUTDOWN_TX.get_or_init(|| RwLock::new(None));
    if let Ok(mut tx_guard) = lock.write() {
        if let Some(tx) = tx_guard.take() {
            let _ = tx.send(());
        }
    }

    set_proxy_running(false);
    set_proxy_actual_port(None);
    logger::log_info("[ApiProxy] 正在停止反向代理服务...");
    Ok(())
}

/// 获取代理服务状态
pub fn get_proxy_status() -> ProxyStatus {
    let config = load_proxy_config();
    let enabled_providers: Vec<String> = config
        .providers
        .iter()
        .filter(|(_, v)| v.enabled)
        .map(|(k, _)| k.clone())
        .collect();

    ProxyStatus {
        running: is_proxy_running(),
        port: config.port,
        actual_port: get_proxy_actual_port(),
        enabled_providers,
    }
}
