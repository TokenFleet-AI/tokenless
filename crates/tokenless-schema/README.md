# tokenless-schema

[![Crates.io](https://img.shields.io/crates/v/tokenless-schema.svg)](https://crates.io/crates/tokenless-schema)
[![Docs](https://docs.rs/tokenless-schema/badge.svg)](https://docs.rs/tokenless-schema)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://github.com/TokenFleet-AI/tokenless/blob/master/LICENSE)

Schema and response compression for LLM token optimization.

Part of the [tokenless](https://github.com/TokenFleet-AI/tokenless) toolkit.

## Why tokenless-schema?

Every byte sent to an LLM costs tokens. OpenAI Function Calling schemas routinely contain 500–2000+ characters of descriptions and metadata that the model doesn't need. API responses carry `debug`, `trace`, and `stacktrace` fields that are useless to an AI agent. This crate trims the fat — structurally compressing both function definitions and tool outputs before they reach the model.

## Quick Start

```toml
[dependencies]
tokenless-schema = "0.4"
```

## SchemaCompressor

Compresses OpenAI Function Calling tool definitions:

```rust
use tokenless_schema::SchemaCompressor;

let compressed = SchemaCompressor::new()
    .with_func_desc_max_len(200)
    .with_param_desc_max_len(160)
    .with_max_enum_values(8)
    .with_remove_examples(true)
    .compress(&tool_json);
```

| Builder Method | Default | Description |
|---------------|---------|-------------|
| `with_func_desc_max_len(n)` | 256 | Max function description chars |
| `with_param_desc_max_len(n)` | 160 | Max parameter description chars |
| `with_max_enum_values(n)` | 16 | Truncate enum arrays |
| `with_remove_examples(bool)` | true | Drop `examples` fields |
| `with_max_depth(n)` | 8 | Max nesting depth |
| `with_profile(profile)` | — | Apply a `CompressionProfile` preset |

### `CompressionProfile`

Presets: `Minimal`, `Standard`, `Aggressive`, `Custom`.

## ResponseCompressor

Compresses JSON API/tool responses:

```rust
use tokenless_schema::ResponseCompressor;

let compressed = ResponseCompressor::new()
    .with_truncate_arrays_at(10)
    .with_truncate_strings_at(512)
    .with_max_depth(8)
    .with_profile(CompressionProfile::Aggressive)
    .compress(&response_json);
```

- Drops debug fields (`debug`, `trace`, `stacktrace`, `logs`)
- Removes `null` values and empty fields
- Truncates strings and arrays
- Depth-limited traversal
- Returns original unchanged if no savings

## Format Router

Intelligently selects the optimal encoding strategy based on JSON structure:

```rust
use tokenless_schema::{compress_auto, Strategy, select_strategy, strategy_name};

let strategy = select_strategy(&json_value);
println!("Selected: {}", strategy_name(strategy));
// Strategy::ToonHrv, Strategy::EnhancedToon, Strategy::CjsonCompact, Strategy::Core

let compressed = compress_auto(&json_value);
```

## Shape Analyzer

```rust
use tokenless_schema::{analyze, JsonShape, TopType};

let shape = analyze(&json_value);
// JsonShape { top_type: TopType::Object, depth: 3, total_nodes: 42, ... }
```

## Experimental Mode

Control enhanced features (format router, enhanced TOON encoders):

```rust
use tokenless_schema;

tokenless_schema::set_experimental_mode(false);
// Falls back to core compression only

if tokenless_schema::is_experimental_mode() {
    // Enhanced features available
}
```

## Minimum Rust Version

Rust 2024 edition, MSRV 1.89.

License: Apache-2.0
