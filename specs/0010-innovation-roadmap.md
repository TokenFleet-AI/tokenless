# Tokenless Innovation Roadmap

> Last updated: 2026-05-30

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
| 13 | CLI UX Enhancements | 📋 待实施 | — | — | 用户验证与留存优化 |

✅ 已完成：4 项 &emsp; 📝 已 Spec：1 项 &emsp; 📋 待实施：1 项 &emsp; 🔄 其他项目：1 项 &emsp; ❌ 放弃：1 项 &emsp; 🔮 未来：5 项

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

---

## 13. CLI UX Enhancements 📋

> 来源：2026-05-30 多角色 UX 评审（新用户体验、文档结构、DevRel、PM）

基于 `docs/user-guide.md` 的多角色评审，识别出以下 CLI 层面改进，可显著降低用户上手门槛和留存率。

### P0 — Before/After Token 对比输出

**问题**：`compress-response` / `compress-schema` 静默输出，用户无法亲眼看到压缩效果。文档宣称 60-90% 节省，但用户无法在自己的环境中复现。

**方案**：
- 每个压缩命令默认输出对比行：`before: 1234 bytes (~308 tokens) → after: 456 bytes (~114 tokens) — saved 62.9%`
- 新增 `--report` 标志，输出详细对比报告（JSON / 人类可读）
- 新增 `--quiet` 标志，恢复到当前静默行为（脚本/管道场景）

### P0 — `tokenless stats diff` 累计节省

**问题**：`tokenless stats summary` 只显示计数和汇总，无法直观回答"这周省了多少 Token = 省了多少钱"。

**方案**：
- `tokenless stats diff --since yesterday` / `--since 7d` / `--range 2026-05-01..2026-05-30`
- 输出：`累计节省 1,234,567 bytes / 308,642 tokens / ≈ $9.26 API 费用`
- 显示 by-agent 和 by-operation 分组
- 可选输出 JSON 用于 CI/仪表盘集成

### P1 — `tokenless demo` 一键演示

**问题**：新用户安装后需手动构造测试 JSON 或找 fixture 文件，跑第一个压缩命令门槛高。

**方案**：
- `tokenless demo` 使用内嵌测试数据一键跑 4 种压缩 + 输出对比
- `tokenless demo --interactive` 逐步引导用户体验每个功能
- 替代方案：将 `tests/fixtures/` 路径暴露为内置命令，消除文件路径依赖

### P2 — 安装方式多样化

**问题**：当前仅支持源码构建（需 Rust 工具链），对非 Rust 开发者门槛高。

**方案**：
- `cargo install tokenless`（需 crates.io 发布）
- GitHub Releases 预编译二进制（macOS/Linux x86_64 + ARM64）
- Homebrew formula：`brew install tokenfleet/tap/tokenless`

### 预期影响

- **转化率**：P0 改动让用户 1 条命令看到效果，大幅降低安装→验证→放弃的转化漏斗
- **留存率**：`stats diff` 提供定期回访的量化锚点（"这周省了 $12"）
- **传播性**：`demo` + `--report` 输出天然适合截图分享、社交媒体传播
