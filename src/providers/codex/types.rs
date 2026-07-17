//! Tipos internos do Codex (snake_case = formato do session-log; Raw cacheável).

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

// ---- Formato interno (snake_case = formato do session-log; é o Raw cacheável) ----

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexWindowRaw {
    pub used_percent: f64,
    pub window_minutes: i64,
    pub resets_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexLimitBucket {
    pub limit_id: String,
    #[serde(default)]
    pub limit_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub primary: Option<CodexWindowRaw>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secondary: Option<CodexWindowRaw>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexCredits {
    pub has_credits: bool,
    pub unlimited: bool,
    pub balance: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CodexRateLimits {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub primary: Option<CodexWindowRaw>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secondary: Option<CodexWindowRaw>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credits: Option<CodexCredits>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub buckets: Option<IndexMap<String, CodexLimitBucket>>,
}
