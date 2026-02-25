use regex::Regex;
use std::path::PathBuf;
use tauri::AppHandle;

use super::model_manager::{
    apply_project_preset, clear_project_lock, download_model_with_policy, load_model_from_disk,
    lock_project_to_current_model, model_provenance_from_status, read_project_lock,
    read_project_preset, resolve_target_model, verify_model, write_project_lock,
    write_project_preset,
};
use super::settings::{load_llm_settings, save_llm_settings, UpdatePolicy};
use super::types::{LlmModelLock, LlmProjectPreset, ModelStatus};

fn root_opt(project_root: Option<String>) -> Option<PathBuf> {
    project_root
        .map(PathBuf::from)
        .filter(|p| !p.as_os_str().is_empty())
}

#[tauri::command]
pub fn llm_get_settings(app: AppHandle) -> Result<super::settings::LlmSettings, String> {
    load_llm_settings(&app)
}

#[tauri::command]
pub fn llm_save_settings(
    app: AppHandle,
    settings: super::settings::LlmSettings,
) -> Result<super::settings::LlmSettings, String> {
    save_llm_settings(&app, &settings)?;
    Ok(settings)
}

#[tauri::command]
pub fn llm_set_model_dir(
    app: AppHandle,
    model_dir: String,
) -> Result<super::settings::LlmSettings, String> {
    let mut settings = load_llm_settings(&app)?;
    settings.model_dir = model_dir;
    save_llm_settings(&app, &settings)?;
    Ok(settings)
}

#[tauri::command]
pub fn llm_set_update_policy(
    app: AppHandle,
    policy: String,
) -> Result<super::settings::LlmSettings, String> {
    let mut settings = load_llm_settings(&app)?;
    settings.update_policy = match policy.trim().to_lowercase().as_str() {
        "stable" => UpdatePolicy::Stable,
        "latest" => UpdatePolicy::Latest,
        _ => return Err("policy must be 'stable' or 'latest'.".to_string()),
    };
    save_llm_settings(&app, &settings)?;
    Ok(settings)
}

#[tauri::command]
pub fn llm_set_allow_prerelease(
    app: AppHandle,
    allow: bool,
) -> Result<super::settings::LlmSettings, String> {
    let mut settings = load_llm_settings(&app)?;
    settings.allow_prerelease = allow;
    save_llm_settings(&app, &settings)?;
    Ok(settings)
}

#[tauri::command]
pub fn llm_set_auto_check_days(
    app: AppHandle,
    days: u32,
) -> Result<super::settings::LlmSettings, String> {
    let mut settings = load_llm_settings(&app)?;
    settings.auto_check_days = days.max(1);
    save_llm_settings(&app, &settings)?;
    Ok(settings)
}

#[tauri::command]
pub fn llm_get_model_status(
    app: AppHandle,
    project_root: Option<String>,
) -> Result<ModelStatus, String> {
    let root = root_opt(project_root);
    let settings = load_llm_settings(&app)?;
    let target = resolve_target_model(root.clone(), &settings)?;
    let mut status = verify_model(&app, root)?;
    status.asset_name = target.asset_name;
    status.selected_tag = Some(target.tag);
    status.lock = target.lock;
    Ok(status)
}

#[tauri::command]
pub fn llm_download_model_if_needed(
    app: AppHandle,
    project_root: Option<String>,
) -> Result<ModelStatus, String> {
    download_model_with_policy(&app, root_opt(project_root), false)
}

#[tauri::command]
pub fn llm_force_update_model(
    app: AppHandle,
    project_root: Option<String>,
) -> Result<ModelStatus, String> {
    download_model_with_policy(&app, root_opt(project_root), true)
}

#[tauri::command]
pub fn llm_verify_model(
    app: AppHandle,
    project_root: Option<String>,
) -> Result<ModelStatus, String> {
    verify_model(&app, root_opt(project_root))
}

#[tauri::command]
pub fn llm_load_model_from_disk(
    app: AppHandle,
    project_root: Option<String>,
) -> Result<ModelStatus, String> {
    load_model_from_disk(&app, root_opt(project_root))
}

#[tauri::command]
pub fn llm_get_project_lock(project_root: String) -> Result<Option<LlmModelLock>, String> {
    read_project_lock(&PathBuf::from(project_root))
}

#[tauri::command]
pub fn llm_set_project_lock(
    project_root: String,
    lock: LlmModelLock,
) -> Result<LlmModelLock, String> {
    write_project_lock(&PathBuf::from(project_root), &lock)?;
    Ok(lock)
}

