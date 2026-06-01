# Template Migration Analysis: rust-tui-template

> 评估日期：2026-06-01 | 评估方法：RuFlo 6 角色多 Agent Swarm | 模板仓库：`byx-darwin/rust-tui-template`

## 概述

本文档记录了使用 `rust-tui-template` 模板替换/对齐 `tokenless` 项目的完整评估分析。评估由 6 个专业 Agent 并行执行，覆盖架构、依赖、构建/CI、源码、文档/配置、安全六个维度。

## 评估结论

**不建议完全替换。** 推荐策略：将 tokenless 业务代码渐进式迁移到模板的更严格安全/构建基础设施中（即"基础设施先行，代码逐步对齐"）。

**整体风险等级：HIGH** — 主要风险来自业务逻辑丢失（模板为空脚手架），而非安全退化。实际上，模板的安全配置在每一个可测量维度上都优于 tokenless。

---

## 维度一：架构差异

### 当前 tokenless 架构

```
tokenless/ (workspace: crates/*)
├── crates/tokenless-cli/      # 二进制 — CLI + MCP + TUI 启动
├── crates/tokenless-schema/   # 库 — 压缩引擎 + TOON 编码
├── crates/tokenless-stats/    # 库 — SQLite 统计存储
└── crates/tokenless-tui/      # 库 — ratatui 仪表盘
```

- 1 个二进制 + 3 个库，全在 `crates/` 下
- TUI 作为库被 CLI 调用（`tokenless tui` → `tokenless_tui::run_tui()`）
- CLI 巨石：`main.rs` ~1357 行包含所有子命令处理
- 无 `build.rs`、无 benches、无 examples 目录

### 模板架构

```
rust-tui-template/ (workspace: crates/*, apps/*)
├── crates/core/               # 库 — CoreError, Config, SafePath
└── apps/tui/                  # 独立二进制 — TUI 应用
```

- 明确的库/二进制分离（`crates/` vs `apps/`）
- 集中的核心类型（CoreError, SafePath）
- build.rs + benches + examples + 结构化测试目录
- 更严格的 lint 规则（`deny(todo)`, `deny(dbg_macro)`）

### 需要改动

| 改动 | 说明 |
|------|------|
| 新增 `crates/core/` | 共享类型：CoreError, Config, SafePath |
| 新增 `apps/tui/` 或调整目录结构 | TUI 二进制包装器（或保持库形态） |
| Workspace members 改为 `["crates/*", "apps/*"]` | 支持 apps/ 目录 |
| 新增 `build.rs` | 编译期元数据（built crate） |
| 新增 benches/examples/tests 目录 | Criterion + rstest + proptest |
| CLI 模块化 | 拆分 main.rs 巨石为独立 handler 模块 |

---

## 维度二：依赖差异

### tokenless 独有依赖（必须保留）

这些是 tokenless 核心业务依赖，模板中没有：

| 依赖 | 版本 | 用途 | 保留优先级 |
|------|------|------|:---:|
| `chrono` | 0.4 | 时间戳序列化 | P0 |
| `clap` | 4 (derive) | CLI 参数解析 | P0 |
| `regex` | 1.10 | 响应模式匹配/压缩 | P0 |
| `rusqlite` | 0.32 (bundled) | 统计持久化 | P0 |
| `toon-format` | 0.5 | TOON 令牌编码 | P0 |
| `rtk-registry` | 0.1.0 | RTK 命令注册表 | P0 |
| `blake3` | 1 | 缓存内容哈希 | P1 |
| `lru` | 0.18 | LRU 预测缓存 | P1 |
| `libc` | 0.2 | 平台级系统调用 | P1 |
| `insta` | 1 (dev) | 快照测试 | P2 |

### 模板独有依赖（可选引入）

| 依赖 | 建议 |
|------|------|
| `secrecy` 0.11 | ✅ 推荐：敏感数据安全封装 |
| `rstest` 0.25 | ✅ 推荐：参数化测试 |
| `proptest` 1 | ✅ 推荐：属性测试 |
| `built` 0.7 | ✅ 推荐：编译期元数据 |
| `tracing-appender` 0.2 | ⚠️ 按需：TUI 文件日志 |
| `sysinfo` 0.33 | ❌ 不需要：系统监控非 tokenless 业务 |
| `toml` 0.8 | ❌ 不需要：tokenless 用 JSON 配置 |

### 版本差异处理

| 依赖 | tokenless | 模板 | 决策 |
|------|:---:|:---:|------|
| `ratatui` | 0.30 | 0.29 | 保留 0.30（更新） |
| `crossterm` | 0.29 | 0.28 | 保留 0.29（更新） |
| `tokio` | 1.52 | 1.52.1 | 锁定 1.52.1 |
| `tracing` | 0.1 | 0.1.41 | 锁定 0.1.41 |

---

## 维度三：构建/CI 差异

### 可直接从模板复制的文件

