# Tokenless Innovation Roadmap

> Last updated: 2026-05-27

## Status Summary

| # | Innovation | Status | Spec | Code | Notes |
|---|-----------|--------|------|------|-------|
| 1 | Semantic-Aware Compression | 📝 Spec 完成 | [0014](./0014-semantic-aware-compression.md) | 待实现 | 三级架构：规则→ONNX→远程 API |
| 2 | Streaming Proxy | 🔄 其他项目 | — | — | 在其他项目中开发 |
| 3 | TON / LLM-Native Format | ✅ 已完成 | [0012](./0012-format-router.md) | ✅ | 格式路由器：Enhanced TOON + TOON HRV + CJSON |
| 4 | Predictive Cache | ✅ 已完成 | — | ✅ | LRU + blake3，512 条默认容量 |
| 5 | Cross-Session Learning | 🔮 未来 | — | — | 优先级低 |
| 6 | Multi-Modal | 🔮 未来 | — | — | 研究级 |
| 7 | Tool-Aware Schema Rewriting | 🔮 未来 | — | — | 可合入 Semantic-Aware |
| 8 | Differential Response | ✅ 已完成 | [0013](./0013-differential-response.md) | ✅ | Unified diff + 70% 阈值可配置 |
| 9 | RTK Binary Elimination | ❌ 放弃 | — | — | 意义不大，RTK 二进制已稳定运行 |
| 10 | WASM Build | 🔮 未来 | — | — | 新市场 |
| 11 | MCP Server | ✅ 已完成 | [0011](./0011-mcp-server.md) | ✅ | 7 个 Tool，JSON-RPC stdio |
| 12 | RL Compression Policy | 🔮 未来 | — | — | 研究级 |

✅ 已完成：4 项 &emsp; 📝 已 Spec：1 项 &emsp; 🔄 其他项目：1 项 &emsp; ❌ 放弃：1 项 &emsp; 🔮 未来：5 项

---

## 1. Semantic-Aware Compression 📝

> Spec: [0014-semantic-aware-compression.md](./0014-semantic-aware-compression.md)

Three-level architecture: rule-based field matching (Level 1, zero deps) → ONNX embedding model (Level 2, local inference) → remote embedding API (Level 3). Auto-degradation: Level 2/3 failures fall back to Level 1.

### Expected Impact
- **40-60% additional savings** beyond structural compression
- Context-aware: same API response compressed differently based on task

---

## 2. Streaming Compression Proxy 🔄

Being developed in a separate project. Transparent HTTP MITM proxy between Agent Runtime and LLM Provider.

---

## 3. LLM-Native Compression Format ✅

> Spec: [0012-format-router.md](./0012-format-router.md)

Superseded by the Intelligent Format Router. Instead of a single new format (TON), the router analyzes JSON structure and selects the optimal encoding:

- **TOON HRV**: Header-Row-Value for uniform arrays (50-60% savings)
- **Enhanced TOON**: Type abbreviation + inline constraints (40-55%)
- **CJSON Compact**: Safe fallback for irregular structures (30-40%)

Better than a single TON format because different JSON shapes get different optimal encodings.

---

## 4. Predictive Compression Cache ✅

Implemented in `crates/tokenless-cli/src/cache.rs`. LRU cache with blake3 hashing (u64 keys). Default 512 entries, configurable. Wraps all 4 compression paths (compress-schema, compress-response, rewrite, compress-toon).

---

## 5. Cross-Session Learning 🔮

Stats data in SQLite could be analyzed to learn optimal compression parameters per tool type. Deferred — requires significant infra for config export/import.

---

## 6. Multi-Modal Compression 🔮

Extend beyond text to images/audio/video. Requires CLIP-style embeddings and multi-modal model integration. Research-grade, deferred.

---

## 7. Tool-Aware Schema Rewriting 🔮

Per-tool-type compression profiles (Read→keep patterns, Bash→drop all schema, WebFetch→keep URL only). Could be folded into Semantic-Aware Compression (Level 1 rules already support contextual profiles).

---

## 8. Differential Response Compression ✅

> Spec: [0013-differential-response.md](./0013-differential-response.md)

For polling-style tool calls (git status every 30s), sends only unified diff from previous call. Threshold gate: diff must be <70% of full output (configurable via `TOKENLESS_DIFF_THRESHOLD`). Saves 90-95% for polling patterns.

---

## 9. RTK Binary Elimination ❌

Discussed and rejected. RTK binary is stable and well-integrated via hooks. Embedding it into tokenless adds no user-visible value.

---

## 10. WebAssembly Build Target 🔮

Compile tokenless-schema to WASM for browser use. Would require replacing rusqlite (IndexedDB) and disabling subprocess features. New market opportunity — `@tokenfleet/tokenless-wasm` npm package.

---

## 11. MCP Server Protocol ✅

> Spec: [0011-mcp-server.md](./0011-mcp-server.md)

`tokenless mcp start` launches a JSON-RPC 2.0 server over stdin/stdout exposing 7 tools: compress_schema, compress_response, rewrite_command, compress_toon, decompress_toon, env_check, stats_summary. Compatible with any MCP-capable agent.

---

## 12. Reinforcement Learning Compression Policy 🔮

Train a small RL policy to make per-field keep/drop decisions. Academic research direction — requires training infrastructure, reward modeling, and offline evaluation framework.
