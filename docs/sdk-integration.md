# Tokenless SDK Integration Guide

> Version: 0.4.0 | Language: English

## Overview

Tokenless provides three standalone Rust crates for embedding compression capabilities into third-party applications:

| Crate | Purpose | Dependencies |
|------|------|:---:|
| `tokenless-schema` | Schema & response compression engine | 3 (serde_json, regex, tracing) |
| `tokenless-stats` | SQLite-based compression metrics | 7 (rusqlite, serde, etc.) |
| `tokenless-semantic` | Context-aware field filtering (optional ONNX) | 3 (or 6 with `onnx` feature) |

All crates enforce `#![forbid(unsafe_code)]` and target Rust 2024 edition (MSRV 1.89.0).

---

## Quick Start

### Cargo.toml

```toml
[dependencies]
tokenless-schema = "0.4.0"
tokenless-stats = "0.4.0"
tokenless-semantic = { version = "0.4.0", features = ["onnx"] }  # optional
```

### Minimal Example

```rust
use tokenless_schema::ResponseCompressor;

fn main() {
    let json = r#"{"name":"Alice","debug":{"query_ms":42},"trace":"req-123"}"#;
    let value: serde_json::Value = serde_json::from_str(json).unwrap();

    let compressor = ResponseCompressor::new();
    let compressed = compressor.compress(&value);

    // debug and trace fields are dropped automatically
    println!("{}", serde_json::to_string(&compressed).unwrap());
}
```

---

## Crate Reference

### tokenless-schema

#### SchemaCompressor

Compresses OpenAI Function Calling tool definitions.

```rust
use tokenless_schema::SchemaCompressor;

let compressor = SchemaCompressor::new()
    .with_func_desc_max_len(256)      // function-level description cap
    .with_param_desc_max_len(160)     // parameter-level description cap
    .with_func_desc_max_tokens(40)    // token-aware limit (optional)
    .with_max_enum_items(20)          // enum truncation (optional)
    .with_drop_titles(true)
    .with_drop_examples(true);

let tool = serde_json::json!({
    "function": {
        "name": "get_weather",
        "description": "...",
        "parameters": { /* ... */ }
    }
});
let compressed = compressor.compress(&tool);
// Returns original if no savings (zero-cost fallback).
```

| Builder Method | Default | Description |
|------|:---:|------|
| `with_func_desc_max_len(n)` | 256 | Max chars for function-level description |
| `with_param_desc_max_len(n)` | 160 | Max chars for parameter-level description |
| `with_func_desc_max_tokens(n)` | MAX | Token-aware soft limit (CJK-aware estimator) |
| `with_param_desc_max_tokens(n)` | MAX | Token-aware soft limit for params |
| `with_max_enum_items(n)` | MAX | Truncate enum arrays, mark with `x-tokenless-enum-truncated` |
| `with_drop_titles(b)` | true | Remove `title` from all schema levels |
| `with_drop_examples(b)` | true | Remove `examples` from all schema levels |
| `with_drop_markdown(b)` | true | Strip markdown formatting from descriptions |

#### ResponseCompressor

Compresses JSON API responses: drops debug fields, truncates long strings/arrays, removes nulls/empty values.

```rust
use tokenless_schema::{ResponseCompressor, CompressionProfile};

// Standard profile: 512 chars, 16 items, 8 depth
let standard = ResponseCompressor::new();

// High-fidelity for shell command output: 4096 chars, 128 items
let hf = ResponseCompressor::new()
    .with_profile(CompressionProfile::HighFidelity);

// Fully custom
let custom = ResponseCompressor::new()
    .with_string_truncate(256)
    .with_array_truncate(8)
    .with_max_depth(6)
    .with_drop_nulls(true)
    .with_drop_empty_fields(false)
    .with_drop_field("internal_debug_key");
```

| Builder Method | Default | Description |
|------|:---:|------|
| `with_string_truncate(n)` | 512 | Max string length (char-safe at UTF-8 boundaries) |
| `with_array_truncate(n)` | 16 | Max array items |
| `with_max_depth(n)` | 8 | Max nesting depth |
| `with_drop_nulls(b)` | true | Remove `null` values |
| `with_drop_empty_fields(b)` | true | Remove `""`, `[]`, `{}` |
| `with_drop_field(name)` | — | Add a custom field name to the drop list |
| `with_profile(profile)` | Standard | Apply a preset: `Standard` or `HighFidelity` |

#### Format Router

Intelligently selects the optimal encoding strategy based on JSON structure:

```rust
use tokenless_schema::{compress_auto, strategy_name, Strategy};

let (strategy, result) = compress_auto(&value, &original_json);
// Strategy: ToonHrv | EnhancedToon | CjsonCompact | CompressorOnly
```

