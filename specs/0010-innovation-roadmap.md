# Tokenless Innovation Roadmap

## Overview

This document identifies high-impact innovation opportunities for tokenless, organized by feasibility and strategic value. Each entry includes the core idea, technical approach, and expected impact.

---

## 1. Semantic-Aware Compression (High Impact / Medium Effort)

### Concept
Current compression is **structural** — truncate by length, drop by field name. Semantic compression would understand what information is **actually useful to the LLM** for the current task.

### Technical Approach
- Build a lightweight classifier that scores fields by **task relevance**
- Use a tiny embedding model (e.g., `all-MiniLM-L6-v2` via `ort` or `candle`) to compare field semantics against the current conversation context
- Drop fields below a relevance threshold instead of using hardcoded field name lists

### Example
```json
// Input: weather API response with 50 fields
{"temp": 22, "humidity": 65, "station_id": "WX-001", "last_maintenance": "2024-...", ...}

// Current: drops debug fields, truncates long strings
// Semantic: keeps temp + humidity (relevant to "what's the weather?")
//           drops station_id + maintenance (irrelevant to query)
```

### Expected Impact
- **40-60% additional savings** beyond structural compression for complex API responses
- Context-aware: same API response compressed differently based on task

---

## 2. Streaming Compression Proxy (High Impact / High Effort)

### Concept
Instead of intercepting at the agent hook level, operate as a **transparent HTTP proxy** between the LLM provider and the agent runtime, compressing both request schemas and response data in-flight.

### Technical Approach
```
Agent Runtime ←→ Tokenless Proxy ←→ LLM Provider (Anthropic/OpenAI)
                     │
                ┌────┴────┐
                │ Compress │  Request: tools → compressed schemas
                │ Decompress│ Response: compressed text → original (for tool exec)
                └─────────┘
```
- MITM the Anthropic/OpenAI API stream
- Compress `tools` array before sending to model
- Decompress tool call results before agent execution
- Zero agent modifications required (transparent to hooks)

### Expected Impact
- **Universal compatibility** — works with any LLM agent without hook configuration
- **Stream-level compression** — no JSON re-parsing overhead
- **80-90% schema token reduction** (more aggressive than current ~57% since proxy can rewrite `$defs`/`$ref`)

---

## 3. LLM-Native Compression Format (High Impact / High Effort)

### Concept
Design a **token-optimized wire format** specifically for LLM consumption — more aggressive than TOON, with LLM-aware encoding decisions.

### TOON Limitations
- Still human-readable (wastes tokens on formatting)
- No type elision (redundant type info repeated)
- No shared reference compression

### Proposed: TON (Token-Optimized Notation)

```
// JSON (147 chars):
{"tools":[{"type":"function","function":{"name":"get_weather","parameters":{"type":"object","properties":{"location":{"type":"string"}}}}}]}

// TOON (87 chars, ~41% savings):
tools:
- type: function
  function:
    name: get_weather
    parameters:
      type: object
      properties:
        location:
          type: string

// TON (43 chars, ~71% savings):
⚡get_weather(location:s)
```

**Key innovations**:
- **Type shorthands**: `s`=string, `n`=number, `b`=boolean, `o`=object, `a`=array
- **Structural elision**: `type:object` + `properties` = default for tool params → omit both
- **Emoji prefixes**: single-char discriminators for message roles, tool vs response
- **Reference compression**: repeated schemas stored once, referenced by index

