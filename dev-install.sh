#!/usr/bin/env bash
set -euo pipefail

# Dev-mode install script — builds and installs tokenless to ~/.cargo/bin

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
cd "$SCRIPT_DIR"

echo "[dev-install] Building tokenless..."
cargo build --release

echo "[dev-install] Installing to ~/.cargo/bin/tokenless"
mkdir -p "$HOME/.cargo/bin"
cp target/release/tokenless "$HOME/.cargo/bin/tokenless"

echo "[dev-install] Done — $(tokenless --version 2>/dev/null || echo 'verify with: tokenless --version')"
