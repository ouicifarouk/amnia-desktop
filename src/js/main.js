import { invoke, listen } from "./ipc.js";

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------
const state = {
  assistantName: "amnia",
  models: [],
  filteredModels: [],
};

// ---------------------------------------------------------------------------
// Boot
// ---------------------------------------------------------------------------
document.addEventListener("DOMContentLoaded", async () => {
  await bootstrap();
});

async function bootstrap() {
  // Decide which screen to show first.
  const savedName = localStorage.getItem("amnia.assistantName");
  if (savedName) {
    state.assistantName = savedName;
    showMainScreen();
  } else {
    showSetupScreen();
  }
  bindSetupHandlers();
  bindMainHandlers();
  await refreshSystemInfo();
}

// ---------------------------------------------------------------------------
// Screens
// ---------------------------------------------------------------------------
function showSetupScreen() {
  document.getElementById("screen-setup").classList.remove("hidden");
  document.getElementById("screen-main").classList.add("hidden");
}

function showMainScreen() {
  document.getElementById("screen-setup").classList.add("hidden");
  document.getElementById("screen-main").classList.remove("hidden");
  document.getElementById("assistant-name-display").textContent =
    state.assistantName;
}

// ---------------------------------------------------------------------------
// Setup screen
// ---------------------------------------------------------------------------
function bindSetupHandlers() {
  const input = document.getElementById("assistant-name-input");
  const err = document.getElementById("assistant-name-error");
  const btn = document.getElementById("btn-setup-continue");

  btn.addEventListener("click", async () => {
    const name = (input.value || "").trim();
    if (!/^[a-z][a-z0-9_-]{0,31}$/i.test(name)) {
      err.textContent =
        "Use 1–32 chars: letters, digits, _ or -, starting with a letter.";
      err.classList.remove("hidden");
      return;
    }
    err.classList.add("hidden");
    btn.disabled = true;
    btn.textContent = "Registering…";
    try {
      await invoke("register_shell_alias", { name });
      localStorage.setItem("amnia.assistantName", name);
      state.assistantName = name;
      showMainScreen();
    } catch (e) {
      err.textContent = `Failed to register alias: ${e}`;
      err.classList.remove("hidden");
    } finally {
      btn.disabled = false;
      btn.textContent = "Continue";
    }
  });
}

// ---------------------------------------------------------------------------
// Main screen
// ---------------------------------------------------------------------------
function bindMainHandlers() {
  document
    .getElementById("btn-import")
    .addEventListener("click", onImportClicked);

  document
    .getElementById("btn-models")
    .addEventListener("click", onBrowseModelsClicked);

  document
    .getElementById("btn-models-close")
    .addEventListener("click", () =>
      document.getElementById("model-picker").classList.add("hidden"),
    );

  document
    .getElementById("model-search")
    .addEventListener("input", onModelSearchInput);
}

// ----- system info -----
async function refreshSystemInfo() {
  try {
    const info = await invoke("get_system_info");
    document.getElementById("sysinfo-ram").textContent = `${fmtGB(
      info.ram_total_bytes,
    )} GB total / ${fmtGB(info.ram_available_bytes)} GB free`;
    document.getElementById("sysinfo-vram").textContent =
      info.vram_total_bytes > 0
        ? `${fmtGB(info.vram_total_bytes)} GB`
        : "n/a";
    document.getElementById("sysinfo-gpu").textContent = info.gpu_detected
      ? info.gpu_name || "Detected"
      : "Not detected";
    document.getElementById("sysinfo-budget").textContent = `${fmtGB(
      info.safe_budget_bytes,
    )} GB`;

    if (!info.gpu_detected) {
      pushWarning(
        "no-gpu",
        "warn",
        "No GPU detected. Falling back to CPU (slower).",
      );
    }
  } catch (e) {
    console.error("get_system_info failed", e);
  }
}

