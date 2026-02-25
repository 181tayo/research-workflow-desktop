use chrono::{DateTime, Duration, Utc};
use sha2::Digest;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use super::github::{download_asset_and_sha256, fetch_release_by_tag, find_asset, newest_release};
use super::settings::{load_llm_settings, save_llm_settings, LlmSettings, UpdatePolicy};
use super::types::{LlmModelLock, LlmProjectPreset, ModelProvenance, ModelStatus, TargetModel};

static LOADED_MODEL: OnceLock<Mutex<Option<String>>> = OnceLock::new();

fn loaded_model_cell() -> &'static Mutex<Option<String>> {
    LOADED_MODEL.get_or_init(|| Mutex::new(None))
}

fn normalize_sha(value: &str) -> String {
    value.trim().trim_start_matches("sha256:").to_lowercase()
}

fn compute_sha256(path: &Path) -> Result<String, String> {
    let bytes = fs::read(path).map_err(|e| format!("Unable to read {}: {e}", path.display()))?;
    Ok(hex::encode(sha2::Sha256::digest(bytes)))
}

pub fn lock_file_path(project_root: &Path) -> PathBuf {
    project_root.join(".researchapp").join("llm_lock.json")
}

pub fn read_project_lock(project_root: &Path) -> Result<Option<LlmModelLock>, String> {
    let path = lock_file_path(project_root);
    if !path.exists() {
        return Ok(None);
    }
    let raw =
        fs::read_to_string(&path).map_err(|e| format!("Unable to read {}: {e}", path.display()))?;
    let lock: LlmModelLock =
        serde_json::from_str(&raw).map_err(|e| format!("Invalid {}: {e}", path.display()))?;
    Ok(Some(lock))
}

pub fn write_project_lock(project_root: &Path, lock: &LlmModelLock) -> Result<(), String> {
    let path = lock_file_path(project_root);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let payload = serde_json::to_string_pretty(lock).map_err(|e| e.to_string())?;
    fs::write(&path, payload).map_err(|e| format!("Unable to write {}: {e}", path.display()))
}

pub fn clear_project_lock(project_root: &Path) -> Result<(), String> {
    let path = lock_file_path(project_root);
    if path.exists() {
        fs::remove_file(&path).map_err(|e| format!("Unable to remove {}: {e}", path.display()))?;
    }
    Ok(())
}

pub fn preset_file_path(project_root: &Path) -> PathBuf {
    project_root.join(".researchapp").join("llm_preset.json")
}

pub fn read_project_preset(project_root: &Path) -> Result<Option<LlmProjectPreset>, String> {
    let path = preset_file_path(project_root);
    if !path.exists() {
        return Ok(None);
    }
    let raw =
        fs::read_to_string(&path).map_err(|e| format!("Unable to read {}: {e}", path.display()))?;
    let preset: LlmProjectPreset =
        serde_json::from_str(&raw).map_err(|e| format!("Invalid {}: {e}", path.display()))?;
    Ok(Some(preset))
}

pub fn write_project_preset(project_root: &Path, preset: &LlmProjectPreset) -> Result<(), String> {
    let path = preset_file_path(project_root);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let payload = serde_json::to_string_pretty(preset).map_err(|e| e.to_string())?;
    fs::write(&path, payload).map_err(|e| format!("Unable to write {}: {e}", path.display()))
}

pub fn apply_project_preset(
    app: &tauri::AppHandle,
    preset: &LlmProjectPreset,
) -> Result<LlmSettings, String> {
    let mut settings = load_llm_settings(app)?;
    settings.update_policy = match preset.update_policy.trim().to_lowercase().as_str() {
        "latest" => UpdatePolicy::Latest,
        _ => UpdatePolicy::Stable,
    };
    settings.stable_tag = preset.stable_tag.clone();
    settings.asset_name = preset.asset_name.clone();
    settings.allow_prerelease = preset.allow_prerelease;
    settings.auto_check_days = preset.auto_check_days.max(1);
    save_llm_settings(app, &settings)?;
    Ok(settings)
}

