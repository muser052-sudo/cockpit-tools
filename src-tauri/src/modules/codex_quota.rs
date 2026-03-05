use crate::models::codex::{CodexAccount, CodexQuota, CodexQuotaErrorInfo};
use crate::modules::{codex_account, logger};
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, AUTHORIZATION};


// 使用 wham/usage 端点（Quotio 使用的）
const USAGE_URL: &str = "https://chatgpt.com/backend-api/codex/responses";

fn get_header_value(headers: &HeaderMap, name: &str) -> String {
    headers
        .get(name)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("-")
        .to_string()
}

fn extract_detail_code_from_body(body: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(body).ok()?;

    if let Some(code) = value
        .get("detail")
        .and_then(|detail| detail.get("code"))
        .and_then(|code| code.as_str())
    {
        return Some(code.to_string());
    }

    if let Some(code) = value.get("code").and_then(|code| code.as_str()) {
        return Some(code.to_string());
    }

    None
}

fn extract_error_code_from_message(message: &str) -> Option<String> {
    let marker = "[error_code:";
    let start = message.find(marker)?;
    let code_start = start + marker.len();
    let end = message[code_start..].find(']')?;
    Some(message[code_start..code_start + end].to_string())
}

fn write_quota_error(account: &mut CodexAccount, message: String) {
    account.quota_error = Some(CodexQuotaErrorInfo {
        code: extract_error_code_from_message(&message),
        message,
        timestamp: chrono::Utc::now().timestamp(),
    });
}

