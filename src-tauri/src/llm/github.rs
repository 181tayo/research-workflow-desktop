use reqwest::blocking::Client;
use reqwest::header::{ACCEPT, AUTHORIZATION, USER_AGENT};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{Read, Write};
use std::path::Path;

use super::settings::LlmSettings;

#[derive(Debug, Deserialize, Clone)]
pub struct GithubAsset {
    pub name: String,
    pub browser_download_url: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct GithubRelease {
    pub tag_name: String,
    pub prerelease: bool,
    pub assets: Vec<GithubAsset>,
}

fn github_client() -> Result<Client, String> {
    Client::builder().build().map_err(|e| e.to_string())
}

fn auth_token() -> Option<String> {
    std::env::var("GITHUB_TOKEN")
        .ok()
        .filter(|v| !v.trim().is_empty())
}

fn github_get<T: for<'de> serde::Deserialize<'de>>(url: &str) -> Result<T, String> {
    let client = github_client()?;
    let mut request = client
        .get(url)
        .header(USER_AGENT, "research-workflow/0.1")
        .header(ACCEPT, "application/vnd.github+json");
    if let Some(token) = auth_token() {
        request = request.header(AUTHORIZATION, format!("Bearer {token}"));
    }
    let response = request
        .send()
        .map_err(|e| format!("GitHub request failed: {e}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "GitHub request failed with status {}",
            response.status()
        ));
    }
    response
        .json::<T>()
        .map_err(|e| format!("Unable to parse GitHub response: {e}"))
}

pub fn fetch_releases(settings: &LlmSettings) -> Result<Vec<GithubRelease>, String> {
    if settings.github_owner.trim().is_empty() || settings.github_repo.trim().is_empty() {
        return Err("GitHub owner/repo are required.".to_string());
    }
    let url = format!(
        "https://api.github.com/repos/{}/{}/releases",
        settings.github_owner, settings.github_repo
    );
    github_get(&url)
}

pub fn fetch_release_by_tag(settings: &LlmSettings, tag: &str) -> Result<GithubRelease, String> {
    if settings.github_owner.trim().is_empty() || settings.github_repo.trim().is_empty() {
        return Err("GitHub owner/repo are required.".to_string());
    }
    let url = format!(
        "https://api.github.com/repos/{}/{}/releases/tags/{}",
        settings.github_owner, settings.github_repo, tag
    );
    github_get(&url)
}

pub fn newest_release(settings: &LlmSettings) -> Result<GithubRelease, String> {
    let releases = fetch_releases(settings)?;
    releases
        .into_iter()
        .find(|r| !r.prerelease || settings.allow_prerelease)
        .ok_or_else(|| "No eligible release found.".to_string())
}

pub fn find_asset<'a>(
    release: &'a GithubRelease,
    asset_name: &str,
) -> Result<&'a GithubAsset, String> {
    release
        .assets
        .iter()
        .find(|a| a.name == asset_name)
        .ok_or_else(|| {
            format!(
                "Asset '{}' not found in release '{}'.",
                asset_name, release.tag_name
            )
        })
}

pub fn download_asset_and_sha256(
    url: &str,
    model_dir: &Path,
    asset_name: &str,
) -> Result<(String, u64), String> {
    fs::create_dir_all(model_dir).map_err(|e| e.to_string())?;
    let final_path = model_dir.join(asset_name);
    let part_path = model_dir.join(format!("{asset_name}.part"));

    let client = github_client()?;
    let mut request = client
        .get(url)
        .header(USER_AGENT, "research-workflow/0.1")
        .header(ACCEPT, "application/octet-stream");
    if let Some(token) = auth_token() {
        request = request.header(AUTHORIZATION, format!("Bearer {token}"));
    }

    let mut response = request
        .send()
        .map_err(|e| format!("Download failed: {e}"))?;
    if !response.status().is_success() {
        return Err(format!("Download failed with status {}", response.status()));
    }

    let mut file = fs::File::create(&part_path)
        .map_err(|e| format!("Unable to create {}: {e}", part_path.display()))?;
    let mut hasher = Sha256::new();
    let mut bytes_total: u64 = 0;
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = response
            .read(&mut buf)
            .map_err(|e| format!("Unable to read download stream: {e}"))?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n])
            .map_err(|e| format!("Unable to write {}: {e}", part_path.display()))?;
        hasher.update(&buf[..n]);
        bytes_total += n as u64;
    }

    fs::rename(&part_path, &final_path)
        .map_err(|e| format!("Unable to finalize {}: {e}", final_path.display()))?;
    let sha = hex::encode(hasher.finalize());
    Ok((sha, bytes_total))
}
