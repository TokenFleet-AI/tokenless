#!/usr/bin/env bash
set -euo pipefail

echo "[tokenless] Uninstalling OpenClaw plugin..."

if command -v openclaw &>/dev/null; then
    openclaw plugins uninstall tokenless-openclaw --force || true
fi

# Clean up config entries
OPENCLAW_CFG="$HOME/.openclaw/openclaw.json"
if [ -f "$OPENCLAW_CFG" ] && command -v jq &>/dev/null; then
    jq '(.plugins.allow // [] | map(select(. != "tokenless-openclaw"))) as $allow |
        (.plugins.entries // {} | del(.["tokenless-openclaw"])) as $entries |
        .plugins.allow = $allow | .plugins.entries = $entries' \
        "$OPENCLAW_CFG" > "${OPENCLAW_CFG}.tmp" && mv "${OPENCLAW_CFG}.tmp" "$OPENCLAW_CFG"
fi

echo "[tokenless] OpenClaw plugin uninstalled."
