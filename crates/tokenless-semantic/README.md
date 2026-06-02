# tokenless-semantic

Semantic-aware JSON field compression with ONNX embeddings.

Part of the [tokenless](https://github.com/TokenFleet-AI/tokenless) toolkit.

## Features

```rust
use tokenless_semantic::SemanticCompressor;

let compressor = SemanticCompressor::new();

// Level 1 (default): keyword-based context detection
let result = compressor.compress_fields(
    &json_value,
    &["temperature", "humidity", "wind_speed"],
);

// Level 2 (onnx feature): ONNX embedding similarity
let result = compressor.compress_with_context(
    &json_value,
    "weather forecast for Tokyo",
);
```

### Levels

| Level | Feature | Description |
|-------|---------|-------------|
| **Level 1** | default | Keyword rules from compiled-in TOML profiles. Zero deps, no model download. |
| **Level 2** | `onnx` | ONNX model (`all-MiniLM-L6-v2`, ~15 MB). Cosine similarity between field names and task context. Falls back to Level 1 automatically. |

Enable Level 2:

```toml
[dependencies]
tokenless-semantic = { version = "0.4", features = ["onnx"] }
```

## Minimum Rust Version

Rust 2024 edition, MSRV 1.89.

License: Apache-2.0