| 文件 | 状态 | 收益 |
|------|:---:|------|
| `.cargo/config.toml` | **新建** | 链接器安全加固（RELRO, NX, ASLR） |
| `.env.example` | **新建** | 环境变量模板 |
| `.github/workflows/` (改进项) | **合并** | 多平台 CI + cargo-audit + deny check |
| `deny.toml` (安全规则) | **合并** | wildcards=deny, unknown=deny |

### 必须保留的 tokenless 配置

| 文件 | 原因 |
|------|------|
| `clippy.toml` | 强制执行 async `tokio::fs`（禁止 `std::fs`）|
| `dev-install.sh` | 开发安装流程 |
| `adapters/` | 12 个 Agent 的钩子适配器 |
| `_typos.toml` (扩展豁免词) | `caf`, `wriet` 业务词汇 |
| `.pre-commit-config.yaml` (独有钩子) | `psf/black`, `check-agent-sync` |
| CI crates.io 发布逻辑 | tokenless 发布 3 个 crate |

### CI 改进清单

- [x] 多平台矩阵（ubuntu + macos + windows）
- [x] `cargo audit` 在 CI 中执行
- [x] `cargo deny check` 在 CI 中执行
- [x] `persist-credentials: false`
- [x] 显式 `permissions:` 块
- [x] `actions/attest-build-provenance@v2`

---

## 维度四：源码差异

### 必须保留的 tokenless 代码

| 优先级 | 文件 | 说明 |
|:---:|------|------|
| P0 | `schema_compressor.rs` | OpenAI 函数调用 schema 压缩 |
| P0 | `response_compressor.rs` | JSON 响应压缩 |
| P0 | `format_router.rs` | 智能编码策略选择 |
| P0 | `encoding/*.rs` | TOON HRV / Enhanced / CJSON 编码器 |
| P0 | `shape_analyzer.rs` | JSON 结构分析 |
| P0 | `recorder.rs` | SQLite 统计存储 + 自动迁移 |
| P0 | `tokenizer.rs` | Token 估计算法 |
| P1 | `main.rs` + `handlers.rs` | CLI 定义 + 命令处理 |
| P1 | `mcp.rs` | MCP JSON-RPC 2.0 服务器 |
| P1 | `cache.rs` | LRU 预测缓存 + 差分压缩 |
| P1 | `init/mod.rs` | 12 个 Agent 钩子安装 |
| P1 | `env_check/` | 并行环境就绪检查 |
| P1 | `app.rs` + `lang.rs` | TUI 事件循环 + 中英双语 |
| P2 | `ui/*.rs` | 8 个 TUI 面板 |
| P2 | `query.rs` + `config.rs` | 统计格式化输出 |

### 代码质量差距

| 指标 | tokenless | 模板 |
|------|:---:|:---:|
| 生产代码 unwrap/expect | **168 处** | **0 处** |
| `#![allow(unwrap_used)]` | 多个 crate 全局抑制 | 不存在 |
| `#![forbid(unsafe_code)]` crate-level | 仅 tokenless-tui | 所有 crate |
| `todo!()` lint 级别 | `warn` | `deny` |
| `dbg!()` lint 检查 | 无 | `deny` |
| `indexing_slicing` lint | 无 | `warn` |

---

## 维度五：文档/配置差异

### 从模板引入

| 文件 | 说明 |
|------|------|
| `CODE_OF_CONDUCT.md` | Rust 行为准则 |
| CLAUDE.md `Completion Discipline` | Polish Bar 质量门禁 |
| CLAUDE.md 验证范围化策略 | HEAVY/LIGHT gate 规则 |

### tokenless 必须保留

| 文件 | 说明 |
|------|------|
| `specs/` (14 份文档) | 核心设计规格资产 |
| `README.zh.md` | 中文产品文档 |
| `docs/` (18 个文件) | 用户指南、Agent 工作流、分析报告 |
| `CHANGELOG.md` | 完整版本历史 v0.1.0–v0.3.0 |
| `SECURITY.md` | 详细安全策略 + 版本表 |
| `CONTRIBUTING.md` | 贡献指南 (3.5K) |

### 不需要引入

| 文件 | 原因 |
|------|------|
| `cargo-generate.toml` | tokenless 是产品仓库，非模板 |
| `SYNC.md` | 不同步其他模板仓库 |
| `docs/monitoring-panel.md` | 系统监控 Demo（tokenless 有自己的仪表盘） |

---

## 维度六：安全风险

### 安全差距矩阵（tokenless vs 模板）

