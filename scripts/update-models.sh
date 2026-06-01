#!/usr/bin/env bash
set -euo pipefail

# Update ONNX model files and publish a new GitHub Release.
# Usage: ./scripts/update-models.sh <version>
# Example: ./scripts/update-models.sh v2

VERSION="${1:?Usage: $0 <version>  (e.g. v2, v3)}"
SCRIPT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
MODEL_DIR="$SCRIPT_DIR/crates/tokenless-semantic/models"
EMBEDDER_FILE="$SCRIPT_DIR/crates/tokenless-semantic/src/embedder.rs"

echo "=== Step 1: Download model files ==="

# ONNX model (all-MiniLM-L6-v2)
if [ ! -f "$MODEL_DIR/all-MiniLM-L6-v2.onnx" ]; then
  echo "Downloading all-MiniLM-L6-v2.onnx..."
  curl -L \
    "https://huggingface.co/sentence-transformers/all-MiniLM-L6-v2/resolve/main/onnx/model.onnx" \
    -o "$MODEL_DIR/all-MiniLM-L6-v2.onnx"
else
  echo "Model already exists, skipping download (remove to re-download)"
fi

# Tokenizer
if [ ! -f "$MODEL_DIR/tokenizer.json" ]; then
  echo "Downloading tokenizer.json..."
  curl -L \
    "https://huggingface.co/sentence-transformers/all-MiniLM-L6-v2/resolve/main/tokenizer.json" \
    -o "$MODEL_DIR/tokenizer.json"
else
  echo "Tokenizer already exists, skipping download"
fi

echo ""
echo "=== Step 2: Create GitHub Release models-$VERSION ==="
gh release create "models-$VERSION" \
  --title "ONNX Model Files $VERSION" \
  --notes "all-MiniLM-L6-v2 ONNX model and tokenizer.

Files:
- \`all-MiniLM-L6-v2.onnx\` — FP32 ONNX model
- \`tokenizer.json\` — Tokenizer configuration" \
  "$MODEL_DIR/all-MiniLM-L6-v2.onnx" \
  "$MODEL_DIR/tokenizer.json"

echo ""
echo "=== Step 3: Update download URL in embedder.rs ==="
# Replace models-v1 / models-v2 / ... with the new version
if [[ "$OSTYPE" == "darwin"* ]]; then
  sed -i '' "s|models-v[0-9]*|models-$VERSION|g" "$EMBEDDER_FILE"
else
  sed -i "s|models-v[0-9]*|models-$VERSION|g" "$EMBEDDER_FILE"
fi

echo ""
echo "=== Done ==="
echo "Next steps:"
echo "  1. Review changes: git diff"
echo "  2. Test locally:  make models-install"
echo "  3. Commit:         git add -A && git commit -m 'chore: update ONNX models to $VERSION'"
echo "  4. Push:           git push origin master"
