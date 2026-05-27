# Tokenless Error Handling & Graceful Degradation

## Design Philosophy

> **Compression must never block the agent.** If tokenless fails for any reason, the agent proceeds with original (uncompressed) data.

This means:
- Every error path has a fallback to original content
- Stats recording failures are silent (never surface to user)
- Exit codes distinguish configuration errors from operational failures
- Hook protocols degrade gracefully when binaries are missing

## Error Type Hierarchy

```
                    ┌─────────────┐
                    │  App Error   │
                    └──────┬──────┘
                           │
            ┌──────────────┼──────────────┐
            │              │              │
     ┌──────┴──────┐ ┌────┴─────┐ ┌──────┴──────┐
     │ Usage/Config│ │  Parse   │ │ Operational │
     │  (exit 1)   │ │ (exit 2) │ │  (fallback) │
     └─────────────┘ └──────────┘ └─────────────┘
```

## Error Codes

| Code | Category | Examples | Behavior |
|------|----------|---------|----------|
| 0 | Success | Compression applied or no savings | Proceed |
| 1 | Usage/Config | Missing file, invalid flags, DB directory creation failure | Report to stderr, exit |
| 2 | Parse/Serialization | Invalid JSON, TOON decode failure | Report to stderr, exit |

**Exit code 1** covers:
- Missing required file argument
- Invalid flag combinations
- Database directory creation failure
- RTK not installed (non-fatal: outputs original + warning)
- Missing spec file

**Exit code 2** covers:
- JSON parse errors (invalid syntax)
- TOON decode errors (malformed input)
- Serialization errors (should not happen with valid data)

## Operational Failures → Fallback (Never Exit Non-Zero)

These conditions produce **exit 0** with original content:

| Condition | Path | Behavior |
|-----------|------|----------|
| Compression yields no savings | schema/response/toon | Output original JSON |
| RTK not installed | rewrite | Output original command + install hint to stderr |
| No rewrite available | rewrite | Output original command |
| Stats DB error | all stats paths | Skip recording (fail-silent) |
| Stats recording disabled | all stats paths | Skip recording (no error) |
| Empty input | response compress | Output empty or original |
| Non-JSON input to compress | response/toon compress | Pass-through original text |

## Graceful Degradation by Component

### SchemaCompressor

```rust
pub fn compress(&self, tool: &Value) -> Value {
    let original_text = serde_json::to_string(tool).unwrap_or_default();
    // ... compression logic ...
    let compressed_text = serde_json::to_string(&result).unwrap_or_default();
    if original_text == compressed_text {
        return tool.clone(); // No change → return original
    }
    result
}
```

**Failure modes handled**:
- `serde_json::to_string()` failure: falls back to empty string, comparison triggers "no change" path
- Invalid JSON structure (non-object, null): `as_object_mut()` returns None, compression skipped
- UTF-8 truncation: `find_char_boundary()` prevents splitting multi-byte characters
- CJK text without sentence boundaries: hard truncation at max_len with character boundary safety

### ResponseCompressor

```rust
fn compress_value(&self, value: &Value, depth: usize) -> Value {
    if depth > self.max_depth {
        return Value::String(format!("<{type_name} truncated at depth {depth}>"));
    }
    match value {
        Value::String(s) => self.compress_string(s),
        // ... all variants handled explicitly
    }
}
```

