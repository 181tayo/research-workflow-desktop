use std::fs;
use std::path::{Path, PathBuf};

pub fn ensure_dir(path: &Path) -> Result<(), String> {
    fs::create_dir_all(path)
        .map_err(|e| format!("Unable to create directory {}: {e}", path.display()))
}

pub fn write_string(path: &Path, content: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        ensure_dir(parent)?;
    }
    fs::write(path, content).map_err(|e| format!("Unable to write {}: {e}", path.display()))
}

pub fn analysis_paths(base: &Path) -> (PathBuf, PathBuf, PathBuf) {
    (
        base.join("analysis").join("spec.json"),
        base.join("analysis").join("analysis.Rmd"),
        base.join("analysis").join("analysis.R"),
    )
}
