# tokenless-schema

Schema and response compression for LLM token optimization.

Part of the [tokenless](https://github.com/TokenFleet-AI/tokenless) toolkit.

## SchemaCompressor

Compresses OpenAI Function Calling tool definitions:

```rust
use tokenless_schema::SchemaCompressor;

let compressed = SchemaCompressor::new()
    .with_func_desc_max_len(200)
    .compress(&tool_json);
```

- Truncates function/parameter descriptions (default 256/160 chars)
- Removes `title` and `examples` fields
- Strips markdown formatting from descriptions
- Handles nested `anyOf`/`oneOf`/`allOf` schemas
- Returns original unchanged if no savings

## ResponseCompressor

Compresses JSON API/tool responses:

```rust
use tokenless_schema::ResponseCompressor;

let compressed = ResponseCompressor::new()
    .with_truncate_arrays_at(10)
    .compress(&response_json);
```

- Drops debug fields (`debug`, `trace`, `stacktrace`, `logs`)
- Removes `null` values and empty fields
- Truncates strings (default 512 chars) and arrays (default 16 items)
- Depth-limited (default 8 levels)
- Returns original unchanged if no savings

License: Apache-2.0