/// 查询单个账号的配额
pub async fn fetch_quota(account: &CodexAccount) -> Result<CodexQuota, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| format!("构建HTTP客户端失败: {}", e))?;

    let mut headers = HeaderMap::new();
    headers.insert(
        AUTHORIZATION,
        HeaderValue::from_str(&format!("Bearer {}", account.tokens.access_token))
            .map_err(|e| format!("构建 Authorization 头失败: {}", e))?,
    );
    headers.insert(ACCEPT, HeaderValue::from_static("application/json"));

    // 添加 ChatGPT-Account-Id 头（关键！）
    let account_id = account.account_id.clone().or_else(|| {
        codex_account::extract_chatgpt_account_id_from_access_token(&account.tokens.access_token)
    });

    if let Some(ref acc_id) = account_id {
        if !acc_id.is_empty() {
            headers.insert(
                "ChatGPT-Account-Id",
                HeaderValue::from_str(acc_id)
                    .map_err(|e| format!("构建 Account-Id 头失败: {}", e))?,
            );
        }
    }

    logger::log_info(&format!(
        "Codex 配额请求: {} (account_id: {:?})",
        USAGE_URL, account_id
    ));

    let mut retries = 0;
    let max_retries = 2;
    let mut response = None;
    let mut last_error = String::new();

    let test_payload = serde_json::json!({
        "model": "gpt-5.1-codex",
        "input": [
            {
                "role": "user",
                "content": [
                    {
                        "type": "input_text",
                        "text": "hi"
                    }
                ]
            }
        ],
        "stream": true,
        "store": false,
        "instructions": "You are a helpful AI assistant."
    });

    while retries <= max_retries {
        match client
            .post(USAGE_URL)
            .headers(headers.clone())
            .json(&test_payload)
            .send()
            .await
        {
            Ok(res) => {
                response = Some(res);
                break;
            }
            Err(e) => {
                last_error = e.to_string();
                logger::log_warn(&format!("Codex 配额请求失败 (第 {} 次尝试): {}", retries + 1, e));
                retries += 1;
                if retries <= max_retries {
                    tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
                }
            }
        }
    }

    let response = response.ok_or_else(|| format!("请求失败 (已重试 {} 次): {}", max_retries, last_error))?;

    let status = response.status();
    let headers = response.headers().clone();
    let body = response
        .text()
        .await
        .map_err(|e| format!("读取响应失败: {}", e))?;

    let request_id = get_header_value(&headers, "request-id");
    let x_request_id = get_header_value(&headers, "x-request-id");
    let cf_ray = get_header_value(&headers, "cf-ray");
    let body_len = body.len();

    logger::log_info(&format!(
        "Codex 配额响应元信息: url={}, status={}, request-id={}, x-request-id={}, cf-ray={}, body_len={}",
        USAGE_URL, status, request_id, x_request_id, cf_ray, body_len
    ));

    // 无论响应状体如何，只要 headers 里有 x-codex-* 就尝试解析
    let primary_used = get_header_value(&headers, "x-codex-primary-used-percent").parse::<f64>().ok();
    let primary_reset = get_header_value(&headers, "x-codex-primary-reset-after-seconds").parse::<i64>().ok();
    let primary_window = get_header_value(&headers, "x-codex-primary-window-minutes").parse::<i64>().ok();
    
    let secondary_used = get_header_value(&headers, "x-codex-secondary-used-percent").parse::<f64>().ok();
    let secondary_reset = get_header_value(&headers, "x-codex-secondary-reset-after-seconds").parse::<i64>().ok();
    let secondary_window = get_header_value(&headers, "x-codex-secondary-window-minutes").parse::<i64>().ok();

    if !status.is_success() && primary_used.is_none() && secondary_used.is_none() {
        let detail_code = extract_detail_code_from_body(&body);

        logger::log_error(&format!(
            "Codex 配额接口返回非成功状态: url={}, status={}, request-id={}, x-request-id={}, cf-ray={}, detail_code={:?}, body={}",
            USAGE_URL, status, request_id, x_request_id, cf_ray, detail_code, body
        ));

        let body_preview = if body.len() > 200 {
            &body[..200]
        } else {
            &body
        };
        let mut error_message = format!("API 返回错误 {}", status);
        if let Some(code) = detail_code {
            error_message.push_str(&format!(" [error_code:{}]", code));
        }
        error_message.push_str(&format!(" - {}", body_preview));
        return Err(error_message);
    }

    // 根据窗口大小（分钟）来决定哪个是 5小时（<=360）和 7天
    let mut use_5h_from_primary = false;
    let mut use_7d_from_primary = false;

    if let (Some(p_min), Some(s_min)) = (primary_window, secondary_window) {
        if p_min < s_min {
            use_5h_from_primary = true;
        } else {
            use_7d_from_primary = true;
        }
    } else if let Some(p_min) = primary_window {
        if p_min <= 360 {
            use_5h_from_primary = true;
        } else {
            use_7d_from_primary = true;
        }
    } else if let Some(s_min) = secondary_window {
        if s_min <= 360 {
            use_7d_from_primary = true;
        } else {
            use_5h_from_primary = true;
        }
    } else {
        use_7d_from_primary = true; // 默认
    }

    let (used_5h, reset_5h, window_5h, used_7d, reset_7d, window_7d) = if use_5h_from_primary {
        (primary_used, primary_reset, primary_window, secondary_used, secondary_reset, secondary_window)
    } else if use_7d_from_primary {
        (secondary_used, secondary_reset, secondary_window, primary_used, primary_reset, primary_window)
    } else {
        (None, None, None, None, None, None)
    };

    let hourly_percentage = used_5h.map(|v| (100.0 - v).max(0.0).round() as i32).unwrap_or(100);
    let hourly_reset_time = reset_5h.map(|s| {
        chrono::Utc::now().timestamp() + s
    });

    let weekly_percentage = used_7d.map(|v| (100.0 - v).max(0.0).round() as i32).unwrap_or(100);
    let weekly_reset_time = reset_7d.map(|s| {
        chrono::Utc::now().timestamp() + s
    });

    let raw_data = serde_json::json!({
        "primary_used_percent": primary_used,
        "primary_reset_after_seconds": primary_reset,
        "primary_window_minutes": primary_window,
        "secondary_used_percent": secondary_used,
        "secondary_reset_after_seconds": secondary_reset,
        "secondary_window_minutes": secondary_window,
        "response_body": if status.is_success() { "success_stream_omitted" } else { &body }
    });

    Ok(CodexQuota {
        hourly_percentage,
        hourly_reset_time,
        hourly_window_minutes: window_5h,
        hourly_window_present: Some(window_5h.is_some()),
        weekly_percentage,
        weekly_reset_time,
        weekly_window_minutes: window_7d,
        weekly_window_present: Some(window_7d.is_some()),
        raw_data: Some(raw_data),
    })
}

