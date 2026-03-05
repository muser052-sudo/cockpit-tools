use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

use crate::models::warp::{WarpAccount, WarpAccountIndex};
use crate::modules::account;

const ACCOUNTS_INDEX_FILE: &str = "warp_accounts.json";
const ACCOUNTS_DIR: &str = "warp_accounts";

lazy_static::lazy_static! {
    static ref WARP_ACCOUNT_INDEX_LOCK: Mutex<()> = Mutex::new(());
}

fn get_data_dir() -> Result<PathBuf, String> {
    account::get_data_dir()
}

fn get_accounts_dir() -> Result<PathBuf, String> {
    let base = get_data_dir()?;
    let dir = base.join(ACCOUNTS_DIR);
    if !dir.exists() {
        fs::create_dir_all(&dir).map_err(|e| format!("创建 Warp 账号目录失败: {}", e))?;
    }
    Ok(dir)
}

fn get_accounts_index_path() -> Result<PathBuf, String> {
    Ok(get_data_dir()?.join(ACCOUNTS_INDEX_FILE))
}

fn resolve_account_file_path(account_id: &str) -> Result<PathBuf, String> {
    if account_id.contains('/') || account_id.contains('\\') || account_id.contains("..") {
        return Err("账号 ID 非法".to_string());
    }
    Ok(get_accounts_dir()?.join(format!("{}.json", account_id)))
}

pub fn load_account(account_id: &str) -> Option<WarpAccount> {
    let path = resolve_account_file_path(account_id).ok()?;
    if !path.exists() {
        return None;
    }
    let content = fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

fn save_account_file(account: &WarpAccount) -> Result<(), String> {
    let path = resolve_account_file_path(&account.id)?;
    let content = serde_json::to_string_pretty(account).map_err(|e| format!("序列化失败: {}", e))?;
    fs::write(path, content).map_err(|e| format!("保存失败: {}", e))
}

fn delete_account_file(account_id: &str) -> Result<(), String> {
    let path = resolve_account_file_path(account_id)?;
    if path.exists() {
        fs::remove_file(path).map_err(|e| format!("删除失败: {}", e))?;
    }
    Ok(())
}

fn load_account_index() -> WarpAccountIndex {
    let path = match get_accounts_index_path() {
        Ok(p) => p,
        Err(_) => return WarpAccountIndex::new(),
    };
    if !path.exists() {
        return WarpAccountIndex::new();
    }
    match fs::read_to_string(path) {
        Ok(c) => serde_json::from_str(&c).unwrap_or_else(|_| WarpAccountIndex::new()),
        Err(_) => WarpAccountIndex::new(),
    }
}

fn save_account_index(index: &WarpAccountIndex) -> Result<(), String> {
    let path = get_accounts_index_path()?;
    let content = serde_json::to_string_pretty(index).map_err(|e| format!("序列化失败: {}", e))?;
    fs::write(path, content).map_err(|e| format!("保存失败: {}", e))
}

pub fn list_accounts() -> Vec<WarpAccount> {
    let index = load_account_index();
    let mut accounts = Vec::new();
    for summary in index.accounts {
        if let Some(account) = load_account(&summary.id) {
            accounts.push(account);
        }
    }
    accounts
}

pub fn upsert_account(account: WarpAccount) -> Result<WarpAccount, String> {
    let _lock = WARP_ACCOUNT_INDEX_LOCK.lock().unwrap();
    let mut index = load_account_index();
    
    save_account_file(&account)?;
    
    if let Some(summary) = index.accounts.iter_mut().find(|a| a.id == account.id) {
        *summary = account.summary();
    } else {
        index.accounts.push(account.summary());
    }
    
    save_account_index(&index)?;
    Ok(account)
}

pub fn delete_accounts(account_ids: &[String]) -> Result<(), String> {
    let _lock = WARP_ACCOUNT_INDEX_LOCK.lock().unwrap();
    let mut index = load_account_index();
    
    for id in account_ids {
        index.accounts.retain(|a| &a.id != id);
        let _ = delete_account_file(id);
    }
    save_account_index(&index)?;
    Ok(())
}

pub fn update_tags(account_id: &str, tags: Vec<String>) -> Result<WarpAccount, String> {
    let mut account = load_account(account_id).ok_or_else(|| "Warp 账号不存在".to_string())?;
    
    let mut clean_tags = Vec::new();
    let mut seen = HashSet::new();
    for t in tags {
        let trimmed = t.trim().to_lowercase();
        if !trimmed.is_empty() && seen.insert(trimmed.clone()) {
            clean_tags.push(trimmed);
        }
    }
    
    account.tags = if clean_tags.is_empty() { None } else { Some(clean_tags) };
    upsert_account(account)
}