#[tauri::command]
pub fn llm_clear_project_lock(project_root: String) -> Result<(), String> {
    clear_project_lock(&PathBuf::from(project_root))
}

#[tauri::command]
pub fn llm_lock_project_to_current_model(
    app: AppHandle,
    project_root: String,
    note: Option<String>,
) -> Result<LlmModelLock, String> {
    lock_project_to_current_model(&app, &PathBuf::from(project_root), note)
}

#[tauri::command]
pub fn llm_unlock_project(project_root: String) -> Result<(), String> {
    clear_project_lock(&PathBuf::from(project_root))
}

#[tauri::command]
pub fn llm_get_project_preset(project_root: String) -> Result<Option<LlmProjectPreset>, String> {
    read_project_preset(&PathBuf::from(project_root))
}

#[tauri::command]
pub fn llm_set_project_preset(
    project_root: String,
    preset: LlmProjectPreset,
) -> Result<LlmProjectPreset, String> {
    write_project_preset(&PathBuf::from(project_root), &preset)?;
    Ok(preset)
}

#[tauri::command]
pub fn llm_apply_project_preset(
    app: AppHandle,
    project_root: String,
) -> Result<super::settings::LlmSettings, String> {
    let preset = read_project_preset(&PathBuf::from(project_root))?
        .ok_or_else(|| "No preset saved for this project.".to_string())?;
    apply_project_preset(&app, &preset)
}

#[tauri::command]
pub fn llm_extract_model_spec(
    app: AppHandle,
    text: String,
    qsf_context_json: String,
    project_root: Option<String>,
) -> Result<String, String> {
    let status = llm_load_model_from_disk(app, project_root)?;
    let provenance = model_provenance_from_status(&status);
    let lower = text.to_lowercase();
    let (dv, iv) = if let Some(idx) = lower.find(" from ") {
        (
            text[..idx]
                .replace("predict", "")
                .replace("predicting", "")
                .trim()
                .to_string(),
            text[idx + 6..]
                .split(['+', ',', ';'])
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect::<Vec<String>>(),
        )
    } else if let Some(idx) = lower.find(" on ") {
        (
            text[..idx]
                .replace("regress", "")
                .replace("regression", "")
                .trim()
                .to_string(),
            text[idx + 4..]
                .split(['+', ',', ';'])
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect::<Vec<String>>(),
        )
    } else {
        (String::new(), Vec::new())
    };
    let mut ambiguities = Vec::<String>::new();
    if dv.trim().is_empty() {
        ambiguities.push("Could not confidently identify dependent variable.".to_string());
    }
    if iv.is_empty() {
        ambiguities.push("Could not confidently identify independent variable(s).".to_string());
    }
    Ok(serde_json::json!({
      "kind": "model_spec",
      "text": text,
      "qsfContextJson": qsf_context_json,
      "model": provenance,
      "extracted": {
        "dv": dv,
        "iv": iv,
        "controls": Vec::<String>::new(),
        "ambiguities": ambiguities
      }
    })
    .to_string())
}

