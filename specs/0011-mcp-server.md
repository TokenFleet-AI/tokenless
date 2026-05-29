# 0011 — MCP Server Integration

## 1. Motivation

Tokenless currently integrates with 11 AI coding agents via per-agent hook configurations. Each agent has a unique hook protocol (Claude's `updatedInput`, Cursor's `updated_input`, Gemini's `BeforeTool`, etc.). MCP (Model Context Protocol) eliminates this fragmentation — write one server, all MCP-compatible agents gain access.

**Key value**: From "install hooks per agent" to "add one MCP server to agent config."

## 2. Architecture Decision: Subcommand Mode (方案 A)

```
tokenless mcp start    # New subcommand, no new crate
```

**Rationale**:
- Zero new crate, zero new dependency (MCP = JSON-RPC over stdio, serde_json already exists)
- Shares `tokenless-schema` + `tokenless-stats` directly — no inter-crate API duplication
- Predictive cache lives in the long-lived MCP process (higher hit rate vs one-shot CLI invocations)
- Single binary: `tokenless mcp start` for MCP mode, `tokenless compress-schema` for one-shot mode

**Not 方案 B** (separate `tokenless-mcp` crate): Adds maintenance burden — two binaries, synced config paths, duplicated dependency tree.

## 3. MCP Tools (7 tools)

| Tool | Input Schema | Output | Maps to CLI |
|------|-------------|--------|-------------|
| `compress_schema` | `{schema: object, function_desc_max_len?: int, param_desc_max_len?: int, max_enum_items?: int}` | `{compressed: object, savings: {chars: int, tokens: int}}` | `compress-schema` |
| `compress_response` | `{response: object, truncate_strings_at?: int, truncate_arrays_at?: int, drop_nulls?: bool}` | `{compressed: object, savings: {chars: int, tokens: int}}` | `compress-response` |
| `rewrite_command` | `{command: string}` | `{rewritten: string, savings_pct: float}` | `rewrite` |
| `compress_toon` | `{json: object}` | `{toon: string, savings: {chars: int, tokens: int}}` | `compress-toon` |
| `decompress_toon` | `{toon: string}` | `{json: object}` | `decompress-toon` |
| `env_check` | `{tool?: string}` | `{status: string, tool_name: string, diagnostics?: string, deps: [...]}` | `env-check` |
| `stats_summary` | `{limit?: int}` | `{summary: {total_records, total_saved_chars, total_saved_tokens, ...}}` | `stats summary` |

## 4. MCP Protocol Flow

```
Client (Claude Desktop / Cursor / Continue)    Server (tokenless mcp start)
════════════════════════════════════════════    ══════════════════════════════
                                                      │
  {"jsonrpc":"2.0","id":1,"method":"initialize",      │
   "params":{"clientInfo":{"name":"claude"},...}}      │
  ──────────────────────────────────────────────────▶  │
                                                      │ Extract agent_id from clientInfo
  {"jsonrpc":"2.0","id":1,"result":{                  │
   "capabilities":{"tools":{}},"serverInfo":{         │
   "name":"tokenless","version":"0.2.0"}}}             │
  ◀──────────────────────────────────────────────────  │
                                                      │
  {"jsonrpc":"2.0","id":2,"method":"tools/list",       │
   "params":{}}                                       │
  ──────────────────────────────────────────────────▶  │
                                                      │ Return 7 tool definitions
  {"jsonrpc":"2.0","id":2,"result":{"tools":[...]}}    │
  ◀──────────────────────────────────────────────────  │
                                                      │
  {"jsonrpc":"2.0","id":3,"method":"tools/call",       │
   "params":{"name":"compress_schema",                 │
   "arguments":{"schema":{...}}}}                      │
  ──────────────────────────────────────────────────▶  │
                                                      │ Execute SchemaCompressor
  {"jsonrpc":"2.0","id":3,"result":{                  │  → cache check
   "content":[{"type":"text","text":"{...}"}]}}        │  → compress
  ◀──────────────────────────────────────────────────  │  → cache insert
```

## 5. Agent ID Extraction

The `initialize` request carries `clientInfo.name`. This becomes `StatsRecord.agent_id`:

| clientInfo.name | agent_id |
|----------------|----------|
| `"claude-desktop"` | `"claude-desktop"` |
| `"cursor"` | `"cursor"` |
| `"continue"` | `"continue"` |
| `"claude-code"` | `"claude-code"` |
| (unknown) | `"mcp-{name}"` |

## 6. Implementation Plan

### File: `crates/tokenless-cli/src/mcp.rs` (~200 lines)

```
mcp.rs
├── McpMessage { jsonrpc, id, method, params, result, error }  ← serde types
├── McpServer { agent_id, stats_enabled }
│   ├── handle_message(&self, msg: &McpMessage) → McpMessage
│   │   ├── "initialize"    → return capabilities
│   │   ├── "tools/list"    → return tool definitions
│   │   └── "tools/call"    → execute tool, return result
│   ├── exec_compress_schema(args)    → {compressed, savings}
│   ├── exec_compress_response(args)  → {compressed, savings}
│   ├── exec_rewrite_command(args)    → {rewritten, savings_pct}
│   ├── exec_compress_toon(args)      → {toon, savings}
│   ├── exec_decompress_toon(args)    → {json}
│   ├── exec_env_check(args)          → {status, deps}
│   └── exec_stats_summary(args)      → {summary}
└── pub fn run_mcp() → main loop: read stdin line → handle → write stdout line
```

### CLI integration (main.rs)

```rust
Commands::Mcp { .. } => {
    mcp::run_mcp();
    Ok(())
}
```

## 7. Security

### 7.1 Input Size Limit

MCP server rejects JSON-RPC lines exceeding 10 MB with a `-32700 Parse error` response to prevent memory exhaustion from oversized payloads.

### 7.2 Static Compressor Reuse

Schema and response compressors are initialized once as `LazyLock` statics and reused across all MCP tool calls, eliminating per-request allocation overhead.

## 8. MCP ↔ Hook Coexistence

| | MCP | Hook |
|---|---|---|
| Trigger | Agent explicitly calls tool | Auto-intercepted on Bash/PostToolUse |
| Latency | One JSON-RPC round-trip | Zero (inline stdio) |
| Agent support | All MCP agents | 11 per-agent configs |
| Cache scope | Process lifetime (long) | Per invocation (short) |
| Use case | Agent plans to compress | Transparent optimization |

**Coexistence**: MCP server also serves as the hook backend. Instead of `tokenless hook rewrite claude` spawning a short-lived process, the MCP server process can handle hook requests via a lightweight internal channel. Future optimization.

## 9. Test Strategy

| Test | Method |
|------|--------|
| `test_mcp_initialize` | Send `initialize`, verify capabilities in response |
| `test_mcp_tools_list` | Send `tools/list`, verify 7 tools returned |
| `test_mcp_compress_schema` | Send `tools/call` with schema, verify compressed result |
| `test_mcp_rewrite_command` | Send `tools/call` with "git status", verify "rtk git status" |
| `test_mcp_unknown_tool` | Send unknown tool name, verify error response |
| `test_mcp_invalid_json` | Send malformed line, verify error response |

## 10. Related Specs

- [0001 Architecture](./0001-architecture.md) — system context, RuFlo integration
- [0003 Data Flow](./0003-data-flow-pipeline-design.md) — compression pipeline
- [0004 Hook Protocol](./0004-hook-protocol-spec.md) — existing 11-agent hook protocols