#### Strategy Selection

Lower-level exports for custom routing logic:

```rust
use tokenless_schema::{select_strategy, JsonShape, TopType, analyze};

// Inspect JSON structure before deciding on a strategy
let shape: JsonShape = analyze(&value);
// shape.top_type: TopType::Object | Array | Primitive
// shape.field_count, shape.max_depth, shape.total_string_chars

// Select optimal strategy manually
let strategy = select_strategy(&shape);
```

| Export | Purpose |
|------|------|
| `select_strategy(shape)` | Choose best compression strategy from a `JsonShape` |
| `JsonShape` | Structural summary: `top_type`, `field_count`, `max_depth`, `total_string_chars` |
| `TopType` | Enum: `Object`, `Array`, `Primitive` |
| `analyze(value)` | Compute `JsonShape` from any `&serde_json::Value` |

---

### tokenless-stats

SQLite-backed metrics tracking for compression operations.

#### Recording

```rust
use tokenless_stats::{StatsRecorder, StatsRecord, OperationType};
use tokenless_stats::estimate_tokens_from_bytes;

let recorder = StatsRecorder::new("/path/to/stats.db")?;

let record = StatsRecord::new(
    OperationType::CompressResponse,
    "my-agent".into(),         // agent_id
    before_chars,              // byte length before
    before_tokens,             // estimated tokens before
    after_chars,               // byte length after
    after_tokens,              // estimated tokens after
    "my-project".into(),       // project
    "default".into(),          // namespace
    before_output,             // byte length of output before
    after_output,              // byte length of output after
)
.with_session_id("session-abc")
.with_tool_use_id("call_xyz")
.with_before_text(before_json)
.with_after_text(after_json)
.with_experimental_mode(false)
.with_source_pid(12345)
.with_text("my text".into())
.with_output("my output".into());

recorder.record(&record)?;
```

#### StatsRecord Builder Methods

| Method | Type | Description |
|------|:---:|------|
| `with_session_id(id)` | `&str` | Associate with a session |
| `with_tool_use_id(id)` | `&str` | Associate with a tool-use call |
| `with_before_text(t)` | `&str` | Raw text before compression |
| `with_after_text(t)` | `&str` | Raw text after compression |
| `with_project(p)` | `&str` | Set project name |
| `with_namespace(n)` | `&str` | Set namespace |
| `with_experimental_mode(b)` | `bool` | Mark as experimental-mode record |
| `with_source_pid(pid)` | `u32` | Set source process PID |
| `with_text(t)` | `&str` | Context text (e.g., command) |
| `with_output(t)` | `&str` | Context output text |

#### StatsRecord Fields

| Field | Type | Description |
|------|:---:|------|
| `project` | `String` | Project name |
| `namespace` | `String` | Namespace |
| `experimental_mode` | `bool` | Whether experimental mode was active |
| `source_pid` | `Option<u32>` | Source process PID |
| `before_output` | `usize` | Byte length of output before compression |
| `after_output` | `usize` | Byte length of output after compression |

#### Operation Types

```rust
pub enum OperationType {
    CompressSchema,      // Schema compression
    CompressResponse,    // Response compression
    CompressToon,        // TOON encoding
    RewriteCommand,      // RTK command rewriting
}
```

#### Querying

```rust
// Summary with breakdown by operation type
let records = recorder.all_records(Some(100))?;
println!("{}", format_summary(&records, Some("Daily Report")));

// Single record by ID
let record = recorder.record_by_id(42)?.unwrap();

// Time range query
let records = recorder.records_since(Some("2026-05-01"), Some("2026-06-01"))?;

// Agent listing
let agents = recorder.all_agents()?;

// Filtered queries (accept optional agent_id, project, namespace, operation_type)
let records = recorder.records_filtered(None, None, None, None, Some(100))?;
let records = recorder.all_records_filtered(
    Some("my-agent"),       // agent_id
    Some("my-project"),     // project
    Some("default"),        // namespace
    Some("CompressResponse"), // operation_type
    Some(100),              // limit
)?;
let records = recorder.records_since_filtered(
    Some("2026-05-01"),
    Some("2026-06-01"),
    Some("my-agent"),
    Some("my-project"),
    Some("default"),
    Some(100),
)?;

// Project-level queries
let projects = recorder.all_projects()?;
let summary = recorder.project_summary("my-project")?;        // single project
let summaries = recorder.projects_summary()?;                 // all projects
let trends = recorder.project_daily_trends("my-project", 30)?; // last N days

// Recorder management
let total = recorder.count()?;
recorder.clear()?;
let agent_stats = recorder.agent_summary()?;
```

