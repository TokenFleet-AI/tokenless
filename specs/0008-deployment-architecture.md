# Tokenless Deployment Architecture

## Build Pipeline

```
Source Code (crates/*)
    │
    ├─▶ cargo build --release
    │     ├─ opt-level = 3
    │     ├─ lto = true (fat LTO)
    │     ├─ codegen-units = 1
    │     ├─ panic = "abort"
    │     └─ strip = true
    │
    ▼
Single static binary: target/release/tokenless
    │
    ├─▶ make install
    │     └─▶ ~/.local/bin/tokenless
    │
    ├─▶ make adapter-install
    │     └─▶ ~/.local/share/anolisa/adapters/tokenless/
    │         ├── common/tool-ready-spec.json
    │         ├── common/tokenless-env-fix.sh
    │         ├── common/hooks/
    │         ├── openclaw/
    │         └── hermes/
    │
    └─▶ make setup  (install + adapter-install)
```

## Installation Matrix

| Platform | Binary | Adapters | RTK Required |
|----------|--------|----------|-------------|
| macOS (arm64/x86_64) | `~/.local/bin/tokenless` | `~/.local/share/anolisa/` | Command rewrite only |
| Linux (x86_64/aarch64) | `~/.local/bin/tokenless` | `~/.local/share/anolisa/` | Command rewrite only |
| Windows | Not yet supported | N/A | N/A |

## Runtime Dependencies

```
tokenless binary
    │
    ├── Required (bundled):
    │   └── None — pure Rust, single static binary
    │
    ├── Optional (runtime detection):
    │   ├── rtk binary          → command rewriting (60-90% savings)
    │   ├── curl                → network check in env-check
    │   ├── bash                → env-check auto-fix script execution
    │   └── Package manager     → auto-fix (dnf/yum/apt/apk)
    │
    └── Runtime state:
        ├── ~/.tokenfleet-ai/tokenless/stats.db        → compression metrics (SQLite)
        ├── ~/.tokenfleet-ai/tokenless/config.json     → persistent configuration
        └── ~/.tokenfleet-ai/tokenless/tool-ready-spec.json → tool dependency specs
```

## Configuration Layers

Priority order (highest first):

```
1. Environment variables
   ├── TOKENLESS_STATS_DB        → Override stats DB path
   ├── TOKENLESS_STATS_ENABLED   → Force enable/disable stats
   ├── TOKENLESS_TOOL_READY_SPEC → Override spec file path
   ├── TOKENLESS_PACKAGE_MANAGER → Force package manager
   └── TOKENLESS_ENV_FIX_SCRIPT  → Override fix script path

2. Config file (~/.tokenfleet-ai/tokenless/config.json)
   └── { "stats_enabled": true/false }

3. CLI flags (per-invocation)
   ├── --agent-id, --session-id, --tool-use-id
   └── --batch, --fix, --checklist, --json

4. Compiled defaults
   ├── SchemaCompressor::default()
   ├── ResponseCompressor::default()
   └── DB path: ~/.tokenfleet-ai/tokenless/stats.db
```

## Agent Hook Installation

```
tokenless init [--global] [--agent <name>]

    ├── Agent = claude (default)
    │   └── Writes/merges: {.claude|~/.claude}/settings.json
    │       └── PreToolUse(Bash) + PostToolUse(*)
    │
    ├── Agent = cursor
    │   └── Writes: {.cursor|~/.cursor}/hooks.json
    │
    ├── Agent = gemini
    │   ├── Writes: .gemini/settings.json
    │   └── Writes: .gemini/hooks/tokenless-hook-gemini.sh (chmod 755)
    │
    ├── Agent = copilot
    │   └── Writes: .github/hooks/rtk-rewrite.json
    │
    ├── Agents: windsurf, cline, kilocode, antigravity, augment
    │   └── Writes: rules file with RTK usage instructions
    │
    ├── Agent = hermes
    │   └── Writes: .hermes/plugins/tokenless-rewrite/ (Python plugin)
    │
    ├── Agent = opencode (global only)
    │   └── Writes: ~/.opencode/plugins/tokenless/plugin.json
    │
    └── Agent = pi
        └── Writes: .pi/agent/extensions/tokenless.ts
```

## CI/CD Pipeline (GitHub Actions)

```yaml
name: Release
on:
  push:
    tags: ['v*']

jobs:
  build:
    strategy:
      matrix:
        target: [x86_64-unknown-linux-gnu, aarch64-unknown-linux-gnu,
                 x86_64-apple-darwin, aarch64-apple-darwin]
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo build --release --target ${{ matrix.target }}
      - uses: actions/upload-artifact@v4
        with:
          name: tokenless-${{ matrix.target }}
          path: target/${{ matrix.target }}/release/tokenless

  publish-crates-io:
    if: github.event.inputs.publish-crates-io == 'true'
    steps:
      - run: cargo publish -p tokenless-schema
      - run: cargo publish -p tokenless-stats
      - run: cargo publish -p tokenless-cli

  release:
    needs: build
    steps:
      - uses: softprops/action-gh-release@v2
        with:
          files: artifacts/*
          generate_release_notes: true
```

## Version Management

- **Workspace version**: `Cargo.toml` → `[workspace.package] version`
- **Release automation**: `release-please` via `release-please-config.json`
- **Changelog**: `git cliff` via `cliff.toml` (conventional commits)
- **Manual publish gate**: `publish-crates-io` flag controls crates.io publishing

## Monitoring & Observability

```
tokenless stats summary
    ┌──────────────┬──────────┬──────────────┬─────────────┐
    │ Operation     │ Count   │ Chars Saved  │ Tokens Saved│
    ├──────────────┼──────────┼──────────────┼─────────────┤
    │ compress-schema   │ 1,234   │ 45,678 (57%) │ 11,420 (57%)│
    │ compress-response │ 5,678   │ 234,567 (52%)│ 58,642 (52%)│
    │ rewrite-command   │ 12,345  │ —            │ —           │
    │ compress-toon     │ 456     │ 12,345 (30%) │ 3,086 (30%) │
    └──────────────┴──────────┴──────────────┴─────────────┘
```

Stats database path: `~/.tokenfleet-ai/tokenless/stats.db`
- SQLite WAL mode with 5s busy timeout
- Indexed on: timestamp, operation, agent_id, session_id
- No built-in retention — manual clear via `tokenless stats clear`

## Docker (Future)

```dockerfile
FROM rust:1.85-slim AS builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
COPY --from=builder /app/target/release/tokenless /usr/local/bin/tokenless
ENTRYPOINT ["tokenless"]
```

## Resource Limits

| Resource | Limit | Configurable |
|----------|-------|-------------|
| Max JSON input size | No explicit limit (streaming from stdin) | No |
| Max nesting depth | 8 levels | Yes (`with_max_depth`) |
| Max string length | 512 chars (response), 256/160 (schema) | Yes (builder API) |
| Max array length | 16 items | Yes (`with_truncate_arrays_at`) |
| Max stats DB size | No limit (SQLite) | Manual clear |
| Concurrency | Single-threaded (no async I/O needed) | N/A |
| Memory | O(input_size) — one copy of input + compressed output | N/A |
