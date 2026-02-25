use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::AppHandle;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "lowercase")]
pub enum UpdatePolicy {
    Stable,
    Latest,
}

impl UpdatePolicy {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Stable => "stable",
            Self::Latest => "latest",
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct LlmSettings {
    pub model_dir: String,
    pub update_policy: UpdatePolicy,
    pub stable_tag: String,
    pub asset_name: String,
    pub stable_sha256: Option<String>,
    pub github_owner: String,
    pub github_repo: String,
    pub allow_prerelease: bool,
    pub auto_check_days: u32,
    #[serde(default)]
    pub last_checked_utc: Option<String>,
    #[serde(default)]
    pub last_error: Option<String>,
}

pub fn app_data_root(app: &AppHandle) -> Result<PathBuf, String> {
    let base = tauri::api::path::app_data_dir(&app.config())
        .ok_or_else(|| "Unable to resolve app data dir".to_string())?;
    let root = base.join("research-workflow");
    fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    Ok(root)
}

pub fn settings_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(app_data_root(app)?.join("settings").join("llm.json"))
}

impl LlmSettings {
    pub fn default_for(app: &AppHandle) -> Result<Self, String> {
        let model_dir = app_data_root(app)?
            .join("models")
            .join("llm")
            .to_string_lossy()
            .to_string();
        Ok(Self {
            model_dir,
            update_policy: UpdatePolicy::Stable,
            stable_tag: "".to_string(),
            asset_name: "model.gguf".to_string(),
            stable_sha256: None,
            github_owner: "".to_string(),
            github_repo: "".to_string(),
            allow_prerelease: false,
            auto_check_days: 1,
            last_checked_utc: None,
            last_error: None,
        })
    }
}

pub fn load_llm_settings(app: &AppHandle) -> Result<LlmSettings, String> {
    let path = settings_path(app)?;
    if !path.exists() {
        let defaults = LlmSettings::default_for(app)?;
        save_llm_settings(app, &defaults)?;
        return Ok(defaults);
    }
    let raw =
        fs::read_to_string(&path).map_err(|e| format!("Unable to read {}: {e}", path.display()))?;
    if raw.trim().is_empty() {
        let defaults = LlmSettings::default_for(app)?;
        save_llm_settings(app, &defaults)?;
        return Ok(defaults);
    }
    serde_json::from_str(&raw).map_err(|e| format!("Invalid llm settings JSON: {e}"))
}

pub fn save_llm_settings(app: &AppHandle, settings: &LlmSettings) -> Result<(), String> {
    let path = settings_path(app)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let payload = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
    fs::write(&path, payload).map_err(|e| format!("Unable to write {}: {e}", path.display()))
}
