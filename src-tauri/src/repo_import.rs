//! Downloads the upstream `amnia` project archive (zip) from the URL declared
//! by the Python sidecar config, streams it to disk with progress events,
//! and unpacks it under the user's data directory.

use crate::sidecar;
use futures_util::StreamExt;
use serde::Serialize;
use std::io::Cursor;
use std::path::PathBuf;
use tauri::{AppHandle, Emitter};
use tokio::fs;
use tokio::io::AsyncWriteExt;

#[derive(Serialize, Clone)]
struct ImportProgress {
    percent: f32,
    message: String,
}

#[derive(Serialize)]
pub struct ImportOutcome {
    pub path: String,
    pub bytes: u64,
}

#[tauri::command]
pub async fn import_amnia_project(app: AppHandle) -> Result<ImportOutcome, String> {
    let cfg = sidecar::load_config(&app).map_err(|e| e.to_string())?;
    let url = zip_url_for(&cfg.github_repo_url);

    emit(&app, 1.0, "Resolving project URL");

    // Stream the archive.
    let resp = reqwest::get(&url)
        .await
        .map_err(|e| format!("network error: {e}"))?;
    let status = resp.status();
    if !status.is_success() {
        return Err(format!("download failed: HTTP {status}"));
    }
    let total = resp.content_length().unwrap_or(0);
    let mut stream = resp.bytes_stream();

    let target_dir = dirs::data_dir()
        .ok_or_else(|| "no data dir".to_string())?
        .join("amnia-desktop")
        .join("project");
    fs::create_dir_all(&target_dir).await.map_err(io_err)?;

    let zip_path = target_dir.join("__incoming.zip");
    let mut file = fs::File::create(&zip_path).await.map_err(io_err)?;

    let mut downloaded: u64 = 0;
    while let Some(chunk) = stream.next().await {
        let bytes = chunk.map_err(|e| format!("stream error: {e}"))?;
        downloaded += bytes.len() as u64;
        file.write_all(&bytes).await.map_err(io_err)?;
        let pct = if total > 0 {
            (downloaded as f32 / total as f32) * 95.0
        } else {
            // Indeterminate — sweep slowly up to 90%.
            (((downloaded / 1024) as f32) % 90.0).min(90.0)
        };
        emit(&app, pct, "Downloading project");
    }
    file.flush().await.map_err(io_err)?;
    drop(file);

    emit(&app, 96.0, "Extracting archive");
    extract_zip(zip_path.clone(), target_dir.clone()).await?;
    let _ = fs::remove_file(&zip_path).await;

    emit(&app, 100.0, "Project ready");
    Ok(ImportOutcome {
        path: target_dir.display().to_string(),
        bytes: downloaded,
    })
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Turn a `https://github.com/owner/repo` URL into a zip download URL pointing
/// at the default branch. If the URL is already a zip we pass it through.
fn zip_url_for(repo_url: &str) -> String {
    let trimmed = repo_url.trim_end_matches('/').trim_end_matches(".git");
    if trimmed.ends_with(".zip") {
        return trimmed.to_string();
    }
    // Heuristic for github.com — codeload returns the default branch zip.
    if let Some(rest) = trimmed.strip_prefix("https://github.com/") {
        return format!("https://codeload.github.com/{rest}/zip/refs/heads/main");
    }
    trimmed.to_string()
}

fn emit(app: &AppHandle, percent: f32, message: &str) {
    let _ = app.emit(
        "import-progress",
        ImportProgress {
            percent,
            message: message.to_string(),
        },
    );
}

fn io_err<E: std::fmt::Display>(e: E) -> String {
    format!("io error: {e}")
}

async fn extract_zip(zip_path: PathBuf, target: PathBuf) -> Result<(), String> {
    let bytes = fs::read(&zip_path).await.map_err(io_err)?;
    tokio::task::spawn_blocking(move || -> Result<(), String> {
        let reader = Cursor::new(bytes);
        let mut archive = zip::ZipArchive::new(reader).map_err(|e| format!("zip: {e}"))?;
        for i in 0..archive.len() {
            let mut entry = archive.by_index(i).map_err(|e| format!("zip entry: {e}"))?;
            let Some(rel) = entry.enclosed_name() else {
                continue; // skip unsafe entries
            };
            let out_path = target.join(rel);
            if entry.is_dir() {
                std::fs::create_dir_all(&out_path).map_err(io_err)?;
                continue;
            }
            if let Some(parent) = out_path.parent() {
                std::fs::create_dir_all(parent).map_err(io_err)?;
            }
            let mut out = std::fs::File::create(&out_path).map_err(io_err)?;
            std::io::copy(&mut entry, &mut out).map_err(io_err)?;
        }
        Ok(())
    })
    .await
    .map_err(|e| format!("join error: {e}"))?
}
