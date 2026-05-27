# Token-Less

**LLM token optimization toolkit** — schema/response compression + intelligent format routing + differential response + predictive cache + TOON encoding + command rewriting + MCP server + tool environment readiness.

Token-Less combines complementary strategies to minimize LLM token consumption:

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
│   └── env_check.rs                # Tool environment readiness (parallel checks)
├── adapters/tokenless/             # FHS adapter bundle
├── specs/                          # Design specifications (14 docs)
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
tokenless compress-auto -f response.json --json # Output with strategy info
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

```bash
tokenless mcp start    # Start JSON-RPC 2.0 server over stdin/stdout
# Exposes 7 tools: compress_schema, compress_response, rewrite_command,
# compress_toon, decompress_toon, env_check, stats_summary
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

## Build

| Target | Description |
|---|---|
| `make build` | Build `tokenless` (release mode) |
| `make test` | Run all tests (257 passing) |
| `make lint` | Run fmt + clippy + cargo-audit |
| `make fmt` | Format code |
| `make clean` | Clean build artifacts |

## Prerequisites

- **Rust** toolchain >= 1.85 (Rust 2024 edition)
- **RTK** binary — required for command rewriting output filtering

## Design Specs

See [specs/](./specs/) for 14 design documents covering architecture, data flow, hook protocols, security model, error handling, testing strategy, deployment, optimization analysis, and innovation roadmap.

## License

Apache License 2.0 — see [LICENSE](LICENSE.md) for details.
