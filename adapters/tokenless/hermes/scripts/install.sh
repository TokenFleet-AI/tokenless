#!/usr/bin/env bash
set -euo pipefail

AGENT="${ANOLISA_TARGET:-hermes}"
ADAPTER_DIR="${ANOLISA_ADAPTER_DIR:-$(cd "$(dirname "$0")/../.." && pwd)}"

PLUGIN_SRC="$ADAPTER_DIR/hermes"
PLUGIN_DST="$HOME/.hermes/plugins/tokenless"

echo "[tokenless] Installing $AGENT plugin..."

if [ ! -d "$PLUGIN_SRC" ]; then
    echo "[tokenless] Plugin source not found: $PLUGIN_SRC"
    exit 1
fi

if [ ! -f "$PLUGIN_SRC/plugin.yaml" ] || [ ! -f "$PLUGIN_SRC/__init__.py" ]; then
    echo "[tokenless] Missing plugin.yaml or __init__.py in $PLUGIN_SRC"
    exit 1
fi

mkdir -p "$PLUGIN_DST"
ln -sfn "$PLUGIN_SRC/__init__.py" "$PLUGIN_DST/__init__.py"
ln -sfn "$PLUGIN_SRC/plugin.yaml" "$PLUGIN_DST/plugin.yaml"

echo "[tokenless] $AGENT plugin linked to $PLUGIN_DST"

if command -v hermes &>/dev/null; then
    echo "[tokenless] Enabling $AGENT plugin..."
    hermes plugins enable tokenless || {
        echo "[tokenless] Warning: enable failed — do it manually in config.yaml"
    }
else
    echo "[tokenless] hermes CLI not found — add 'tokenless' to plugins.enabled in config.yaml"
fi
