"""
Minimal HTTP client for the local Ollama daemon.

We deliberately avoid the third-party ``ollama`` Python SDK so the sidecar
ships with zero runtime dependencies beyond the standard library.
"""

from __future__ import annotations

import json
import urllib.error
import urllib.request
from typing import Any, Iterable, Iterator

import config


class OllamaError(RuntimeError):
    pass


def _post_json(path: str, payload: dict[str, Any]) -> Iterator[dict[str, Any]]:
    """POST JSON and yield each line of the streaming response as a dict."""
    url = f"{config.OLLAMA_HOST}{path}"
    data = json.dumps(payload).encode("utf-8")
    req = urllib.request.Request(  # noqa: S310 — fixed localhost URL
        url,
        data=data,
        headers={"Content-Type": "application/json"},
        method="POST",
    )
    try:
        with urllib.request.urlopen(req, timeout=config.OLLAMA_TIMEOUT_SECONDS) as resp:
            for raw in resp:
                line = raw.decode("utf-8").strip()
                if not line:
                    continue
                try:
                    yield json.loads(line)
                except json.JSONDecodeError as exc:
                    raise OllamaError(f"non-JSON line from ollama: {line!r}") from exc
    except urllib.error.URLError as exc:
        raise OllamaError(f"failed to reach ollama at {url}: {exc}") from exc


def generate(prompt: str, *, model: str | None = None, system: str | None = None) -> str:
    """Run a non-streaming chat completion and return the concatenated reply."""
    model = model or config.DEFAULT_MODEL
    payload: dict[str, Any] = {
        "model": model,
        "prompt": prompt,
        "stream": True,
        "options": {"temperature": 0.2},
    }
    if system:
        payload["system"] = system

    chunks: list[str] = []
    for event in _post_json("/api/generate", payload):
        if "response" in event:
            chunks.append(event["response"])
        if event.get("done"):
            break
    return "".join(chunks).strip()


def list_models() -> Iterable[dict[str, Any]]:
    """Return installed models via ``GET /api/tags``."""
    url = f"{config.OLLAMA_HOST}/api/tags"
    try:
        with urllib.request.urlopen(url, timeout=10) as resp:  # noqa: S310
            body = json.loads(resp.read().decode("utf-8"))
    except urllib.error.URLError as exc:
        raise OllamaError(f"failed to reach ollama at {url}: {exc}") from exc
    return body.get("models", [])
