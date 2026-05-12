//! Thin wrapper around the local Ollama HTTP API (default
//! `http://127.0.0.1:11434`). We avoid shelling out to the `ollama` CLI so
//! that the same code path works whether the user installed Ollama as a
//! systemd service or via the standalone binary.

use serde::{Deserialize, Serialize};

const OLLAMA_BASE: &str = "http://127.0.0.1:11434";

#[derive(Debug, Serialize, Clone)]
pub struct ModelEntry {
    pub name: String,
    pub size_bytes: u64,
    pub installed: bool,
}

#[derive(Debug, Serialize)]
pub struct PullResult {
    pub model: String,
    pub message: String,
}

// ---------------------------------------------------------------------------
// `GET /api/tags` — list installed models, then fold in a small curated list
// of popular models that the user might want to download but doesn't have
// yet. This gives the picker something useful even on a fresh install.
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct TagsResponse {
    models: Vec<TagsModel>,
}
#[derive(Deserialize)]
struct TagsModel {
    name: String,
    #[serde(default)]
    size: u64,
}

#[tauri::command]
pub async fn list_ollama_models() -> Result<Vec<ModelEntry>, String> {
    let installed = fetch_installed().await.unwrap_or_default();
    let installed_names: std::collections::HashSet<String> =
        installed.iter().map(|m| m.name.clone()).collect();

    let mut out: Vec<ModelEntry> = installed;
    for suggestion in suggested_models() {
        if !installed_names.contains(&suggestion.name) {
            out.push(suggestion);
        }
    }
    Ok(out)
}

async fn fetch_installed() -> Result<Vec<ModelEntry>, String> {
    let url = format!("{OLLAMA_BASE}/api/tags");
    let resp = reqwest::get(&url)
        .await
        .map_err(|e| format!("ollama unreachable: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("ollama /api/tags returned {}", resp.status()));
    }
    let body: TagsResponse = resp
        .json()
        .await
        .map_err(|e| format!("ollama bad json: {e}"))?;
    Ok(body
        .models
        .into_iter()
        .map(|m| ModelEntry {
            name: m.name,
            size_bytes: m.size,
            installed: true,
        })
        .collect())
}

// A small curated catalog so the picker is never empty.
fn suggested_models() -> Vec<ModelEntry> {
    [
        ("llama3.2:1b", 1_300_000_000_u64),
        ("llama3.2:3b", 2_000_000_000_u64),
        ("qwen2.5:7b", 4_700_000_000_u64),
        ("mistral:7b", 4_400_000_000_u64),
        ("phi3:mini", 2_300_000_000_u64),
        ("gemma2:2b", 1_600_000_000_u64),
    ]
    .into_iter()
    .map(|(n, s)| ModelEntry {
        name: n.to_string(),
        size_bytes: s,
        installed: false,
    })
    .collect()
}

// ---------------------------------------------------------------------------
// `POST /api/pull` — kick off a model download. We use stream=false so the
// HTTP request blocks until the model is fully pulled; the call returns when
// the daemon is done. The frontend can show a generic "Pulling..." indicator.
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct PullRequest<'a> {
    model: &'a str,
    stream: bool,
}

#[tauri::command]
pub async fn pull_ollama_model(name: String) -> Result<PullResult, String> {
    let url = format!("{OLLAMA_BASE}/api/pull");
    let resp = reqwest::Client::new()
        .post(&url)
        .json(&PullRequest {
            model: &name,
            stream: false,
        })
        .send()
        .await
        .map_err(|e| format!("ollama pull failed: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("ollama /api/pull returned {}", resp.status()));
    }
    Ok(PullResult {
        model: name.clone(),
        message: format!("Pulled {name}"),
    })
}
