#!/usr/bin/env bash
set -euo pipefail

echo "[tokenless] Uninstalling Hermes plugin..."

PLUGIN_DST="$HOME/.hermes/plugins/tokenless"

if command -v hermes &>/dev/null; then
    hermes plugins disable tokenless || true
fi

rm -rf "$PLUGIN_DST"
echo "[tokenless] Hermes plugin uninstalled."
