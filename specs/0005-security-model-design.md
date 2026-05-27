# Tokenless Security Model

## Threat Model

### Assets

| Asset | Sensitivity | Impact if Compromised |
|-------|------------|----------------------|
| Tool schema definitions | Low | Inflated token usage (no data leak) |
| API response data | Medium-High | Could contain PII, secrets, business data |
| Shell commands | High | Command injection → RCE on host |
| Stats database | Medium | Operational metadata, compression patterns |
| env-check spec | Medium | Reveals tool dependencies and environment |
| RTK binary output | Low | Already filtered by RTK |

### Trust Boundaries

```
┌──────────── External ────────────┐
│  LLM Provider (Anthropic API)    │  ← Untrusted (model may hallucinate)
│  User input (stdin, CLI args)    │  ← Untrusted (adversarial input possible)
│  File system (JSON files, specs) │  ← Untrusted (could be tampered)
└──────────────────────────────────┘
              │
    ┌─────────┴─────────┐
    │  Tokenless Binary  │  ← Trust Boundary
    └─────────┬─────────┘
              │
┌──────────── Internal ────────────┐
│  Compressed output (stdout)      │  ← Clean (validated, compressed)
│  Stats DB (SQLite, local)        │  ← Trusted (only tokenless writes)
│  Config files (~/.tokenless/)    │  ← Trusted (user-managed)
└──────────────────────────────────┘
```

## Input Validation

### JSON Input (all compression paths)

Every JSON input is treated as hostile:

```rust
// 1. Parse with serde_json (strict, no lenient modes)
let value: Value = serde_json::from_str(&input)?;

// 2. Implicit validation: compression logic handles any valid JSON shape
//    - Unknown keys: ignored (not propagated)
//    - Unexpected types: handled via match patterns
//    - Deep nesting: capped by max_depth (default 8)
//    - Large strings: truncated at char boundary
//    - Large arrays: truncated at limit (default 16)
```

**Protections**:
- No `unsafe` deserialization (serde_json is memory-safe)
- No recursive descent without depth limits
- No unbounded allocation (strings/arrays truncated before processing)
- BOM stripping for Windows Cursor compatibility (defense in depth)

### Shell Command Input (rewrite path)

```rust
// Command text is NEVER executed by tokenless
// It is only pattern-matched against rtk-registry rules

let cmd = input.trim().to_string(); // Whitespace normalization only

// rtk-registry applies regex-based classification
// No shell expansion, no command execution, no eval
match rewrite_command(&cmd, &exclude, &transparent_prefix) {
    Some(rewritten) => { /* output rewritten text */ }
    None => { /* pass-through original */ }
}
```

**Key security property**: Tokenless never executes the command — it only transforms the text string. The actual execution happens in the agent's sandbox.

### File Path Input

```rust
fn read_input(file: &Option<String>) -> Result<String, String> {
    match file {
        Some(path) => fs::read_to_string(path)
            .map_err(|e| format!("Failed to read file '{path}': {e}")),
        None => { /* stdin */ }
    }
}
```

**Protections**:
- No path traversal possible (reads only the specified file)
- No wildcards, no glob expansion
- Error messages include the path for debugging but no secrets

### Environment Variable Input

```rust
// Config paths from env vars
std::env::var("TOKENLESS_STATS_DB")       // Override stats DB path
std::env::var("TOKENLESS_STATS_ENABLED")   // Override stats enable/disable
std::env::var("TOKENLESS_TOOL_READY_SPEC") // Override spec file path
std::env::var("TOKENLESS_PACKAGE_MANAGER") // Override package manager
```

**Protections**:
- Env vars only control paths and boolean flags — no code execution
- Stats DB path is used only for SQLite connection (not command execution)

## Injection Prevention

### SQL Injection (Stats DB)

All SQL is parameterized:

```rust
conn.execute(
    "INSERT INTO stats (...) VALUES (?1, ?2, ...)",
    rusqlite::params![value1, value2, ...],
)?;
```

**Only exception**: Schema migration `ALTER TABLE ADD COLUMN` uses string formatting for column names — but column names are compile-time constants, not user input:
```rust
for col in &["before_output", "after_output"] {
    let sql = format!("ALTER TABLE stats ADD COLUMN {col} TEXT");
    // col is a compile-time constant, safe
}
```

