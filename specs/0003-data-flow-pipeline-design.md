# Tokenless Data Flow & Pipeline Design

## Overview

Tokenless operates as a multi-stage compression pipeline that intercepts LLM agent tool calls at three critical points: **before tool execution** (schema compression, command rewriting), **during execution** (environment pre-check), and **after tool execution** (response compression, TOON/format encoding, differential response).

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
│       │         │Predict- │     │ env-check  │    │Response │      │
│       │         │ Cache   │     │ (pre-exe)  │    │Compress │      │
│       │         │    │    │     └────────────┘    │    │    │      │
│       │         │    ▼    │                       │    ▼    │      │
│       │         │ ShapeAn-│                       │  Format │      │
│       │         │ alyzer  │                       │  Router │      │
│       │         │    │    │                       │    │    │      │
│       │         │    ▼    │                       │    ▼    │      │
│       │         │Format   │                       │Predict- │      │
│       │         │ Router  │                       │ Cache   │      │
│       │         └─────────┘                       └─────────┘      │
│       │              │                            │                │
│       │         ┌────┴────┐                       │                │
│       │         │Command  │                       │                │
│       │         │Rewrite  │                       │                │
│       │         └─────────┘                       │                │
│       │              │                            │                │
│       │         ┌────┴────┐                       │                │
│       │         │ Diff    │                       │                │
│       │         │ (poll)  │                       │                │
│       │         └─────────┘                       │                │
│       │                                            │               │
│       └────────────────────────────────────────────┘               │
│                   (compressed context returns to LLM)              │
└─────────────────────────────────────────────────────────────────────┘
```

## Stage 1: Pre-Tool-Use (Before Execution)

### 1a. Predictive Cache Lookup

**Trigger**: All compression/rewrite operations.

```
Input: Compression/rewrite payload
  │
  ├─▶ blake3 hash of input → u64 key
  │     ├─▶ PredictCache lookup (LRU, default 512 entries)
  │     │   ├─ Hit → return cached result immediately
  │     │   └─ Miss → proceed to compression pipeline
  │     └─▶ Cache store after compression (on miss)
  │
  └─▶ TOKENLESS_CACHE_SIZE=0 disables entirely
```

### 1b. Schema Compression

**Trigger**: Intercepted before model sends tool definitions.

```
Input: OpenAI Function Calling schema JSON
  │
  ├─▶ ShapeAnalyzer.analyze() → JsonShape (TopType, uniformity, depth)
  │
  ├─▶ FormatRouter.select_strategy(shape) → Strategy
  │   ├─ SchemaCompressor (function calling schema)
  │   ├─ ResponseCompressor (generic JSON)
  │   ├─ ToonHrv (uniform object arrays, ≥5 items)
  │   ├─ EnhancedToon (schemas with enums/constraints)
  │   └─ CjsonCompact (irregular structures)
  │
  ├─▶ Selected Strategy executes
  │     ├─ SchemaCompressor.compress()
  │     │   ├─ Remove "title" fields
  │     │   ├─ Remove "examples" fields
  │     │   ├─ Strip markdown
  │     │   ├─ Truncate descriptions at sentence boundaries
  │     │   │   ├─ Function-level: max 256 chars
  │     │   │   └─ Parameter-level: max 160 chars
  │     │   └─ Recursively process: properties, items, anyOf, oneOf, allOf
  │     └─ FormatRouter → encoding strategy result
  │
  ▼
Output: Compressed/encoded schema (~57%+ reduction)
```

**Zero-savings guard**: If compressed JSON is identical to input, return original unchanged to avoid pointless processing.

### 1c. Command Rewriting

**Trigger**: Intercepted before Bash/Shell tool execution.

```
Input: Shell command string
  │
  ├─▶ PredictCache lookup (blake3 hash)
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
  └─▶ PredictCache store (on miss)
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

### 1d. Differential Response (Polling)

**Trigger**: Repeated invocation of the same tool command (e.g., `git status` polling).

```
Input: {"command": "<cmd>", "output": "<response text>"}
  │
  ├─▶ Lookup last output by command key (in-process HashMap)
  │   ├─ First call → store baseline, return full output
  │   └─ Subsequent call → compute unified diff
  │       ├─ diff_len < threshold * full_len → emit diff (+/- lines)
  │       ├─ No changes → emit "(unchanged)" marker
  │       └─ diff too large → fall back to full output
  │
  └─▶ TOKENLESS_DIFF_THRESHOLD=0.7 (configurable)
  │
  ▼
Output: unified diff or "(unchanged)" or full output (saves 90-95% for polling)
```

