use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

use crate::commands::assets::{
    read_file_bytes, read_file_text, resolve_project_root, resolve_study_root,
};
use crate::llm::commands::llm_extract_prereg_models;
use crate::llm::model_manager::{
    download_model_with_policy, model_provenance_from_status, read_project_lock,
};
use crate::prereg::parse_docx::parse_prereg_docx;
use crate::prereg::parse_json::parse_prereg_json;
use crate::prereg::parse_md::parse_prereg_md;
use crate::prereg::types::PreregSpec;
use crate::qsf::parse::{parse_qsf_json, parse_qsf_json_with_tokens};
use crate::qsf::types::QsfSurveySpec;
use crate::render::helpers::{analysis_paths, ensure_dir, write_string};
use crate::render::templates::{render_from_spec, template_root_from_cwd};
use crate::spec::builder::build_analysis_spec;
use crate::spec::types::{AnalysisSpec, MappingResult};
use tauri::AppHandle;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateSpecArgs {
    pub project_id: String,
    pub study_id: String,
    pub analysis_id: String,
    pub qsf_path: String,
    pub prereg_path: String,
    #[serde(default)]
    pub candidate_tokens: Vec<String>,
    pub template_set: String,
    pub style_profile: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParseQsfArgs {
    pub qsf_path: String,
    #[serde(default)]
    pub candidate_tokens: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveSpecArgs {
    pub project_id: String,
    pub study_id: String,
    pub analysis_id: String,
    pub spec: AnalysisSpec,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MappingUpdate {
    pub prereg_var: String,
    pub resolved_to: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolveMappingsArgs {
    pub project_id: String,
    pub study_id: String,
    pub analysis_id: String,
    pub mapping_updates: Vec<MappingUpdate>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderArgs {
    pub project_id: String,
    pub study_id: String,
    pub analysis_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderOutput {
    pub rmd_path: String,
    pub r_path: String,
}

#[tauri::command]
pub fn parse_qsf(args: ParseQsfArgs) -> Result<QsfSurveySpec, String> {
    let raw = read_file_text(&args.qsf_path)?;
    if args.candidate_tokens.is_empty() {
        parse_qsf_json(&raw)
    } else {
        parse_qsf_json_with_tokens(&raw, &args.candidate_tokens)
    }
}

#[tauri::command]
pub fn parse_prereg(prereg_path: String) -> Result<PreregSpec, String> {
    if prereg_path.ends_with(".docx") {
        return parse_prereg_docx(&prereg_path);
    }
    if prereg_path.ends_with(".md") || prereg_path.ends_with(".markdown") {
        return Ok(parse_prereg_md(&read_file_text(&prereg_path)?));
    }
    if prereg_path.ends_with(".json") {
        return parse_prereg_json(&read_file_text(&prereg_path)?);
    }
    let text = read_file_text(&prereg_path)?;
    Ok(parse_prereg_md(&text))
}

fn analysis_root(
    app: &AppHandle,
    project_id: &str,
    study_id: &str,
    analysis_id: &str,
) -> Result<PathBuf, String> {
    let study_root = resolve_study_root(app, project_id, study_id)?;
    Ok(study_root.join("06_analysis").join(analysis_id))
}

#[tauri::command]
pub fn generate_analysis_spec(
    _app: AppHandle,
    args: GenerateSpecArgs,
) -> Result<AnalysisSpec, String> {
    let qsf_bytes = read_file_bytes(&args.qsf_path)?;
    let prereg_bytes = read_file_bytes(&args.prereg_path)?;
    let prereg = parse_prereg(args.prereg_path.clone())?;
    let inferred_tokens = if args.candidate_tokens.is_empty() {
        collect_candidate_tokens_from_prereg(&prereg)
    } else {
        args.candidate_tokens.clone()
    };
    let qsf = parse_qsf(ParseQsfArgs {
        qsf_path: args.qsf_path.clone(),
        candidate_tokens: inferred_tokens,
    })?;
    let prereg_text = read_file_text(&args.prereg_path).unwrap_or_else(|_| String::new());
    let project_root = resolve_project_root(&_app, &args.project_id)?;
    let qsf_context_for_llm = serde_json::json!({
      "expectedColumns": qsf.expected_columns,
      "labelMap": qsf.label_map
    })
    .to_string();
    let llm_output = llm_extract_prereg_models(
        _app.clone(),
        prereg_text,
        qsf_context_for_llm,
        Some(project_root.to_string_lossy().to_string()),
    )?;
    let mut prereg_for_build = prereg.clone();
    apply_llm_prereg_enrichment(&mut prereg_for_build, &llm_output);

    let mut spec = build_analysis_spec(
        &args.project_id,
        &args.study_id,
        &args.analysis_id,
        &args.qsf_path,
        &args.prereg_path,
        &qsf_bytes,
        &prereg_bytes,
        &qsf,
        &prereg_for_build,
        &args.template_set,
        &args.style_profile,
    );
    if let Ok(saved) = load_saved_spec(&_app, &args.project_id, &args.study_id, &args.analysis_id) {
        apply_saved_mappings(&mut spec, &saved);
    }
    let model_status = download_model_with_policy(&_app, Some(project_root), false)?;
    spec.model_provenance = model_provenance_from_status(&model_status);
    spec.model_lock = model_status.lock.clone();
    spec.warnings.push(crate::spec::types::WarningItem {
        code: "LLM_ENRICHMENT_APPLIED".to_string(),
        message: "LLM extraction enrichment applied to prereg parsing.".to_string(),
        details: serde_json::json!({}),
    });
    Ok(spec)
}

fn apply_llm_prereg_enrichment(prereg: &mut PreregSpec, llm_output_json: &str) {
    let parsed = serde_json::from_str::<serde_json::Value>(llm_output_json).ok();
    let parsed_ref = parsed
        .as_ref()
        .and_then(|v| v.get("parsed"))
        .cloned()
        .unwrap_or_else(|| serde_json::json!({}));

    let map_models = |items: &serde_json::Value| -> Vec<crate::prereg::types::AnalysisModelSpec> {
        items
            .as_array()
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|item| {
                let id = item.get("id")?.as_str()?.to_string();
                let dv = item.get("dv")?.as_str()?.to_string();
                let iv = item
                    .get("iv")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|x| x.as_str().map(|s| s.to_string()))
                            .collect::<Vec<String>>()
                    })
                    .unwrap_or_default();
                let controls = item
                    .get("controls")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|x| x.as_str().map(|s| s.to_string()))
                            .collect::<Vec<String>>()
                    })
                    .unwrap_or_default();
                let interaction_terms = item
                    .get("interactionTerms")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|x| x.as_str().map(|s| s.to_string()))
                            .collect::<Vec<String>>()
                    })
                    .unwrap_or_default();
                let formula = item
                    .get("formula")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                Some(crate::prereg::types::AnalysisModelSpec {
                    id,
                    dv,
                    iv,
                    controls,
                    interaction_terms,
                    formula,
                })
            })
            .collect()
    };

    let llm_main = map_models(
        parsed_ref
            .get("mainModels")
            .unwrap_or(&serde_json::Value::Null),
    );
    if !llm_main.is_empty() {
        prereg.main_analyses = llm_main;
    }
    let llm_expl = map_models(
        parsed_ref
            .get("exploratoryModels")
            .unwrap_or(&serde_json::Value::Null),
    );
    if !llm_expl.is_empty() {
        prereg.exploratory_analyses = llm_expl;
    }
    let llm_mech = map_models(
        parsed_ref
            .get("mechanismModels")
            .unwrap_or(&serde_json::Value::Null),
    );
    if !llm_mech.is_empty() {
        prereg
            .exploratory_analyses
            .extend(llm_mech.into_iter().map(|mut m| {
                if m.id.is_empty() {
                    m.id = "llm_mechanism".to_string();
                }
                m
            }));
    }

    if let Some(arr) = parsed_ref
        .get("robustnessChecks")
        .and_then(|v| v.as_array())
    {
        prereg.robustness_checks = arr
            .iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect();
    }

    if let Some(vars) = parsed_ref.get("variables") {
        if let Some(mediators) = vars.get("mediators").and_then(|v| v.as_array()) {
            prereg.variables.mediators = mediators
                .iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect();
        }
        if let Some(moderators) = vars.get("moderators").and_then(|v| v.as_array()) {
            prereg.variables.moderators = moderators
                .iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect();
        }
    }

    if let Some(amb) = parsed_ref.get("ambiguities").and_then(|v| v.as_array()) {
        prereg.warnings.extend(
            amb.iter()
                .filter_map(|v| v.as_str().map(|s| format!("LLM_AMBIGUITY: {s}"))),
        );
    }
}

