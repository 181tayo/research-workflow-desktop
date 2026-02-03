#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use chrono::Utc;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tauri::{AppHandle, Manager};
use uuid::Uuid;

const PROJECT_FOLDERS: &[&str] = &["studies", "project_docs", "materials", "data"];
const STUDY_FOLDERS: &[&str] = &[
  "01_admin",
  "02_materials",
  "03_data",
  "03_data/raw",
  "03_data/processed",
  "04_analysis",
  "05_results",
  "06_manuscript",
  "07_assets",
  "08_osf_release",
  "pilots"
];

#[derive(Debug, Serialize, Deserialize)]
struct Project {
  id: String,
  name: String,
  root_path: String,
  created_at: String
}

#[derive(Debug, Serialize, Deserialize)]
struct Study {
  id: String,
  project_id: String,
  internal_name: String,
  paper_label: Option<String>,
  status: String,
  folder_path: String,
  created_at: String
}

#[derive(Debug, Serialize, Deserialize)]
struct Artifact {
  id: String,
  study_id: String,
  kind: String,
  value: String,
  label: Option<String>,
  created_at: String
}

#[derive(Debug, Serialize, Deserialize)]
struct StudyDetail {
  study: Study,
  artifacts: Vec<Artifact>
}

fn app_root(app: &AppHandle) -> Result<PathBuf, String> {
  let base = tauri::api::path::app_data_dir(&app.config())
    .ok_or_else(|| "Unable to resolve app data dir".to_string())?;
  let root = base.join("research-workflow");
  fs::create_dir_all(&root).map_err(|err| err.to_string())?;
  Ok(root)
}

fn db_path(app: &AppHandle) -> Result<PathBuf, String> {
  let root = app_root(app)?;
  Ok(root.join("db.sqlite3"))
}

fn connection(app: &AppHandle) -> Result<Connection, String> {
  let path = db_path(app)?;
  Connection::open(path).map_err(|err| err.to_string())
}

fn init_schema(conn: &Connection) -> Result<(), String> {
  conn.execute_batch(
    "CREATE TABLE IF NOT EXISTS projects (
        id TEXT PRIMARY KEY,
        name TEXT NOT NULL,
        root_path TEXT NOT NULL,
        created_at TEXT NOT NULL
      );
      CREATE TABLE IF NOT EXISTS studies (
        id TEXT PRIMARY KEY,
        project_id TEXT NOT NULL,
        internal_name TEXT NOT NULL,
        paper_label TEXT,
        status TEXT NOT NULL,
        folder_path TEXT NOT NULL,
        created_at TEXT NOT NULL,
        FOREIGN KEY(project_id) REFERENCES projects(id)
      );
      CREATE INDEX IF NOT EXISTS idx_studies_project ON studies(project_id);
      CREATE TABLE IF NOT EXISTS artifacts (
        id TEXT PRIMARY KEY,
        study_id TEXT NOT NULL,
        kind TEXT NOT NULL,
        value TEXT NOT NULL,
        label TEXT,
        created_at TEXT NOT NULL,
        FOREIGN KEY(study_id) REFERENCES studies(id)
      );
      CREATE INDEX IF NOT EXISTS idx_artifacts_study ON artifacts(study_id);"
  )
  .map_err(|err| err.to_string())?;
  Ok(())
}

fn now_string() -> String {
  Utc::now().to_rfc3339()
}

fn ensure_folders(root: &Path, folders: &[&str]) -> Result<(), String> {
  for folder in folders {
    fs::create_dir_all(root.join(folder)).map_err(|err| err.to_string())?;
  }
  Ok(())
}

fn should_skip(path: &Path, include_pilots: bool, condensed: bool) -> bool {
  let path_str = path.to_string_lossy().to_lowercase();
  if path_str.contains("08_osf_release") {
    return true;
  }
  if path_str.contains("/.git") || path_str.contains("node_modules") {
    return true;
  }
  if !include_pilots && (path_str.contains("/pilots/") || path_str.contains("pilot")) {
    return true;
  }
  if condensed {
    if path_str.contains("/raw/")
      || path_str.contains("raw_data")
      || path_str.contains("03_data/raw")
    {
      return true;
    }
  }
  false
}