#[tauri::command]
pub fn llm_extract_prereg_models(
    app: AppHandle,
    doc_text: String,
    qsf_context_json: String,
    project_root: Option<String>,
) -> Result<String, String> {
    let status = llm_load_model_from_disk(app, project_root)?;
    let provenance = model_provenance_from_status(&status);

    let qsf_vars = parse_qsf_variables(&qsf_context_json);
    let lower = doc_text.to_lowercase();

    let mut main_models = Vec::<serde_json::Value>::new();
    let mut exploratory_models = Vec::<serde_json::Value>::new();
    let mut mechanism_models = Vec::<serde_json::Value>::new();
    let mut robustness_checks = Vec::<String>::new();
    let mut ambiguities = Vec::<String>::new();

    let formula_re = Regex::new(r"(?m)([A-Za-z][A-Za-z0-9_]*)\s*~\s*([A-Za-z0-9_ +:*.-]+)")
        .map_err(|e| format!("Regex error: {e}"))?;

    for (idx, cap) in formula_re.captures_iter(&doc_text).enumerate() {
        let dv = cap
            .get(1)
            .map(|m| m.as_str().trim())
            .unwrap_or("")
            .to_string();
        let rhs = cap
            .get(2)
            .map(|m| m.as_str().trim())
            .unwrap_or("")
            .to_string();
        let iv_raw = rhs
            .split('+')
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
            .collect::<Vec<String>>();
        let iv = iv_raw
            .iter()
            .map(|v| match_qsf_var(v, &qsf_vars).unwrap_or_else(|| v.to_string()))
            .collect::<Vec<String>>();
        let dv_mapped = match_qsf_var(&dv, &qsf_vars).unwrap_or(dv.clone());
        let model = serde_json::json!({
          "id": format!("llm_m{}", idx + 1),
          "dv": dv_mapped,
          "iv": iv,
          "controls": [],
          "interactionTerms": extract_interactions(&rhs),
          "formula": format!("{dv} ~ {rhs}")
        });

        if rhs.contains("mediat") || rhs.contains("mechanis") {
            mechanism_models.push(model);
        } else if rhs.contains("explor")
            || rhs.contains("secondary")
            || lower.contains("exploratory")
        {
            exploratory_models.push(model);
        } else {
            main_models.push(model);
        }
    }

    if lower.contains("robust") || lower.contains("sensitivity") {
        robustness_checks.push("with_without_controls".to_string());
    }
    if lower.contains("without controls") || lower.contains("with and without controls") {
        robustness_checks.push("with_without_controls".to_string());
    }
    robustness_checks.sort();
    robustness_checks.dedup();

    if main_models.is_empty() && exploratory_models.is_empty() && mechanism_models.is_empty() {
        ambiguities.push(
            "No explicit model formula found (expected patterns like y ~ x + c).".to_string(),
        );
    }

    let mediators = qsf_vars
        .iter()
        .filter(|v| v.to_lowercase().contains("mediat") || v.to_lowercase().contains("mechanis"))
        .cloned()
        .collect::<Vec<String>>();
    let moderators = qsf_vars
        .iter()
        .filter(|v| v.to_lowercase().contains("moderat") || v.to_lowercase().contains("interact"))
        .cloned()
        .collect::<Vec<String>>();
    let exploratory_vars = qsf_vars
        .iter()
        .filter(|v| v.to_lowercase().contains("explor"))
        .cloned()
        .collect::<Vec<String>>();

    Ok(serde_json::json!({
      "kind": "prereg_models",
      "docTextPreview": doc_text.chars().take(600).collect::<String>(),
      "qsfContextJson": qsf_context_json,
      "model": provenance,
      "parsed": {
        "mainModels": main_models,
        "exploratoryModels": exploratory_models,
        "mechanismModels": mechanism_models,
        "robustnessChecks": robustness_checks,
        "variables": {
          "mediators": mediators,
          "moderators": moderators,
          "exploratory": exploratory_vars
        },
        "ambiguities": ambiguities
      }
    })
    .to_string())
}

#[tauri::command]
pub fn llm_map_to_qsf(
    app: AppHandle,
    model_spec_json: String,
    qsf_context_json: String,
    project_root: Option<String>,
) -> Result<String, String> {
    let status = llm_load_model_from_disk(app, project_root)?;
    let provenance = model_provenance_from_status(&status);
    Ok(serde_json::json!({
      "kind": "map_to_qsf",
      "modelSpecJson": model_spec_json,
      "qsfContextJson": qsf_context_json,
      "model": provenance,
    })
    .to_string())
}

fn parse_qsf_variables(qsf_context_json: &str) -> Vec<String> {
    let parsed = serde_json::from_str::<serde_json::Value>(qsf_context_json).ok();
    let cols = parsed
        .as_ref()
        .and_then(|v| v.get("expectedColumns"))
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    cols.into_iter()
        .filter_map(|v| v.as_str().map(|s| s.to_string()))
        .filter(|s| !s.trim().is_empty())
        .collect()
}

fn match_qsf_var(candidate: &str, qsf_vars: &[String]) -> Option<String> {
    let c = candidate.trim().to_lowercase();
    if c.is_empty() {
        return None;
    }
    if let Some(exact) = qsf_vars.iter().find(|v| v.eq_ignore_ascii_case(&c)) {
        return Some(exact.clone());
    }
    qsf_vars
        .iter()
        .find(|v| v.to_lowercase().contains(&c) || c.contains(&v.to_lowercase()))
        .cloned()
}

fn extract_interactions(rhs: &str) -> Vec<String> {
    rhs.split('+')
        .map(|v| v.trim())
        .filter(|v| v.contains(':') || v.contains('*'))
        .map(|v| v.replace('*', ":"))
        .collect()
}
