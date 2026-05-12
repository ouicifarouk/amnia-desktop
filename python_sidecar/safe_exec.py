"""
Whitelist-based safe wrapper around :mod:`subprocess`.

The AI is *never* given a shell. We always invoke commands as ``argv`` lists
with ``shell=False``, and we reject any command whose argv[0] is not present
in :data:`config.COMMAND_WHITELIST`.
"""

from __future__ import annotations

import shlex
import subprocess
from dataclasses import dataclass
from typing import Sequence

import config


class CommandNotAllowed(Exception):
    """Raised when the AI requests a binary not in the whitelist."""


@dataclass(slots=True)
class CommandResult:
    argv: tuple[str, ...]
    returncode: int
    stdout: str
    stderr: str
    truncated: bool


def parse_command(command: str | Sequence[str]) -> tuple[str, ...]:
    """Normalise an AI-supplied command into a safe argv tuple.

    Strings are parsed with :func:`shlex.split` (POSIX rules, no shell
    expansion). Sequences are coerced to tuples of strings. ``shell=True``
    style invocations (e.g. ``"a && b"``) are rejected because the resulting
    argv contains shell metacharacters that the whitelist could not enforce
    safely.
    """
    if isinstance(command, str):
        argv = tuple(shlex.split(command, posix=True))
    else:
        argv = tuple(str(x) for x in command)

    if not argv:
        raise CommandNotAllowed("empty command")

    for token in argv:
        if any(meta in token for meta in ("|", "&", ";", ">", "<", "`", "$(")):
            raise CommandNotAllowed(f"shell metacharacter in argument: {token!r}")

    binary = argv[0]
    if binary not in config.COMMAND_WHITELIST:
        raise CommandNotAllowed(f"binary {binary!r} is not in the whitelist")

    return argv


def run_safe(command: str | Sequence[str]) -> CommandResult:
    """Execute *command* with subprocess and bounded resources."""
    argv = parse_command(command)
    try:
        proc = subprocess.run(  # noqa: S603 — argv is validated, shell=False
            argv,
            shell=False,
            capture_output=True,
            text=True,
            timeout=config.CMD_TIMEOUT_SECONDS,
            check=False,
        )
    except subprocess.TimeoutExpired as exc:
        return CommandResult(
            argv=argv,
            returncode=124,
            stdout=(exc.stdout or "")[: config.MAX_CMD_OUTPUT_BYTES],
            stderr=f"timed out after {config.CMD_TIMEOUT_SECONDS}s",
            truncated=False,
        )

    stdout = proc.stdout or ""
    stderr = proc.stderr or ""
    truncated = (
        len(stdout) > config.MAX_CMD_OUTPUT_BYTES
        or len(stderr) > config.MAX_CMD_OUTPUT_BYTES
    )
    return CommandResult(
        argv=argv,
        returncode=proc.returncode,
        stdout=stdout[: config.MAX_CMD_OUTPUT_BYTES],
        stderr=stderr[: config.MAX_CMD_OUTPUT_BYTES],
        truncated=truncated,
    )