fn copy_dir_filtered(
  src: &Path,
  dst: &Path,
  include_pilots: bool,
  condensed: bool
) -> Result<u64, String> {
  if should_skip(src, include_pilots, condensed) {
    return Ok(0);
  }

  if !dst.exists() {
    fs::create_dir_all(dst).map_err(|err| err.to_string())?;
  }

  let mut copied = 0;
  for entry in fs::read_dir(src).map_err(|err| err.to_string())? {
    let entry = entry.map_err(|err| err.to_string())?;
    let path = entry.path();
    if should_skip(&path, include_pilots, condensed) {
      continue;
    }
    let target = dst.join(entry.file_name());
    if path.is_dir() {
      copied += copy_dir_filtered(&path, &target, include_pilots, condensed)?;
    } else if path.is_file() {
      if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(|err| err.to_string())?;
      }
      fs::copy(&path, &target).map_err(|err| err.to_string())?;
      copied += 1;
    }
  }
  Ok(copied)
}

#[tauri::command]
fn init_db(app: AppHandle) -> Result<(), String> {
  let conn = connection(&app)?;
  init_schema(&conn)?;
  Ok(())
}

#[tauri::command]
fn list_projects(app: AppHandle) -> Result<Vec<Project>, String> {
  let conn = connection(&app)?;
  init_schema(&conn)?;
  let mut stmt = conn
    .prepare("SELECT id, name, root_path, created_at FROM projects ORDER BY created_at DESC")
    .map_err(|err| err.to_string())?;
  let rows = stmt
    .query_map([], |row| {
      Ok(Project {
        id: row.get(0)?,
        name: row.get(1)?,
        root_path: row.get(2)?,
        created_at: row.get(3)?
      })
    })
    .map_err(|err| err.to_string())?;

  let mut projects = Vec::new();
  for row in rows {
    projects.push(row.map_err(|err| err.to_string())?);
  }
  Ok(projects)
}

#[tauri::command]
fn create_project(app: AppHandle, name: String) -> Result<Project, String> {
  let conn = connection(&app)?;
  init_schema(&conn)?;

  let id = Uuid::new_v4().to_string();
  let root = app_root(&app)?.join("projects").join(&id);
  ensure_folders(&root, PROJECT_FOLDERS)?;

  let project = Project {
    id: id.clone(),
    name,
    root_path: root.to_string_lossy().to_string(),
    created_at: now_string()
  };

  conn
    .execute(
      "INSERT INTO projects (id, name, root_path, created_at) VALUES (?1, ?2, ?3, ?4)",
      params![project.id, project.name, project.root_path, project.created_at]
    )
    .map_err(|err| err.to_string())?;

  Ok(project)
}

#[tauri::command]
fn list_studies(app: AppHandle, project_id: String) -> Result<Vec<Study>, String> {
  let conn = connection(&app)?;
  init_schema(&conn)?;
  let mut stmt = conn
    .prepare(
      "SELECT id, project_id, internal_name, paper_label, status, folder_path, created_at \
      FROM studies WHERE project_id = ?1 ORDER BY created_at DESC"
    )
    .map_err(|err| err.to_string())?;
  let rows = stmt
    .query_map(params![project_id], |row| {
      Ok(Study {
        id: row.get(0)?,
        project_id: row.get(1)?,
        internal_name: row.get(2)?,
        paper_label: row.get(3)?,
        status: row.get(4)?,
        folder_path: row.get(5)?,
        created_at: row.get(6)?
      })
    })
    .map_err(|err| err.to_string())?;

  let mut studies = Vec::new();
  for row in rows {
    studies.push(row.map_err(|err| err.to_string())?);
  }
  Ok(studies)
}

