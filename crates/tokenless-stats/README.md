# tokenless-stats

SQLite-based compression metrics tracking for tokenless.

Part of the [tokenless](https://github.com/TokenFleet-AI/tokenless) toolkit.

## Quick Start

```toml
[dependencies]
tokenless-stats = "0.4"
```

```rust
use tokenless_stats::{StatsRecorder, StatsRecord, OperationType};

let recorder = StatsRecorder::new("~/.tokenless/stats.db")?;

// Builder pattern (preferred)
let record = StatsRecord::new(
    OperationType::CompressResponse,
    "my-agent",
    1000,  // before_chars
    250,   // before_tokens
    500,   // after_chars
    125,   // after_tokens
)
.with_project("my-project")
.with_namespace("production")
.with_session_id("sess-123")
.with_text("original text".into(), "compressed text".into());

recorder.record(&record)?;
```

## Core Types

### `StatsRecorder`

Thread-safe SQLite-backed metrics store.

| Method | Description |
|--------|-------------|
| `new(path)` | Open or create database |
| `record(&self, record)` | Insert a record, returns its `id` |
| `count()` | Total record count |
| `clear()` | Delete all records |
| `all_records(limit)` | All records, newest first |
| `record_by_id(id)` | Fetch a single record |
| `records_since(since, limit)` | Records after a timestamp |
| `all_agents()` | List distinct agent IDs |
| `agent_summary(agent_id)` | Per-agent stats |

### Multi-Project Queries

| Method | Description |
|--------|-------------|
| `all_projects()` | List distinct project names |
| `records_filtered(agent, search, project, namespace, limit)` | Filtered query |
| `all_records_filtered(project, namespace)` | All records with optional filters |
| `records_since_filtered(since, project, namespace, limit)` | Time-based filtered query |
| `project_summary(project)` | Single project stats |
| `projects_summary()` | All project summaries |
| `project_daily_trends(project, days)` | Daily savings per project |

### `StatsRecord`

| Field | Type | Builder Method |
|-------|------|---------------|
| `project` | `Option<String>` | `with_project(name)` |
| `namespace` | `Option<String>` | `with_namespace(ns)` |
| `experimental_mode` | `bool` | `with_experimental_mode(bool)` |
| `session_id` | `Option<String>` | `with_session_id(id)` |
| `tool_use_id` | `Option<String>` | `with_tool_use_id(id)` |
| `before_text` / `after_text` | `Option<String>` | `with_text(before, after)` |
| `before_output` / `after_output` | `Option<String>` | `with_output(before, after)` |

### `TokenlessConfig`

Persisted configuration (`~/.tokenless/config.json`).

```rust
use tokenless_stats::TokenlessConfig;

let mut config = TokenlessConfig::load();
config.stats_enabled = true;
config.experimental_mode = true;
config.save()?;

// Check state
assert!(config.is_stats_enabled());
assert!(config.is_experimental_enabled());
```

### Query Formatters

| Function | Description |
|----------|-------------|
| `format_summary(records)` | Human-readable summary string |
| `format_list(records)` | Tabular list output |
| `format_show(record)` | Single record detail |
| `format_diff(record)` | Before/after diff |
| `format_rewrites(records)` | Rewrite history output |
| `parse_time_range(input)` | Parse "1h"/"30m"/"7d" into SQL |

### Token Estimation

| Function | Description |
|----------|-------------|
| `estimate_tokens(text)` | Character-based (CJK-aware) |
| `estimate_tokens_from_bytes(len)` | Byte-based estimation |
| `estimate_tokens_cjk_aware(text)` | CJK-aware byte-based |

## Minimum Rust Version

Rust 2024 edition, MSRV 1.89.

License: Apache-2.0
