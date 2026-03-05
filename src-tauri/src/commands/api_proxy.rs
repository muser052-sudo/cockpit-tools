//! API 反向代理 Tauri 命令

use crate::modules::api_proxy::{
    self, ApiProxyConfig, ProxyStatus,
};

/// 获取 API 代理配置
#[tauri::command]
pub fn get_api_proxy_config() -> ApiProxyConfig {
    api_proxy::load_proxy_config()
}

/// 保存 API 代理配置
#[tauri::command]
pub fn save_api_proxy_config(config: ApiProxyConfig) -> Result<(), String> {
    api_proxy::save_proxy_config(&config)
}

/// 获取代理服务状态
#[tauri::command]
pub fn get_api_proxy_status() -> ProxyStatus {
    api_proxy::get_proxy_status()
}

/// 启动代理服务
#[tauri::command]
pub async fn start_api_proxy() -> Result<ProxyStatus, String> {
    let config = api_proxy::load_proxy_config();
    api_proxy::start_proxy_server(config).await
}

/// 停止代理服务
#[tauri::command]
pub fn stop_api_proxy() -> Result<(), String> {
    api_proxy::stop_proxy_server()
}

/// 重启代理服务
#[tauri::command]
pub async fn restart_api_proxy() -> Result<ProxyStatus, String> {
    // 先停止
    let _ = api_proxy::stop_proxy_server();
    // 等一下让端口释放
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    // 重新启动
    let config = api_proxy::load_proxy_config();
    api_proxy::start_proxy_server(config).await
}

/// 获取指定账号可用的模型列表及配额状态
#[tauri::command]
pub async fn fetch_models_for_account(email: String) -> Result<Vec<api_proxy::QuotaModelInfo>, String> {
    let accounts = crate::modules::account::list_accounts()
        .map_err(|e| format!("获取账号失败: {}", e))?;
    let account = accounts.iter()
        .find(|a| a.email == email && !a.disabled)
        .ok_or_else(|| format!("未找到账号: {}", email))?;

    let access_token = &account.token.access_token;
    if access_token.is_empty() {
        return Err("账号 access_token 为空".to_string());
    }
    let project_id = account.token.project_id.clone().unwrap_or_default();

    api_proxy::fetch_available_models(access_token, &project_id).await
}

/// 获取 Codex (OpenAI) 的可用模型列表
#[tauri::command]
pub fn fetch_codex_models() -> Vec<String> {
    api_proxy::get_codex_model_list()
}
