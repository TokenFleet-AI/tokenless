"""Tokenless Plugin for Hermes Agent.

Provides response compression, TOON encoding, Tool Ready pre-check,
and command rewriting for Hermes Agent.
"""

from __future__ import annotations

import json
import logging
import os
import re
import shutil
import subprocess
from typing import Any

logger = logging.getLogger(__name__)

AGENT_ID = "hermes-agent"
_MIN_RESPONSE_LEN = 200
_SKIP_TOOLS: set[str] = {"read_file", "list_directory", "glob", "notebook_read"}
_SHELL_TOOLS: set[str] = {"terminal"}
_MIN_RTK_VERSION = (0, 35, 0)
_CONTEXT_DIR = os.path.join(os.path.expanduser("~"), ".tokenfleet-ai", "tokenless")
_CONTEXT_FILE = os.path.join(_CONTEXT_DIR, ".rewrite-context")

_resolved: dict[str, str | None] = {}


def _resolve_binary(name: str) -> str | None:
    if name in _resolved:
        return _resolved[name]
    path = shutil.which(name)
    if path:
        _resolved[name] = path
        return path
    _resolved[name] = None
    return None


def _have(name: str) -> bool:
    return _resolve_binary(name) is not None


def _try_parse_json(data: str) -> Any:
    try:
        return json.loads(data)
    except (json.JSONDecodeError, ValueError):
        return None


def _is_skill_file(text: str) -> bool:
    if not isinstance(text, str) or not text.startswith("---"):
        return False
    for line in text.split("\n", 20)[1:]:
        if line.startswith("name:") or line.startswith("description:"):
            return True
    return False


def _run(
    args: list[str], input_data: str = "", timeout: int = 10
) -> subprocess.CompletedProcess | None:
    try:
        return subprocess.run(
            args, input=input_data, capture_output=True, text=True, timeout=timeout
        )
    except Exception:
        return None


def _parse_version(ver: str) -> tuple | None:
    m = re.search(r"(\d+)\.(\d+)\.(\d+)", ver)
    return (int(m.group(1)), int(m.group(2)), int(m.group(3))) if m else None


# ── 1. Response compression ──────────────────────────────────


def _compress_response(
    tool_name: str, result: str, session_id: str, tool_call_id: str
) -> str | None:
    if not _have("tokenless"):
        return None
    parsed = _try_parse_json(result)
    if not isinstance(parsed, (dict, list)):
        return None
    cmd = ["tokenless", "compress-response", "--agent-id", AGENT_ID]
    if session_id:
        cmd.extend(["--session-id", session_id])
    if tool_call_id:
        cmd.extend(["--tool-use-id", tool_call_id])
    proc = _run(cmd, result)
    if not proc or proc.returncode != 0 or not proc.stdout.strip():
        return None
    compressed = proc.stdout.strip()
    return None if compressed == result else compressed


# ── 2. TOON encoding ────────────────────────────────────────


def _encode_toon(
    data: str, session_id: str = "", tool_call_id: str = ""
) -> tuple[str, int] | None:
    if not _have("tokenless"):
        return None
    parsed = _try_parse_json(data)
    if not isinstance(parsed, (dict, list)):
        return None
    cmd = ["tokenless", "compress-toon", "--agent-id", AGENT_ID]
    if session_id:
        cmd.extend(["--session-id", session_id])
    if tool_call_id:
        cmd.extend(["--tool-use-id", tool_call_id])
    proc = _run(cmd, data)
    if not proc or proc.returncode != 0 or not proc.stdout.strip():
        return None
    text = proc.stdout.strip()
    if text == data or len(text) > len(data):
        return None
    pct = (len(data) - len(text)) * 100 // len(data) if len(data) > 0 else 0
    return text, pct


# ── 3. Tool Ready ───────────────────────────────────────────


def _env_check(tool_name: str) -> str | None:
    if not _have("tokenless"):
        return None
    proc = _run(["tokenless", "env-check", "--tool", tool_name, "--json"])
    if not proc or not proc.stdout.strip():
        return None
    try:
        parsed = json.loads(proc.stdout)
    except json.JSONDecodeError:
        return None
    status = parsed.get("status", "UNKNOWN")
    if status in ("UNKNOWN", "READY"):
        return None
    proc = _run(["tokenless", "env-check", "--tool", tool_name, "--fix", "--json"])
    if not proc or not proc.stdout.strip():
        return f"[tokenless tool-ready] {tool_name}: NOT_READY — environment issue"
    fix_parsed = json.loads(proc.stdout)
    if fix_parsed.get("status") == "READY":
        return None
    return (
        fix_parsed.get("diagnostic") or f"[tokenless tool-ready] {tool_name}: NOT_READY"
    )