fn collect_candidate_tokens_from_prereg(prereg: &PreregSpec) -> Vec<String> {
    let mut tokens = Vec::new();
    tokens.extend(prereg.variables.dv.clone());
    tokens.extend(prereg.variables.iv.clone());
    tokens.extend(prereg.variables.controls.clone());
    for m in &prereg.main_analyses {
        tokens.push(m.dv.clone());
        tokens.extend(m.iv.clone());
        tokens.extend(m.controls.clone());
    }
    tokens.sort();
    tokens.dedup();
    tokens
}

fn load_saved_spec(
    app: &AppHandle,
    project_id: &str,
    study_id: &str,
    analysis_id: &str,
) -> Result<AnalysisSpec, String> {
    let root = analysis_root(app, project_id, study_id, analysis_id)?;
    let (spec_path, _, _) = analysis_paths(&root);
    if !spec_path.exists() {
        return Err("No saved spec".to_string());
    }
    let raw =
        fs::read_to_string(&spec_path).map_err(|e| format!("Unable to read saved spec: {e}"))?;
    serde_json::from_str(&raw).map_err(|e| format!("Invalid saved spec.json: {e}"))
}

fn apply_saved_mappings(spec: &mut AnalysisSpec, saved: &AnalysisSpec) {
    for current in &mut spec.variable_mappings {
        if let Some(previous) = saved
            .variable_mappings
            .iter()
            .find(|m| m.prereg_var.eq_ignore_ascii_case(&current.prereg_var))
        {
            if previous.resolved_to.is_some() {
                current.resolved_to = previous.resolved_to.clone();
            }
        }
    }

    let unresolved = spec
        .variable_mappings
        .iter()
        .filter(|m| m.resolved_to.is_none())
        .map(|m| m.prereg_var.to_lowercase())
        .collect::<std::collections::HashSet<String>>();
    spec.warnings.retain(|w| {
        if w.code != "UNRESOLVED_VARIABLE" {
            return true;
        }
        let prereg_var = w
            .details
            .get("preregVar")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_lowercase();
        unresolved.contains(&prereg_var)
    });
}

