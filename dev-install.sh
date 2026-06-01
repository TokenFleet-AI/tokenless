#!/usr/bin/env bash
set -euo pipefail

# Dev-mode install script — builds and installs tokenless to ~/.cargo/bin

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
cd "$SCRIPT_DIR"

echo "[dev-install] Building tokenless (with ONNX Level 2)..."
cargo build --release --features tokenless-semantic/onnx

echo "[dev-install] Installing to ~/.cargo/bin/tokenless"
mkdir -p "$HOME/.cargo/bin"
cp target/release/tokenless "$HOME/.cargo/bin/tokenless"

# Copy ONNX model files for Level 2 semantic compression
MODEL_SRC="$SCRIPT_DIR/crates/tokenless-semantic/models"
MODEL_DST="$HOME/.tokenless/models"
if [ -f "$MODEL_SRC/all-MiniLM-L6-v2.onnx" ] && [ -f "$MODEL_SRC/tokenizer.json" ]; then
  echo "[dev-install] Copying ONNX models to $MODEL_DST"
  mkdir -p "$MODEL_DST"
  cp "$MODEL_SRC/all-MiniLM-L6-v2.onnx" "$MODEL_DST/"
  cp "$MODEL_SRC/tokenizer.json" "$MODEL_DST/"
fi

echo "[dev-install] Done — $(tokenless --version 2>/dev/null || echo 'verify with: tokenless --version')"
