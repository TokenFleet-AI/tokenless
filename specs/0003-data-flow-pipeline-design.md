# Tokenless Data Flow & Pipeline Design

## Overview

Tokenless operates as a multi-stage compression pipeline that intercepts LLM agent tool calls at three critical points: **before tool execution** (schema compression, command rewriting), **during execution** (environment pre-check), and **after tool execution** (response compression, TOON encoding).

## Pipeline Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                        LLM AGENT SESSION                            │
│                                                                     │
│  ┌─────────┐    ┌──────────┐    ┌──────────┐    ┌───────────────┐  │
│  │  LLM    │───▶│ PreTool  │───▶│  Tool    │───▶│ PostTool      │  │
│  │  Model  │    │ Use Hook │    │ Execution│    │ Use Hook      │  │
│  └─────────┘    └──────────┘    └──────────┘    └───────────────┘  │
│       ▲              │                │                │            │
│       │         ┌────┴────┐     ┌─────┴─────┐    ┌────┴────┐      │
│       │         │ Schema  │     │ env-check  │    │Response │      │
│       │         │Compress │     │ (pre-exe)  │    │Compress │      │
│       │         └─────────┘     └───────────┘    └─────────┘      │
│       │              │                                  │            │
│       │         ┌────┴────┐                      ┌─────┴─────┐     │
│       │         │Command  │                      │   TOON    │     │
│       │         │Rewrite  │                      │  Encode   │     │
│       │         └─────────┘                      └───────────┘     │
│       │                                            │               │
│       └────────────────────────────────────────────┘               │
│                   (compressed context returns to LLM)              │
└─────────────────────────────────────────────────────────────────────┘
```

## Stage 1: Pre-Tool-Use (Before Execution)

### 1a. Schema Compression

**Trigger**: Intercepted before model sends tool definitions.

```
Input: OpenAI Function Calling schema JSON
  │
  ├─▶ SchemaCompressor.compress()
  │     ├─ Remove "title" fields (no semantic value for LLM)
  │     ├─ Remove "examples" fields (inferred from descriptions)
  │     ├─ Strip markdown (code blocks, inline code)
  │     ├─ Truncate descriptions at sentence boundaries
  │     │   ├─ Function-level: max 256 chars
  │     │   └─ Parameter-level: max 160 chars
  │     └─ Recursively process: properties, items, anyOf, oneOf, allOf
  │
  ▼
Output: Compressed schema (~57% reduction)
```

**Zero-savings guard**: If compressed JSON is identical to input, return original unchanged to avoid pointless processing.

### 1b. Command Rewriting

**Trigger**: Intercepted before Bash/Shell tool execution.

```
Input: Shell command string
  │
  ├─▶ rtk_registry::rewrite_command()
  │     ├─ Classify command (Supported/Unsupported/Ignored)
  │     ├─ Rewrite: "git status" → "rtk git status"
  │     └─ Handles: pipes (|), chaining (&&, ;), sub-shells
  │
  ├─▶ RTK availability check (OnceLock cached)
  │     ├─ Installed → rewrite applied
  │     └─ Not installed → pass-through + install hint to stderr
  │
  ▼
Output: Rewritten command (or original if no rewrite available)
```

**Hook protocol variants per agent**:
- **Claude Code**: Direct `updatedInput.command` replacement (zero round-trip)
- **Cursor**: `updated_input.command` replacement (zero round-trip)
- **Gemini**: `hookSpecificOutput.tool_input.command` replacement (zero round-trip)
- **Copilot (CLI)**: Returns `permissionDecision: deny` with suggestion (one round-trip)
- **Copilot (VS Code)**: Same protocol as Claude Code (zero round-trip)

### 1c. Environment Pre-Check

**Trigger**: Optional, before any tool execution.

```
Input: Tool name + tool-ready-spec.json
  │
  ├─▶ Load spec (6 tool categories: Shell, WebFetch, Read, Write, Git, Python)
  │     ├─ Resolve tool name (case-insensitive + alias expansion)
  │     └─ Parse required/recommended dependencies
  │
  ├─▶ Check phase (parallel where possible)
  │     ├─ Binary availability: command -v {binary}
  │     ├─ Version constraints: {binary} --version + semver comparison
  │     ├─ Config files: fs::metadata() for path existence
  │     ├─ Permissions: file_read, file_write, exec_shell
  │     └─ Network: curl -s --max-time 2 https://example.com
  │
  ├─▶ Status classification
  │     ├─ READY: all required deps present
  │     ├─ PARTIAL: recommended deps or config missing
  │     └─ NOT_READY: required deps missing or permissions denied
  │
  ├─▶ Auto-fix (--fix flag)
  │     ├─ Detect system package manager (dnf/yum/apt/apk)
  │     ├─ Run tokenless-env-fix.sh with dependency JSON
  │     └─ Re-check after fix
  │
  ▼