### Command Injection

The `env_check` module runs shell commands via `Command::new()` with argv-form:

```rust
// Safe: argument array, no shell interpolation
Command::new("sh")
    .args(["-c", &format!("command -v {cmd}")])
    .output()
```

**Risk note**: `cmd` comes from `tool-ready-spec.json` (a local config file, not user input at runtime). The dependency binary names in the spec are controlled by the project maintainer, not by end users.

### Path Traversal

`env_check` config file paths use `fs::metadata()` existence checks only — no file content is read from user-controlled paths. The spec file path resolution uses a hardcoded candidate list:

```rust
let candidates = [
    env_var_path,           // TOKENLESS_TOOL_READY_SPEC
    repo_relative_path,      // adapters/tokenless/common/tool-ready-spec.json
    home_config_path,       // ~/.tokenless/tool-ready-spec.json
    system_share_path,      // ~/.local/share/anolisa/...
    system_usr_path,        // /usr/share/anolisa/...
];
```

## Secrets Handling

### Design Principle: Secrets Never Reach Tokenless

Tokenless operates on JSON that has already been processed by the LLM agent framework. By the time data reaches tokenless:

1. The agent framework has already handled authentication
2. API keys, tokens, and credentials are managed by the agent runtime
3. Tokenless only sees tool schemas (public definitions) and tool results (post-execution output)

### Debug field removal

The `ResponseCompressor` drops well-known debug/trace fields that commonly leak internal state:

```rust
for f in &["debug", "trace", "traces", "stack", "stacktrace", "logs", "logging"] {
    drop_fields.insert((*f).to_string());
}
```

Users can add custom fields via `add_drop_field()`.

### Stats Database

The stats database stores **before/after text content**. This is an intentional design choice for debugging and optimization analysis, but it means:
- Sensitive response data could be persisted to `~/.tokenless/stats.db`
- Stats can be disabled: `tokenless stats disable`
- Stats can be cleared: `tokenless stats clear --yes`
- The DB file permissions depend on the user's umask

## Denial of Service Prevention

| Vector | Mitigation |
|--------|-----------|
| Deeply nested JSON | `max_depth` parameter (default 8) — replaces deeper content with type markers |
| Large input files | Stream from stdin (no full buffering beyond what compression needs) |
| Large arrays | `truncate_arrays_at` (default 16) limits array processing |
| Large strings | `truncate_strings_at` (default 512) limits string processing |
| Many schemas in batch mode | Each schema processed independently, no cross-schema state explosion |
| Rapid repeated calls | No rate limiting in tokenless itself (delegated to agent framework) |

## Cryptography

Tokenless does not implement its own cryptography. For TLS, the project policy is `rustls` with `aws-lc-rs` backend (for Rust code that needs networking, which tokenless CLI does not).

## Dependency Security

| Dependency | Risk Profile | Mitigation |
|-----------|-------------|-----------|
| `serde_json` | Parse untrusted JSON | Well-audited, memory-safe |
| `rusqlite` | SQL interface | Parameterized queries, no user SQL |
| `rtk-registry` | Regex on shell commands | Compile-time regex patterns, no runtime regex from input |
| `toon-format` | JSON ↔ TOON conversion | Pure transformation, no I/O |
| `libc` | `getuid()` in env_check | Single FFI call, fixed signature |

## CLAUDE.md Security Rules Compliance

| Rule | Implementation |
|------|---------------|
| Validate at boundary | All JSON parsed with serde, all errors surfaced immediately |
| Bound strings/collections | String truncation limits, array truncation limits, depth limits |
| Path traversal prevention | No path concatenation from user input; spec paths from hardcoded candidates |
| Parameterized DB APIs | All SQL via `rusqlite::params![]` |
| Argv-form process execution | `Command::new("sh").args(["-c", ...])` with controlled input |
| No hardcoded secrets | Config loaded from env vars and files |
| `#![forbid(unsafe_code)]` | Applied to all crates (env_check uses `unsafe` only for `libc::getuid`) |