pub fn resolve_target_model(
    project_root: Option<PathBuf>,
    settings: &LlmSettings,
) -> Result<TargetModel, String> {
    if let Some(root) = project_root {
        if let Some(lock) = read_project_lock(&root)? {
            if lock.locked {
                return Ok(TargetModel {
                    tag: lock.tag.clone(),
                    asset_name: lock.asset_name.clone(),
                    expected_sha256: Some(normalize_sha(&lock.sha256)),
                    is_locked: true,
                    lock: Some(lock),
                });
            }
        }
    }

    match settings.update_policy {
        UpdatePolicy::Stable => Ok(TargetModel {
            tag: settings.stable_tag.clone(),
            asset_name: settings.asset_name.clone(),
            expected_sha256: settings.stable_sha256.as_ref().map(|s| normalize_sha(s)),
            is_locked: false,
            lock: None,
        }),
        UpdatePolicy::Latest => {
            let release = newest_release(settings)?;
            let asset = find_asset(&release, &settings.asset_name)?;
            Ok(TargetModel {
                tag: release.tag_name.clone(),
                asset_name: asset.name.clone(),
                expected_sha256: None,
                is_locked: false,
                lock: None,
            })
        }
    }
}

fn empty_status(settings: &LlmSettings, target: &TargetModel) -> ModelStatus {
    let loaded = loaded_model_cell()
        .lock()
        .ok()
        .and_then(|g| g.clone())
        .is_some();
    ModelStatus {
        loaded,
        model_dir: settings.model_dir.clone(),
        model_path: None,
        update_policy: settings.update_policy.as_str().to_string(),
        selected_tag: if target.tag.trim().is_empty() {
            None
        } else {
            Some(target.tag.clone())
        },
        asset_name: target.asset_name.clone(),
        bytes_on_disk: None,
        sha256: None,
        sha256_ok: None,
        last_checked_utc: settings.last_checked_utc.clone(),
        last_error: settings.last_error.clone(),
        lock: target.lock.clone(),
    }
}

fn finalize_status_with_file(
    status: &mut ModelStatus,
    path: &Path,
    expected: Option<&String>,
) -> Result<(), String> {
    let metadata = fs::metadata(path)
        .map_err(|e| format!("Unable to read metadata for {}: {e}", path.display()))?;
    let sha = compute_sha256(path)?;
    status.model_path = Some(path.to_string_lossy().to_string());
    status.bytes_on_disk = Some(metadata.len());
    status.sha256 = Some(sha.clone());
    status.sha256_ok = expected.map(|e| normalize_sha(e) == sha);
    Ok(())
}

pub fn ensure_model_downloaded(
    target: TargetModel,
    settings: &LlmSettings,
) -> Result<ModelStatus, String> {
    let model_dir = PathBuf::from(settings.model_dir.trim());
    fs::create_dir_all(&model_dir).map_err(|e| e.to_string())?;
    let model_path = model_dir.join(&target.asset_name);
    let mut status = empty_status(settings, &target);

    if model_path.exists() {
        let current_sha = compute_sha256(&model_path)?;
        if target.is_locked {
            if let Some(expected) = &target.expected_sha256 {
                if current_sha != normalize_sha(expected) {
                    return Err(
                        "Locked model hash mismatch; redownload or unlock project.".to_string()
                    );
                }
            }
        }

        if let Some(expected) = &target.expected_sha256 {
            if current_sha == normalize_sha(expected) {
                finalize_status_with_file(&mut status, &model_path, Some(expected))?;
                return Ok(status);
            }
        } else {
            finalize_status_with_file(&mut status, &model_path, None)?;
            return Ok(status);
        }
    }

    if settings.github_owner.trim().is_empty() || settings.github_repo.trim().is_empty() {
        return Err("GitHub owner/repo are required to download model assets.".to_string());
    }

    let release = fetch_release_by_tag(settings, &target.tag)?;
    let asset = find_asset(&release, &target.asset_name)?;

    let downloaded =
        download_asset_and_sha256(&asset.browser_download_url, &model_dir, &target.asset_name);
    let (downloaded_sha, _downloaded_bytes) = match downloaded {
        Ok(v) => v,
        Err(e) => {
            if !target.is_locked && model_path.exists() {
                status.last_error = Some(format!("Download failed; using existing model: {e}"));
                finalize_status_with_file(
                    &mut status,
                    &model_path,
                    target.expected_sha256.as_ref(),
                )?;
                return Ok(status);
            }
            return Err(e);
        }
    };

    if let Some(expected) = &target.expected_sha256 {
        if downloaded_sha != normalize_sha(expected) {
            if target.is_locked {
                return Err("Locked model hash mismatch; redownload or unlock project.".to_string());
            }

            let retry = download_asset_and_sha256(
                &asset.browser_download_url,
                &model_dir,
                &target.asset_name,
            )?;
            if retry.0 != normalize_sha(expected) {
                return Err(format!(
                    "Downloaded model hash mismatch. Expected {}, got {}.",
                    normalize_sha(expected),
                    retry.0
                ));
            }
        }
    }

    finalize_status_with_file(&mut status, &model_path, target.expected_sha256.as_ref())?;
    Ok(status)
}

