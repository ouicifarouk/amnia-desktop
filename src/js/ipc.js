// Thin wrapper around Tauri's IPC so the UI code stays readable and we have
// a single place to fall back to mocks when running the HTML outside Tauri
// (e.g. opening src/index.html directly in a browser for layout work).

const tauri = globalThis.__TAURI__;

export async function invoke(cmd, args = {}) {
  if (!tauri?.core?.invoke) {
    console.warn(`[ipc] no Tauri runtime; mocking ${cmd}`, args);
    return mock(cmd, args);
  }
  return tauri.core.invoke(cmd, args);
}

export async function listen(event, handler) {
  if (!tauri?.event?.listen) {
    console.warn(`[ipc] no Tauri runtime; cannot listen for ${event}`);
    return () => {};
  }
  return tauri.event.listen(event, handler);
}

// --------- Mock fallback for pure-browser previews ---------
function mock(cmd) {
  switch (cmd) {
    case "get_system_info":
      return {
        ram_total_bytes: 16 * 1024 ** 3,
        ram_available_bytes: 12 * 1024 ** 3,
        vram_total_bytes: 8 * 1024 ** 3,
        gpu_name: "Mock GPU",
        gpu_detected: true,
        safe_budget_bytes: Math.floor((24 * 1024 ** 3) * 0.85),
      };
    case "evaluate_model_fit":
      return { fits: true, warning: null };
    case "list_ollama_models":
      return [
        { name: "llama3.2:3b", size_bytes: 2_000_000_000, installed: true },
        { name: "qwen2.5:7b", size_bytes: 4_700_000_000, installed: false },
      ];
    case "get_sidecar_config":
      return {
        github_repo_url: "https://github.com/example/amnia-project",
        default_model: "llama3.2:3b",
      };
    default:
      return null;
  }
}