#[tauri::command]
fn create_study(
  app: AppHandle,
  project_id: String,
  internal_name: String,
  paper_label: Option<String>
) -> Result<Study, String> {
  let conn = connection(&app)?;
  init_schema(&conn)?;

  let project_root: String = conn
    .query_row(
      "SELECT root_path FROM projects WHERE id = ?1",
      params![project_id],
      |row| row.get(0)
    )
    .map_err(|err| err.to_string())?;

  let id = Uuid::new_v4().to_string();
  let folder = PathBuf::from(project_root).join("studies").join(&id);
  ensure_folders(&folder, STUDY_FOLDERS)?;

  let study = Study {
    id: id.clone(),
    project_id,
    internal_name,
    paper_label,
    status: "planning".to_string(),
    folder_path: folder.to_string_lossy().to_string(),
    created_at: now_string()
  };

  conn
    .execute(
      "INSERT INTO studies (id, project_id, internal_name, paper_label, status, folder_path, created_at) \
      VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
      params![
        study.id,
        study.project_id,
        study.internal_name,
        study.paper_label,
        study.status,
        study.folder_path,
        study.created_at
      ]
    )
    .map_err(|err| err.to_string())?;

  Ok(study)
}

#[tauri::command]
fn rename_study(
  app: AppHandle,
  study_id: String,
  internal_name: String,
  paper_label: Option<String>
) -> Result<(), String> {
  let conn = connection(&app)?;
  init_schema(&conn)?;
  conn
    .execute(
      "UPDATE studies SET internal_name = ?1, paper_label = ?2 WHERE id = ?3",
      params![internal_name, paper_label, study_id]
    )
    .map_err(|err| err.to_string())?;
  Ok(())
}

#[tauri::command]
fn update_study_status(
  app: AppHandle,
  study_id: String,
  status: String
) -> Result<(), String> {
  let conn = connection(&app)?;
  init_schema(&conn)?;
  conn
    .execute(
      "UPDATE studies SET status = ?1 WHERE id = ?2",
      params![status, study_id]
    )
    .map_err(|err| err.to_string())?;
  Ok(())
}

#[tauri::command]
fn get_study_detail(app: AppHandle, study_id: String) -> Result<StudyDetail, String> {
  let conn = connection(&app)?;
  init_schema(&conn)?;

  let study: Study = conn
    .query_row(
      "SELECT id, project_id, internal_name, paper_label, status, folder_path, created_at \
      FROM studies WHERE id = ?1",
      params![study_id],
      |row| {
        Ok(Study {
          id: row.get(0)?,
          project_id: row.get(1)?,
          internal_name: row.get(2)?,
          paper_label: row.get(3)?,
          status: row.get(4)?,
          folder_path: row.get(5)?,
          created_at: row.get(6)?
        })
      }
    )
    .map_err(|err| err.to_string())?;

  let mut stmt = conn
    .prepare(
      "SELECT id, study_id, kind, value, label, created_at FROM artifacts WHERE study_id = ?1 \
      ORDER BY created_at DESC"
    )
    .map_err(|err| err.to_string())?;

  let rows = stmt
    .query_map(params![study_id], |row| {
      Ok(Artifact {
        id: row.get(0)?,
        study_id: row.get(1)?,
        kind: row.get(2)?,
        value: row.get(3)?,
        label: row.get(4)?,
        created_at: row.get(5)?
      })
    })
    .map_err(|err| err.to_string())?;

  let mut artifacts = Vec::new();
  for row in rows {
    artifacts.push(row.map_err(|err| err.to_string())?);
  }

  Ok(StudyDetail { study, artifacts })
}

#[tauri::command]
fn add_artifact(
  app: AppHandle,
  study_id: String,
  kind: String,
  value: String,
  label: Option<String>
) -> Result<(), String> {
  let conn = connection(&app)?;
  init_schema(&conn)?;
  let id = Uuid::new_v4().to_string();
  conn
    .execute(
      "INSERT INTO artifacts (id, study_id, kind, value, label, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
      params![id, study_id, kind, value, label, now_string()]
    )
    .map_err(|err| err.to_string())?;
  Ok(())
}

