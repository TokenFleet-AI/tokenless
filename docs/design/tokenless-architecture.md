# Tokenless Architecture Design

## Overview

Tokenless is an LLM token optimization toolkit that reduces token consumption through schema compression, response compression, TOON encoding, command rewriting, and tool environment readiness checks.

Reference: https://github.com/alibaba/anolisa/tree/main/src/tokenless

## Target Architecture

```
tokenless/
├── Cargo.toml                          # Workspace root
├── Makefile                            # Build automation
├── crates/
│   ├── tokenless-schema/               # Core library: SchemaCompressor + ResponseCompressor
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── schema_compressor.rs    # OpenAI Function Calling schema compression
│   │       └── response_compressor.rs  # JSON response compression
│   ├── tokenless-stats/                # SQLite-based metrics tracking
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── record.rs               # StatsRecord, OperationType
│   │       ├── recorder.rs             # SQLite storage
│   │       ├── config.rs               # TokenlessConfig
│   │       ├── query.rs                # Formatting helpers
│   │       └── tokenizer.rs            # Token estimation
│   └── tokenless-cli/                  # CLI binary: `tokenless` command
│       └── src/
│           ├── main.rs                 # CLI entry + subcommands
│           └── env_check.rs            # Tool environment readiness
├── adapters/                           # Adapter bundle (future)
│   └── tokenless/
│       ├── common/                     # Shared hooks, spec, commands
│       ├── openclaw/                   # OpenClaw plugin
│       └── hermes/                     # Hermes Agent plugin
```

## Workspace Cargo.toml Changes

### New workspace dependencies

```toml
regex = "1.10"
clap = { version = "4", features = ["derive"] }
chrono = "0.4"
toon-format = { version = "0.4", default-features = false }
rusqlite = { version = "0.31", features = ["bundled"] }
dirs = "5.0"
libc = "0.2"

# command rewriting engine (path: /Users/byx/Documents/workspace/github.com/TokenFleet-AI/rtk/crates/rtk-registry)
rtk-registry = { path = "../rtk/crates/rtk-registry" }
```

### New workspace members

```toml
members = ["crates/*", "apps/*"]
# (apps/* already present, crates/core becomes tokenless-schema, tokenless-stats, tokenless-cli)
```

## Crate Specifications

### 1. tokenless-schema

**Purpose**: Compress OpenAI Function Calling tool schemas and JSON API responses.

**SchemaCompressor** — builder-pattern struct:
- `with_func_desc_max_len(usize)` — default 256
- `with_param_desc_max_len(usize)` — default 160
- `with_drop_examples(bool)` — default true
- `with_drop_titles(bool)` — default true
- `with_drop_markdown(bool)` — default true
- `compress(&Value) -> Value` — compresses tool schema
- `compress_json_schema(&mut Value, depth)` — recursive JSON Schema compression
- `truncate_description(&str, usize) -> String` — sentence-boundary-aware truncation

**ResponseCompressor** — builder-pattern struct:
- `with_truncate_strings_at(usize)` — default 512
- `with_truncate_arrays_at(usize)` — default 16
- `with_drop_nulls(bool)` — default true
- `with_drop_empty_fields(bool)` — default true
- `with_max_depth(usize)` — default 8
- `with_add_truncation_marker(bool)` — default true
- `add_drop_field(&str)` — custom field exclusion
- `compress(&Value) -> Value` — compresses JSON response

**Dependencies**: `serde_json`, `regex`

### 2. tokenless-stats

**Purpose**: SQLite-based metrics tracking for compression effectiveness.

**Key types**:
- `OperationType` enum: `CompressSchema`, `CompressResponse`, `RewriteCommand`, `CompressToon`
- `StatsRecord` — full record with before/after chars, tokens, text content, output comparison
- `StatsRecorder` — thread-safe SQLite connection with schema migration
- `StatsSummary` — aggregate metrics
- `TokenlessConfig` — persistent config (enable/disable stats)
- `estimate_tokens_from_bytes(usize) -> usize` — quick token estimation

