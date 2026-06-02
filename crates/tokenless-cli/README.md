# tokenless

LLM token optimization CLI — schema/response compression, command rewriting, TOON encoding, environment checks.

```bash
cargo install tokenless
```

## Commands

### Compression

```bash
# Auto-select best format based on JSON structure
tokenless compress-auto -f response.json

# Compress OpenAI Function Calling schemas
tokenless compress-schema -f tool.json --report

# Compress API responses
tokenless compress-response -f response.json --report --context "weather data"

# TOON encoding
echo '{"name":"Alice"}' | tokenless compress-toon

# Decompress TOON data
tokenless decompress-toon -f encoded.toon
```

### Command Rewriting

```bash
tokenless rewrite "git status"
# → rtk git status

tokenless rewrite --exclude "docker" --transparent-prefix "cargo" "cargo build"
```

### Hook System (AI Agent Integration)

```bash
# Install hooks for AI coding agents
tokenless init                                          # Claude Code (project)
tokenless init --global                                 # Claude Code (global)
tokenless init --agent cursor                           # Cursor
tokenless init --agent windsurf                         # Windsurf
tokenless init --debug                                   # Enable compress debug log

# Hook subcommands (used by agent hooks internally)
tokenless hook rewrite --target claude --project my-project
tokenless hook compress --semantic --target claude --project my-project
tokenless hook diff                                      # Unified diff (experimental)
```

Supports 12 agents: Claude, Cursor, Windsurf, Cline, Kilo Code, Antigravity, Augment, Hermes, Pi, Gemini, OpenCode, Copilot.

### Statistics

```bash
tokenless stats summary                                 # Overview
tokenless stats summary --project my-project
tokenless stats list --limit 20
tokenless stats list --project my-project --namespace prod
tokenless stats show 42                                # Record detail
tokenless stats diff 42                                 # Before/after diff
tokenless stats rewrites                                # Rewrite history
tokenless stats clear                                   # Clear all records
tokenless stats status                                  # Recording status
tokenless stats enable                                  # Enable recording
tokenless stats disable                                 # Disable recording
```

### Experimental Features

```bash
tokenless stats experimental-on                          # Enable
tokenless stats experimental-off                         # Disable
```

When enabled, unlocks: TUI dashboard, MCP server, semantic compression (Level 2), format router with enhanced TOON, hook diff.

```bash
tokenless tui                                           # Interactive dashboard
tokenless tui --refresh 2 --lang en
tokenless mcp start --port 3000                         # MCP server
```

### Other

```bash
tokenless env-check --all                                # Tool readiness
tokenless env-check --tool git,rustc --json              # Machine-readable
tokenless demo --sample weather                          # Interactive demo
```

## Global Flags

Most commands accept these for stats attribution:

| Flag | Description |
|------|-------------|
| `--project <name>` | Multi-project stats grouping |
| `--agent-id <id>` | Agent identifier |
| `--session-id <id>` | Session correlation |
| `--tool-use-id <id>` | Individual tool call correlation |

## Minimum Rust Version

Rust 2024 edition, MSRV 1.89.

License: Apache-2.0
