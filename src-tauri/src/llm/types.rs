use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct LlmModelLock {
    pub locked: bool,
    pub tag: String,
    pub asset_name: String,
    pub sha256: String,
    pub locked_at_utc: String,
    pub note: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ModelStatus {
    pub loaded: bool,
    pub model_dir: String,
    pub model_path: Option<String>,
    pub update_policy: String,
    pub selected_tag: Option<String>,
    pub asset_name: String,
    pub bytes_on_disk: Option<u64>,
    pub sha256: Option<String>,
    pub sha256_ok: Option<bool>,
    pub last_checked_utc: Option<String>,
    pub last_error: Option<String>,
    pub lock: Option<LlmModelLock>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ModelProvenance {
    pub model_tag: String,
    pub asset: String,
    pub sha256: String,
    pub locked: bool,
    pub locked_at_utc: Option<String>,
    pub lock_note: Option<String>,
    pub model_path: String,
}

#[derive(Clone, Debug)]
pub struct TargetModel {
    pub tag: String,
    pub asset_name: String,
    pub expected_sha256: Option<String>,
    pub is_locked: bool,
    pub lock: Option<LlmModelLock>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct LlmProjectPreset {
    pub name: String,
    pub update_policy: String,
    pub stable_tag: String,
    pub asset_name: String,
    pub allow_prerelease: bool,
    pub auto_check_days: u32,
    pub note: Option<String>,
}