// ----- Button A: Import project -----
async function onImportClicked() {
  const wrap = document.getElementById("import-progress-wrap");
  const bar = document.getElementById("import-progress-bar");
  const pct = document.getElementById("import-progress-pct");
  const label = document.getElementById("import-progress-label");

  wrap.classList.remove("hidden");
  bar.style.width = "0%";
  pct.textContent = "0%";
  label.textContent = "Starting…";

  const off = await listen("import-progress", (event) => {
    const p = event.payload || {};
    const value = Math.max(0, Math.min(100, Math.round(p.percent || 0)));
    bar.style.width = `${value}%`;
    pct.textContent = `${value}%`;
    if (p.message) label.textContent = p.message;
  });

  try {
    const out = await invoke("import_amnia_project");
    label.textContent = `Imported to ${out.path}`;
    bar.style.width = "100%";
    pct.textContent = "100%";
  } catch (e) {
    label.textContent = `Import failed: ${e}`;
    bar.classList.remove("bg-brand-500");
    bar.classList.add("bg-red-500");
  } finally {
    if (typeof off === "function") off();
  }
}

// ----- Button B: Local models -----
async function onBrowseModelsClicked() {
  const picker = document.getElementById("model-picker");
  picker.classList.remove("hidden");
  const list = document.getElementById("model-list");
  list.innerHTML = `<li class="px-3 py-2 text-slate-400">Loading…</li>`;
  try {
    state.models = await invoke("list_ollama_models");
    state.filteredModels = state.models;
    renderModelList();
  } catch (e) {
    list.innerHTML = `<li class="px-3 py-2 text-red-600">Failed: ${escapeHtml(
      String(e),
    )}</li>`;
  }
}

function onModelSearchInput(e) {
  const q = e.target.value.trim().toLowerCase();
  state.filteredModels = q
    ? state.models.filter((m) => m.name.toLowerCase().includes(q))
    : state.models;
  renderModelList();
}

function renderModelList() {
  const list = document.getElementById("model-list");
  if (!state.filteredModels.length) {
    list.innerHTML = `<li class="px-3 py-2 text-slate-400">No matches.</li>`;
    return;
  }
  list.innerHTML = state.filteredModels
    .map(
      (m) => `
      <li class="px-3 py-2 flex items-center justify-between gap-3">
        <div class="min-w-0">
          <div class="font-medium truncate">${escapeHtml(m.name)}</div>
          <div class="text-xs text-slate-500">
            ${fmtGB(m.size_bytes)} GB
            ${m.installed ? "· installed" : ""}
          </div>
        </div>
        <button data-model="${escapeAttr(m.name)}"
                class="text-xs rounded-md border border-slate-300 px-2 py-1
                       hover:bg-slate-50">
          ${m.installed ? "Select" : "Download"}
        </button>
      </li>`,
    )
    .join("");

  list.querySelectorAll("button[data-model]").forEach((btn) => {
    btn.addEventListener("click", () => onModelChosen(btn.dataset.model));
  });
}

async function onModelChosen(name) {
  const status = document.getElementById("model-pull-status");
  status.classList.remove("hidden");
  status.textContent = `Checking system capacity for "${name}"…`;
  try {
    const fit = await invoke("evaluate_model_fit", { modelName: name });
    if (!fit.fits) {
      pushWarning("capacity", "danger", fit.warning);
      status.textContent = fit.warning;
      return;
    }
    status.textContent = `Pulling "${name}" via Ollama…`;
    const result = await invoke("pull_ollama_model", { name });
    status.textContent = result.message || `"${name}" ready.`;
    await onBrowseModelsClicked(); // refresh list
  } catch (e) {
    status.textContent = `Failed: ${e}`;
  }
}

// ---------------------------------------------------------------------------
// Warnings banner
// ---------------------------------------------------------------------------
function pushWarning(id, severity, text) {
  const container = document.getElementById("warnings");
  if (container.querySelector(`[data-id="${id}"]`)) return;
  const cls =
    severity === "danger"
      ? "bg-red-50 border-red-200 text-red-800"
      : "bg-amber-50 border-amber-200 text-amber-800";
  const node = document.createElement("div");
  node.dataset.id = id;
  node.className = `border ${cls} rounded-lg px-3 py-2 text-sm flex items-start gap-2`;
  node.innerHTML = `<span>${escapeHtml(text)}</span>`;
  container.appendChild(node);
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------
function fmtGB(bytes) {
  if (!bytes && bytes !== 0) return "—";
  return (bytes / 1024 ** 3).toFixed(1);
}
function escapeHtml(s) {
  return String(s)
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;");
}
function escapeAttr(s) {
  return escapeHtml(s).replace(/"/g, "&quot;");
}