pub fn load_model_if_needed(model_path: &str) -> Result<(), String> {
    let path = PathBuf::from(model_path);
    if !path.exists() {
        return Err(format!("Model path does not exist: {}", path.display()));
    }
    let mut guard = loaded_model_cell()
        .lock()
        .map_err(|_| "Unable to acquire model runtime lock".to_string())?;
    if guard.as_deref() != Some(model_path) {
        *guard = Some(model_path.to_string());
    }
    Ok(())
}

fn should_check_latest(settings: &LlmSettings, target: &TargetModel, force: bool) -> bool {
    if force || target.is_locked {
        return true;
    }
    if !matches!(settings.update_policy, UpdatePolicy::Latest) {
        return true;
    }
    let Some(last_checked) = &settings.last_checked_utc else {
        return true;
    };
    let Ok(ts) = DateTime::parse_from_rfc3339(last_checked) else {
        return true;
    };
    let delta = Utc::now() - ts.with_timezone(&Utc);
    delta >= Duration::days(settings.auto_check_days.max(1) as i64)
}

pub fn download_model_with_policy(
    app: &tauri::AppHandle,
    project_root: Option<PathBuf>,
    force: bool,
) -> Result<ModelStatus, String> {
    let mut settings = load_llm_settings(app)?;
    let target = resolve_target_model(project_root, &settings)?;

    if !should_check_latest(&settings, &target, force) {
        let mut status = empty_status(&settings, &target);
        let path = PathBuf::from(&settings.model_dir).join(&target.asset_name);
        if path.exists() {
            finalize_status_with_file(&mut status, &path, target.expected_sha256.as_ref())?;
        }
        return Ok(status);
    }

    let result = ensure_model_downloaded(target.clone(), &settings);
    settings.last_checked_utc = Some(Utc::now().to_rfc3339());
    settings.last_error = result.as_ref().err().cloned();
    save_llm_settings(app, &settings)?;

    result
}

pub fn verify_model(
    app: &tauri::AppHandle,
    project_root: Option<PathBuf>,
) -> Result<ModelStatus, String> {
    let settings = load_llm_settings(app)?;
    let target = resolve_target_model(project_root, &settings)?;
    let model_path = PathBuf::from(&settings.model_dir).join(&target.asset_name);
    let mut status = empty_status(&settings, &target);
    if !model_path.exists() {
        return Ok(status);
    }
    finalize_status_with_file(&mut status, &model_path, target.expected_sha256.as_ref())?;
    if target.is_locked && status.sha256_ok != Some(true) {
        return Err("Locked model hash mismatch; redownload or unlock project.".to_string());
    }
    Ok(status)
}

pub fn load_model_from_disk(
    app: &tauri::AppHandle,
    project_root: Option<PathBuf>,
) -> Result<ModelStatus, String> {
    let status = download_model_with_policy(app, project_root, false)?;
    if let Some(path) = &status.model_path {
        load_model_if_needed(path)?;
    }
    let mut out = status;
    out.loaded = true;
    Ok(out)
}

pub fn lock_project_to_current_model(
    app: &tauri::AppHandle,
    project_root: &Path,
    note: Option<String>,
) -> Result<LlmModelLock, String> {
    let settings = load_llm_settings(app)?;
    let target = resolve_target_model(None, &settings)?;
    let status = ensure_model_downloaded(target.clone(), &settings)?;
    let lock = LlmModelLock {
        locked: true,
        tag: target.tag,
        asset_name: target.asset_name,
        sha256: status.sha256.clone().unwrap_or_default(),
        locked_at_utc: Utc::now().to_rfc3339(),
        note,
    };
    write_project_lock(project_root, &lock)?;
    Ok(lock)
}

