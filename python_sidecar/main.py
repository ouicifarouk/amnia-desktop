#!/usr/bin/env python3
"""
amnia Python sidecar entrypoint.

Roles:

1. Expose runtime config to the Rust core via ``--print-config`` (used at
   app startup and by the import-project flow).
2. Provide a CLI the user invokes from any terminal through the alias that
   the Rust core writes to ``~/.bashrc`` / ``~/.zshrc``. The CLI:
   * sends the user's natural-language question to the local Ollama model,
   * forces the model to respond in a strict JSON schema,
   * if the model proposes a shell command, executes it through the safe
     whitelist wrapper (`safe_exec.run_safe`).

The strict JSON contract is the only thing standing between the user's shell
and arbitrary model output, so this module is deliberately small and defensive.
"""

from __future__ import annotations

import argparse
import dataclasses
import json
import sys
from typing import Any

# Make the sidecar directory importable whether we're launched as a script
# (via the user's shell alias) or as `python -m python_sidecar.main`.
import os
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import config  # noqa: E402
import ollama_client  # noqa: E402
import safe_exec  # noqa: E402


# ---------------------------------------------------------------------------
# Strict response schema
# ---------------------------------------------------------------------------
#
# The AI MUST reply with exactly one JSON object matching:
#
#   { "action": "shell",   "command": "ls -la" }   # whitelisted only
#   { "action": "message", "text":    "..."     }
#
# Anything else is rejected and the user is shown the raw model output for
# debugging. Note we never `eval()` or `exec()` model output.
SYSTEM_PROMPT = """\
You are amnia, a local desktop assistant.

You MUST reply with a single JSON object and nothing else. Allowed shapes:

  {"action": "shell", "command": "<safe one-liner>"}
  {"action": "message", "text": "<your reply>"}

Rules:
- Never use shell metacharacters (|, &, ;, >, <, `, $()).
- Only use commands from this whitelist: %s.
- If the user's request is unsafe, refuse with {"action": "message", "text": "..."}.
- Do not include markdown fences, prose, or anything outside the JSON object.
"""


def _parse_strict_json(text: str) -> dict[str, Any]:
    """Validate the model's reply matches the strict schema."""
    text = text.strip()
    if not text:
        raise ValueError("empty response")

    # Some models wrap JSON in ``` fences despite instructions — strip them.
    if text.startswith("```"):
        text = text.strip("`")
        # drop optional language tag on the first line
        if "\n" in text:
            text = text.split("\n", 1)[1]
        text = text.rstrip("`").strip()

    obj = json.loads(text)
    if not isinstance(obj, dict):
        raise ValueError("response must be a JSON object")

    action = obj.get("action")
    if action == "shell":
        cmd = obj.get("command")
        if not isinstance(cmd, str) or not cmd.strip():
            raise ValueError("shell action requires a non-empty 'command' string")
        return {"action": "shell", "command": cmd}
    if action == "message":
        text_field = obj.get("text")
        if not isinstance(text_field, str):
            raise ValueError("message action requires a 'text' string")
        return {"action": "message", "text": text_field}

    raise ValueError(f"unknown action: {action!r}")


# ---------------------------------------------------------------------------
# Subcommands
# ---------------------------------------------------------------------------

def cmd_print_config(_: argparse.Namespace) -> int:
    """Emit the config as JSON so the Rust core can parse it."""
    payload = {
        "github_repo_url": config.GITHUB_REPO_URL,
        "default_model": config.DEFAULT_MODEL,
        "ollama_host": config.OLLAMA_HOST,
        "command_whitelist": list(config.COMMAND_WHITELIST),
    }
    print(json.dumps(payload, indent=2))
    return 0


def cmd_ask(args: argparse.Namespace) -> int:
    """Forward a prompt to Ollama, parse JSON reply, optionally exec command."""
    prompt = " ".join(args.prompt).strip()
    if not prompt:
        print("error: empty prompt", file=sys.stderr)
        return 2

    system = SYSTEM_PROMPT % (", ".join(config.COMMAND_WHITELIST))
    try:
        raw = ollama_client.generate(prompt, model=args.model, system=system)
    except ollama_client.OllamaError as exc:
        print(f"ollama error: {exc}", file=sys.stderr)
        return 3

    try:
        decision = _parse_strict_json(raw)
    except (ValueError, json.JSONDecodeError) as exc:
        print(f"model produced invalid JSON ({exc}). Raw output:\n{raw}", file=sys.stderr)
        return 4

    if decision["action"] == "message":
        print(decision["text"])
        return 0

    # action == "shell"
    cmd = decision["command"]
    try:
        result = safe_exec.run_safe(cmd)
    except safe_exec.CommandNotAllowed as exc:
        print(f"refused: {exc}", file=sys.stderr)
        return 5

    print(json.dumps(dataclasses.asdict(result), indent=2))
    return result.returncode


# ---------------------------------------------------------------------------
# Argparse
# ---------------------------------------------------------------------------

def _build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(prog="amnia", description="amnia local AI sidecar")
    parser.add_argument(
        "--print-config",
        action="store_true",
        help="emit runtime config as JSON and exit (used by the Rust core)",
    )
    parser.add_argument(
        "--model",
        default=None,
        help=f"override the Ollama model (default: {config.DEFAULT_MODEL})",
    )
    parser.add_argument(
        "prompt",
        nargs=argparse.REMAINDER,
        help="natural-language request; everything after the flags",
    )
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = _build_parser()
    args = parser.parse_args(argv)

    if args.print_config:
        return cmd_print_config(args)
    return cmd_ask(args)


if __name__ == "__main__":
    sys.exit(main())