# ── 4. Command rewriting ────────────────────────────────────


def _try_rewrite(
    args: Any, session_id: str, tool_call_id: str
) -> dict[str, str] | None:
    if not _have("rtk"):
        return None
    if not isinstance(args, dict):
        return None
    command = args.get("command", "")
    if not command:
        return None
    try:
        ver = _parse_version(
            subprocess.run(
                ["rtk", "--version"], capture_output=True, text=True, timeout=3
            ).stdout
        )
        if ver and ver < _MIN_RTK_VERSION:
            logger.warning("tokenless: rtk too old, rewrite skipped")
            return None
    except Exception:
        pass
    os.makedirs(_CONTEXT_DIR, exist_ok=True)
    with open(_CONTEXT_FILE, "w") as f:
        f.write(f"{AGENT_ID}\n{session_id}\n{tool_call_id}\n")
    env = os.environ.copy()
    env["TOKENLESS_AGENT_ID"] = AGENT_ID
    if session_id:
        env["TOKENLESS_SESSION_ID"] = session_id
    proc = subprocess.run(
        ["rtk", "rewrite", command], capture_output=True, text=True, timeout=5, env=env
    )
    if proc.returncode != 0:
        return None
    rewritten = proc.stdout.strip()
    if not rewritten or rewritten == command:
        return None
    logger.info("tokenless: rtk rewrite %s -> %s", command, rewritten)
    return {
        "action": "block",
        "message": f"[tokenless] Rewritten: {command} -> {rewritten}",
    }


# ── Hooks ───────────────────────────────────────────────────


def on_session_start(**kwargs: Any) -> None:
    session_id = kwargs.get("session_id", "")
    if session_id:
        os.environ["TOKENLESS_SESSION_ID"] = str(session_id)


def on_pre_tool_call(
    tool_name: str = "",
    args: Any = None,
    session_id: str = "",
    tool_call_id: str = "",
    **kwargs: Any,
) -> dict[str, str] | None:
    # Step 1: env-check
    if _have("tokenless"):
        if session_id:
            os.environ["TOKENLESS_SESSION_ID"] = str(session_id)
        feedback = _env_check(tool_name)
        if feedback:
            return {"action": "block", "message": feedback}
    # Step 2: RTK rewrite
    if tool_name in _SHELL_TOOLS and _have("rtk"):
        result = _try_rewrite(args, str(session_id), str(tool_call_id))
        if result:
            return result
    return None


def on_transform_tool_result(
    tool_name: str = "",
    result: str = "",
    session_id: str = "",
    tool_call_id: str = "",
    **kwargs: Any,
) -> str | None:
    if not _have("tokenless"):
        return None
    if tool_name in _SKIP_TOOLS:
        return None
    if not result or result in ("{}", "[]") or len(result) < _MIN_RESPONSE_LEN:
        return None
    if _is_skill_file(result):
        return None
    if _try_parse_json(result) is None:
        return None
    original_len = len(result)
    compressed = _compress_response(
        tool_name, result, str(session_id), str(tool_call_id)
    )
    current = compressed if compressed else result
    toon_result = _encode_toon(current, str(session_id), str(tool_call_id))
    used_compression = compressed is not None
    used_toon = toon_result is not None
    if not used_compression and not used_toon:
        return None
    if used_toon:
        text, pct = toon_result
        final = f"[TOON format, {pct}% token savings]\n{text}"
    else:
        final = current
    return final


def register(ctx: Any) -> None:
    ctx.register_hook("on_session_start", on_session_start)
    ctx.register_hook("pre_tool_call", on_pre_tool_call)
    ctx.register_hook("transform_tool_result", on_transform_tool_result)
    features = []
    if _have("tokenless"):
        features.extend(["response-compression", "toon-encoding", "tool-ready"])
    if _have("rtk"):
        features.append("rtk-rewrite")
    logger.info(
        "tokenless: Hermes plugin registered — %s",
        ", ".join(features) if features else "no features (install tokenless/rtk)",
    )
