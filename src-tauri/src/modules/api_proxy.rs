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

        Self {
            enabled: false,
            port: DEFAULT_PROXY_PORT,
            allow_lan_access: false,
            api_key: String::new(),
            auto_start: false,
            request_timeout: default_request_timeout(),
            providers,
            selected_account_email: String::new(),
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
    access_token: String,
    project_id: String,
    /// GCP ToS 账号走 prod 端点
    is_gcp_tos: bool,
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
    fn get_next_credential(&self, provider: &str) -> Result<AccountCredential, String> {
        let config = self.config.read().map_err(|e| format!("锁读取失败: {}", e))?;
        let provider_config = config
            .providers
            .get(provider)
            .ok_or_else(|| format!("Provider '{}' 未配置", provider))?;

        if !provider_config.enabled {
            return Err(format!("Provider '{}' 未启用", provider));
        }

        let creds = self.get_available_credentials(provider)?;

        if creds.is_empty() {
            return Err(format!("Provider '{}' 没有可用的账号", provider));
        }

        // 根据策略选择账号
        let idx = match provider_config.strategy.as_str() {
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
    fn get_available_credentials(&self, provider: &str) -> Result<Vec<AccountCredential>, String> {
        match provider {
            "antigravity" => {
                let accounts = super::account::list_accounts()
                    .map_err(|e| format!("获取账号失败: {}", e))?;
                let config = self.config.read().map_err(|e| format!("锁读取失败: {}", e))?;
                let selected_email = &config.selected_account_email;
                let creds: Vec<AccountCredential> = accounts
                    .iter()
                    .filter(|a| !a.disabled && !a.token.access_token.is_empty())
                    .filter(|a| {
                        // 如果指定了账号邮箱，只使用该账号
                        if selected_email.is_empty() {
                            true
                        } else {
                            a.email == *selected_email
                        }
                    })
                    .map(|a| AccountCredential {
                        access_token: a.token.access_token.clone(),
                        project_id: a.token.project_id.clone().unwrap_or_default(),
                        is_gcp_tos: a.token.is_gcp_tos.unwrap_or(false),
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
                        access_token: a.tokens.access_token.clone(),
                        project_id: String::new(),
                        is_gcp_tos: false,
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
    let cred = match state.get_next_credential(&provider) {
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

    // ===== Codex / 其他 Provider: 简单转发 =====
    let upstream_url = format!("{}/{}", upstream.base_url, rest);

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
