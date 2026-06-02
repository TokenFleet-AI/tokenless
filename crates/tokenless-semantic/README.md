# tokenless-semantic

Semantic-aware JSON field compression with ONNX embeddings.

Part of the [tokenless](https://github.com/TokenFleet-AI/tokenless) toolkit.

## Quick Start

```toml
[dependencies]
tokenless-semantic = "0.4"

# With ONNX support:
# tokenless-semantic = { version = "0.4", features = ["onnx"] }
```

```rust
use tokenless_semantic::SemanticCompressor;

let compressor = SemanticCompressor::new();

// The `compress` method works for both Level 1 and Level 2.
// It keeps fields whose names are semantically relevant to the given context.
let filtered = compressor.compress(&json_value, "weather forecast for Tokyo");
```

### Levels

| Level | Feature | Description |
|-------|---------|-------------|
| **Level 1** | default | Keyword rules from compiled-in TOML profiles (weather, devops, database, git, default). Zero deps, no model download. |
| **Level 2** | `onnx` | ONNX model (`all-MiniLM-L6-v2`, ~86 MB). Cosine similarity between field names and task context. Falls back to Level 1 automatically if the model is unavailable. |

To use Level 2, call `load_onnx()` first (requires the `onnx` feature):

```rust
let mut compressor = SemanticCompressor::new();
match compressor.load_onnx() {
    Ok(true) => println!("ONNX model loaded"),
    Ok(false) => eprintln!("Model not found, using Level 1 fallback"),
    Err(e) => eprintln!("Failed to load model: {e}"),
}
```

Model files are auto-downloaded on first use from GitHub Releases and cached in `~/.tokenless/models/`.

## API

| Method | Description |
|--------|-------------|
| `SemanticCompressor::new()` | Create compressor (Level 1, or `Default::default()`) |
| `compress(&self, value, context) -> Value` | Keep only fields relevant to `context` |
| `is_field_kept(&self, field_name, context) -> bool` | Check if a single field would be kept |
| `detect_category(&self, context) -> &str` | Classify context into a predefined category |
| `load_onnx(&mut self) -> Result<bool, EmbedderError>` | Load ONNX model (requires `onnx` feature) |

### `EmbedderError`

Variants: `ModelNotFound`, `SessionError`, `ModelDownloadError`, `CacheError`, `IoError`, `TokenizerError`, `ThreadPanic`.

## Minimum Rust Version

Rust 2024 edition, MSRV 1.89.

License: Apache-2.0
