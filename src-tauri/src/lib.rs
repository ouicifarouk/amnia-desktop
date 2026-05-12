//! amnia-desktop — Tauri core.
//!
//! This crate wires together the Rust system-layer commands that the JS
//! frontend invokes over Tauri IPC. Each domain lives in its own module:
//!
//! * [`system_info`] — RAM/VRAM/GPU detection and capacity warnings.
//! * [`shell_alias`] — safe append/remove of an `amnia` alias in the user's
//!   shell rc file (`~/.bashrc` or `~/.zshrc`).
//! * [`repo_import`] — downloads the upstream project archive from the URL
//!   configured in `python_sidecar/config.py`, emitting progress events.
//! * [`ollama`]      — list/pull models from the local Ollama daemon.
//! * [`sidecar`]     — locate and read the Python sidecar's runtime config.

pub mod ollama;
pub mod repo_import;
pub mod shell_alias;
pub mod sidecar;
pub mod system_info;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Best-effort logger init — never panic.
    let _ = env_logger::try_init();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            // Resolve the sidecar config once at startup so missing config
            // shows up immediately in logs (does not abort startup).
            if let Err(e) = sidecar::log_config_summary(app.handle()) {
                log::warn!("sidecar config unavailable at startup: {e}");
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            system_info::get_system_info,
            system_info::evaluate_model_fit,
            shell_alias::register_shell_alias,
            shell_alias::remove_shell_alias,
            repo_import::import_amnia_project,
            ollama::list_ollama_models,
            ollama::pull_ollama_model,
            sidecar::get_sidecar_config,
        ])
        .run(tauri::generate_context!())
        .expect("error while running amnia-desktop");
}
