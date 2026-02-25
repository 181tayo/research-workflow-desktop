use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use tauri::AppHandle;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetRef {
    pub name: String,
    pub path: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StudyRef {
    id: String,
    #[serde(default)]
    #[serde(alias = "folder_path")]
    folder_path: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProjectRef {
    id: String,
    #[serde(alias = "root_path")]
    root_path: String,
    #[serde(default)]
    studies: Vec<StudyRef>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProjectsStore {
    projects: Vec<ProjectRef>,
}

fn app_data_root(app: &AppHandle) -> Result<PathBuf, String> {
    let base = tauri::api::path::app_data_dir(&app.config())
        .ok_or_else(|| "Unable to resolve app data dir".to_string())?;
    let root = base.join("research-workflow");
    fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    Ok(root)
}

fn projects_store_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(app_data_root(app)?.join("projects.json"))
}

fn read_projects_store(app: &AppHandle) -> Result<ProjectsStore, String> {
    let path = projects_store_path(app)?;
    if !path.exists() {
        return Ok(ProjectsStore {
            projects: Vec::new(),
        });
    }
    let raw = fs::read_to_string(path).map_err(|e| e.to_string())?;
    if raw.trim().is_empty() {
        return Ok(ProjectsStore {
            projects: Vec::new(),
        });
    }
    serde_json::from_str(&raw).map_err(|e| format!("Invalid projects.json: {e}"))
}

pub(crate) fn resolve_study_root(
    app: &AppHandle,
    project_id: &str,
    study_id: &str,
) -> Result<PathBuf, String> {
    let store = read_projects_store(app)?;
    let project = store
        .projects
        .iter()
        .find(|p| p.id == project_id)
        .ok_or_else(|| "Project not found.".to_string())?;
    let study = project
        .studies
        .iter()
        .find(|s| s.id == study_id)
        .ok_or_else(|| "Study not found.".to_string())?;

    if !study.folder_path.trim().is_empty() {
        Ok(PathBuf::from(study.folder_path.clone()))
    } else {
        Ok(PathBuf::from(project.root_path.clone())
            .join("studies")
            .join(study_id))
    }
}

pub(crate) fn resolve_project_root(app: &AppHandle, project_id: &str) -> Result<PathBuf, String> {
    let store = read_projects_store(app)?;
    let project = store
        .projects
        .iter()
        .find(|p| p.id == project_id)
        .ok_or_else(|| "Project not found.".to_string())?;
    Ok(PathBuf::from(project.root_path.clone()))
}

fn visit_files_recursive(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    if !dir.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        let meta = entry.metadata().map_err(|e| e.to_string())?;
        if meta.is_dir() {
            visit_files_recursive(&path, out)?;
        } else if meta.is_file() {
            out.push(path);
        }
    }
    Ok(())
}

fn list_files_in(dir: &Path) -> Result<Vec<AssetRef>, String> {
    let mut files = Vec::new();
    visit_files_recursive(dir, &mut files)?;
    let mut out = files
        .into_iter()
        .filter_map(|path| {
            let name = path.file_name()?.to_string_lossy().to_string();
            Some(AssetRef {
                name,
                path: path.to_string_lossy().to_string(),
            })
        })
        .collect::<Vec<AssetRef>>();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

#[tauri::command]
pub fn list_build_assets(
    app: AppHandle,
    project_id: String,
    study_id: String,
) -> Result<Vec<AssetRef>, String> {
    let root = resolve_study_root(&app, &project_id, &study_id)?;
    let primary = root.join("inputs").join("build");
    let fallback = root.join("02_build");
    let mut out = list_files_in(&primary)?;
    if out.is_empty() {
        out = list_files_in(&fallback)?;
    }
    Ok(out
        .into_iter()
        .filter(|a| {
            let p = a.path.to_lowercase();
            p.ends_with(".qsf") || p.ends_with(".qsf.json") || p.ends_with(".json")
        })
        .collect())
}

#[tauri::command]
pub fn list_prereg_assets(
    app: AppHandle,
    project_id: String,
    study_id: String,
) -> Result<Vec<AssetRef>, String> {
    let root = resolve_study_root(&app, &project_id, &study_id)?;
    let primary = root.join("inputs").join("prereg");
    let fallback = root.join("04_prereg");
    let mut out = list_files_in(&primary)?;
    if out.is_empty() {
        out = list_files_in(&fallback)?;
    }
    Ok(out
        .into_iter()
        .filter(|a| {
            let p = a.path.to_lowercase();
            p.ends_with(".docx")
                || p.ends_with(".md")
                || p.ends_with(".markdown")
                || p.ends_with(".json")
                || p.ends_with(".txt")
        })
        .collect())
}

pub(crate) fn read_file_bytes(path: &str) -> Result<Vec<u8>, String> {
    fs::read(path).map_err(|e| format!("Unable to read bytes from {path}: {e}"))
}

pub(crate) fn read_file_text(path: &str) -> Result<String, String> {
    fs::read_to_string(path).map_err(|e| format!("Unable to read text from {path}: {e}"))
}
