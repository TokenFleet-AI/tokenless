#!/usr/bin/env bash
set -euo pipefail

# Dev-mode install script — builds and installs tokenless to ~/.tokenfleet-ai/bin

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
cd "$SCRIPT_DIR/.."

echo "[dev-install] Building tokenless (with ONNX Level 2)..."
cargo build --release --features tokenless-semantic/onnx

echo "[dev-install] Installing to ~/.tokenfleet-ai/bin/tokenless"
mkdir -p "$HOME/.tokenfleet-ai/bin"
cp target/release/tokenless "$HOME/.tokenfleet-ai/bin/tokenless"
# Ad-hoc sign to prevent macOS from killing the binary
codesign --force --sign - "$HOME/.tokenfleet-ai/bin/tokenless" 2>/dev/null || true

# Copy ONNX model files for Level 2 semantic compression
MODEL_SRC="$SCRIPT_DIR/crates/tokenless-semantic/models"
MODEL_DST="$HOME/.tokenfleet-ai/tokenless/models"
if [ -f "$MODEL_SRC/all-MiniLM-L6-v2.onnx" ] && [ -f "$MODEL_SRC/tokenizer.json" ]; then
  echo "[dev-install] Copying ONNX models to $MODEL_DST"
  mkdir -p "$MODEL_DST"
  cp "$MODEL_SRC/all-MiniLM-L6-v2.onnx" "$MODEL_DST/"
  cp "$MODEL_SRC/tokenizer.json" "$MODEL_DST/"
fi

echo "[dev-install] Done — $(tokenless --version 2>/dev/null || echo 'verify with: tokenless --version')"
