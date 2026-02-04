#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use chrono::Utc;
use pathdiff::diff_paths;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tauri::{AppHandle, Manager};
use uuid::Uuid;

const PROJECT_FOLDERS: &[&str] = &["studies", "paper", "templates"];
const STUDY_FOLDERS: &[&str] = &[
  "00_admin",
  "01_design",
  "02_build",
  "03_pilots",
  "04_prereg",
  "05_data",
  "06_analysis",
  "07_reports",
  "08_osf_release"
];

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Project {
  id: String,
  name: String,
  #[serde(alias = "root_path")]
  root_path: String,
  #[serde(alias = "created_at")]
  created_at: String,
  #[serde(default)]
  #[serde(alias = "updated_at")]
  updated_at: String,
  #[serde(default)]
  #[serde(alias = "google_drive_url")]
  google_drive_url: Option<String>,
  #[serde(default)]
  studies: Vec<Study>
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ProjectsStore {
  projects: Vec<Project>
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Study {
  id: String,
  title: String,
  #[serde(alias = "created_at")]
  created_at: String,
  #[serde(default)]
  #[serde(alias = "folder_path")]
  folder_path: String,
  #[serde(default)]
  files: Vec<FileRef>
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct FileRef {
  pub path: String,
  pub name: String,
  pub kind: String
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct DbStudy {
  id: String,
  #[serde(alias = "project_id")]
  project_id: String,
  #[serde(alias = "internal_name")]
  internal_name: String,
  #[serde(alias = "paper_label")]
  paper_label: Option<String>,
  status: String,
  #[serde(alias = "folder_path")]
  folder_path: String,
  #[serde(alias = "created_at")]
  created_at: String
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Artifact {
  id: String,
  #[serde(alias = "study_id")]
  study_id: String,
  kind: String,
  value: String,
  label: Option<String>,
  #[serde(alias = "created_at")]
  created_at: String
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct StudyDetail {
  study: DbStudy,
  artifacts: Vec<Artifact>
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct RootDirInfo {
  exists: bool,
  is_git_repo: bool
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

fn projects_path(app: &AppHandle) -> Result<PathBuf, String> {
  let root = app_root(app)?;
  Ok(root.join("projects.json"))
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

fn is_valid_study_folder(value: &str) -> bool {
  let mut chars = value.chars();
  if chars.next() != Some('S') || chars.next() != Some('-') {
    return false;
  }
  let rest: Vec<char> = chars.collect();
  if rest.len() != 6 {
    return false;
  }
  rest.iter().all(|ch| ch.is_ascii_alphanumeric())
}

fn generate_study_code() -> String {
  let raw = Uuid::new_v4().simple().to_string().to_uppercase();
  format!("S-{}", &raw[..6])
}

fn read_projects_store(app: &AppHandle) -> Result<ProjectsStore, String> {
  let path = projects_path(app)?;
  if !path.exists() {
    return Ok(ProjectsStore { projects: Vec::new() });
  }
  let raw = fs::read_to_string(&path).map_err(|err| err.to_string())?;
  if raw.trim().is_empty() {
    return Ok(ProjectsStore { projects: Vec::new() });
  }
  let mut store: ProjectsStore =
    serde_json::from_str(&raw).map_err(|err| err.to_string())?;
  for project in &mut store.projects {
    if project.updated_at.is_empty() {
      project.updated_at = project.created_at.clone();
    }
  }
  Ok(store)
}

fn write_projects_store(app: &AppHandle, store: &ProjectsStore) -> Result<(), String> {
  let path = projects_path(app)?;
  let payload = serde_json::to_string_pretty(store).map_err(|err| err.to_string())?;
  fs::write(path, payload).map_err(|err| err.to_string())?;
  Ok(())
}

fn migrate_sqlite_projects(app: &AppHandle) -> Result<(), String> {
  let db = db_path(app)?;
  if !db.exists() {
    return Ok(());
  }

  let conn = Connection::open(db).map_err(|err| err.to_string())?;
  let table_exists: i64 = conn
    .query_row(
      "SELECT COUNT(1) FROM sqlite_master WHERE type='table' AND name='projects'",
      [],
      |row| row.get(0)
    )
    .map_err(|err| err.to_string())?;
  if table_exists == 0 {
    return Ok(());
  }

  let mut stmt = conn
    .prepare("SELECT id, name, root_path, created_at FROM projects")
    .map_err(|err| err.to_string())?;
  let rows = stmt
    .query_map([], |row| {
      Ok(Project {
        id: row.get(0)?,
        name: row.get(1)?,
        root_path: row.get(2)?,
        created_at: row.get(3)?,
        updated_at: row.get(3)?,
        google_drive_url: None,
        studies: Vec::new()
      })
    })
    .map_err(|err| err.to_string())?;

  let mut sqlite_projects = Vec::new();
  for row in rows {
    sqlite_projects.push(row.map_err(|err| err.to_string())?);
  }
  if sqlite_projects.is_empty() {
    return Ok(());
  }

  let mut store = read_projects_store(app)?;
  let mut added = 0;
  for project in sqlite_projects {
    if !store.projects.iter().any(|p| p.id == project.id) {
      store.projects.push(project);
      added += 1;
    }
  }
  if added > 0 {
    write_projects_store(app, &store)?;
    println!("migration: imported {} project(s) from sqlite", added);
  } else {
    println!("migration: no new projects to import from sqlite");
  }

  Ok(())
}

fn ensure_folders(root: &Path, folders: &[&str]) -> Result<(), String> {
  for folder in folders {
    fs::create_dir_all(root.join(folder)).map_err(|err| err.to_string())?;
  }
  Ok(())
}

fn kind_from_ext(ext: Option<&OsStr>) -> String {
  let value = ext
    .and_then(|value| value.to_str())
    .unwrap_or("")
    .to_lowercase();
  match value.as_str() {
    "pdf" => "pdf".to_string(),
    "md" | "markdown" => "md".to_string(),
    "txt" => "txt".to_string(),
    "doc" | "docx" => "docx".to_string(),
    "csv" => "csv".to_string(),
    "json" => "json".to_string(),
    "png" => "png".to_string(),
    "jpg" | "jpeg" => "jpg".to_string(),
    _ => "other".to_string()
  }
}

fn unique_dest_path(dest_dir: &Path, filename: &OsStr) -> PathBuf {
  let candidate = dest_dir.join(filename);
  if !candidate.exists() {
    return candidate;
  }

  let filename_str = filename.to_string_lossy();
  let path = Path::new(&*filename_str);
  let stem = path
    .file_stem()
    .and_then(|value| value.to_str())
    .unwrap_or("file");
  let ext = path.extension().and_then(|value| value.to_str()).unwrap_or("");
  let ext_suffix = if ext.is_empty() {
    String::new()
  } else {
    format!(".{ext}")
  };

  for index in 1..=10_000 {
    let next = format!("{stem} ({index}){ext_suffix}");
    let candidate = dest_dir.join(next);
    if !candidate.exists() {
      return candidate;
    }
  }

  candidate
}

fn move_file_cross_device(src: &Path, dst: &Path) -> Result<(), String> {
  if src == dst {
    return Ok(());
  }
  match fs::rename(src, dst) {
    Ok(()) => Ok(()),
    Err(_) => {
      fs::copy(src, dst).map_err(|err| err.to_string())?;
      fs::remove_file(src).map_err(|err| err.to_string())?;
      Ok(())
    }
  }
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
  migrate_sqlite_projects(&app)?;
  let mut store = read_projects_store(&app)?;
  store
    .projects
    .sort_by(|a, b| b.created_at.cmp(&a.created_at));
  Ok(store.projects)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateProjectArgs {
  name: String,
  root_dir: String,
  #[serde(default)]
  use_existing_root: bool,
  google_drive_url: Option<String>
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateProjectRootArgs {
  project_id: String,
  root_dir: String
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeleteProjectArgs {
  project_id: String,
  #[serde(default)]
  delete_on_disk: bool
}

#[tauri::command]
fn create_project(app: AppHandle, args: CreateProjectArgs) -> Result<Project, String> {
  let id = Uuid::new_v4().to_string();
  let trimmed_name = args.name.trim();
  if trimmed_name.is_empty() {
    return Err("Project name is required.".to_string());
  }
  let root_dir_path = PathBuf::from(args.root_dir.trim());
  if !root_dir_path.exists() || !root_dir_path.is_dir() {
    return Err("Project root location must be an existing folder.".to_string());
  }

  let root = if args.use_existing_root {
    root_dir_path
  } else {
    let root = root_dir_path.join(trimmed_name);
    if root.exists() {
      return Err("Project folder already exists.".to_string());
    }
    root
  };
  ensure_folders(&root, PROJECT_FOLDERS)?;

  let project = Project {
    id: id.clone(),
    name: trimmed_name.to_string(),
    root_path: root.to_string_lossy().to_string(),
    created_at: now_string(),
    updated_at: now_string(),
    google_drive_url: args.google_drive_url
      .and_then(|value| {
        let trimmed = value.trim().to_string();
        if trimmed.is_empty() {
          None
        } else {
          Some(trimmed)
        }
      }),
    studies: Vec::new()
  };

  let mut store = read_projects_store(&app)?;
  store.projects.push(project.clone());
  write_projects_store(&app, &store)?;

  Ok(project)
}

#[tauri::command]
fn update_project_root(app: AppHandle, args: UpdateProjectRootArgs) -> Result<Project, String> {
  let root_dir_path = PathBuf::from(args.root_dir.trim());
  if !root_dir_path.exists() || !root_dir_path.is_dir() {
    return Err("Project root location must be an existing folder.".to_string());
  }

  let mut store = read_projects_store(&app)?;
  let project = store
    .projects
    .iter_mut()
    .find(|project| project.id == args.project_id)
    .ok_or_else(|| "Project not found.".to_string())?;

  ensure_folders(&root_dir_path, PROJECT_FOLDERS)?;
  project.root_path = root_dir_path.to_string_lossy().to_string();
  project.updated_at = now_string();

  let updated = project.clone();
  write_projects_store(&app, &store)?;
  Ok(updated)
}

#[tauri::command]
fn delete_project(app: AppHandle, args: DeleteProjectArgs) -> Result<(), String> {
  let mut store = read_projects_store(&app)?;
  let mut root_to_delete: Option<PathBuf> = None;
  let before = store.projects.len();
  store.projects.retain(|project| {
    if project.id == args.project_id {
      if args.delete_on_disk {
        root_to_delete = Some(PathBuf::from(project.root_path.clone()));
      }
      return false;
    }
    true
  });
  if store.projects.len() == before {
    return Err("Project not found.".to_string());
  }

  if let Some(root) = root_to_delete {
    let normalized = root.to_path_buf();
    let component_count = normalized.components().count();
    if component_count < 2 {
      return Err("Refusing to delete an unsafe root directory.".to_string());
    }
    if normalized.exists() && normalized.is_dir() {
      fs::remove_dir_all(&normalized).map_err(|err| err.to_string())?;
    }
  }
  write_projects_store(&app, &store)?;
  Ok(())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AddStudyArgs {
  project_id: String,
  folder_name: Option<String>,
  title: Option<String>
}

#[tauri::command]
fn add_study(app: AppHandle, args: AddStudyArgs) -> Result<Project, String> {
  println!(
    "add_study called with project_id={}, folder_name={:?}, title={:?}",
    args.project_id, args.folder_name, args.title
  );
  let mut store = read_projects_store(&app)?;
  let project = store
    .projects
    .iter_mut()
    .find(|project| project.id == args.project_id)
    .ok_or_else(|| "Project not found.".to_string())?;
  println!(
    "add_study resolved project root_path={} existing studies={}",
    project.root_path,
    project.studies.len()
  );

  let mut trimmed_folder = args.folder_name.unwrap_or_default().trim().to_uppercase();
  if trimmed_folder.is_empty() {
    for _ in 0..20 {
      let candidate = generate_study_code();
      let candidate_root = PathBuf::from(project.root_path.clone())
        .join("studies")
        .join(&candidate);
      if !candidate_root.exists()
        && !project.studies.iter().any(|study| study.id == candidate)
      {
        trimmed_folder = candidate;
        break;
      }
    }
    if trimmed_folder.is_empty() {
      return Err("Unable to generate a unique study code.".to_string());
    }
  }
  if !is_valid_study_folder(&trimmed_folder) {
    return Err("Study folder name must match S-XXXXXX (letters/numbers).".to_string());
  }
  if trimmed_folder.contains('/') || trimmed_folder.contains('\\') || trimmed_folder.contains("..") {
    return Err("Study folder name must be a single folder name.".to_string());
  }
  if project.studies.iter().any(|study| study.id == trimmed_folder) {
    return Err("Study code already exists.".to_string());
  }

  let trimmed_title = args.title.unwrap_or_else(|| "Untitled Study".to_string());
  let study_root = PathBuf::from(project.root_path.clone())
    .join("studies")
    .join(&trimmed_folder);
  if study_root.exists() {
    return Err("Study folder already exists.".to_string());
  }
  ensure_folders(&study_root, STUDY_FOLDERS)?;

  let new_study = Study {
    id: trimmed_folder.to_string(),
    title: if trimmed_title.trim().is_empty() {
      "Untitled Study".to_string()
    } else {
      trimmed_title
    },
    created_at: now_string(),
    folder_path: study_root.to_string_lossy().to_string(),
    files: Vec::new()
  };

  project.studies.push(new_study);
  project.updated_at = now_string();
  let updated = project.clone();
  write_projects_store(&app, &store)?;
  Ok(updated)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RenameStudyJsonArgs {
  project_id: String,
  study_id: String,
  title: String
}

#[tauri::command]
fn rename_study_json(app: AppHandle, args: RenameStudyJsonArgs) -> Result<Project, String> {
  let mut store = read_projects_store(&app)?;
  let project = store
    .projects
    .iter_mut()
    .find(|project| project.id == args.project_id)
    .ok_or_else(|| "Project not found.".to_string())?;

  let study = project
    .studies
    .iter_mut()
    .find(|study| study.id == args.study_id)
    .ok_or_else(|| "Study not found.".to_string())?;

  let trimmed = args.title.trim();
  if trimmed.is_empty() {
    return Err("Study title is required.".to_string());
  }

  study.title = trimmed.to_string();
  project.updated_at = now_string();
  let updated = project.clone();
  write_projects_store(&app, &store)?;
  Ok(updated)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RenameStudyFolderArgs {
  project_id: String,
  study_id: String,
  folder_name: String
}

#[tauri::command]
fn rename_study_folder_json(app: AppHandle, args: RenameStudyFolderArgs) -> Result<Project, String> {
  let mut store = read_projects_store(&app)?;
  let project = store
    .projects
    .iter_mut()
    .find(|project| project.id == args.project_id)
    .ok_or_else(|| "Project not found.".to_string())?;

  let trimmed_folder = args.folder_name.trim();
  if trimmed_folder.is_empty() {
    return Err("Study folder name is required.".to_string());
  }
  if !is_valid_study_folder(trimmed_folder) {
    return Err("Study folder name must match S-XXXXXX (letters/numbers).".to_string());
  }
  if trimmed_folder.contains('/') || trimmed_folder.contains('\\') || trimmed_folder.contains("..") {
    return Err("Study folder name must be a single folder name.".to_string());
  }
  if project
    .studies
    .iter()
    .any(|study| study.id == trimmed_folder && study.id != args.study_id)
  {
    return Err("Study code already exists.".to_string());
  }

  let study = project
    .studies
    .iter_mut()
    .find(|study| study.id == args.study_id)
    .ok_or_else(|| "Study not found.".to_string())?;

  let base = PathBuf::from(project.root_path.clone()).join("studies");
  let old_root = if study.folder_path.trim().is_empty() {
    base.join(&study.id)
  } else {
    PathBuf::from(study.folder_path.clone())
  };
  let new_root = base.join(trimmed_folder);

  if old_root != new_root {
    if new_root.exists() {
      return Err("Study folder already exists.".to_string());
    }
    if !old_root.exists() {
      return Err("Study folder does not exist.".to_string());
    }
    fs::rename(&old_root, &new_root).map_err(|err| err.to_string())?;
  }

  study.id = trimmed_folder.to_string();
  study.folder_path = new_root.to_string_lossy().to_string();
  project.updated_at = now_string();

  let updated = project.clone();
  write_projects_store(&app, &store)?;
  Ok(updated)
}

#[tauri::command]
fn migrate_json_to_sqlite(app: AppHandle) -> Result<String, String> {
  let conn = connection(&app)?;
  init_schema(&conn)?;
  let store = read_projects_store(&app)?;

  let mut projects_added = 0;
  let mut studies_added = 0;

  for project in store.projects {
    let project_id = project.id.clone();
    let project_name = project.name.clone();
    let project_root = project.root_path.clone();
    let project_created = project.created_at.clone();
    let exists: i64 = conn
      .query_row(
        "SELECT COUNT(1) FROM projects WHERE id = ?1",
        params![&project_id],
        |row| row.get(0)
      )
      .map_err(|err| err.to_string())?;

    if exists == 0 {
      conn
        .execute(
          "INSERT INTO projects (id, name, root_path, created_at) VALUES (?1, ?2, ?3, ?4)",
          params![&project_id, &project_name, &project_root, &project_created]
        )
        .map_err(|err| err.to_string())?;
      projects_added += 1;
    }

    for study in project.studies {
      let study_exists: i64 = conn
        .query_row(
          "SELECT COUNT(1) FROM studies WHERE id = ?1",
          params![study.id],
          |row| row.get(0)
        )
        .map_err(|err| err.to_string())?;
      if study_exists > 0 {
        continue;
      }

      let folder_path = if !study.folder_path.trim().is_empty() {
        study.folder_path
      } else {
        PathBuf::from(project_root.clone())
          .join("studies")
          .join(&study.id)
          .to_string_lossy()
          .to_string()
      };

      conn
        .execute(
          "INSERT INTO studies (id, project_id, internal_name, paper_label, status, folder_path, created_at) \
          VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
          params![
            study.id,
            &project_id,
            study.title,
            Option::<String>::None,
            "planning",
            folder_path,
            study.created_at
          ]
        )
        .map_err(|err| err.to_string())?;
      studies_added += 1;
    }
  }

  Ok(format!(
    "Migration complete. Projects added: {projects_added}. Studies added: {studies_added}."
  ))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ListStudiesArgs {
  project_id: String
}

#[tauri::command]
fn list_studies(app: AppHandle, args: ListStudiesArgs) -> Result<Vec<DbStudy>, String> {
  let conn = connection(&app)?;
  init_schema(&conn)?;
  let mut stmt = conn
    .prepare(
      "SELECT id, project_id, internal_name, paper_label, status, folder_path, created_at \
      FROM studies WHERE project_id = ?1 ORDER BY created_at DESC"
    )
    .map_err(|err| err.to_string())?;
  let rows = stmt
    .query_map(params![args.project_id], |row| {
      Ok(DbStudy {
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

  let mut studies: Vec<DbStudy> = Vec::new();
  for row in rows {
    studies.push(row.map_err(|err| err.to_string())?);
  }
  Ok(studies)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateStudyArgs {
  project_id: String,
  internal_name: String,
  paper_label: Option<String>
}

#[tauri::command]
fn create_study(app: AppHandle, args: CreateStudyArgs) -> Result<DbStudy, String> {
  let conn = connection(&app)?;
  init_schema(&conn)?;

  let store = read_projects_store(&app)?;
  let project_root = store
    .projects
    .iter()
    .find(|project| project.id == args.project_id)
    .map(|project| project.root_path.clone())
    .ok_or_else(|| "Project not found.".to_string())?;

  let id = Uuid::new_v4().to_string();
  let folder = PathBuf::from(project_root).join("studies").join(&id);
  ensure_folders(&folder, STUDY_FOLDERS)?;

  let study = DbStudy {
    id: id.clone(),
    project_id: args.project_id,
    internal_name: args.internal_name,
    paper_label: args.paper_label,
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RenameStudyArgs {
  study_id: String,
  internal_name: String,
  paper_label: Option<String>
}

#[tauri::command]
fn rename_study(app: AppHandle, args: RenameStudyArgs) -> Result<(), String> {
  let conn = connection(&app)?;
  init_schema(&conn)?;
  conn
    .execute(
      "UPDATE studies SET internal_name = ?1, paper_label = ?2 WHERE id = ?3",
      params![args.internal_name, args.paper_label, args.study_id]
    )
    .map_err(|err| err.to_string())?;
  Ok(())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateStudyStatusArgs {
  study_id: String,
  status: String
}

#[tauri::command]
fn update_study_status(app: AppHandle, args: UpdateStudyStatusArgs) -> Result<(), String> {
  let conn = connection(&app)?;
  init_schema(&conn)?;
  conn
    .execute(
      "UPDATE studies SET status = ?1 WHERE id = ?2",
      params![args.status, args.study_id]
    )
    .map_err(|err| err.to_string())?;
  Ok(())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GetStudyDetailArgs {
  study_id: String
}

#[tauri::command]
fn get_study_detail(app: AppHandle, args: GetStudyDetailArgs) -> Result<StudyDetail, String> {
  let conn = connection(&app)?;
  init_schema(&conn)?;

  let study: DbStudy = conn
    .query_row(
      "SELECT id, project_id, internal_name, paper_label, status, folder_path, created_at \
      FROM studies WHERE id = ?1",
      params![args.study_id],
      |row| {
        Ok(DbStudy {
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
    .query_map(params![args.study_id], |row| {
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AddArtifactArgs {
  study_id: String,
  kind: String,
  value: String,
  label: Option<String>
}

#[tauri::command]
fn add_artifact(app: AppHandle, args: AddArtifactArgs) -> Result<(), String> {
  let conn = connection(&app)?;
  init_schema(&conn)?;
  let id = Uuid::new_v4().to_string();
  conn
    .execute(
      "INSERT INTO artifacts (id, study_id, kind, value, label, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
      params![id, args.study_id, args.kind, args.value, args.label, now_string()]
    )
    .map_err(|err| err.to_string())?;
  Ok(())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoveArtifactArgs {
  artifact_id: String
}

#[tauri::command]
fn remove_artifact(app: AppHandle, args: RemoveArtifactArgs) -> Result<(), String> {
  let conn = connection(&app)?;
  init_schema(&conn)?;
  conn
    .execute("DELETE FROM artifacts WHERE id = ?1", params![args.artifact_id])
    .map_err(|err| err.to_string())?;
  Ok(())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GenerateOsfPackagesArgs {
  study_id: String,
  include_pilots: bool
}

#[tauri::command]
fn generate_osf_packages(app: AppHandle, args: GenerateOsfPackagesArgs) -> Result<String, String> {
  let conn = connection(&app)?;
  init_schema(&conn)?;

  let folder_path: String = conn
    .query_row(
      "SELECT folder_path FROM studies WHERE id = ?1",
      params![args.study_id],
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

  let complete_count = copy_dir_filtered(&study_root, &complete_root, args.include_pilots, false)?;
  let condensed_count = copy_dir_filtered(&study_root, &condensed_root, args.include_pilots, true)?;

  Ok(format!(
    "OSF packages generated. COMPLETE: {complete_count} files, CONDENSED: {condensed_count} files."
  ))
}

#[tauri::command]
fn check_root_dir(root_dir: String) -> Result<RootDirInfo, String> {
  let path = PathBuf::from(root_dir.trim());
  let exists = path.exists() && path.is_dir();
  let is_git_repo = exists && path.join(".git").exists();
  Ok(RootDirInfo { exists, is_git_repo })
}

#[tauri::command]
fn import_files(
  app: AppHandle,
  project_id: String,
  study_id: String,
  paths: Vec<String>
) -> Result<Study, String> {
  let mut store = read_projects_store(&app)?;
  let project = store
    .projects
    .iter_mut()
    .find(|project| project.id == project_id)
    .ok_or_else(|| "Project not found.".to_string())?;
  let project_root = PathBuf::from(project.root_path.clone());

  let study = project
    .studies
    .iter_mut()
    .find(|study| study.id == study_id)
    .ok_or_else(|| "Study not found.".to_string())?;

  let dest_dir = project_root
    .join("studies")
    .join(&study.id)
    .join("sources");
  fs::create_dir_all(&dest_dir).map_err(|err| err.to_string())?;

  let mut known_paths: HashSet<String> =
    study.files.iter().map(|file| file.path.clone()).collect();

  for source in paths {
    let trimmed = source.trim();
    if trimmed.is_empty() {
      continue;
    }
    let src = PathBuf::from(trimmed);
    if !src.exists() || !src.is_file() {
      continue;
    }
    let filename = match src.file_name() {
      Some(value) => value,
      None => continue
    };

    let dest_path = if src.starts_with(&dest_dir) {
      src.clone()
    } else {
      unique_dest_path(&dest_dir, filename)
    };

    let rel_path = diff_paths(&dest_path, &project_root).unwrap_or(dest_path.clone());
    let mut rel_string = rel_path.to_string_lossy().to_string();
    if rel_string.contains('\\') {
      rel_string = rel_string.replace('\\', "/");
    }

    if known_paths.contains(&rel_string) {
      continue;
    }

    if src != dest_path {
      move_file_cross_device(&src, &dest_path)?;
    }

    let name = dest_path
      .file_name()
      .and_then(|value| value.to_str())
      .unwrap_or("file")
      .to_string();
    let kind = kind_from_ext(dest_path.extension());

    study.files.push(FileRef {
      path: rel_string.clone(),
      name,
      kind
    });
    known_paths.insert(rel_string);
  }

  project.updated_at = now_string();
  let updated = study.clone();
  write_projects_store(&app, &store)?;
  Ok(updated)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoveFileArgs {
  project_id: String,
  study_id: String,
  path: String
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeleteStudyArgs {
  project_id: String,
  study_id: String,
  #[serde(default)]
  delete_on_disk: bool
}

#[tauri::command]
fn remove_file_ref(app: AppHandle, args: RemoveFileArgs) -> Result<Study, String> {
  let mut store = read_projects_store(&app)?;
  let project = store
    .projects
    .iter_mut()
    .find(|project| project.id == args.project_id)
    .ok_or_else(|| "Project not found.".to_string())?;
  let project_root = PathBuf::from(project.root_path.clone());

  let study = project
    .studies
    .iter_mut()
    .find(|study| study.id == args.study_id)
    .ok_or_else(|| "Study not found.".to_string())?;

  let rel = args.path.trim();
  if !rel.is_empty() {
    let candidate = project_root.join(rel);
    let candidate = fs::canonicalize(&candidate).unwrap_or(candidate);
    let root = fs::canonicalize(&project_root).unwrap_or(project_root.clone());
    if candidate.starts_with(&root) && candidate.is_file() {
      let _ = fs::remove_file(&candidate);
    }
  }

  study.files.retain(|file| file.path != rel);
  project.updated_at = now_string();
  let updated = study.clone();
  write_projects_store(&app, &store)?;
  Ok(updated)
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

#[tauri::command]
fn delete_study(app: AppHandle, args: DeleteStudyArgs) -> Result<Project, String> {
  let mut store = read_projects_store(&app)?;
  let project = store
    .projects
    .iter_mut()
    .find(|project| project.id == args.project_id)
    .ok_or_else(|| "Project not found.".to_string())?;

  let mut removed_path: Option<PathBuf> = None;
  let before = project.studies.len();
  project.studies.retain(|study| {
    if study.id == args.study_id {
      if args.delete_on_disk {
        if !study.folder_path.trim().is_empty() {
          removed_path = Some(PathBuf::from(study.folder_path.clone()));
        } else {
          removed_path = Some(
            PathBuf::from(project.root_path.clone())
              .join("studies")
              .join(&study.id)
          );
        }
      }
      return false;
    }
    true
  });

  if project.studies.len() == before {
    return Err("Study not found.".to_string());
  }

  if let Some(folder) = removed_path {
    let root = fs::canonicalize(PathBuf::from(project.root_path.clone()))
      .unwrap_or_else(|_| PathBuf::from(project.root_path.clone()));
    let target = fs::canonicalize(&folder).unwrap_or(folder);
    if target.starts_with(&root) && target.is_dir() {
      fs::remove_dir_all(&target).map_err(|err| err.to_string())?;
    }
  }

  project.updated_at = now_string();
  let updated = project.clone();
  write_projects_store(&app, &store)?;
  Ok(updated)
}

fn main() {
  tauri::Builder::default()
    .invoke_handler(tauri::generate_handler![
      init_db,
      list_projects,
      create_project,
      update_project_root,
      delete_project,
      add_study,
      rename_study_json,
      rename_study_folder_json,
      migrate_json_to_sqlite,
      check_root_dir,
      import_files,
      remove_file_ref,
      delete_study,
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
