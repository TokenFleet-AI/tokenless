# Token-Less

**LLM token optimization toolkit** — schema/response compression + TOON encoding + command rewriting + tool environment readiness.

Token-Less combines complementary strategies to minimize LLM token consumption:

- **Schema Compression** — Compresses OpenAI Function Calling tool definitions, reducing structural overhead by ~57% before tokens reach the context window.
- **Response Compression** — Compresses API/tool responses by removing debug fields, truncating strings, limiting arrays, and eliminating null/empty values (~26–78% savings).
- **TOON Context Compression** — Encodes JSON responses to TOON (Token-Oriented Object Notation) format, reducing token usage by 15–40% for structured data.
- **Command Rewriting** — Delegates to [RTK](https://github.com/TokenFleet-AI/rtk) via the `rtk-registry` crate for filtered command output (60–90% savings on 70+ commands).
- **Tool Ready** — Pre-checks tool execution environments (binaries, configs, permissions, network), auto-fixes missing dependencies, and classifies execution failures as environment issues vs logic errors.

## Token Savings

| Strategy | Savings | Details |
|---|---|---|
| Schema compression | ~57% | Compresses OpenAI Function Calling tool schemas |
| Response compression | ~26–78% | Compresses API / tool responses (varies by content type) |
| TOON context compression | 15–40% | Encodes JSON to TOON format for LLMs |
| Command rewriting | 60–90% | Filters CLI output via RTK (70+ commands supported) |
| Tool Ready | reduces retry waste | Pre-check env, auto-fix deps, failure attribution |
| Zero runtime deps | — | Pure Rust, single static binary |

## Architecture

```
tokenless/
├── crates/tokenless-schema/        # Core library: SchemaCompressor + ResponseCompressor
├── crates/tokenless-stats/         # SQLite-based compression metrics tracking
├── crates/tokenless-cli/           # CLI binary: `tokenless` command
├── adapters/tokenless/             # FHS adapter bundle (future: hooks, plugins)
│   ├── manifest.json
│   ├── common/
│   │   ├── hooks/                  # copilot-shell / hermes hooks
│   │   ├── tool-ready-spec.json    # Tool dependency spec (4 categories)
│   │   └── tokenless-env-fix.sh    # Auto-fix script for missing deps
│   ├── openclaw/                   # OpenClaw plugin (future)
│   └── hermes/                     # Hermes Agent plugin (future)
```

**Command rewriting** is handled by the [`rtk-registry`](https://github.com/TokenFleet-AI/rtk/tree/main/crates/rtk-registry) crate (no shelling out to the RTK binary):

```rust
use rtk_registry::rewrite_command;

// "git status" → Some("rtk git status")
let rewritten = rewrite_command("git status", &[], &[]);
```

The actual RTK binary is still required at runtime for output filtering — the registry only handles command transformation.

## CLI Usage

### compress-schema

Compress OpenAI Function Calling tool schemas:

```bash
# From file
tokenless compress-schema -f tool.json

# From stdin (single schema)
cat tool.json | tokenless compress-schema

# Batch mode (JSON array)
tokenless compress-schema -f tools.json --batch
```

### compress-response

Compress API / tool responses:

```bash
# From file
tokenless compress-response -f response.json

# From stdin
curl -s https://api.example.com/data | tokenless compress-response
```

### compress-toon / decompress-toon

Encode JSON to TOON format (or decode back to JSON):

```bash
# Encode JSON to TOON
echo '{"name":"Alice","age":30}' | tokenless compress-toon
# name: Alice
# age: 30

# Decode TOON back to JSON
echo 'name: Alice\nage: 30' | tokenless decompress-toon
# {"name":"Alice","age":30}
```

### env-check

Check tool environment readiness:

```bash
# Check a specific tool
tokenless env-check --tool Shell

# Check all tools
tokenless env-check --all

# Output checklist
tokenless env-check --checklist

# Check and auto-fix missing deps
tokenless env-check --tool Shell --fix
```

### stats

View compression statistics:

```bash
# Summary
tokenless stats summary

# Recent records
tokenless stats list --limit 20

# Show record details
tokenless stats show 5

# Enable/disable recording
tokenless stats enable
tokenless stats disable
```

## Build

| Target | Description |
|---|---|
| `make build` | Build `tokenless` (release mode) |
| `make test` | Run all tests |
| `make lint` | Run clippy checks |
| `make fmt` | Format code |
| `make clean` | Clean build artifacts |

## Prerequisites

- **Rust** toolchain >= 1.85 (Rust 2024 edition)
- **RTK** binary — required for command rewriting output filtering

## License

Apache License 2.0 — see [LICENSE](LICENSE.md) for details.
