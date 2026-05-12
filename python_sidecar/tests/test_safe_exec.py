"""Unit tests for the whitelist-based command runner."""
from __future__ import annotations

import os
import sys

import pytest

# Make python_sidecar/ importable without installing the package.
HERE = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, HERE)

import safe_exec  # noqa: E402


def test_whitelisted_command_runs():
    result = safe_exec.run_safe("echo hello")
    assert result.returncode == 0
    assert "hello" in result.stdout


def test_argv_form_accepted():
    result = safe_exec.run_safe(["echo", "amnia"])
    assert result.returncode == 0
    assert "amnia" in result.stdout


@pytest.mark.parametrize(
    "cmd",
    [
        "rm -rf /tmp/x",
        "sudo ls",
        "python3 -c 'print(1)'",
    ],
)
def test_non_whitelisted_command_rejected(cmd):
    with pytest.raises(safe_exec.CommandNotAllowed):
        safe_exec.run_safe(cmd)


@pytest.mark.parametrize(
    "cmd",
    [
        "echo hi | cat",
        "echo a && echo b",
        "echo `whoami`",
        "echo $(whoami)",
        "echo a > /tmp/out",
        "echo a; echo b",
    ],
)
def test_shell_metacharacters_rejected(cmd):
    with pytest.raises(safe_exec.CommandNotAllowed):
        safe_exec.run_safe(cmd)


def test_empty_command_rejected():
    with pytest.raises(safe_exec.CommandNotAllowed):
        safe_exec.run_safe("")
