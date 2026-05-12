//! System resource detection and Ollama-model capacity evaluation.
//!
//! The frontend calls [`get_system_info`] on every main-screen mount and
//! [`evaluate_model_fit`] before pulling a new model. We treat
//! `(total_RAM + total_VRAM) * 0.85` as the safe budget for an Ollama
//! workload — anything larger triggers a critical UI warning.

use serde::Serialize;
use std::process::Command;

const OS_BUFFER: f64 = 0.15; // 15% reserved for the OS / other processes
const BYTES_PER_GIB: u64 = 1024 * 1024 * 1024;

#[derive(Debug, Serialize, Clone)]
pub struct SystemInfo {
    pub ram_total_bytes: u64,
    pub ram_available_bytes: u64,
    pub vram_total_bytes: u64,
    pub gpu_detected: bool,
    pub gpu_name: Option<String>,
    pub safe_budget_bytes: u64,
}

#[derive(Debug, Serialize)]
pub struct ModelFit {
    pub fits: bool,
    pub warning: Option<String>,
}

// ---------------------------------------------------------------------------
// Tauri commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn get_system_info() -> Result<SystemInfo, String> {
    let ram = read_ram();
    let (vram, gpu_name) = read_vram_and_gpu();
    let safe_budget = compute_safe_budget(ram.0, vram);

    Ok(SystemInfo {
        ram_total_bytes: ram.0,
        ram_available_bytes: ram.1,
        vram_total_bytes: vram,
        gpu_detected: vram > 0 || gpu_name.is_some(),
        gpu_name,
        safe_budget_bytes: safe_budget,
    })
}

#[tauri::command]
pub fn evaluate_model_fit(model_size_bytes: Option<u64>) -> Result<ModelFit, String> {
    let info = get_system_info()?;
    let Some(size) = model_size_bytes else {
        return Ok(ModelFit {
            fits: true,
            warning: None,
        });
    };

    if size > info.safe_budget_bytes {
        return Ok(ModelFit {
            fits: false,
            warning: Some(
                "Warning: Model exceeds system capacity. Proceeding may cause crashes.".to_string(),
            ),
        });
    }

    if !info.gpu_detected {
        return Ok(ModelFit {
            fits: true,
            warning: Some("No GPU detected. Falling back to CPU (slower).".to_string()),
        });
    }

    Ok(ModelFit {
        fits: true,
        warning: None,
    })
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

fn read_ram() -> (u64, u64) {
    use sysinfo::System;
    let mut sys = System::new();
    sys.refresh_memory();
    (sys.total_memory(), sys.available_memory())
}

/// Detects (VRAM bytes, GPU name) by trying `nvidia-smi` first, then falling
/// back to AMD/Intel sysfs entries. Returns `(0, None)` if no GPU is found.
fn read_vram_and_gpu() -> (u64, Option<String>) {
    if let Some((vram, name)) = read_nvidia() {
        return (vram, Some(name));
    }
    if let Some((vram, name)) = read_amd_intel_sysfs() {
        return (vram, Some(name));
    }
    (0, None)
}

fn read_nvidia() -> Option<(u64, String)> {
    let out = Command::new("nvidia-smi")
        .args([
            "--query-gpu=memory.total,name",
            "--format=csv,noheader,nounits",
        ])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let first = text.lines().next()?;
    let mut parts = first.split(',').map(str::trim);
    let mem_mib: u64 = parts.next()?.parse().ok()?;
    let name = parts.next()?.to_string();
    Some((mem_mib * 1024 * 1024, name))
}

fn read_amd_intel_sysfs() -> Option<(u64, String)> {
    let drm = std::fs::read_dir("/sys/class/drm").ok()?;
    for entry in drm.flatten() {
        let path = entry.path();
        let mem_file = path.join("device").join("mem_info_vram_total");
        if !mem_file.exists() {
            continue;
        }
        if let Ok(content) = std::fs::read_to_string(&mem_file) {
            if let Ok(bytes) = content.trim().parse::<u64>() {
                let name = std::fs::read_to_string(path.join("device").join("uevent"))
                    .ok()
                    .and_then(|s| {
                        s.lines()
                            .find(|l| l.starts_with("DRIVER="))
                            .map(|l| l.trim_start_matches("DRIVER=").to_string())
                    })
                    .unwrap_or_else(|| "AMD/Intel GPU".to_string());
                return Some((bytes, name));
            }
        }
    }
    None
}

fn compute_safe_budget(ram: u64, vram: u64) -> u64 {
    let total = ram.saturating_add(vram) as f64;
    ((total) * (1.0 - OS_BUFFER)) as u64
}

#[allow(dead_code)]
pub(crate) fn bytes_to_gib(bytes: u64) -> f64 {
    bytes as f64 / BYTES_PER_GIB as f64
}
