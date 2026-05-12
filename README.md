# amnia-desktop

A Linux desktop application built with a strict three-layer architecture:

| Layer | Tech | Role |
| --- | --- | --- |
| **Backend (Core/System)** | Rust + Tauri 2 | Window, system metrics (RAM/VRAM/GPU), shell-alias management, repo import with progress, Ollama orchestration |
| **Frontend (UI)** | Plain HTML + JS + Tailwind CSS | Minimal English UI: setup screen + main screen with project import & local-model selection |
| **Sidecar (AI/Middleware)** | Python 3 | Talks to local Ollama, parses strict JSON responses, executes only whitelisted bash commands |

The Python sidecar is intentionally a separate process so that AI logic, prompt
templates, and runtime config can be updated **post-build** by editing
`python_sidecar/config.py` without recompiling the Rust binary.

## Directory layout

```
amnia-desktop/
├── package.json              # Tailwind build scripts
├── tailwind.config.js
├── postcss.config.js
├── src/                      # Frontend (HTML/JS/Tailwind)
│   ├── index.html
│   ├── styles.css            # Tailwind input
│   ├── assets/styles.css     # Tailwind output (built, gitignored)
│   └── js/
│       ├── main.js           # UI state machine + Tauri IPC wiring
│       └── ipc.js            # Thin wrapper around `window.__TAURI__.core.invoke`
├── src-tauri/                # Rust core (Tauri 2)
│   ├── Cargo.toml
│   ├── build.rs
│   ├── tauri.conf.json
│   ├── capabilities/default.json
│   └── src/
│       ├── main.rs           # Entry point
│       ├── lib.rs            # Tauri builder + command registry
│       ├── system_info.rs    # RAM/VRAM/GPU detection + capacity warnings
│       ├── shell_alias.rs    # Safe ~/.bashrc / ~/.zshrc alias management
│       ├── repo_import.rs    # Project import with progress events
│       └── ollama.rs         # `ollama list` / `ollama pull` wrappers
└── python_sidecar/           # AI middleware
    ├── main.py               # CLI entry — receives system args, talks to Ollama
    ├── config.py             # Post-build editable (GITHUB_REPO_URL, DEFAULT_MODEL, …)
    ├── ollama_client.py
    ├── safe_exec.py          # Whitelist-based subprocess wrapper
    └── requirements.txt
```

## Prerequisites (Linux)

```bash
sudo apt-get install -y \
    libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev \
    librsvg2-dev libsoup-3.0-dev patchelf build-essential curl wget file pkg-config

# Rust (stable)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Node 20+ and Python 3.10+
# Ollama (optional, for the model features): https://ollama.com/download
```

## Development

```bash
npm install
npm run tauri dev      # build Tailwind in watch + launch Tauri dev window
```

Or run pieces independently:

```bash
npm run tailwind:watch          # frontend CSS
cargo run --manifest-path src-tauri/Cargo.toml
python3 python_sidecar/main.py --help
```

## Build

```bash
npm run tauri build    # produces a .deb / .AppImage under src-tauri/target/release/bundle/
```

## Post-build configuration

After installing the bundle, the Python sidecar lives alongside the app and is
launched by the registered `amnia` shell alias. To change the upstream project
URL, default model, or any other AI behaviour without recompiling Rust, edit:

```
python_sidecar/config.py
```

The Rust core never hard-codes these values — it reads them from the sidecar at
runtime via stdout JSON.

## License

MIT