#[tauri::command]
fn remove_artifact(app: AppHandle, artifact_id: String) -> Result<(), String> {
  let conn = connection(&app)?;
  init_schema(&conn)?;
  conn
    .execute("DELETE FROM artifacts WHERE id = ?1", params![artifact_id])
    .map_err(|err| err.to_string())?;
  Ok(())
}

#[tauri::command]
fn generate_osf_packages(
  app: AppHandle,
  study_id: String,
  include_pilots: bool
) -> Result<String, String> {
  let conn = connection(&app)?;
  init_schema(&conn)?;

  let folder_path: String = conn
    .query_row(
      "SELECT folder_path FROM studies WHERE id = ?1",
      params![study_id],
      |row| row.get(0)
    )
    .map_err(|err| err.to_string())?;

  let study_root = PathBuf::from(folder_path);
  if !study_root.exists() {
    return Err("Study folder does not exist".to_string());
  }

  let osf_root = study_root.join("08_osf_release");
  let complete_root = osf_root.join("COMPLETE");
  let condensed_root = osf_root.join("CONDENSED");

  if complete_root.exists() {
    fs::remove_dir_all(&complete_root).map_err(|err| err.to_string())?;
  }
  if condensed_root.exists() {
    fs::remove_dir_all(&condensed_root).map_err(|err| err.to_string())?;
  }

  let complete_count = copy_dir_filtered(&study_root, &complete_root, include_pilots, false)?;
  let condensed_count = copy_dir_filtered(&study_root, &condensed_root, include_pilots, true)?;

  Ok(format!(
    "OSF packages generated. COMPLETE: {complete_count} files, CONDENSED: {condensed_count} files."
  ))
}

#[tauri::command]
fn git_status() -> Result<String, String> {
  let repo_root = std::env::current_dir().map_err(|err| err.to_string())?;
  let output = Command::new("git")
    .args(["status", "-sb"])
    .current_dir(repo_root)
    .output()
    .map_err(|err| err.to_string())?;
  if !output.status.success() {
    return Err(String::from_utf8_lossy(&output.stderr).to_string());
  }
  Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

#[tauri::command]
fn git_commit_push(message: String) -> Result<String, String> {
  let repo_root = std::env::current_dir().map_err(|err| err.to_string())?;

  let add_output = Command::new("git")
    .args(["add", "-A"])
    .current_dir(&repo_root)
    .output()
    .map_err(|err| err.to_string())?;
  if !add_output.status.success() {
    return Err(String::from_utf8_lossy(&add_output.stderr).to_string());
  }

  let commit_output = Command::new("git")
    .args(["commit", "-m", &message])
    .current_dir(&repo_root)
    .output()
    .map_err(|err| err.to_string())?;

  let commit_stdout = String::from_utf8_lossy(&commit_output.stdout).to_string();
  let commit_stderr = String::from_utf8_lossy(&commit_output.stderr).to_string();

  let no_changes = commit_stdout.contains("nothing to commit") || commit_stderr.contains("nothing to commit");
  if !commit_output.status.success() && !no_changes {
    return Err(commit_stderr);
  }

  let push_output = Command::new("git")
    .args(["push"])
    .current_dir(&repo_root)
    .output()
    .map_err(|err| err.to_string())?;

  if !push_output.status.success() {
    return Err(String::from_utf8_lossy(&push_output.stderr).to_string());
  }

  let push_stdout = String::from_utf8_lossy(&push_output.stdout).to_string();

  Ok(format!("{}{}", commit_stdout, push_stdout))
}

fn main() {
  tauri::Builder::default()
    .invoke_handler(tauri::generate_handler![
      init_db,
      list_projects,
      create_project,
      list_studies,
      create_study,
      rename_study,
      update_study_status,
      get_study_detail,
      add_artifact,
      remove_artifact,
      generate_osf_packages,
      git_status,
      git_commit_push
    ])
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}