#### Query Formatters

Human-readable display formatters for CLI/TUI output:

```rust
use tokenless_stats::{format_list, format_show, format_diff, format_rewrites, parse_time_range};

// List records as a table
println!("{}", format_list(&records));

// Show a single record in detail
println!("{}", format_show(&record));

// Show before/after diff for a compression record
println!("{}", format_diff(&record));

// Show command rewrite history
println!("{}", format_rewrites(&rewrite_records));

// Parse flexible time range strings
let range = parse_time_range("2026-05-01..2026-06-01")?;
let range = parse_time_range("today")?;
let range = parse_time_range("last_7d")?;
```

#### Configuration

```rust
use tokenless_stats::TokenlessConfig;

let mut config = TokenlessConfig::load();
config.stats_enabled = true;
config.experimental_mode = true;
config.save()?;
```

#### Token Estimation

```rust
// Standard: ~4 chars per token for ASCII
let tokens = estimate_tokens_from_bytes(json_string.len());

// CJK-aware: 1 char = 1 token for Chinese/Japanese/Korean
let tokens = estimate_tokens_cjk_aware(text);

// Byte-count mode
let tokens = estimate_tokens_from_bytes(text.len());

// String-based alias
let tokens = estimate_tokens("hello world");

// Error handling
use tokenless_stats::{StatsError, StatsResult};

fn query() -> StatsResult<Vec<StatsRecord>> {
    let recorder = StatsRecorder::new("stats.db").map_err(StatsError::from)?;
    Ok(recorder.all_records(None)?)
}
```

#### StatsError and StatsResult

| Type | Description |
|------|------|
| `StatsError` | Error enum for stats operations (DB errors, invalid input, not found) |
| `StatsResult<T>` | Type alias for `Result<T, StatsError>` |

#### StatsSummary

Aggregated compression metrics returned by `format_summary` and `agent_summary`:

```rust
use tokenless_stats::StatsSummary;

// Built by format_summary or returned directly
let summary: StatsSummary = recorder.agent_summary()?;
// Fields: total_records, total_chars_before, total_chars_after,
//         total_tokens_before, total_tokens_after,
//         avg_savings_pct, records_by_type, records_by_agent
```

#### Experimental Mode API

Global toggle for enabling experimental compression behaviors:

```rust
use tokenless_stats::experimental;

// Enable experimental features (higher compression, possibly less stable)
experimental::set_experimental_mode(true);

// Check current experimental mode status
if experimental::is_experimental_mode() {
    // Use experimental code paths
    let compressor = SchemaCompressor::new()
        .with_func_desc_max_tokens(20); // more aggressive
}
```

| Function | Returns | Description |
|------|:---:|------|
| `set_experimental_mode(enabled)` | — | Enable or disable experimental mode globally |
| `is_experimental_mode()` | `bool` | Query current experimental mode status |

Experimental mode enables:
- More aggressive token limits in `SchemaCompressor`
- Extra field-dropping rules in `ResponseCompressor`
- Persistent record tagging via `StatsRecord::experimental_mode`

---

### tokenless-semantic

Context-aware field filtering. Level 1 (keyword rules) has zero extra dependencies; Level 2 adds ONNX embedding model support.

#### Level 1: Rule-Based (default)

```rust
use tokenless_semantic::SemanticCompressor;

let compressor = SemanticCompressor::new();

// Compress based on user context
let result = compressor.compress(&json_value, "今天天气怎么样");
// weather rules: keep temp*, wind*; drop station_id, sensor_*

// Detect context category
let category = compressor.detect_category("kubectl get pods");
// → "devops"

// Check if a single field should be kept
let kept = compressor.is_field_kept("temperature", "天气怎么样");
// → true
```

#### Level 2: ONNX Model (opt-in)

```rust
let mut compressor = SemanticCompressor::new();
compressor.load_onnx()?;  // auto-download ~86MB model on first call

let result = compressor.compress(&json_value, "排查线上故障");
// Uses cosine similarity between field names and context embedding.
// Fields with similarity < 0.3 are dropped.
// Falls back to Level 1 automatically on model failure.
```

#### Built-in Rule Domains

| Domain | Triggers | Keep | Drop |
|------|------|------|------|
| `weather` | weather, 天气, temperature | temp*, wind*, humid* | station_id, sensor_* |
| `devops` | k8s, kubectl, deploy, pod | pod*, cpu*, status* | uid, self_link, owner_ref* |
| `database` | sql, query, 查询 | query*, table*, result* | internal_* |
| `git` | git, commit, branch | branch*, status, diff* | author_*, committer_* |
| `default` | anything else | — | debug, trace, logs, stack* |

