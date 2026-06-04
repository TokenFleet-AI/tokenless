[![CI](https://github.com/TokenFleet-AI/tokenless/actions/workflows/ci.yml/badge.svg)](https://github.com/TokenFleet-AI/tokenless/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/TokenFleet-AI/tokenless)](https://github.com/TokenFleet-AI/tokenless/releases)
[![Rust 2024](https://img.shields.io/badge/Rust-2024-orange?logo=rust)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/License-Apache%202.0-blue)](LICENSE.md)

<p align="center">
  <img src="./assets/tokenless.svg" alt="tokenless" width="520">
</p>

# Tokenless

> LLM Token Optimization Toolkit — schema/response compression + intelligent format routing + differential response + predictive cache + TOON encoding + command rewriting + MCP server + tool environment readiness.

Chinese docs: [README.zh.md](README.zh.md). Design specs and delivery docs live in [specs/index.md](specs/index.md) and [docs/index.md](docs/index.md).

**Quick links:** [Prerequisites](#prerequisites) · [Quick Start](#quick-start) · [Token Savings](#token-savings) · [CLI Usage](#cli-usage) · [Architecture](#architecture) · [Build](#build) · [Contributing](#contributing)

Tokenless combines complementary strategies to minimize LLM token consumption:

- **Schema Compression** — Compresses OpenAI Function Calling tool definitions, reducing structural overhead by ~57% before tokens reach the context window.
- **Response Compression** — Compresses API/tool responses by removing debug fields, truncating strings, limiting arrays, and eliminating null/empty values (~26–78% savings).
- **Intelligent Format Router** — Auto-selects optimal encoding per JSON shape: TOON HRV for uniform arrays (50-60%), Enhanced TOON for schemas (40-55%), CJSON compact as fallback (30-40%).
- **Differential Response** — Sends unified diff instead of full output for repeated tool calls (up to 95% savings for polling patterns like `git status`).
- **Predictive Cache** — LRU cache with blake3 hashing skips redundant compression on cache hit (near-zero latency for repeat operations).
- **TOON Context Compression** — Encodes JSON responses to TOON (Token-Oriented Object Notation) format, reducing token usage by 15–40% for structured data.
- **Command Rewriting** — Delegates to [RTK](https://github.com/TokenFleet-AI/rtk) via the `rtk-registry` crate for filtered command output (60–90% savings on 70+ commands).
- **Tool Ready** — Pre-checks tool execution environments (binaries, configs, permissions, network), auto-fixes missing dependencies.
- **MCP Server** — JSON-RPC 2.0 over stdio, exposes 7 tools for any MCP-compatible agent (Claude Desktop, Cursor, Continue, etc.).

## Token Savings

| Strategy | Savings | Details |
|---|---|---|
| Schema compression | ~57% | Compresses OpenAI Function Calling tool schemas |
| Response compression | ~26–78% | Compresses API / tool responses (varies by content type) |
| Format router | 30–60% | Auto-selects TOON HRV / Enhanced TOON / CJSON per JSON shape |
| TOON context compression | 15–40% | Encodes JSON to TOON format for LLMs |
| Differential response | up to 95% | Unified diff for polling-style repeated tool calls |
| Predictive cache | near-zero latency | LRU + blake3 skips redundant re-compression |
| Command rewriting | 60–90% | Filters CLI output via RTK (70+ commands supported) |
| MCP Server | 7 tools | JSON-RPC over stdio, any MCP agent compatible |
| Tool Ready | reduces retry waste | Pre-check env, auto-fix deps, failure attribution |
| Zero runtime deps | — | Pure Rust, single static binary |

## Prerequisites

- **Rust** toolchain >= 1.89 (Rust 2024 edition) — for `cargo install` or source build
- **RTK** binary — optional, only needed for command rewriting (`cargo install rtk`). Core compression works without it.

## Quick Start

```bash
# 1. Install
git clone https://github.com/TokenFleet-AI/tokenless && cd tokenless && make setup

# Ensure ~/.local/bin is on PATH (add to ~/.bashrc / ~/.zshrc for persistence)
export PATH="$HOME/.local/bin:$PATH"

# 2. One-click agent integration (Claude Code, Cursor, Windsurf, etc.)
tokenless init

# 3. Done! All shell commands are now auto-rewritten and responses compressed.
#    Run stats later to see your savings:
tokenless stats summary
```

**Install options:** `cargo install tokenless`, download from [GitHub Releases](https://github.com/TokenFleet-AI/tokenless/releases), or `brew install tokenfleet/tap/tokenless`.

> Supports **12 agents**: Claude Code, Cursor, Windsurf, Cline, Kilo Code, Antigravity, Augment, Hermes CLI, Pi, Gemini CLI, OpenCode, GitHub Copilot.
> `tokenless init` auto-installs hooks. See [user guide §4](./docs/user-guide.md#4-agent-integration) for all 12 agents and manual configuration.

## Architecture

```
tokenless/
├── crates/tokenless-schema/        # Core library
│   ├── schema_compressor.rs        # SchemaCompressor (+P1/P2/P3 enhancements)
│   ├── response_compressor.rs      # ResponseCompressor (+6 fixes + breadth limit)
│   ├── shape_analyzer.rs           # JSON structure analyzer for format routing
│   ├── format_router.rs            # Intelligent encoding strategy selector
│   └── encoding/                   # Encoding strategies
│       ├── enhanced_toon.rs        # Enhanced TOON (type abbrev + inline constraints)
│       ├── toon_hrv.rs             # TOON Header-Row-Value for uniform arrays
│       └── cjson_compact.rs        # CJSON compact fallback encoder
├── crates/tokenless-stats/         # SQLite-based compression metrics tracking
├── crates/tokenless-cli/           # CLI binary: `tokenless` command
│   ├── cache.rs                    # Predictive cache (LRU + blake3) + differential response
│   ├── mcp.rs                      # MCP JSON-RPC server (7 tools)
│   └── env_check/                   # Tool environment readiness (parallel checks)
├── adapters/tokenless/             # FHS adapter bundle
├── specs/                          # Design specifications (17+ docs)
└── docs/                           # User-facing documentation
```

**Command rewriting** is handled by the [`rtk-registry`](https://github.com/TokenFleet-AI/rtk/tree/master/crates/rtk-registry) crate (no shelling out to the RTK binary):

```rust
use rtk_registry::rewrite_command;

// "git status" → Some("rtk git status")
let rewritten = rewrite_command("git status", &[], &[]);
```

The actual RTK binary is still required at runtime for output filtering — the registry only handles command transformation.

## CLI Usage

### init (Agent Integration)

```bash
tokenless init                  # Project-level: hooks + --project tag for per-project stats
tokenless init --global         # Global: hooks for all projects, auto-detect project at runtime
tokenless init --agent cursor   # Install for Cursor editor
```

Auto-installs hooks into `.claude/settings.json` (or the equivalent for other agents). Once installed, all shell commands are automatically rewritten and responses compressed — zero manual steps after `init`.

**Project-level vs Global:**

| Mode | Command | `--project` in hooks | Stats behavior |
|------|---------|---------------------|----------------|
| Project-level | `tokenless init` | ✅ Written (detected from dir) | Stats attributed to this project only |
| Global | `tokenless init --global` | ❌ Omitted | Auto-detects project per invocation |

Use **project-level** `init` when you want per-project statistics isolation (e.g., `tokenless stats summary` shows only this project's savings). Use **global** `init` for a fire-and-forget setup across all repositories.

> See [user guide §4](./docs/user-guide.md#4-agent-integration) for all 12 agents and manual configuration.

### compress-schema / compress-response

```bash
tokenless compress-schema -f tool.json       # Compress tool schemas
tokenless compress-response -f response.json  # Compress API responses
cat tool.json | tokenless compress-schema --batch  # Batch mode
```

### compress-auto (Intelligent Format Router)

Auto-selects the optimal encoding strategy based on JSON structure:

```bash
tokenless compress-auto -f response.json       # Auto: TOON HRV / Enhanced TOON / CJSON
```

### compress-toon / decompress-toon

```bash
echo '{"name":"Alice","age":30}' | tokenless compress-toon    # JSON → TOON
echo 'name: Alice\nage: 30' | tokenless decompress-toon       # TOON → JSON
```

### hook diff (Differential Response)

```bash
# PostToolUse hook: sends unified diff for repeated tool calls
echo '{"command":"git status","output":"M src/main.rs\n"}' | tokenless hook diff
# Configurable threshold: TOKENLESS_DIFF_THRESHOLD=0.7 (default)
```

### mcp start (MCP Server)

> Requires `tokenless stats experimental-on` to enable experimental features.

```bash
tokenless mcp start    # Start JSON-RPC 2.0 server over stdin/stdout
# Exposes 7 tools: compress_schema, compress_response, rewrite_command,
# compress_toon, decompress_toon, env_check, stats_summary
```

### demo

```bash
tokenless demo    # Run all 4 compression demos with embedded test data
```

### env-check

```bash
tokenless env-check --tool Shell         # Check specific tool
tokenless env-check --all                # Check all tools
tokenless env-check --tool Shell --fix   # Auto-fix missing deps
```

### stats

```bash
tokenless stats summary              # Aggregated metrics
tokenless stats list --limit 20      # Recent records
tokenless stats show 5               # Record details
```

### tui (Interactive Dashboard)

> Requires `tokenless stats experimental-on` to enable experimental features.

```bash
tokenless tui                        # Launch TUI dashboard (zh, 1s refresh)
tokenless tui --lang en              # English UI
tokenless tui --refresh 3            # 3-second refresh
```

4-tab terminal dashboard: Dashboard · Records · Agents · Trends. Keyboard-driven with search, export, time-range filtering. See [user guide §3.11](./docs/user-guide.md#311-tui-dashboard) for full keybindings.

## Build

| Target | Description |
|---|---|
| `make build` | Build `tokenless` (release mode) |
| `make test` | Run all tests |
| `make lint` | Run fmt + clippy + cargo-audit |
| `make fmt` | Format code |
| `make clean` | Clean build artifacts |

## Further Reading

| What | Where |
|---|---|
| Full usage guide (installation, CLI, plugins, API) | [docs/user-guide.md](./docs/user-guide.md) |
| Design specs (17+ docs) — architecture, data flow, hook protocols, security, testing, and more | [specs/](./specs/) |
| Contribution guidelines | [CONTRIBUTING.md](CONTRIBUTING.md) |

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for development workflow, coding conventions, and testing guidelines.

## Community

- [GitHub Issues](https://github.com/TokenFleet-AI/tokenless/issues) — Bug reports and feature requests
- [GitHub Discussions](https://github.com/TokenFleet-AI/tokenless/discussions) — Q&A and ideas

**WeChat Developer Group** (微信开发者群):

<p align="center">
  <img src="assets/wechat-dev-group.png" alt="WeChat Developer Group" width="200">
</p>

<p align="center"><strong>Scan to join / 扫码加入微信开发者群</strong></p>

<p align="center">Share feedback, report issues, discuss features / 交流使用心得、反馈问题、参与功能讨论</p>

## Troubleshooting

- **`tokenless: command not found`** — Ensure `~/.local/bin/` is on your `PATH` (see Quick Start).
- **TUI/MCP shows "experimental feature" error** — Run `tokenless stats experimental-on` first.
- **Hooks not working** — Re-run `tokenless init` and restart your agent.
- **Stats show no data** — Ensure stats recording is enabled: `tokenless stats enable`.
- See [user guide](./docs/user-guide.md) for detailed troubleshooting.

## License

Apache License 2.0 — see [LICENSE](LICENSE.md) for details.