**Failure modes handled**:
- Depth exceeded: returns descriptive marker instead of panicking
- String truncation: character-boundary-safe, UTF-8 aware
- Custom drop fields silently applied (no error if field doesn't exist)

### TOON Encoding

```rust
let output = toon_format::encode_default(&value)?;
let before_tokens = estimate_tokens_from_bytes(input.len());
let after_tokens = estimate_tokens_from_bytes(output.len());

if output.is_empty() || after_tokens >= before_tokens {
    input.clone() // No savings → original
} else {
    output
}
```

### Command Rewriting

```rust
if !rtk_available() {
    eprintln!("[tokenless] RTK is not installed — using original command.");
    eprintln!("  Install: cargo install rtk");
    println!("{cmd}"); // Output original command
    return Ok(());
}

match rewrite_command(&cmd, &exclude, &transparent_prefix) {
    Some(rewritten) => println!("{rewritten}"),
    None => {
        eprintln!("[tokenless] No rewrite available — passing through original.");
        println!("{cmd}");
    }
}
```

### Stats Recording (Fail-Silent Pattern)

```rust
fn record_compression_stats(...) {
    if !TokenlessConfig::load().is_stats_enabled() {
        return; // Not an error — stats are optional
    }

    // Early exit: no savings recorded
    if after_tokens >= before_tokens && op != RewriteCommand {
        return;
    }

    // Database errors are silently ignored
    if let Ok(recorder) = open_recorder() {
        let _ = recorder.record(&record); // Result explicitly ignored
    }
}
```

### env_check

```rust
pub fn run(tool, all, fix, checklist, json) -> Result<(), (String, i32)> {
    let spec_path = find_spec_path().map_err(|e| (e, 1))?;
    let specs = load_spec(&spec_path).map_err(|e| (e, 1))?;

    // Unknown tool → UNKNOWN status (not an error)
    if !specs.contains_key(&resolved) {
        if json {
            println!("{}", build_json_result(&resolved, &Unknown, &[], &[]));
            return Ok(());
        }
        println!("{}: UNKNOWN", t);
        return Ok(());
    }

    // Auto-fix failure → reports remaining missing deps (not an error)
    if fix && !missing_deps.is_empty() {
        let fix_output = auto_fix(&missing_deps).map_err(|e| (e, 1))?;
        // ... re-check after fix, report results
    }
}
```

## Hook Protocol Error Handling

All hook subcommands (`hook rewrite *`, `hook compress`) follow a strict rule:

> **Never produce an error response that could break the agent's tool execution loop.**

### Rewrite Hooks

| Agent | No-Rewrite Case | Error Case |
|-------|----------------|-----------|
| Claude | No stdout output | Original command passes through |
| Cursor | `{}` | Original command passes through |
| Gemini | `{"decision": "allow"}` | Original command passes through |
| Copilot CLI | `no output` | Agent uses original command |
| Copilot VS Code | No stdout output | Original command passes through |

### Compress Hook (PostToolUse)

Reads stdin → compresses → writes stdout. On any failure:
- Return original stdin content unchanged
- Write diagnostic to stderr (not stdout, to avoid protocol corruption)

## Database Error Resilience

```rust
impl StatsRecorder {
    pub fn new(db_path: P) -> StatsResult<Self> {
        let conn = Connection::open(db_path)?;
        // WAL mode: readers don't block writers
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA busy_timeout=5000;")?;
        // Schema migration for new columns: ignore "duplicate column" errors
        for col in &["before_output", "after_output"] {
            if let Err(e) = conn.execute(&format!("ALTER TABLE stats ADD COLUMN {col} TEXT"), []) {
                if !e.to_string().contains("duplicate column name") {
                    return Err(StatsError::Database(e));
                }
            }
        }
    }

    pub fn record(&self, record: &StatsRecord) -> StatsResult<i64> {
        let conn = self.conn.lock().unwrap_or_else(|e| {
            self.conn.clear_poison(); // Handle poisoned mutex gracefully
            e.into_inner()
        });
        // ...
    }
}
```

## Testing Error Paths

Each error condition has explicit test coverage:

```rust
// OperationType parsing
assert!(OperationType::from_str("unknown").is_err());

// Unknown tool
let result = build_json_result("UnknownTool", &Unknown, &[], &[]);
assert_eq!(result["status"], "UNKNOWN");

// Empty input
let compressor = ResponseCompressor::new();
assert_eq!(compressor.compress(&json!("short")), json!("short")); // No change

// Zero division safety
let record = StatsRecord::new(..., before_chars: 0, ...);
assert_eq!(record.chars_percent(), 0.0); // Not NaN

// Mutex poison recovery
conn.lock().unwrap_or_else(|e| {
    conn.clear_poison();
    e.into_inner()
});
```