Custom rules can be added via `~/.tokenless/context_rules.toml`.

---

### tokenless-stats: History Management (planned)

> Status: 📝 Spec complete, implementation pending (includes TUI management panel).
> See [0017-stats-management](../specs/0017-stats-management.md).

```rust
use tokenless_stats::{StatsRecorder, DeleteFilter};

let recorder = StatsRecorder::new("/path/to/stats.db")?;

// Overview
let count = recorder.record_count()?;
let size = recorder.db_size_bytes()?;
let (earliest, latest) = recorder.time_range()?;

// Selective delete
recorder.delete_by_id(42)?;                          // single record
recorder.delete_by_agent("copilot-shell")?;           // by agent
recorder.delete_before("90d")?;                       // older than 90 days
recorder.delete_before("2026-03-01")?;                // before date

// Export before deletion
recorder.export_json("backup.json".as_ref())?;
recorder.export_csv("backup.csv".as_ref())?;

// Maintenance
recorder.vacuum()?;  // reclaim disk space

// Conditional delete with dry-run preview
let filter = DeleteFilter {
    agent_id: Some("old-agent".into()),
    before: Some("2026-01-01".into()),
    ..Default::default()
};
let result = recorder.delete_where(&filter)?;
println!("Deleted {} records, freed ~{} KB",
    result.deleted, result.freed_bytes / 1024);
```

---

## Integration Patterns

### 1. Agent Platform (Hook Protocol)

```rust
// Called by the PostToolUse hook after each tool execution.
fn handle_post_tool_use(payload: &str) -> String {
    let val: serde_json::Value = serde_json::from_str(payload).unwrap();
    let tool_name = val["tool_name"].as_str().unwrap_or("");
    let output = val["output"].as_str().unwrap_or("");
    let command = val.pointer("/tool_input/command")
        .and_then(|v| v.as_str()).unwrap_or("");

    let mut output_val: serde_json::Value = serde_json::from_str(output).unwrap();

    // 1. Semantic filter: drop fields irrelevant to the user's command
    let semantic = SemanticCompressor::new();
    let _ = semantic.load_onnx(); // no-op if model unavailable
    output_val = semantic.compress(&output_val, command);

    // 2. Structural compression
    let compressor = ResponseCompressor::new();
    output_val = compressor.compress(&output_val);

    // 3. Record stats
    let recorder = StatsRecorder::new("stats.db").unwrap();
    let record = StatsRecord::new(
        OperationType::CompressResponse, "agent".into(),
        output.len(), est_tokens(output.len()),
        output_val.to_string().len(), est_tokens(output_val.to_string().len()),
    );
    recorder.record(&record).ok();

    serde_json::to_string(&output_val).unwrap()
}
```

### 2. API Gateway / Proxy

```rust
// Intercept API responses between agent and LLM provider.
fn proxy_response(api_response: &str, user_query: &str) -> String {
    let value = serde_json::from_str(api_response).unwrap();

    let semantic = SemanticCompressor::new();
    let value = semantic.compress(&value, user_query);

    let compressor = ResponseCompressor::new();
    let compressed = compressor.compress(&value);

    serde_json::to_string(&compressed).unwrap()
}
```

### 3. MCP Server

```rust
// Register as an MCP tool.
fn tool_call(name: &str, args: serde_json::Value) -> String {
    match name {
        "compress_schema" => {
            let compressor = SchemaCompressor::new();
            let result = compressor.compress(&args);
            serde_json::to_string(&result).unwrap()
        }
        "compress_response" => {
            let compressor = ResponseCompressor::new();
            let result = compressor.compress(&args);
            serde_json::to_string(&result).unwrap()
        }
        _ => "{}".into(),
    }
}
```

---

## Installation

The SDK crates are published on [crates.io](https://crates.io):

```bash
cargo add tokenless-schema tokenless-stats tokenless-semantic
```

For ONNX Level 2 support:

```bash
cargo add tokenless-semantic --features onnx
```

ONNX model files are auto-downloaded on first use, or installed locally:

```bash
make models-install  # copies model files to ~/.tokenless/models/
```

---

## License

Apache 2.0. See [LICENSE.md](../LICENSE.md) for details.

---

## Related Documentation

- [User Guide](./user-guide.md) — Full CLI usage guide
- [Architecture Design](../specs/0001-architecture.md) — Crate dependency graph
- [Security Model](../specs/0005-security-model-design.md) — Threat model & input validation
- [Error Handling](../specs/0006-error-handling-strategy.md) — Error types & propagation
