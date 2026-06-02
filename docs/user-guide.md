# Tokenless User Guide

> 📖 Quick links: [README](../README.md) | 中文版: [用户指南 (中文)](./user-guide-zh.md) | Just want to get started? Jump to [Quick Start](#quick-start-5-minutes)

## Table of Contents

- [Quick Start (5 minutes)](#quick-start-5-minutes)
- [1. Overview](#1-overview)
- [2. Installation](#2-installation)
- [3. CLI Usage](#3-cli-usage)
  - [3.11 TUI Dashboard](#311-tui-dashboard)
- [4. Agent Integration](#4-agent-integration)
- [5. OpenClaw Plugin](#5-openclaw-plugin)
- [6. Hermes Agent Plugin](#6-hermes-agent-plugin)
- [7. Workflow Comparison](#7-workflow-comparison)
- [8. Crate API (Rust Library)](#8-crate-api-rust-library)
- [9. Token Proxy Integration](#9-token-proxy-integration)
- [10. Test Data](#10-test-data)
- [11. Build & Development](#11-build--development)

## Choose Your Path

| You are... | Start here | Est. time |
|------------|-----------|-----------|
| 🚀 Want auto token savings in your agent | [Quick Start](#quick-start-5-minutes) → `tokenless init` | 3 min |
| 🔍 Want to verify compression first | [CLI Usage](#3-cli-usage) → run with fixtures | 10 min |
| 🎯 Want a one-command demo | [`tokenless demo`](#312-demo) → all strategies at once | 10 sec |
| 📊 Want to visualize savings | [`tokenless tui`](#311-tui-dashboard) → interactive dashboard | 1 min |
| 🔧 Integrating with OpenClaw/Hermes | [Workflow Comparison](#7-workflow-comparison) → plugin chapters | 15 min |
| 📦 Embedding compression in your system | [Crate API](#8-crate-api-rust-library) | 5 min |
| 🛠 Want to contribute | [Build & Development](#11-build--development) | — |

---

## Quick Start (5 minutes)

```bash
# 1. Install
git clone https://github.com/TokenFleet-AI/tokenless && cd tokenless && make setup

# 2. Verify environment (optional but recommended)
tokenless env-check --checklist

# 3. Run a real compression, see the result instantly
echo '{"debug":"removed","data":{"items":[1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20]}}' | tokenless compress-response

# 4. One-click agent integration (Claude Code / Cursor / Windsurf ...)
tokenless init

# 5. Done! All shell commands are now auto-rewritten and responses compressed.
#    Check back later to see your savings:
tokenless stats summary
```

> `tokenless init` installs hooks into `.claude/settings.json` (or the equivalent config for your agent). Zero manual steps afterwards. Supports **12 agents**. See [Agent Integration](#4-agent-integration) for details.

---

## 1. Overview

Tokenless is an LLM token optimization toolkit that reduces token consumption through complementary strategies:

| Strategy | Savings | Description |
|----------|---------|-------------|
| Schema Compression | ~57% | Compresses OpenAI Function Calling tool definitions |
| Response Compression | ~26–78% | Strips debug/null/empty fields, truncates strings/arrays |
| TOON Encoding | 15–40% | Converts JSON to TOON format |
| Command Rewriting | 60–90% | Filters CLI output via RTK |

### Architecture

```
tokenless/
├── crates/
│   ├── tokenless-schema/   SchemaCompressor + ResponseCompressor
│   ├── tokenless-stats/    SQLite metrics tracking
│   └── tokenless-cli/      CLI binary
├── adapters/               FHS adapter bundle
│   └── tokenless/
│       ├── common/                Tool deps + env-fix scripts
│       ├── openclaw/              OpenClaw v5 plugin
│       └── hermes/                Hermes Agent plugin
├── docs/
│   ├── design/             Architecture design docs
│   └── user-guide.md       This file
└── tests/fixtures/         Test data
```

### Dependencies

```
rtk-registry (external crate)
    ↕ command text rewriting
tokenless-schema ← tokenless-cli → tokenless-stats
    ↕ compression        ↕ CLI entry    ↕ SQLite store
                  init module (12 agents)
```

---

## 2. Installation

### Cargo Install (Recommended)

```bash
cargo install tokenless
```

Requires Rust >= 1.85. Installs the `tokenless` binary to `~/.cargo/bin/`.

### Pre-built Binaries

Download the latest binary for your platform from [GitHub Releases](https://github.com/TokenFleet-AI/tokenless/releases):

| Platform | Archive |
|----------|---------|
| macOS (Apple Silicon) | `tokenless-aarch64-apple-darwin.tar.gz` |
| macOS (Intel) | `tokenless-x86_64-apple-darwin.tar.gz` |
| Linux (x86_64) | `tokenless-x86_64-unknown-linux-musl.tar.gz` |
| Windows (x86_64) | `tokenless-x86_64-pc-windows-msvc.zip` |

Extract and place the binary in your `PATH`.

### Homebrew

```bash
brew install tokenfleet/tap/tokenless
```

### Build from Source

```bash
git clone https://github.com/TokenFleet-AI/tokenless
cd tokenless

# Build + install
make setup

# Or step by step
make install           # Install binary
make adapter-install   # Install adapter files
```

Installs binary to `~/.local/bin/tokenless`, adapter files to `~/.local/share/anolisa/adapters/tokenless/`.

**Development mode** (installs to `~/.cargo/bin/`):

```bash
./scripts/dev-install.sh
```

This is equivalent to `make install` but places the binary in `~/.cargo/bin/` for local development convenience.

### Prerequisites

- Rust >= 1.85 (build from source / cargo install)
- Command rewriting requires [RTK](https://github.com/TokenFleet-AI/rtk): `cargo install rtk` (optional — core compression works without it)

---

## 3. CLI Usage

### 3.1 Schema Compression

Compress OpenAI Function Calling tool definitions:

```bash
# Single file
tokenless compress-schema -f tool.json

# Standard input
cat tool.json | tokenless compress-schema

# Batch mode (JSON array)
tokenless compress-schema -f tools.json --batch
```

Compression effects: removes `title`, `examples`, truncates descriptions, strips markdown formatting.

```json
// Before
{"function": {"name": "get_weather", "description": "Very long description...", "parameters": {"properties": {"loc": {"description": "...", "examples": ["Beijing"]}}}}}

// After: description truncated to 256/160 chars, examples removed
```

Options: `-f`/`--file`, `--batch`, `--report`, `--project`, `--agent-id`, `--session-id`, `--tool-use-id`

### 3.2 Response Compression

Compress API response JSON:

```bash
tokenless compress-response -f response.json
curl -s https://api.example.com/data | tokenless compress-response
```

Compression rules:
- Drops `debug`, `trace`, `stacktrace`, `logs` fields
- Removes `null` values
- Removes empty strings `""`, empty arrays `[]`, empty objects `{}`
- Truncates strings to 512 characters
- Truncates arrays to 16 items
- Truncates nesting depth beyond 8 levels

Options: `-f`/`--file`, `--report`, `--context`, `--semantic`, `--project`, `--agent-id`, `--session-id`, `--tool-use-id`

### 3.3 TOON Encoding

```bash
echo '{"name":"Alice","age":30}' | tokenless compress-toon
# → name: Alice
# → age: 30

echo 'name: Alice\nage: 30' | tokenless decompress-toon
# → {"name":"Alice","age":30}
```

### 3.4 Auto Compression

Auto-selects the best compression format based on JSON structure:

```bash
tokenless compress-auto -f input.json
cat input.json | tokenless compress-auto
```

Options: `-f`/`--file`, `--report`, `--project`, `--agent-id`, `--session-id`, `--tool-use-id`

### 3.5 Command Rewriting

```bash
tokenless rewrite "git status"
# → rtk git status

tokenless rewrite "cargo test && git push"
# → rtk cargo test && rtk git push
```

Falls back to the original command with an install prompt when RTK is not installed.

Options: `--exclude`, `--transparent-prefix`, `--project`

### 3.6 Hook Diff

PostToolUse hook variant that returns a unified diff instead of full text (experimental):

```bash
tokenless hook diff -f response.json
curl -s https://api.example.com/data | tokenless hook diff
```

### 3.7 Environment Check

```bash
# Check a specific tool
tokenless env-check --tool Shell

# Check all tools
tokenless env-check --all

# Output checklist
tokenless env-check --checklist

# Auto-fix missing dependencies
tokenless env-check --tool Shell --fix
```

Dependency declarations live in `adapters/tokenless/common/tool-ready-spec.json`, covering 6 tool categories: Shell, WebFetch, Read, Write, Git, Python.

Options: `--json` (output results as JSON)

### 3.8 Statistics

```bash
tokenless stats summary [--project <NAME>] [--namespace <NS>]  # Aggregated metrics
tokenless stats list [--project <NAME>] [--namespace <NS>]     # Recent records
tokenless stats show <ID>                                       # Record details
tokenless stats enable/disable                                  # Toggle recording
tokenless stats clear                                           # Clear all records
tokenless stats rewrites                                        # Show rewrite history
tokenless stats status                                          # Show recording status
tokenless stats diff                                            # Show diff-based savings
tokenless stats experimental-on                                 # Enable experimental features
tokenless stats experimental-off                                # Disable experimental features
```

### 3.9 MCP Server

Start a local MCP server for AI agent integration:

```bash
tokenless mcp start --port <PORT>
```

> **Prerequisite**: Run `tokenless stats experimental-on` first to enable experimental features.

Options: `--port` (TCP port for the MCP server)

### 3.10 Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `TOKENLESS_CACHE_SIZE` | 512 | Predictive cache capacity (set to 0 to disable) |
| `TOKENLESS_DIFF_THRESHOLD` | 0.7 | Diff threshold ratio — fall back to full output when diff exceeds this proportion |
| `TOKENLESS_STATS_DB` | `~/.tokenless/stats.db` | Statistics database path |
| `TOKENLESS_STATS_ENABLED` | — | Disable stats via env var (set to `0` or `false`) |
| `TOKENLESS_LANG` | `zh` | TUI language (`zh` or `en`). Also resolved from `LANG` env var |

### 3.11 TUI Dashboard

Launch an interactive terminal dashboard for real-time compression statistics:

```bash
tokenless tui                     # Default: zh, 1s refresh
tokenless tui --lang en           # English UI
tokenless tui --refresh 3         # 3-second refresh interval
```

> **Prerequisite**: Run `tokenless stats experimental-on` first to enable experimental features.

The dashboard has **4 tabs**, switchable with `h` / `Tab` (next) and `Shift+Tab` (prev):

| Tab | Description |
|-----|-------------|
| **Dashboard** | Total tokens/chars saved, per-operation breakdown, recent activity |
| **Records** | Scrollable record list with columns (ID, timestamp, operation, agent, before, after, savings) |
| **Agents** | Per-agent summaries: record count, chars/tokens saved; drill down with `Enter` |
| **Trends** | Daily chars and tokens saved over time (bar charts) |

**Global keyboard shortcuts:**

| Key | Action |
|-----|--------|
| `h` / `Tab` (next), `Shift+Tab` (prev) | Switch tabs |
| `↑↓` / `j``k` | Scroll / navigate |
| `Enter` | View detail (record or agent drill-down) |
| `d` | Back from detail view |
| `/` | Search / filter records |
| `t` | Cycle time range: Today → This Week → All Time |
| `p` | Toggle project filter picker |
| `e` | Export filtered records to JSON file |
| `c` | Toggle config panel (stats, cache, threshold, experimental mode) |
| `?` | Toggle help overlay |
| `q` / `Esc` | Quit |

> In the config panel, pressing `e` toggles experimental mode.

> The TUI reads from the same SQLite database as `tokenless stats`. Enable stats recording with `tokenless stats enable` if you don't see data.

### 3.12 Demo

Run a one-command demo showing all compression strategies with built-in test data:

```bash
tokenless demo
```

Outputs a formatted showcase of schema compression, response compression, TOON encoding, and command rewriting — each with before/after token counts and savings percentages. No files, no setup required.

---

### 3.13 Multi-Project Support

All compression and rewrite commands accept a `--project <name>` flag that tags records for later filtering. This lets you track token savings separately for different projects, repositories, or teams — all in a single SQLite database.

**Recording data per project:**

```bash
# Tag compression operations with a project name
tokenless compress-schema -f tool.json --project my-api
tokenless compress-response -f resp.json --project frontend
tokenless rewrite "git push" --project devops
```

**Querying per project:**

```bash
# Filter stats by project
tokenless stats summary --project my-api
tokenless stats list --project my-api --limit 10
```

**TUI project picker:** Press `p` in the TUI dashboard to open the project picker overlay. Use `↑` `↓` to select a project, `Enter` to apply the filter. Selecting "All Projects" clears the filter. The picker lists all projects found in the database — no manual registration needed.

**How it works:**

- `--project` is always optional. Omitting it records metrics without a project association.
- Projects are discovered automatically from recorded data — you don't create them upfront.
- The TUI status bar shows the current project filter: `[p:project]` when filtered, `[p:所有项目]` when unfiltered.
- The `--namespace` flag provides a secondary grouping dimension (e.g., "production" vs "staging").

### 3.14 Experimental Features

Some features are gated behind an **experimental mode** toggle to keep the default installation stable and lightweight. When disabled, tokenless uses core compression only (Level 1 semantic rules, no format router, no ONNX models).

**Gated features:**

| Feature | Requires experimental mode |
|---------|:---:|
| TUI dashboard | ✅ |
| MCP server | ✅ |
| Semantic compression Level 2 (ONNX) | ✅ |
| Format router (auto compression) | ✅ |
| Enhanced TOON encoding | ✅ |
| `hook diff` (unified diff responses) | ✅ |
| Core compression (schema/response) | — always available |
| Command rewriting | — always available |
| Statistics recording | — always available |

**Enabling / disabling:**

```bash
# Enable all experimental features
tokenless stats experimental-on

# Disable (back to core-only)
tokenless stats experimental-off

# Check current state
tokenless stats status
# → Stats recording: enabled | Experimental mode: on
```

**TUI toggle:** Open the config panel with `c`, then press `e` to toggle experimental mode. Changes take effect immediately and persist across sessions.

**Persistence:** The experimental mode setting is stored in `~/.tokenless/config.json` and survives reboots and upgrades. It is separate from the stats recording toggle — you can record stats without enabling experimental features.

**Note:** Disabling experimental mode while the TUI is open will prevent re-launching it after exit. Re-enable with `tokenless stats experimental-on` if needed.

---

## 4. Agent Integration

### 4.1 Quick Install

```bash
# Install Claude Code hooks (project-local)
tokenless init

# Install globally to ~/.claude/settings.json
tokenless init --global

# Other agents
tokenless init --global --agent cursor
tokenless init --agent windsurf

# Debug mode (verbose output)
tokenless init --debug
```

Supported 12 agents:

| Agent | Config Path | Install Command |
|-------|------------|-----------------|
| Claude Code | `.claude/settings.json` | `tokenless init` |
| Cursor | `.cursor/hooks/` | `tokenless init --global --agent cursor` |
| Windsurf | `.windsurf/settings.json` | `tokenless init --agent windsurf` |
| Cline | VS Code globalStorage | `tokenless init --agent cline` |
| Kilo Code | `.kilocode/settings.json` | `tokenless init --agent kilocode` |
| Antigravity | `.antigravity/settings.json` | `tokenless init --agent antigravity` |
| Augment | `.augment/settings.json` | `tokenless init --agent augment` |
| Hermes CLI | `.hermes/plugins/tokenless-rewrite/` | `tokenless init --agent hermes` |
| Pi | `.pi/agent/extensions/` | `tokenless init --agent pi` |
| Gemini CLI | `.gemini/` | `tokenless init --agent gemini` |
| OpenCode | `~/.opencode/plugins/tokenless/` | `tokenless init --global --agent opencode` |
| GitHub Copilot | `.github/hooks/rtk-rewrite.json` | `tokenless init --agent copilot` |

### 4.2 Manual Configuration

Full Claude Code hooks config (auto-generated by `tokenless init`):

```json
{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Bash",
        "hooks": [
          {
            "type": "command",
            "command": "tokenless hook rewrite --target claude --project <project-name>"
          }
        ]
      }
    ],
    "PostToolUse": [
      {
        "matcher": "^(?!Bash$).*",
        "hooks": [
          {
            "type": "command",
            "command": "tokenless hook compress --semantic --target claude --project <project-name>"
          }
        ]
      }
    ]
  }
}
```

> **Note**: Hooks use the `tokenless hook` subcommand, not `tokenless rewrite` or `tokenless compress-response` directly. The `hook` subcommand communicates with Claude Code via stdin/stdout JSON protocol, automatically reading the command from hook input and outputting the rewritten result. Use `tokenless hook rewrite --target claude --project <project-name>` and `tokenless hook compress --semantic --target claude --project <project-name>` when configuring manually.

---

## 5. OpenClaw Plugin

> 💡 **Not sure which integration to choose?** Read [7. Workflow Comparison](#7-workflow-comparison) first to understand the differences between Claude Code hooks, OpenClaw, and Hermes.

### 5.1 Overview

The OpenClaw plugin is a TypeScript plugin that integrates tokenless at two event points: **before_tool_call** and **tool_result_persist**.

```
OpenClaw Session
    ↓
session_start  → Records sessionId mapping
    ↓
before_tool_call (priority 5)  → Tool Ready env check
before_tool_call (priority 10) → RTK command rewrite (exec tool only)
    ↓
Tool Execution
    ↓
tool_result_persist → Response Compression → TOON Encoding
```

### 5.2 File Structure

```
adapters/tokenless/openclaw/
├── index.ts               # Plugin main logic (TypeScript)
├── openclaw.plugin.json   # Plugin manifest (config schema)
├── package.json           # NPM package
└── scripts/
    ├── install.sh         # Install to OpenClaw
    └── uninstall.sh       # Remove from OpenClaw
```

### 5.3 Configuration Options

Configurable fields in `openclaw.plugin.json`:

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `rtk_enabled` | boolean | true | Enable RTK command rewriting |
| `response_compression_enabled` | boolean | true | Enable response compression |
| `tool_ready_enabled` | boolean | true | Enable environment readiness checks |
| `toon_compression_enabled` | boolean | false | Enable TOON encoding (opt-in) |
| `skip_tools` | string[] | ["Read","read_file","Glob",...] | Tools excluded from compression |
| `verbose` | boolean | false | Verbose logging |

### 5.4 Installation

```bash
# Option 1: openclaw CLI install
make openclaw-install

# Option 2: manual registration
openclaw plugins install adapters/tokenless/openclaw --force
openclaw gateway restart
```

### 5.5 Event Handling

**before_tool_call (priority 5) — Tool Ready:**
- Calls `tokenless env-check --tool {name} --json`
- UNKNOWN/READY → skip
- NOT_READY → auto-fix → on failure, inject `contextPrefix` to skip retry

**before_tool_call (priority 10) — RTK Rewrite:**
- Only processes `exec` tool
- Calls `rtk rewrite {command}` for text replacement
- Success → replaces command param with rewritten version (`updatedInput`)
- Failure/unsupported → pass through

**tool_result_persist — Response Compression:**
- Skips responses under 200 characters
- Skips tools in the `skip_tools` list
- Skips skill files (YAML format)
- Step 1: Response compression (drop debug/null/empty, truncate)
- Step 2: TOON encoding (if enabled)
- Returns compressed `{ message }`

### 5.6 Behavior Notes

- All features **gracefully degrade**: binaries (tokenless/rtk) not installed → corresponding feature auto-skipped
- Binary detection results are cached for the session duration
- TOON encoding preserves `toolResult` message structure to avoid session repair injection errors
- sessionId mapped via `session_start` event: `sessionKey → sessionId`

---

## 6. Hermes Agent Plugin

### 6.1 Overview

The Hermes plugin is a Python plugin that registers three hooks:

```
Hermes Session
    ↓
on_session_start        → Records sessionId to env var
    ↓
pre_tool_call           → Tool Ready check → RTK command rewrite
    ↓
Tool Execution
    ↓
transform_tool_result   → Response Compression → TOON Encoding
```

### 6.2 File Structure

```
adapters/tokenless/hermes/
├── __init__.py           # Plugin main logic (Python)
├── plugin.yaml           # Plugin manifest
└── scripts/
    ├── install.sh        # Install to Hermes
    └── uninstall.sh      # Remove from Hermes
```

### 6.3 Installation

```bash
# Automatic install
make hermes-install

# Verify
hermes plugins list
# → tokenless    enabled

# If not enabled
hermes plugins enable tokenless
```

### 6.4 Hook Details

**on_session_start:**
- Records sessionId to env var `TOKENLESS_SESSION_ID`
- For downstream stats recording

**pre_tool_call:**
- **Step 1 — Tool Ready:** Calls `tokenless env-check --tool {name} --json`
  - UNKNOWN/READY → skip
  - NOT_READY → auto-fix → returns `{action: "block"}` to skip retry on failure
- **Step 2 — RTK Rewrite (terminal only):**
  - Calls `rtk rewrite {command}`
  - Version check >= 0.35.0
  - Success → returns `{action: "block", message: "Suggest using rewritten command"}` for agent re-execution

**transform_tool_result:**
- Skips content-retrieval tools
- Skips skill files, non-JSON, responses under 200 characters
- Step 1: Response compression (`tokenless compress-response`)
- Step 2: TOON encoding (`tokenless compress-toon`)
- Returns `None` when no compression effect

### 6.5 How Command Rewriting Works

Hermes' `pre_tool_call` cannot modify command arguments — it can only block + suggest. RTK rewriting therefore requires one extra round-trip:

```
Agent executes: kubectl get pods
    ↓
pre_tool_call hook: rtk rewrite "kubectl get pods" → "rtk kubectl get pods"
    ↓
Returns {action: "block", message: "Suggest using rtk kubectl get pods"}
    ↓
Agent sees suggestion, re-executes: rtk kubectl get pods
    ↓
RTK filters output, saves 85% tokens
```

This limitation is inherent to the Hermes hook system and does not affect the final token savings.

### 6.6 Graceful Degradation

- `tokenless` not installed → skip all compression/toon/tool-ready
- `rtk` not installed → skip rewrite
- Version too low → skip rewrite with warning

---

## 7. Workflow Comparison

> **Recommended reading order**: review this comparison of the three integration approaches before diving into individual plugin chapters.

### Claude Code Hooks (Recommended)

```
Agent executes: git status
    ↓
PreToolUse hook → tokenless rewrite → "rtk git status"
    ↓ Zero extra round-trip — command params modified directly
Agent executes: rtk git status
    ↓
PostToolUse hook → tokenless compress-response
```

### OpenClaw Plugin

```
Agent executes: exec("git status")
    ↓
before_tool_call → rtk rewrite → replace command param
    ↓ Zero extra round-trip — params modified directly
Agent executes: rtk git status
    ↓
tool_result_persist → compress → TOON encoding
```

### Hermes Plugin

```
Agent executes: kubectl get pods
    ↓
pre_tool_call → rtk rewrite → block + suggest
    ↓ One extra round-trip (Hermes hook limitation)
Agent re-executes: rtk kubectl get pods
    ↓
transform_tool_result → compress → TOON
```

---

## 8. Crate API (Rust Library)

### tokenless-schema

```rust
use tokenless_schema::{SchemaCompressor, ResponseCompressor};

// Compress schema
let compressed = SchemaCompressor::new()
    .with_func_desc_max_len(200)
    .compress(&tool_json);

// Compress response
let compressed = ResponseCompressor::new()
    .with_truncate_arrays_at(10)
    .with_drop_nulls(true)
    .compress(&response_json);
```

### tokenless-stats

```rust
use tokenless_stats::{StatsRecorder, StatsRecord, OperationType};

let recorder = StatsRecorder::new(":memory:")?;
let record = StatsRecord::new(
    OperationType::CompressResponse,
    "my-agent".into(),
    1000,  // before_chars
    250,   // before_tokens
    500,   // after_chars
    125,   // after_tokens
);
recorder.record(&record)?;
```

---

## 9. Token Proxy Integration

When using Tokenless as an LLM API proxy:

```rust
use tokenless_schema::{SchemaCompressor, ResponseCompressor};
use tokenless_stats::{StatsRecorder, StatsRecord, OperationType};

// Request phase: compress tool schemas
let compressed_schemas = SchemaCompressor::new().compress(&tools);

// Response phase: compress response
let compressed = ResponseCompressor::new().compress(&response);

// Record
recorder.record(&StatsRecord::new(
    OperationType::CompressResponse, "proxy", before, bt, after, at
));
```

No RTK or tokenless CLI required — just add the Rust crate dependency.

---

## 10. Test Data

```bash
cd tokenless

# Schema compression
tokenless compress-schema -f tests/fixtures/tool-schema.json

# Response compression
tokenless compress-response -f tests/fixtures/response.json
tokenless compress-response -f tests/fixtures/response-large.json

# TOON encoding
tokenless compress-toon -f tests/fixtures/response.json

# Command rewriting
tokenless rewrite "git log --oneline -10"
tokenless rewrite "docker ps && cargo test"
```

`tests/fixtures/tool-schema.json` — OpenAI Function Calling schema with long descriptions
`tests/fixtures/response.json` — API response with debug/logs fields
`tests/fixtures/response-large.json` — Large response with nulls/empties/arrays

---

## 11. Build & Development

```bash
make build     # Release build
make test      # Run all tests
make lint      # fmt + clippy
make install   # Install to ~/.local/bin
make setup     # Build + install + adapter
```