#[tauri::command]
pub fn save_analysis_spec(app: AppHandle, args: SaveSpecArgs) -> Result<(), String> {
    let root = analysis_root(&app, &args.project_id, &args.study_id, &args.analysis_id)?;
    ensure_dir(&root.join("analysis"))?;
    let (spec_path, _, _) = analysis_paths(&root);
    write_string(
        &spec_path,
        &serde_json::to_string_pretty(&args.spec).map_err(|e| e.to_string())?,
    )
}

fn read_spec(
    app: &AppHandle,
    project_id: &str,
    study_id: &str,
    analysis_id: &str,
) -> Result<AnalysisSpec, String> {
    let root = analysis_root(app, project_id, study_id, analysis_id)?;
    let (spec_path, _, _) = analysis_paths(&root);
    let raw = fs::read_to_string(&spec_path).map_err(|e| format!("Unable to read spec: {e}"))?;
    serde_json::from_str(&raw).map_err(|e| format!("Invalid spec.json: {e}"))
}

#[tauri::command]
pub fn resolve_mappings(app: AppHandle, args: ResolveMappingsArgs) -> Result<AnalysisSpec, String> {
    let mut spec = read_spec(&app, &args.project_id, &args.study_id, &args.analysis_id)?;
    for upd in args.mapping_updates {
        if let Some(m) = spec
            .variable_mappings
            .iter_mut()
            .find(|m| m.prereg_var.eq_ignore_ascii_case(&upd.prereg_var))
        {
            m.resolved_to = Some(upd.resolved_to.clone());
        } else {
            spec.variable_mappings.push(MappingResult {
                prereg_var: upd.prereg_var,
                resolved_to: Some(upd.resolved_to),
                candidates: Vec::new(),
            });
        }
    }
    spec.warnings
        .retain(|w| !(w.code == "UNRESOLVED_VARIABLE" && is_mapped(&spec.variable_mappings, w)));

    let root = analysis_root(&app, &args.project_id, &args.study_id, &args.analysis_id)?;
    let (spec_path, _, _) = analysis_paths(&root);
    write_string(
        &spec_path,
        &serde_json::to_string_pretty(&spec).map_err(|e| e.to_string())?,
    )?;
    Ok(spec)
}

fn is_mapped(mappings: &[MappingResult], warning: &crate::spec::types::WarningItem) -> bool {
    let prereg_var = warning
        .details
        .get("preregVar")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    mappings
        .iter()
        .any(|m| m.prereg_var.eq_ignore_ascii_case(prereg_var) && m.resolved_to.is_some())
}

#[tauri::command]
pub fn render_analysis_from_spec(app: AppHandle, args: RenderArgs) -> Result<RenderOutput, String> {
    let spec = read_spec(&app, &args.project_id, &args.study_id, &args.analysis_id)?;
    let root = analysis_root(&app, &args.project_id, &args.study_id, &args.analysis_id)?;
    ensure_dir(&root.join("analysis"))?;
    ensure_dir(&root.join("tables"))?;
    ensure_dir(&root.join("figures"))?;

    let (_, rmd_path, r_path) = analysis_paths(&root);
    let metadata_path = root.join("analysis").join("analysis_provenance.json");
    let project_root = resolve_project_root(&app, &args.project_id)?;
    let project_lock = read_project_lock(&project_root)?;
    let template_root = template_root_from_cwd()?;
    render_from_spec(&spec, &template_root, &rmd_path, &r_path)?;
    write_string(
        &metadata_path,
        &serde_json::to_string_pretty(&serde_json::json!({
          "analysisId": spec.analysis_id,
          "projectId": spec.project_id,
          "studyId": spec.study_id,
          "appVersion": env!("CARGO_PKG_VERSION"),
          "modelProvenance": spec.model_provenance,
          "projectLock": spec.model_lock.clone().or(project_lock),
        }))
        .map_err(|e| e.to_string())?,
    )?;

    Ok(RenderOutput {
        rmd_path: rmd_path.to_string_lossy().to_string(),
        r_path: r_path.to_string_lossy().to_string(),
    })
}
