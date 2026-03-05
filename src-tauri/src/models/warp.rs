use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WarpAccount {
    pub id: String,
    pub email: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,

    // 凭据字段
    pub auth_token: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<i64>,

    // 配额/计划状态信息
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quota_status: Option<serde_json::Value>,

    // 存储元数据
    pub created_at: i64,
    pub last_used: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WarpAccountSummary {
    pub id: String,
    pub email: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan_type: Option<String>,
    pub created_at: i64,
    pub last_used: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WarpAccountIndex {
    pub version: String,
    pub accounts: Vec<WarpAccountSummary>,
}

impl WarpAccountIndex {
    pub fn new() -> Self {
        Self {
            version: "1.0".to_string(),
            accounts: Vec::new(),
        }
    }
}

impl Default for WarpAccountIndex {
    fn default() -> Self {
        Self::new()
    }
}

impl WarpAccount {
    pub fn summary(&self) -> WarpAccountSummary {
        WarpAccountSummary {
            id: self.id.clone(),
            email: self.email.clone(),
            tags: self.tags.clone(),
            plan_type: self.plan_type.clone(),
            created_at: self.created_at,
            last_used: self.last_used,
        }
    }
}
