"""
Post-build editable configuration for the amnia Python sidecar.

These values are intentionally kept in a plain Python module (not a compiled
binary) so they can be edited *after* the Tauri app is installed without
recompiling the Rust core. The Rust core reads them at runtime via
`python3 main.py --print-config` (see `src-tauri/src/sidecar.rs`).

Keep this file dependency-free.
"""

from __future__ import annotations

# ---------------------------------------------------------------------------
# Project import
# ---------------------------------------------------------------------------

# Upstream repository that the "Import amnia Project" button downloads.
GITHUB_REPO_URL: str = "https://github.com/ouicifarouk/amnia-project"

# ---------------------------------------------------------------------------
# Local model
# ---------------------------------------------------------------------------

# The model the sidecar uses by default if the user has not chosen one yet.
DEFAULT_MODEL: str = "llama3.2:3b"

# HTTP endpoint of the local Ollama daemon.
OLLAMA_HOST: str = "http://127.0.0.1:11434"

# Hard cap on how long any single Ollama generation may run, in seconds.
OLLAMA_TIMEOUT_SECONDS: int = 120

# ---------------------------------------------------------------------------
# Safety: AI -> bash whitelist
# ---------------------------------------------------------------------------
#
# The AI sidecar is forbidden from running arbitrary commands. When the model
# outputs a JSON {"action": "shell", "command": "..."} payload the sidecar
# verifies the *binary* (argv[0]) is in this whitelist BEFORE executing.
#
# Add entries cautiously — every binary here is something the assistant can
# invoke on the user's machine on its own initiative.
COMMAND_WHITELIST: tuple[str, ...] = (
    "ls",
    "cat",
    "pwd",
    "echo",
    "grep",
    "find",
    "head",
    "tail",
    "wc",
    "uname",
    "df",
    "free",
    "uptime",
    "whoami",
    "date",
)

# Maximum number of bytes captured from stdout/stderr of any command.
MAX_CMD_OUTPUT_BYTES: int = 64 * 1024

# How long a single shell command may run, in seconds.
CMD_TIMEOUT_SECONDS: int = 15
