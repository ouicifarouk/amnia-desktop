//! Bridge between Rust core and the Python sidecar's *config*.
//!
//! The sidecar holds runtime knobs like `GITHUB_REPO_URL` and `DEFAULT_MODEL`
//! in [`python_sidecar/config.py`] so they can be edited post-build. The
//! Rust side never re-implements these values — it asks the sidecar for them
//! by invoking `python3 .../main.py --print-config` and parsing the JSON.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Command;
use tauri::{AppHandle, Manager};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SidecarError {
    #[error("sidecar resource not found ({0})")]
    NotFound(String),
    #[error("sidecar execution failed: {0}")]
    Exec(String),
    #[error("sidecar produced invalid JSON: {0}")]
    Parse(#[from] serde_json::Error),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SidecarConfig {
    pub github_repo_url: String,
    pub default_model: String,
    pub ollama_host: String,
    pub command_whitelist: Vec<String>,
}

#[tauri::command]
pub fn get_sidecar_config(app: AppHandle) -> Result<SidecarConfig, String> {
    load_config(&app).map_err(|e| e.to_string())
}

/// Resolve the sidecar `main.py` path, prefer the bundled resource (production)
/// then fall back to the project-relative path (dev).
fn resolve_sidecar(app: &AppHandle) -> Result<PathBuf, SidecarError> {
    if let Ok(path) = app.path().resolve(
        "python_sidecar/main.py",
        tauri::path::BaseDirectory::Resource,
    ) {
        if path.exists() {
            return Ok(path);
        }
    }
    // Dev fallback: walk up from CWD looking for the workspace.
    let mut here = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    for _ in 0..4 {
        let candidate = here.join("python_sidecar").join("main.py");
        if candidate.exists() {
            return Ok(candidate);
        }
        if !here.pop() {
            break;
        }
    }
    Err(SidecarError::NotFound("python_sidecar/main.py".into()))
}

pub fn load_config(app: &AppHandle) -> Result<SidecarConfig, SidecarError> {
    let main_py = resolve_sidecar(app)?;
    let out = Command::new("python3")
        .arg(&main_py)
        .arg("--print-config")
        .output()
        .map_err(|e| SidecarError::Exec(e.to_string()))?;
    if !out.status.success() {
        return Err(SidecarError::Exec(
            String::from_utf8_lossy(&out.stderr).into_owned(),
        ));
    }
    Ok(serde_json::from_slice(&out.stdout)?)
}

pub fn log_config_summary(app: &AppHandle) -> Result<(), SidecarError> {
    let cfg = load_config(app)?;
    log::info!(
        "sidecar config: repo={} model={} host={}",
        cfg.github_repo_url,
        cfg.default_model,
        cfg.ollama_host
    );
    Ok(())
}