**Database schema**:
```sql
CREATE TABLE stats (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp TEXT NOT NULL,
    operation TEXT NOT NULL,
    agent_id TEXT NOT NULL,
    source_pid INTEGER,
    session_id TEXT,
    tool_use_id TEXT,
    before_chars INTEGER NOT NULL,
    before_tokens INTEGER NOT NULL,
    after_chars INTEGER NOT NULL,
    after_tokens INTEGER NOT NULL,
    before_text TEXT,
    after_text TEXT,
    before_output TEXT,
    after_output TEXT
);
```

**Dependencies**: `serde`, `serde_json`, `chrono`, `rusqlite`, `thiserror`, `dirs`

### 3. tokenless-cli

**Purpose**: CLI binary with all subcommands.

**Subcommands**:
- `compress-schema [-f FILE] [--batch] [--agent-id] [--session-id] [--tool-use-id]`
- `compress-response [-f FILE] [--agent-id] [--session-id] [--tool-use-id]`
- `compress-toon [-f FILE] [--agent-id] [--session-id] [--tool-use-id]`
- `decompress-toon [-f FILE]`
- `env-check [--tool NAME|--all] [--fix] [--checklist] [--json]`
- `stats summary [--limit N]`
- `stats list [-l N]`
- `stats show <ID>`
- `stats clear [--yes]`
- `stats status`
- `stats enable`
- `stats disable`

**Key design decisions**:
- If compression yields zero token savings, output original unchanged (not compressed)
- Stats recording is fail-silent — database errors never block compression output
- Exit codes: 0=success, 1=usage/config error, 2=parse/serialization error

**env_check module**:
- Loads `tool-ready-spec.json` (string or object dep format)
- Checks binary availability, version constraints, config files, permissions, network
- Auto-fix via `tokenless-env-fix.sh` script (config-driven install engine)
- Resolves aliases and case-insensitive tool names
- Detects native package manager (dnf/yum/apt/apk)
- Version comparison: semver-like with `v` prefix and build suffix handling

### rtk-registry Integration

**Source**: `/Users/byx/Documents/workspace/github.com/TokenFleet-AI/rtk/crates/rtk-registry`
**Dependencies**: `regex 1`, `lazy_static 1.4`, `serde 1`, `which 8`

Command rewriting is delegated to `rtk-registry` as a library dependency (no shelling out to `rtk` binary). The crate is added to the workspace via path dependency:

```toml
rtk-registry = { path = "../rtk/crates/rtk-registry" }
```

**Public API used by tokenless-cli**:

| Function | Purpose |
|---|---|
| `rewrite_command(cmd, excluded, transparent_prefixes) -> Option<String>` | Rewrite a shell command to its RTK equivalent |
| `classify_command(cmd) -> Classification` | Classify command as Supported/Unsupported/Ignored |
| `is_rtk_installed() -> RtkInstallStatus` | Check if RTK binary is available |

**Error handling**: `rtk-registry` returns `None` for unsupported/ignored commands — tokenless should pass through the original command unchanged when rewriting yields `None`.

**Dependencies**: `tokenless-schema`, `tokenless-stats`, `rtk-registry`, `dirs`, `clap`, `serde_json`, `toon-format`, `chrono`, `rusqlite`, `libc`

## Implementation Order

1. **tokenless-schema** — schema_compressor.rs + response_compressor.rs (ported from reference)
2. **tokenless-stats** — record, recorder, config, query, tokenizer modules
3. **tokenless-cli** — main.rs + env_check.rs
4. **Cargo.toml** — update workspace deps + new crate manifests
5. **Makefile** — build/install targets (future: adapters)

## Profile Release (copy from reference)

```toml
[profile.release]
opt-level = 3
lto = true
codegen-units = 1
panic = "abort"
strip = true
```

## CLAUDE.md Compliance Notes

- All code passes `clippy::pedantic` with `-D warnings`
- No `unwrap()`/`expect()` in production code
- `tracing` instead of `println!`/`dbg!` in production
- All public items documented
- Error handling via `thiserror` (library) + `anyhow` (app-level context)
- Rust 2024 edition, `#![forbid(unsafe_code)]` (except libc::getuid in env_check which needs `unsafe`)