| 类别 | tokenless 现状 | 模板标准 | 严重度 | 修复 |
|------|------|------|:---:|:---:|
| `.gitignore` 密钥排除 | 0 条规则 | ~15 条 | 🔴 | 复制模板配置 |
| gitleaks 预提交扫描 | 无 | 有 | 🔴 | 添加到 pre-commit |
| `deny.toml` wildcards | `allow` | `deny` | 🔴 | 改为 deny |
| `deny.toml` 未知源 | `warn` | `deny` | 🔴 | 改为 deny |
| CI `cargo audit` | 不在 CI 中 | 每次 PR | 🔴 | 添加到 CI |
| 链接器加固 | 无 `.cargo/config.toml` | 全平台 | 🟡 | 新建配置 |
| CI 最小权限 | 未设置 | `contents: read` | 🟡 | 添加 permissions |
| CI `cargo deny check` | 仅 pre-commit | CI 中执行 | 🟡 | 添加到 CI |
| 构建来源证明 | 无 | SLSA attestation | 🟢 | 后续添加 |
| `secrecy` crate | 未使用 | 已引入 | 🟢 | 后续引入 |

### 关键发现

1. **供应链攻击面**：tokenless 的 `deny.toml` 允许通配符版本和未知源，存在依赖替换攻击风险
2. **密钥泄露**：`.gitignore` 无任何密钥排除规则，且无 gitleaks 扫描
3. **已知漏洞**：`cargo audit` 不在 CI 中运行，无法阻断已知漏洞依赖合并
4. **运行时崩溃**：168 处 unwrap/expect，模板为 0

---

## 分阶段实施计划

### 阶段一：安全加固（~0.5 天，低风险）

纯配置文件变更，不涉及代码：

1. 复制 `.cargo/config.toml`（链接器安全标志）
2. 增强 `.gitignore`（密钥排除：`.env`, `*.pem`, `*.key`, `secrets/`, `credentials/`）
3. 添加 gitleaks 到 `.pre-commit-config.yaml`
4. 强化 `deny.toml`（wildcards=deny, unknown=deny, 禁止 openssl-sys）
5. 创建 `.env.example`
6. 创建 `CODE_OF_CONDUCT.md`

### 阶段二：CI 加固（~0.5 天）

1. CI 增加 `cargo audit` 步骤
2. CI 增加 `cargo deny check` 步骤
3. 所有 jobs 添加 `permissions: contents: read`
4. 所有 `actions/checkout` 添加 `persist-credentials: false`
5. 添加 MSRV 检查 job

### 阶段三：基础设施对齐（~2 天）

需要新增 crate 和调整结构：

1. 新增 `crates/core/`（CoreError, SafePath, Config）
2. Workspace members 改为 `["crates/*", "apps/*"]`
3. 新增 `apps/tui/`（TUI 二进制包装器或保持库形态）
4. 统一所有依赖到 `[workspace.dependencies]`
5. 添加 workspace dev-deps（`criterion`, `rstest`, `proptest`）
6. 锁定松弛的补丁版本

### 阶段四：lint 严格化（~1 天）

1. 升级 `todo` 从 `warn` → `deny`
2. 添加 `dbg_macro = "deny"`
3. 添加 `indexing_slicing = "warn"`
4. 添加 `missing_errors_doc = "warn"`
5. 添加 `missing_panics_doc = "warn"`
6. 添加 `unwrap_in_result = "warn"`
7. 添加 crate-level `#![forbid(unsafe_code)]` 到缺失的 crate

### 阶段五：代码质量提升（持续进行）

1. 系统消除 168 处 unwrap/expect
2. 移除全局 `#![allow(clippy::unwrap_used)]` 抑制
3. 拆分 CLI 巨石为模块化 handler
4. 添加 `build.rs`（built crate）
5. 添加 Criterion benchmarks
6. 添加 property-based tests（proptest）
7. 添加构建来源证明

---

## 收益量化

| 维度 | 当前 → 目标 | 提升 |
|------|------|:---:|
| 安全缺口 | 9 项 → 0 项 | 100% |
| CI 平台覆盖 | 1 → 3 | 3× |
| unwrap/expect 消除 | 168 → 0 | 消除全部运行时崩溃隐患 |
| lint 静态检查 | ~12 条 → ~20 条 | +67% |
| 密钥保护层 | 0 → 3 | gitignore + gitleaks + secrecy |
| 供应链检查点 | 1 → 4 | deny + audit + vet + 来源证明 |
| 测试基础设施 | 基础 → 完整 | +bench +proptest +rstest +coverage |

---

## 风险注意事项

1. **阶段三的 crate 重组**：谨慎处理 tokenless-cli 对 tokenless-tui 的依赖关系，避免破坏现有功能
2. **lint 严格化**：阶段四会产生大量 clippy 警告，建议逐 crate 处理
3. **crates.io 发布**：任何 crate 重命名/重组都需要协调 crates.io 上的已有包
4. **向后兼容**：`tokenless` 二进制名称和 CLI 接口需要保持不变

---

> 评估引擎：RuFlo Swarm (`swarm-1780300371035-1y4ejs`) | 6 角色 | 分析深度：Deep
> 相关规格：[0015-security-hardening](../specs/0015-security-hardening.md) | [0016-architecture-alignment](../specs/0016-architecture-alignment.md)