### 1e. Environment Pre-Check

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
  ├─▶ ShapeAnalyzer.analyze() → JsonShape
  │
  ├─▶ FormatRouter.select_strategy(shape) → Strategy
  │   ├─ ResponseCompressor.compress()
  │   │   ├─ Drop debug fields: debug, trace, traces, stack, stacktrace, logs, logging
  │   │   ├─ Drop null values (configurable)
  │   │   ├─ Drop empty fields: "", [], {}
  │   │   ├─ Truncate strings > 512 chars (UTF-8 safe)
  │   │   ├─ Truncate arrays > 16 items (with truncation marker)
  │   │   └─ Depth limit: objects nested > 8 levels replaced with type marker
  │   │
  │   ├─ ToonHrv (uniform arrays → Header-Row-Value)
  │   ├─ EnhancedToon (schema-like with constraints)
  │   └─ CjsonCompact (irregular mixed structures)
  │
  ├─▶ Zero-savings guard: if compressed == original, return original
  │
  └─▶ PredictCache store (on miss)
  │
  ▼
Output: Compressed/encoded JSON (~26-78%+ reduction)
```

### 3b. Differential Response (Post-Execution)

For polling-style commands, the PostToolUse hook also computes diffs:

```
Input: Tool output + command key
  │
  ├─▶ Store current output as new baseline
  │
  ├─▶ Compare with previous baseline → unified diff
  │     ├─ Common prefix/suffix detection + 3 lines context
  │     ├─ Removed lines: "-" prefix
  │     └─ Added lines: "+" prefix
  │
  └─▶ Threshold gate: diff must be < 70% of full output
  │
  ▼
Output: Diff-encoded response or "(unchanged)" or full output
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

## MCP Server Mode

In addition to per-agent hooks, Tokenless exposes all compression operations via MCP (Model Context Protocol):

```
Client (any MCP-capable agent)                    Server (tokenless mcp start)
═══════════════════════════════════                ══════════════════════════════
                                                          │
  {"jsonrpc":"2.0","method":"initialize",...}              │
  ─────────────────────────────────────────────────────▶  │ Extract agent_id from clientInfo
                                                          │
  {"jsonrpc":"2.0","method":"tools/call",                  │
   "params":{"name":"compress_schema",                     │
   "arguments":{"schema":{...}}}}                          │
  ─────────────────────────────────────────────────────▶  │ Execute pipeline:
                                                          │   → cache check (blake3)
                                                          │   → ShapeAnalyzer
                                                          │   → FormatRouter
                                                          │   → compress/encode
                                                          │   → cache insert
  {"content":[{"type":"text","text":"{compressed}"}]}      │
  ◀──────────────────────────────────────────────────────  │
```

**7 MCP tools**: `compress_schema`, `compress_response`, `rewrite_command`, `compress_toon`, `decompress_toon`, `env_check`, `stats_summary`.

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

### Predictive Cache

All compression/rewrite/encoding operations are pure functions — same input always produces the same output. The `PredictCache` uses blake3 hashing (first 8 bytes as u64 key) with an LRU eviction policy (default 512 entries, configurable via `TOKENLESS_CACHE_SIZE`):

```rust
// On cache miss:
let hash = blake3::hash(input_bytes);
let key = u64_from_first_8_bytes(&hash);
if let Some(cached) = cache.get(key) {
    return cached;
}
let result = compress(input);
cache.insert(key, result.clone());
result
```

Set `TOKENLESS_CACHE_SIZE=0` to disable caching entirely.

### UTF-8 Safety

All string truncation is character-boundary aware:
- `find_char_boundary()` ensures truncation never splits multi-byte characters
- CJK text handled correctly (tested with repeated 中 characters)
- Truncation markers appended after valid boundary

### Concurrency Model

- **`StatsRecorder`**: `Mutex<Connection>` — single writer, thread-safe
- **`rtk_available()`**: `OnceLock<bool>` — check-once, cache forever
- **`PredictCache`**: `LazyLock<Mutex<PredictCache>>` — shared across all compression paths
- **No other shared state** — each compression call is independent and immutable

## End-to-End Example

```
1. LLM generates: Bash(command="git log --oneline -50")

2. PreToolUse hook fires:
   ├─ PredictCache: miss (first time)
   ├─ Schema: n/a (Bash tool, not schema)
   ├─ Rewrite: "git log --oneline -50" → "rtk git log --oneline -50"
   │   └─ PredictCache: store rewrite result
   └─ Env-check: rtk binary present ✓

3. Tool executes: rtk git log --oneline -50
   Output: 5000 bytes of filtered git log

4. PostToolUse hook fires:
   ├─ ShapeAnalyzer: detects flat object array (commit entries)
   ├─ FormatRouter: selects ToonHrv for uniform array structure
   ├─ Response compress + TOON HRV encode: 5000 → 1200 bytes (76% reduction)
   ├─ Differential: first call → store baseline, return full output
   └─ PredictCache: store compression result

5. Stats recorded: CompressResponse, 5000→1200 bytes, 1250→300 tokens

6. LLM receives compressed output: saves ~950 tokens

7. Next poll (same command, different output):
   ├─ PreToolUse: PredictCache hit for rewrite → skip computation
   └─ PostToolUse: differential emits unified diff (~200 bytes vs 5000 full)
```
