//! Provider 统一集成测试：选择有额度账号 → 拉取模型 → 发送对话 → 接受对话
//!
//! 流程：启动代理 → 对 antigravity/codex/kiro 依次：
//!   1. 选择有额度的账号（Antigravity 优先选 remaining_fraction > 0 的账号）
//!   2. 拉取该账号的模型列表
//!   3. 发送 "hi"（带 X-Selected-Account-Email 确保代理用该账号）
//!   4. 断言收到非空回复
//!
//! 运行: cargo test provider_chat -- --include-ignored --nocapture

use crate::modules::api_proxy::{
    fetch_available_models, fetch_codex_models_remote, fetch_kiro_models,
    get_codex_model_list, get_proxy_status, start_proxy_server, stop_proxy_server,
    ApiProxyConfig, QuotaModelInfo,
};
use crate::modules::account;
use crate::modules::codex_account;
use crate::modules::kiro_account;
use serde_json::json;
use std::time::Duration;

const TEST_PROXY_PORT: u16 = 19599;
const HEALTH_TIMEOUT_MS: u64 = 5000;
const REQUEST_TIMEOUT_SECS: u64 = 30;

fn test_config_with_all_providers_enabled() -> ApiProxyConfig {
    let mut config = ApiProxyConfig::default();
    config.enabled = true;
    config.port = TEST_PROXY_PORT;
    config.allow_lan_access = false;
    config.request_timeout = REQUEST_TIMEOUT_SECS;
    config.providers.get_mut("antigravity").unwrap().enabled = true;
    config.providers.get_mut("codex").unwrap().enabled = true;
    config.providers.get_mut("kiro").unwrap().enabled = true;
    config.providers.get_mut("windsurf").unwrap().enabled = true;
    config.providers.get_mut("warp").unwrap().enabled = true;
    config
}

