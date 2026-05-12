"""Tests for the strict JSON parser and the --print-config command."""
from __future__ import annotations

import json
import os
import subprocess
import sys

import pytest

HERE = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, HERE)

import main as sidecar_main  # noqa: E402


def test_parse_strict_json_message():
    out = sidecar_main._parse_strict_json('{"action":"message","text":"hi"}')
    assert out == {"action": "message", "text": "hi"}


def test_parse_strict_json_shell():
    out = sidecar_main._parse_strict_json('{"action":"shell","command":"ls -la"}')
    assert out == {"action": "shell", "command": "ls -la"}


def test_parse_strict_json_strips_code_fence():
    raw = "```json\n{\"action\":\"message\",\"text\":\"hi\"}\n```"
    out = sidecar_main._parse_strict_json(raw)
    assert out["action"] == "message"


@pytest.mark.parametrize(
    "bad",
    [
        "",
        "not json",
        '{"action":"unknown"}',
        '{"action":"shell"}',
        '{"action":"shell","command":""}',
        '{"action":"message"}',
        '["array","not","object"]',
    ],
)
def test_parse_strict_json_rejects(bad):
    with pytest.raises((ValueError, json.JSONDecodeError)):
        sidecar_main._parse_strict_json(bad)


def test_print_config_subcommand():
    """Running `python main.py --print-config` must emit valid JSON to stdout."""
    main_py = os.path.join(HERE, "main.py")
    out = subprocess.run(
        [sys.executable, main_py, "--print-config"],
        capture_output=True,
        text=True,
        check=True,
    )
    payload = json.loads(out.stdout)
    assert "github_repo_url" in payload
    assert "default_model" in payload
    assert "ollama_host" in payload
    assert isinstance(payload["command_whitelist"], list)