### Expected Impact
- **60-75% token reduction** vs JSON (compared to TOON's 15-40%)
- **Faster LLM parsing**: less text to tokenize and process
- **Model-agnostic**: works with all LLM providers

---

## 4. Predictive Compression Cache (Medium Impact / Medium Effort)

### Concept
Many tool calls are **repetitive** — same schema, similar responses. Cache compressed versions and serve from cache instead of re-compressing.

### Technical Approach
```rust
struct CompressionCache {
    schema_cache: LruCache<u64, Value>,     // Hash → compressed schema
    response_cache: LruCache<u64, Value>,   // Hash → compressed response
    rewrite_cache: LruCache<String, String>, // cmd → rewritten cmd
}
```
- Content-hash inputs (blake3 for speed)
- LRU eviction with configurable capacity (default 1000 entries)
- Thread-safe via `DashMap` + `ArcSwap`

### Expected Impact
- **Near-zero latency** for repeated operations (cache hit)
- Particularly effective for CI/CD pipelines where same tools run repeatedly
- **90%+ cache hit rate** for typical agent sessions (repeated `git status`, `ls`, etc.)

---

## 5. Cross-Session Learning (Medium Impact / Medium Effort)

### Concept
Stats data currently sits in SQLite unused beyond manual queries. Use this data to **learn optimal compression parameters** per tool type, agent, and content pattern.

### Technical Approach
- Analyze `tokenless stats` data to identify:
  - Which field names are most commonly dropped (auto-suggest `add_drop_field`)
  - Optimal truncation lengths per content type
  - Commands that should be transparent (never rewritten)
- Auto-tune compressor parameters based on historical savings data
- Export learning results as recommended config

### Implementation
```rust
struct CompressionOptimizer {
    // Per-field: "debug" → dropped 99.7% of time, avg savings 200 tokens
    // → Recommend: add to always-drop list
    field_stats: HashMap<String, FieldImpact>,

    // Per-tool: "Bash" → avg response compression 65%
    // "Read" → avg response compression 12%
    // → Recommend: skip compression for Read tool
    tool_stats: HashMap<String, ToolImpact>,
}
```

### Expected Impact
- **Adaptive compression** that improves over time per user/project
- **Eliminates manual tuning** of drop fields and truncation limits
- **Detects regressions** — alerts when compression ratio degrades

---

## 6. Multi-Modal Compression (Future / Research)

### Concept
Extend tokenless beyond text to compress **images, audio, and video** in LLM context windows.

### Technical Approach
- **Image**: Drop metadata, re-encode at lower resolution before base64, extract only semantic descriptions
- **Audio**: Extract transcript only (discard waveform) unless audio nuance is task-relevant
- **Video**: Extract key frames + transcript, discard full video stream
- Use CLIP-style embeddings to score frame relevance to task

### Expected Impact
- **90%+ token reduction** for multi-modal content (images are token-expensive)
- Critical as multi-modal LLM usage grows

---

## 7. Tool-Aware Schema Rewriting (Medium Impact / Medium Effort)

### Concept
Current schema compression is **tool-agnostic** — same rules for all tools. Different tools benefit from different strategies.

### Examples

| Tool Type | Strategy | Rationale |
|-----------|----------|----------|
| Read/Glob | Keep `pattern` descriptions, drop everything else | Pattern syntax is critical |
| Bash/Shell | Drop all schema — just keep command string | LLM knows shell syntax |
| WebFetch | Keep URL + method, drop response schema | URL is the only decision point |
| Write/Edit | Keep `old_str` + `new_str` descriptions | Diff context matters |

### Technical Approach
- Tool-type-specific compression profiles
- Auto-detect tool type from schema name/description
- Plugin system for custom tool-specific compressors

---

## 8. Differential Response Compression (Medium Impact / Low Effort)

### Concept
For repeated tool calls (e.g., `git status` polled every 30s), send only the **diff** from the previous call instead of the full response.

### Technical Approach
```rust
struct DiffCompressor {
    last_response: Option<Value>,
}

impl DiffCompressor {
    fn compress(&mut self, response: &Value) -> Value {
        match &self.last_response {
            Some(prev) => json_diff(prev, response), // Only changed fields
            None => response.clone(),                  // First call: full response
        }
        self.last_response = Some(response.clone());
    }
}
```

### Expected Impact
- **95%+ token reduction** for polling-style tool calls
- Particularly powerful for watch commands, CI status checks, monitoring

---

## 9. RTK Binary Elimination (Medium Impact / High Effort)

### Concept
Currently tokenless shells out to the RTK binary for command output filtering. The `rtk-registry` crate handles rewriting, but output filtering requires the binary. Bundle the filtering logic directly.

### Technical Approach
- Port RTK's output filtering rules into a pure Rust library (`rtk-filter` crate)
- Use the same YAML/TOML rule definitions
- Execute filtering in-process instead of spawning `rtk`
- Progressive enhancement: try in-process first, fall back to binary

### Expected Impact
- **Zero external dependencies** for command rewriting (currently requires `rtk` binary)
- **Lower latency** (no subprocess spawn per command)
- **Simpler deployment** (single binary)

---

## 10. WebAssembly Build Target (Future / Medium Effort)

### Concept
Compile tokenless to WASM for in-browser use. LLM web apps (ChatGPT, Claude.ai) could use tokenless in the browser to compress prompts before sending.

### Technical Approach
```toml
[lib]
crate-type = ["cdylib", "rlib"]

[target.wasm32-unknown-unknown]
# Disable features that don't work in WASM:
# - libc (no getuid)
# - rusqlite (no native FS) → use IndexedDB via web-sys
# - Command::new (no subprocess) → disable env-check, rewrite
```
- WASM-compatible subset: schema compression + response compression + TOON
- `wasm-bindgen` + `js-sys` for browser API access
- Publish as npm package: `@tokenfleet/tokenless-wasm`

### Expected Impact
- **New market**: browser-based LLM tools
- **~50% prompt token reduction** for web LLM users
- Integrates with Chrome extensions, web apps

---

## 11. Integration: MCP Server Protocol (Medium Impact / Low Effort)

### Concept
Wrap tokenless as an **MCP (Model Context Protocol) server** so any MCP-compatible agent (Claude Desktop, Continue, Cline, etc.) can use it without per-agent hook configuration.

### Technical Approach
- Implement MCP `tools/list` → expose `compress_schema`, `compress_response`
- Implement MCP `tools/call` → execute compression and return result
- Zero-config: agents discover tokenless via MCP, no hook setup needed

### Expected Impact
- **Universal agent compatibility** via MCP standard
- **Eliminates per-agent hook installation** (currently 11 separate implementations)
- **Simpler onboarding** — `claude mcp add tokenless`

---

## 12. Reinforcement Learning Compression Policy (Research / High Effort)

### Concept
Train a tiny RL policy to make per-field keep/drop decisions, optimizing for downstream task success rate rather than raw token reduction.

### Technical Approach
- **State**: JSON field path + semantic embedding + task context
- **Action**: keep / drop / truncate(n)
- **Reward**: task success (1) − λ × token_cost
- **Training**: Offline from stats DB + online fine-tuning via user feedback
- **Inference**: ~1ms per field decision using a small neural net (ONNX runtime)

### Expected Impact
- **Optimal compression** for each specific task type
- **Learns user preferences** (some users tolerate aggressive compression, others don't)
- **Research paper potential** — first application of RL to LLM context optimization

---

## Innovation Priority Matrix

| # | Innovation | Impact | Effort | Timeline | Unique Value |
|---|-----------|--------|--------|---------|-------------|
| 1 | Semantic-Aware Compression | High | Medium | Q3 2026 | Context-aware, beats structural |
| 2 | Streaming Proxy | High | High | Q4 2026 | Universal agent compatibility |
| 3 | TON Format | High | High | Q3 2026 | 60-75% vs TOON's 15-40% |
| 4 | Predictive Cache | Medium | Medium | Q2 2026 | Near-zero latency for repeats |
| 5 | Cross-Session Learning | Medium | Medium | Q3 2026 | Self-improving compression |
| 8 | Differential Compression | Medium | Low | Q2 2026 | 95% for polling patterns |
| 11 | MCP Server Protocol | Medium | Low | Q2 2026 | Universal compatibility |
| 9 | RTK Elimination | Medium | High | Q4 2026 | True zero-dependency |
| 7 | Tool-Aware Schemas | Medium | Medium | Q3 2026 | Per-tool optimization |
| 12 | RL Compression Policy | Research | High | 2027 | Academic novelty |
| 6 | Multi-Modal | Future | Research | 2027+ | Next-gen LLM support |
| 10 | WASM Build | Future | Medium | 2027 | Browser market |

## Quick Wins (Q2 2026)

These three innovations can be prototyped in 2-4 weeks each and deliver immediate value:

1. **Differential Compression** — Simple JSON diff logic, massive savings for polling
2. **Predictive Cache** — `LruCache` + `blake3` hash, trivial implementation
3. **MCP Server Protocol** — Standard protocol, replaces 11 agent-specific integrations