Output: Status + diagnostic + fixed/missing lists
```

## Stage 2: During Execution

**Tool execution proceeds with rewritten command (if applicable).** Tokenless does not intercept the actual tool runtime — it only modifies the input and output at the agent boundary.

## Stage 3: Post-Tool-Use (After Execution)

### 3a. Response Compression

```
Input: Tool execution result JSON
  │
  ├─▶ ResponseCompressor.compress()
  │     ├─ Drop debug fields: debug, trace, traces, stack, stacktrace, logs, logging
  │     ├─ Drop null values (configurable)
  │     ├─ Drop empty fields: "", [], {}
  │     ├─ Truncate strings > 512 chars (UTF-8 safe, char boundary aware)
  │     ├─ Truncate arrays > 16 items (with truncation marker)
  │     └─ Depth limit: objects nested > 8 levels replaced with type marker
  │
  ├─▶ Zero-savings guard: if compressed == original, return original
  │
  ▼
Output: Compressed JSON (~26-78% reduction)
```

### 3b. TOON Encoding (optional, configurable)

```
Input: Compressed JSON response (from stage 3a)
  │
  ├─▶ toon_format::encode_default()
  │     ├─ Key: Value pairs (one per line)
  │     ├─ Nested objects indented
  │     └─ Arrays as indexed entries
  │
  ├─▶ Zero-savings guard: if TOON output >= JSON input, return original JSON
  │
  ▼
Output: TOON-encoded text (~15-40% additional reduction)
```

## Stats Recording Flow

All compression stages feed into the stats pipeline (fail-silent):

```
CompressionResult { before_text, after_text, operation_type }
  │
  ├─▶ Check: TokenlessConfig.is_stats_enabled()
  │     ├─ Enabled → proceed
  │     └─ Disabled → skip (zero overhead)
  │
  ├─▶ Estimate tokens: estimate_tokens_from_bytes(len)
  │     ├─ Quick estimation: bytes / 4 (4 chars per token approx.)
  │     └─ Early exit: if after_tokens >= before_tokens, skip recording
  │
  ├─▶ Build StatsRecord
  │     ├─ operation, agent_id, session_id, tool_use_id
  │     ├─ before_chars, before_tokens, after_chars, after_tokens
  │     └─ before_text, after_text (for diff export)
  │
  ▼
StatsRecorder::record() → SQLite (WAL mode, 5s busy timeout)
  Failure → silent (never blocks compression output)
```

## Cross-Cutting Concerns

### Zero-Savings Guard

Every compression stage implements this pattern:

```rust
let original_text = serde_json::to_string(&input).unwrap_or_default();
let result = compressor.compress(&input);
let compressed_text = serde_json::to_string(&result).unwrap_or_default();

if original_text == compressed_text {
    return input.clone(); // Return original, not processed result
}
result
```

This prevents wasteful round-trips where compression provides no benefit.

### UTF-8 Safety

All string truncation is character-boundary aware:
- `find_char_boundary()` ensures truncation never splits multi-byte characters
- CJK text handled correctly (tested with repeated 中 characters)
- Truncation markers appended after valid boundary

### Concurrency Model

- **`StatsRecorder`**: `Mutex<Connection>` — single writer, thread-safe
- **`rtk_available()`**: `OnceLock<bool>` — check-once, cache forever
- **No other shared state** — each compression call is independent and immutable

## End-to-End Example

```
1. LLM generates: Bash(command="git log --oneline -50")

2. PreToolUse hook fires:
   ├─ Schema: n/a (Bash tool, not schema)
   ├─ Rewrite: "git log --oneline -50" → "rtk git log --oneline -50"
   └─ Env-check: rtk binary present ✓

3. Tool executes: rtk git log --oneline -50
   Output: 5000 bytes of filtered git log

4. PostToolUse hook fires:
   ├─ Response compress: 5000 → 1200 bytes (76% reduction)
   │   └─ truncated long commit messages, removed empty fields
   └─ TOON encode: skipped (non-JSON output)

5. Stats recorded: CompressResponse, 5000→1200 bytes, 1250→300 tokens

6. LLM receives compressed output: saves ~950 tokens
```