/// 刷新账号配额并保存（包含 token 自动刷新）
pub async fn refresh_account_quota(account_id: &str) -> Result<CodexQuota, String> {
    let mut account = codex_account::load_account(account_id)
        .ok_or_else(|| format!("账号不存在: {}", account_id))?;

    // 检查 token 是否过期，如果过期则刷新
    if crate::modules::codex_oauth::is_token_expired(&account.tokens.access_token) {
        logger::log_info(&format!("账号 {} 的 Token 已过期，尝试刷新", account.email));

        if let Some(ref refresh_token) = account.tokens.refresh_token {
            match crate::modules::codex_oauth::refresh_access_token(refresh_token).await {
                Ok(new_tokens) => {
                    logger::log_info(&format!("账号 {} 的 Token 刷新成功", account.email));
                    account.tokens = new_tokens;
                    codex_account::save_account(&account)?;
                }
                Err(e) => {
                    logger::log_error(&format!("账号 {} Token 刷新失败: {}", account.email, e));
                    let message = format!("Token 已过期且刷新失败: {}", e);
                    write_quota_error(&mut account, message.clone());
                    if let Err(save_err) = codex_account::save_account(&account) {
                        logger::log_warn(&format!("写入 Codex 配额错误失败: {}", save_err));
                    }
                    return Err(message);
                }
            }
        } else {
            let message = "Token 已过期且无 refresh_token".to_string();
            write_quota_error(&mut account, message.clone());
            if let Err(save_err) = codex_account::save_account(&account) {
                logger::log_warn(&format!("写入 Codex 配额错误失败: {}", save_err));
            }
            return Err(message);
        }
    }

    let quota = match fetch_quota(&account).await {
        Ok(quota) => quota,
        Err(e) => {
            write_quota_error(&mut account, e.clone());
            if let Err(save_err) = codex_account::save_account(&account) {
                logger::log_warn(&format!("写入 Codex 配额错误失败: {}", save_err));
            }
            return Err(e);
        }
    };

    account.quota = Some(quota.clone());
    account.quota_error = None;
    codex_account::save_account(&account)?;

    Ok(quota)
}

/// 刷新所有账号配额
pub async fn refresh_all_quotas() -> Result<Vec<(String, Result<CodexQuota, String>)>, String> {
    use futures::future::join_all;
    use std::sync::Arc;
    use tokio::sync::Semaphore;

    const MAX_CONCURRENT: usize = 5;
    let accounts = codex_account::list_accounts();

    let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT));
    let tasks: Vec<_> = accounts
        .into_iter()
        .map(|account| {
            let account_id = account.id;
            let semaphore = semaphore.clone();
            async move {
                let _permit = semaphore
                    .acquire_owned()
                    .await
                    .map_err(|e| format!("获取 Codex 刷新并发许可失败: {}", e))?;
                let result = refresh_account_quota(&account_id).await;
                Ok::<(String, Result<CodexQuota, String>), String>((account_id, result))
            }
        })
        .collect();

    let mut results = Vec::with_capacity(tasks.len());
    for task in join_all(tasks).await {
        match task {
            Ok(item) => results.push(item),
            Err(err) => return Err(err),
        }
    }

    Ok(results)
}