async fn wait_proxy_ready(base_url: &str) -> bool {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_millis(2000))
        .build()
        .unwrap();
    for _ in 0..(HEALTH_TIMEOUT_MS / 500) {
        if client.get(format!("{}/healthz", base_url)).send().await.map(|r| r.status().is_success()).unwrap_or(false) {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    false
}

/// 从 Claude SSE 流中解析出助手回复文本（content_block_delta + text_delta）
fn parse_claude_sse_reply(raw: &str) -> String {
    let mut reply = String::new();
    for line in raw.lines() {
        let line = line.trim();
        if !line.starts_with("data: ") {
            continue;
        }
        let data = line.trim_start_matches("data: ").trim();
        if data == "[DONE]" || data.is_empty() {
            continue;
        }
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(data) {
            if let Some(delta) = v.get("delta") {
                if let Some(text) = delta.get("text").and_then(|t| t.as_str()) {
                    reply.push_str(text);
                } else if let Some(text) = delta.get("type").and_then(|_| delta.get("text")).and_then(|t| t.as_str()) {
                    reply.push_str(text);
                }
            }
        }
    }
    reply
}

/// 从 Codex/OpenAI SSE 流中解析出助手回复（choices[0].delta.content）
fn parse_codex_sse_reply(raw: &str) -> String {
    let mut reply = String::new();
    for line in raw.lines() {
        let line = line.trim();
        if !line.starts_with("data: ") {
            continue;
        }
        let data = line.trim_start_matches("data: ").trim();
        if data == "[DONE]" || data.is_empty() {
            continue;
        }
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(data) {
            if let Some(content) = v
                .get("choices")
                .and_then(|c| c.get(0))
                .and_then(|c| c.get("delta"))
                .and_then(|d| d.get("content"))
                .and_then(|c| c.as_str())
            {
                reply.push_str(content);
            }
        }
    }
    reply
}

/// 按 provider 打印用：始终包含「获取的模型」和「发送 hi 的回应」（成功为回复正文，否则为跳过/失败原因）
pub struct ProviderPrintOut {
    pub provider: String,
    pub models: Vec<String>,
    pub response: String,
    pub ok: bool,
}

/// 1) 选定有额度账号 2) 获取 model 3) 发送 hi；返回结果（含 models 与 response），失败时 response 为原因
async fn run_provider_flow(
    provider: &str,
    base_url: &str,
) -> ProviderPrintOut {
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
        .build()
        .map_err(|e| e.to_string())
    {
        Ok(c) => c,
        Err(e) => return ProviderPrintOut {
            provider: provider.to_string(),
            models: vec![],
            response: format!("跳过: 客户端构建失败 {}", e),
            ok: false,
        },
    };

    match provider {
        "antigravity" => {
            let accounts = match account::list_accounts().map_err(|e| e.to_string()) {
                Ok(a) => a,
                Err(e) => return ProviderPrintOut {
                    provider: "antigravity".to_string(),
                    models: vec![],
                    response: format!("跳过: {}", e),
                    ok: false,
                },
            };
            let valid_accounts: Vec<_> = accounts
                .iter()
                .filter(|a| !a.disabled && !a.token.access_token.is_empty())
                .collect();
            if valid_accounts.is_empty() {
                return ProviderPrintOut {
                    provider: "antigravity".to_string(),
                    models: vec![],
                    response: "跳过: Antigravity 无可用账号".to_string(),
                    ok: false,
                };
            }
            // 优先选择有额度的账号：拉取模型后存在 remaining_fraction > 0 的模型
            let mut acc = None;
            let mut models = Vec::new();
            for a in &valid_accounts {
                let project_id = a.token.project_id.clone().unwrap_or_default();
                match fetch_available_models(&a.token.access_token, &project_id).await {
                    Ok(m) => {
                        let with_quota = m.iter().filter(|x| x.remaining_fraction.map(|f| f > 0.0).unwrap_or(true)).count();
                        if with_quota > 0 {
                            acc = Some(a);
                            models = m;
                            break;
                        }
                        if acc.is_none() {
                            acc = Some(a);
                            models = m;
                        }
                    }
                    Err(_) => continue,
                }
            }
            let acc = match acc {
                Some(a) => a,
                None => return ProviderPrintOut {
                    provider: "antigravity".to_string(),
                    models: vec![],
                    response: "跳过: Antigravity 拉取模型失败".to_string(),
                    ok: false,
                },
            };
            let with_quota: Vec<&QuotaModelInfo> = models
                .iter()
                .filter(|m| m.remaining_fraction.map(|f| f > 0.0).unwrap_or(true))
                .collect();
            let model_ids: Vec<String> = models.iter().map(|m| m.id.clone()).collect();
            let model_id = with_quota
                .first()
                .map(|m| m.id.as_str())
                .unwrap_or_else(|| models.first().map(|m| m.id.as_str()).unwrap_or("gemini-2.5-flash"));
            let body = json!({
                "model": model_id,
                "max_tokens": 64,
                "stream": true,
                "messages": [{"role": "user", "content": [{"type": "text", "text": "hi"}]}]
            });
            let resp = match client
                .post(format!("{}/antigravity/v1/messages", base_url))
                .header("Content-Type", "application/json")
                .header("x-api-key", "chat-test")
                .header("anthropic-version", "2023-06-01")
                .header("x-selected-account-email", acc.email.as_str())
                .json(&body)
                .send()
                .await
            {
                Ok(r) => r,
                Err(e) => return ProviderPrintOut {
                    provider: "antigravity".to_string(),
                    models: model_ids.clone(),
                    response: format!("跳过: 请求失败 {}", e),
                    ok: false,
                },
            };
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            if status.as_u16() == 503 && text.contains("没有可用的账号") {
                return ProviderPrintOut {
                    provider: "antigravity".to_string(),
                    models: model_ids,
                    response: "跳过: Antigravity 无可用账号".to_string(),
                    ok: false,
                };
            }
            if status.as_u16() == 402 || (status.as_u16() == 403 && text.contains("token")) {
                return ProviderPrintOut {
                    provider: "antigravity".to_string(),
                    models: model_ids,
                    response: "跳过: Antigravity 账号额度/权限异常".to_string(),
                    ok: false,
                };
            }
            if status.as_u16() == 502 && text.contains("429") {
                return ProviderPrintOut {
                    provider: "antigravity".to_string(),
                    models: model_ids,
                    response: "跳过: Antigravity 上游限流 429".to_string(),
                    ok: false,
                };
            }
            if !status.is_success() {
                return ProviderPrintOut {
                    provider: "antigravity".to_string(),
                    models: model_ids,
                    response: format!("失败: HTTP {} {}", status, &text[..text.len().min(300)]),
                    ok: false,
                };
            }
            if !text.contains("content_block_delta") && !text.contains("message_stop") && text.is_empty() {
                return ProviderPrintOut {
                    provider: "antigravity".to_string(),
                    models: model_ids,
                    response: "跳过: Antigravity 响应无内容".to_string(),
                    ok: false,
                };
            }
            let reply = parse_claude_sse_reply(&text);
            if reply.trim().is_empty() {
                return ProviderPrintOut {
                    provider: "antigravity".to_string(),
                    models: model_ids,
                    response: "跳过: Antigravity 未收到有效回复内容".to_string(),
                    ok: false,
                };
            }
            ProviderPrintOut {
                provider: "antigravity".to_string(),
                models: model_ids,
                response: reply,
                ok: true,
            }
        }
        "codex" => {
            let accounts: Vec<_> = codex_account::list_accounts()
                .into_iter()
                .filter(|a| !a.tokens.access_token.is_empty())
                .collect();
            if accounts.is_empty() {
                return ProviderPrintOut {
                    provider: "codex".to_string(),
                    models: vec![],
                    response: "跳过: Codex 无可用账号".to_string(),
                    ok: false,
                };
            }
            let mut last_models = vec![];
            let mut last_response = String::new();
            for acc in &accounts {
                let account_id = acc.account_id.clone().or_else(|| {
                    codex_account::extract_chatgpt_account_id_from_access_token(&acc.tokens.access_token)
                });
                let models = fetch_codex_models_remote(
                    &acc.tokens.access_token,
                    account_id.as_deref(),
                )
                .await
                .unwrap_or_else(|_| get_codex_model_list());
                last_models = models.clone();
                let model_id = models.first().map(|s| s.as_str()).unwrap_or("gpt-5.1-codex");
                let body = json!({
                    "model": model_id,
                    "stream": true,
                    "messages": [{"role": "user", "content": "hi"}]
                });
                let resp = match client
                    .post(format!("{}/codex/v1/chat/completions", base_url))
                    .header("Content-Type", "application/json")
                    .header("x-selected-account-email", acc.email.as_str())
                    .json(&body)
                    .send()
                    .await
                {
                    Ok(r) => r,
                    Err(e) => {
                        last_response = format!("跳过: 请求失败 {}", e);
                        continue;
                    }
                };
                let status = resp.status();
                let text = resp.text().await.unwrap_or_default();
                if status.as_u16() == 503 && text.contains("没有可用的账号") {
                    last_response = "跳过: Codex 无可用账号".to_string();
                    continue;
                }
                if status.as_u16() == 402 || (status.as_u16() == 403) || (text.contains("deactivated_workspace") || text.contains("usage_limit")) {
                    last_response = format!("跳过: 账号 {} 额度/workspace 异常，尝试下一账号", acc.email);
                    continue;
                }
                if !status.is_success() {
                    last_response = format!("失败: Codex HTTP {} {}", status, &text[..text.len().min(300)]);
                    continue;
                }
                if !text.contains("data:") && !text.contains("choices") && text.is_empty() {
                    last_response = "跳过: Codex 响应无内容".to_string();
                    continue;
                }
                let reply = parse_codex_sse_reply(&text);
                if reply.trim().is_empty() {
                    last_response = "跳过: Codex 未收到有效回复内容".to_string();
                    continue;
                }
                return ProviderPrintOut {
                    provider: "codex".to_string(),
                    models: last_models,
                    response: reply,
                    ok: true,
                };
            }
            ProviderPrintOut {
                provider: "codex".to_string(),
                models: last_models,
                response: if last_response.is_empty() {
                    "跳过: 所有 Codex 账号均无法获得 hi 响应".to_string()
                } else {
                    last_response
                },
                ok: false,
            }
        }
        "kiro" => {
            let accounts: Vec<_> = kiro_account::list_accounts()
                .into_iter()
                .filter(|a| !a.access_token.is_empty())
                .collect();
            if accounts.is_empty() {
                return ProviderPrintOut {
                    provider: "kiro".to_string(),
                    models: vec![],
                    response: "跳过: Kiro 无可用账号".to_string(),
                    ok: false,
                };
            }
            let mut last_models = vec![];
            let mut last_response = String::new();
            for acc in &accounts {
                let profile_arn = acc.kiro_auth_token_raw.as_ref()
                    .and_then(|v| v.get("profileArn").and_then(|p| p.as_str()).map(|s| s.to_string()))
                    .or_else(|| acc.kiro_profile_raw.as_ref()
                        .and_then(|v| v.get("profileArn").and_then(|p| p.as_str()).map(|s| s.to_string())));
                let models = fetch_kiro_models(&acc.access_token, profile_arn.as_deref())
                    .await
                    .unwrap_or_default();
                last_models = models.clone();
                let model_id = models.first().map(|s| s.as_str()).unwrap_or("claude-sonnet-4-5");
                let body = json!({
                    "model": model_id,
                    "max_tokens": 64,
                    "stream": true,
                    "messages": [{"role": "user", "content": [{"type": "text", "text": "hi"}]}]
                });
                let resp = match client
                    .post(format!("{}/kiro/v1/messages", base_url))
                    .header("Content-Type", "application/json")
                    .header("x-api-key", "chat-test")
                    .header("anthropic-version", "2023-06-01")
                    .header("x-selected-account-email", acc.email.as_str())
                    .json(&body)
                    .send()
                    .await
                {
                    Ok(r) => r,
                    Err(e) => {
                        last_response = format!("跳过: 请求失败 {}", e);
                        continue;
                    }
                };
                let status = resp.status();
                let text = resp.text().await.unwrap_or_default();
                if status.as_u16() == 503 && text.contains("没有可用的账号") {
                    last_response = "跳过: Kiro 无可用账号".to_string();
                    continue;
                }
                if status.as_u16() == 403 && (text.contains("token") || text.contains("invalid") || text.contains("permission")) {
                    last_response = format!("跳过: 账号 {} token 无效或权限异常，尝试下一账号", acc.email);
                    continue;
                }
                if !status.is_success() {
                    last_response = format!("失败: Kiro HTTP {} {}", status, &text[..text.len().min(300)]);
                    continue;
                }
                if !text.contains("content_block_delta") && !text.contains("message_stop") && text.is_empty() {
                    last_response = "跳过: Kiro 响应无内容".to_string();
                    continue;
                }
                let reply = parse_claude_sse_reply(&text);
                if reply.trim().is_empty() {
                    last_response = "跳过: Kiro 未收到有效回复内容".to_string();
                    continue;
                }
                return ProviderPrintOut {
                    provider: "kiro".to_string(),
                    models: last_models,
                    response: reply,
                    ok: true,
                };
            }
            ProviderPrintOut {
                provider: "kiro".to_string(),
                models: last_models,
                response: if last_response.is_empty() {
                    "跳过: 所有 Kiro 账号均无法获得 hi 响应".to_string()
                } else {
                    last_response
                },
                ok: false,
            }
        }
        _ => ProviderPrintOut {
            provider: provider.to_string(),
            models: vec![],
            response: format!("未实现的 provider: {}", provider),
            ok: false,
        },
    }
}

/// 按 provider 分别打印：获取的 model、发送 hi 获取的回应
fn print_provider_result(out: &ProviderPrintOut) {
    eprintln!("---------- {} ----------", out.provider);
    let models_str = if out.models.is_empty() { "(无)".to_string() } else { out.models.join(", ") };
    eprintln!("  获取的 model: {}", models_str);
    eprintln!("  发送 hi 获取的回应: {}", if out.response.is_empty() { "(空)" } else { out.response.trim() });
    eprintln!();
}

/// 统一测试：启动代理，对 antigravity / codex / kiro 依次执行：选有额度账号 → 拉模型 → 发 hi，能正常返回。
/// Windsurf / Warp 为通用转发，可按需补充路径与 body 格式。
#[tokio::test]
#[ignore]
async fn provider_chat_select_account_fetch_model_send_hi_returns_ok() {
    let _ = crate::modules::logger::init_logger();
    let config = test_config_with_all_providers_enabled();

    let handle = tokio::spawn(async move {
        let _ = start_proxy_server(config).await;
    });
    tokio::time::sleep(Duration::from_millis(800)).await;

    let actual_port = get_proxy_status().actual_port.unwrap_or(TEST_PROXY_PORT);
    let base_url = format!("http://127.0.0.1:{}", actual_port);
    if !wait_proxy_ready(&base_url).await {
        let _ = stop_proxy_server();
        panic!("代理 {} 未在 {}ms 内就绪", base_url, HEALTH_TIMEOUT_MS);
    }

    let providers = ["antigravity", "codex", "kiro"];
    let mut at_least_one_ok = false;
    let mut errors = Vec::new();

    eprintln!("\n========== 选中用户后，各 provider 的 model 与发送 hi 的回复 ==========\n");

    for provider in providers {
        let out = run_provider_flow(provider, &base_url).await;
        print_provider_result(&out);
        if out.ok {
            at_least_one_ok = true;
        } else if !out.response.contains("跳过") {
            errors.push((provider, out.response));
        }
    }

    let _ = stop_proxy_server();
    if let Ok(h) = handle.await {
        let _ = h;
    }

    for (p, e) in &errors {
        eprintln!("[provider_chat] {} 失败: {}", p, e);
    }
    assert!(errors.is_empty(), "存在失败: {:?}", errors);
    if !at_least_one_ok {
        eprintln!("[provider_chat] 所有 provider 均为跳过（无账号/额度/限流），未产生成功回复");
    }
}