pub fn model_provenance_from_status(status: &ModelStatus) -> Option<ModelProvenance> {
    Some(ModelProvenance {
        model_tag: status.selected_tag.clone()?,
        asset: status.asset_name.clone(),
        sha256: status.sha256.clone()?,
        locked: status.lock.as_ref().map(|l| l.locked).unwrap_or(false),
        locked_at_utc: status.lock.as_ref().map(|l| l.locked_at_utc.clone()),
        lock_note: status.lock.as_ref().and_then(|l| l.note.clone()),
        model_path: status.model_path.clone()?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::settings::{LlmSettings, UpdatePolicy};

    fn test_settings() -> LlmSettings {
        LlmSettings {
            model_dir: "/tmp/model-dir".to_string(),
            update_policy: UpdatePolicy::Stable,
            stable_tag: "v1.0.0".to_string(),
            asset_name: "m.gguf".to_string(),
            stable_sha256: Some("abc".to_string()),
            github_owner: "o".to_string(),
            github_repo: "r".to_string(),
            allow_prerelease: false,
            auto_check_days: 1,
            last_checked_utc: None,
            last_error: None,
        }
    }

    #[test]
    fn resolve_target_model_uses_stable_policy_without_lock() {
        let resolved = resolve_target_model(None, &test_settings()).expect("resolve");
        assert_eq!(resolved.tag, "v1.0.0");
        assert_eq!(resolved.asset_name, "m.gguf");
        assert_eq!(resolved.expected_sha256.as_deref(), Some("abc"));
        assert!(!resolved.is_locked);
    }

    #[test]
    fn latest_policy_requires_release_lookup() {
        let mut settings = test_settings();
        settings.update_policy = UpdatePolicy::Latest;
        settings.github_owner = "".to_string();
        settings.github_repo = "".to_string();
        let err = resolve_target_model(None, &settings).expect_err("should fail");
        assert!(err.contains("GitHub owner/repo"));
    }

    #[test]
    fn locked_sha_mismatch_hard_fails() {
        let temp = std::env::temp_dir().join(format!("llm-sha-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&temp).expect("mkdir");
        let model = temp.join("m.gguf");
        fs::write(&model, b"content").expect("write");
        let mut settings = test_settings();
        settings.model_dir = temp.to_string_lossy().to_string();
        let target = TargetModel {
            tag: "v1".to_string(),
            asset_name: "m.gguf".to_string(),
            expected_sha256: Some("deadbeef".to_string()),
            is_locked: true,
            lock: None,
        };
        let err = ensure_model_downloaded(target, &settings).expect_err("should fail");
        assert_eq!(
            err,
            "Locked model hash mismatch; redownload or unlock project."
        );
        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn resolve_target_model_prefers_lock() {
        let temp = std::env::temp_dir().join(format!("llm-lock-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(temp.join(".researchapp")).expect("mkdir");
        let lock = LlmModelLock {
            locked: true,
            tag: "v9".to_string(),
            asset_name: "x.gguf".to_string(),
            sha256: "123".to_string(),
            locked_at_utc: Utc::now().to_rfc3339(),
            note: None,
        };
        write_project_lock(&temp, &lock).expect("write lock");
        let resolved = resolve_target_model(Some(temp.clone()), &test_settings()).expect("resolve");
        assert_eq!(resolved.tag, "v9");
        assert!(resolved.is_locked);
        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn lock_roundtrip() {
        let temp = std::env::temp_dir().join(format!("llm-lock-{}", uuid::Uuid::new_v4()));
        let lock = LlmModelLock {
            locked: true,
            tag: "v1".to_string(),
            asset_name: "m.gguf".to_string(),
            sha256: "deadbeef".to_string(),
            locked_at_utc: Utc::now().to_rfc3339(),
            note: Some("n".to_string()),
        };
        write_project_lock(&temp, &lock).expect("write");
        let loaded = read_project_lock(&temp).expect("read").expect("some");
        assert_eq!(loaded.tag, lock.tag);
        assert_eq!(loaded.sha256, lock.sha256);
        let _ = fs::remove_dir_all(temp);
    }
}
