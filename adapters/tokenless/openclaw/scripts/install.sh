#!/usr/bin/env bash
set -euo pipefail

ADAPTER_DIR="$(cd "$(dirname "$0")/../.." && pwd)"
PLUGIN_SRC="$ADAPTER_DIR/openclaw"

echo "[tokenless] Installing OpenClaw plugin..."

if ! command -v openclaw &>/dev/null; then
    echo "[tokenless] openclaw CLI not found — skipping."
    echo "  Install OpenClaw first, then run this script again."
    exit 0
fi

if [ ! -d "$PLUGIN_SRC" ]; then
    echo "[tokenless] Plugin source not found: $PLUGIN_SRC"
    exit 1
fi

# Compile TS to JS
if command -v npx &>/dev/null; then
    npx --yes esbuild "$PLUGIN_SRC/index.ts" --bundle --platform=node --format=esm --outfile="$PLUGIN_SRC/index.js" 2>/dev/null || \
    sed 's/: [^,)]*//g' "$PLUGIN_SRC/index.ts" > "$PLUGIN_SRC/index.js"
else
    sed 's/: [^,)]*//g' "$PLUGIN_SRC/index.ts" > "$PLUGIN_SRC/index.js"
fi

openclaw plugins install "$PLUGIN_SRC" --force --dangerously-force-unsafe-install || {
    echo "[tokenless] openclaw CLI install failed"
    exit 1
}

echo "[tokenless] OpenClaw plugin installed."
echo "  Run 'openclaw gateway restart' to activate."
