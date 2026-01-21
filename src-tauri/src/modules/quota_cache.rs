use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::models::{Account, QuotaData};
use crate::modules;

const CACHE_DIR: &str = "cache/quota";
const CACHE_VERSION: u8 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct QuotaCacheModel {
    id: String,
    display_name: Option<String>,
    remaining_percentage: Option<i32>,
    remaining_fraction: Option<f64>,
    reset_time: Option<String>,
    is_recommended: Option<bool>,
    tag_title: Option<String>,
    supports_images: Option<bool>,
    supported_mime_types: Option<std::collections::HashMap<String, bool>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct QuotaCacheRecord {
    version: u8,
    source: String,
    email: Option<String>,
    updated_at: i64,
    subscription_tier: Option<String>,
    is_forbidden: Option<bool>,
    models: Vec<QuotaCacheModel>,
}

fn hash_email(email: &str) -> String {
    let normalized = email.trim().to_lowercase();
    let mut hasher = Sha256::new();
    hasher.update(normalized.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn cache_dir(source: &str) -> Result<PathBuf, String> {
    let data_dir = modules::account::get_data_dir()?;
    let dir = data_dir.join(CACHE_DIR).join(source);
    if !dir.exists() {
        fs::create_dir_all(&dir).map_err(|e| format!("Failed to create quota cache dir: {}", e))?;
    }
    Ok(dir)
}

fn cache_path(source: &str, email: &str) -> Result<PathBuf, String> {
    let dir = cache_dir(source)?;
    Ok(dir.join(format!("{}.json", hash_email(email))))
}

pub(crate) fn read_quota_cache(source: &str, email: &str) -> Option<QuotaCacheRecord> {
    let path = cache_path(source, email).ok()?;
    let content = fs::read_to_string(path).ok()?;
    let record = serde_json::from_str::<QuotaCacheRecord>(&content).ok()?;
    if record.version != CACHE_VERSION {
        return None;
    }
    if record.source != source {
        return None;
    }
    Some(record)
}

pub fn write_quota_cache(source: &str, email: &str, quota: &QuotaData) -> Result<(), String> {
    // 容错：如果 models 为空，不写入缓存，避免覆盖已有的有效缓存
    if quota.models.is_empty() {
        return Ok(());
    }
    
    let path = cache_path(source, email)?;
    let temp_path = path.with_extension("json.tmp");
    let updated_at = chrono::Utc::now().timestamp_millis();

    let models = quota
        .models
        .iter()
        .map(|model| QuotaCacheModel {
            id: model.name.clone(),
            display_name: None,
            remaining_percentage: Some(model.percentage),
            remaining_fraction: Some(model.percentage as f64 / 100.0),
            reset_time: Some(model.reset_time.clone()),
            is_recommended: None,
            tag_title: None,
            supports_images: None,
            supported_mime_types: None,
        })
        .collect::<Vec<_>>();

    let record = QuotaCacheRecord {
        version: CACHE_VERSION,
        source: source.to_string(),
        email: Some(email.to_string()),
        updated_at,
        subscription_tier: quota.subscription_tier.clone(),
        is_forbidden: Some(quota.is_forbidden),
        models,
    };

    let content = serde_json::to_string_pretty(&record)
        .map_err(|e| format!("Failed to serialize quota cache: {}", e))?;
    fs::write(&temp_path, content)
        .map_err(|e| format!("Failed to write quota cache: {}", e))?;
    fs::rename(temp_path, path)
        .map_err(|e| format!("Failed to save quota cache: {}", e))?;
    Ok(())
}

pub fn apply_cached_quota(account: &mut Account, source: &str) -> Result<bool, String> {
    let record = match read_quota_cache(source, &account.email) {
        Some(record) => record,
        None => return Ok(false),
    };

    let cache_updated = record.updated_at / 1000;
    let current_updated = account
        .quota
        .as_ref()
        .map(|quota| quota.last_updated)
        .unwrap_or(0);

    if current_updated >= cache_updated && account.quota.is_some() {
        return Ok(false);
    }

    let mut quota = QuotaData::new();
    quota.last_updated = cache_updated;
    quota.subscription_tier = record.subscription_tier.clone();
    quota.is_forbidden = record.is_forbidden.unwrap_or(false);

    for model in record.models {
        let name = model.id;
        let percentage = model.remaining_percentage.unwrap_or_else(|| {
            model.remaining_fraction
                .map(|value| (value * 100.0).round() as i32)
                .unwrap_or(0)
        });
        let reset_time = model.reset_time.unwrap_or_default();

        if name.contains("gemini") || name.contains("claude") {
            quota.add_model(name, percentage, reset_time);
        }
    }

    // 容错：如果缓存的 models 为空，但账号已有配额数据，保留原有 models
    if quota.models.is_empty() {
        if let Some(ref existing_quota) = account.quota {
            if !existing_quota.models.is_empty() {
                // 只更新非 models 字段
                let mut merged_quota = existing_quota.clone();
                merged_quota.subscription_tier = quota.subscription_tier.clone();
                merged_quota.is_forbidden = quota.is_forbidden;
                // 不更新 last_updated，保留原有的时间戳
                account.update_quota(merged_quota);
                return Ok(true);
            }
        }
    }

    account.update_quota(quota);
    Ok(true)
}
