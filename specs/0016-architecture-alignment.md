# Architecture Alignment Specification

> 目标：对齐 rust-tui-template 架构 | 评估日期：2026-06-01 | 优先级：P1

## 背景

经由 RuFlo 6 角色多 Agent Swarm 评估，tokenless 与 `rust-tui-template` 模板在 workspace 组织、crate 职责分离、lint 纪律、测试基础设施等方面存在结构性差异。本文档定义对齐方案，以模板的工程最佳实践为目标，同时保留 tokenless 全部业务逻辑。

参考：[模板迁移分析报告](../docs/research/template-migration-analysis.md)

---

## 目标架构

```
tokenless/ (workspace: crates/*, apps/*)
│
├── crates/core/                    # 🆕 共享类型 crate
│   ├── src/lib.rs                  # CoreError, Config, SafePath, Result<T>
│   ├── benches/                    # Criterion benchmarks
│   ├── examples/                   # 使用示例
│   └── tests/                      # 集成测试 (rstest + proptest)
│
├── crates/tokenless-schema/        # 📦 保持：压缩引擎
│   ├── src/
│   │   ├── lib.rs                  # + #![forbid(unsafe_code)]
│   │   ├── response_compressor.rs
│   │   ├── schema_compressor.rs
│   │   ├── shape_analyzer.rs
│   │   ├── format_router.rs
│   │   └── encoding/
│   └── tests/
│
├── crates/tokenless-stats/         # 📦 保持：统计存储
│   ├── src/
│   │   ├── lib.rs                  # + #![forbid(unsafe_code)]
│   │   ├── config.rs
│   │   ├── record.rs
│   │   ├── recorder.rs
│   │   ├── tokenizer.rs
│   │   └── query.rs
│   └── tests/
│
├── crates/tokenless-cli/           # 📦 保持：CLI 入口
│   ├── src/
│   │   ├── main.rs                 # 🔧 模块化拆分
│   │   ├── handlers.rs             # 🔧 按子命令拆分
│   │   ├── cache.rs
│   │   ├── mcp.rs
│   │   ├── init/
│   │   └── env_check/
│   └── tests/
│
└── apps/tui/                       # 🆕 TUI 二进制包装器（可选）
    ├── Cargo.toml
    ├── build.rs                    # built crate
    └── src/
        ├── main.rs                 # 独立入口点
        ├── app.rs                  # 从 tokenless-tui 迁移
        ├── i18n.rs                 # 从 lang/ 重构
        ├── theme.rs                # 🆕 主题抽象
        └── ui/                     # 从 tokenless-tui/ui/ 迁移
```

---

## 改动项详情

### Item 1：新增 `crates/core/`

**动机**：模板有集中的 `CoreError`、`SafePath`、`Config` 类型。tokenless 的错误类型分散在各 crate 中，且无安全路径验证。

**Cargo.toml**:
```toml
[package]
name = "tokenless-core"
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
serde.workspace = true
serde_json.workspace = true
thiserror.workspace = true

[dev-dependencies]
criterion.workspace = true
rstest.workspace = true
proptest.workspace = true

[[bench]]
name = "core_bench"
harness = false
```

**lib.rs 摘要**:
```rust
#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod error;
mod config;
mod safe_path;

pub use error::{CoreError, Result};
pub use config::Config;
pub use safe_path::SafePath;
```

**迁移来源**：
- `CoreError` → 从各 crate 的 error types 提炼共享变体
- `SafePath` → 从模板移植，适配 tokenless 需求
- `Config` → 从 `tokenless-stats/src/config.rs` 提取通用部分

---

### Item 2：统一 workspace 依赖

**动机**：当前部分依赖定义在 workspace，部分在 crate 本地。模板全部集中在 `[workspace.dependencies]` 中。

**实施**：将以下 crate 本地依赖提升到 workspace：

| 从 crate | 依赖 | 提升到 workspace |
|------|------|------|
| tokenless-cli | `blake3`, `lru` | ✅ |
| tokenless-schema | `regex` | ✅ |
| tokenless-stats | `rusqlite`, `chrono` | ✅ |
| tokenless-tui | `ratatui`, `crossterm` | ✅ |

**验证**：每个 crate 的 `Cargo.toml` 中 `[dependencies]` 部分仅含 `{ workspace = true }` 引用。

---

### Item 3：lint 规则严格化

**现状对比**：

| lint | tokenless | 目标 (模板) |
|------|:---:|:---:|
| `todo` | `warn` | `deny` |
| `dbg_macro` | 无 | `deny` |
| `indexing_slicing` | 无 | `warn` |
| `unwrap_in_result` | 无 | `warn` |
| `missing_errors_doc` | 无 | `warn` |
| `missing_panics_doc` | 无 | `warn` |
| `cargo` | 无 | `warn` |
| `allow_attributes_without_reason` | 无 | `warn` |
| `rust_2024_compatibility` | 无 | `warn` |

**实施**：在根 `Cargo.toml` 的 `[workspace.lints.clippy]` 中添加：
```toml
todo = "deny"
dbg_macro = "deny"
indexing_slicing = "warn"
unwrap_in_result = "warn"
missing_errors_doc = "warn"
missing_panics_doc = "warn"
cargo = "warn"
allow_attributes_without_reason = "warn"
```

**风险**：添加 `todo = "deny"` 将导致编译错误（若代码中有 `todo!()` 宏）。需先清理。

---

### Item 4：消除 unwrap/expect

**现状**：168 处在生产代码中。`tokenless-schema` 全局抑制了 `unwrap_used` 和 `expect_used`。

**策略**：
1. 移除 `#![allow(clippy::unwrap_used, clippy::expect_used)]` 从各 crate
2. 按优先级逐 crate 修复：
   - P0: `tokenless-schema`（核心压缩逻辑）
   - P1: `tokenless-stats`（统计存储）
   - P2: `tokenless-cli`（CLI 入口，最多 unwrap 使用）

**替换模式**：
```rust
// Before
let x = some_option.unwrap();

// After
let x = some_option.ok_or(CoreError::App("unexpected None".into()))?;

// Before
let lock = MUTEX.lock().unwrap();

// After
let lock = MUTEX.lock().map_err(|e| {
    CoreError::App(format!("mutex poisoned: {e}"))
})?;
```

---

### Item 5：TUI 目录重组（可选）

**方案 A（推荐）**：保持 tokenless-tui 为库，在 `apps/tui/` 添加薄二进制包装器。
- 优点：无破坏性变更，CLI 和独立二进制双入口
- 缺点：多一层间接

**方案 B**：tokenless-tui 直接变为独立二进制。
- 优点：与模板完全一致
- 缺点：破坏 CLI `tokenless tui` 命令

**推荐 A**，实施如下：
```rust
// apps/tui/src/main.rs
fn main() -> anyhow::Result<()> {
    // 独立的 TUI 二进制入口
    // 可被 CLI 调用，也可独立运行
    tokenless_tui::run_tui(recorder, refresh, lang)
}
```

---

### Item 6：添加测试基础设施

| 新增 | 说明 |
|------|------|
| `crates/core/benches/` | Criterion benchmarks |
| `crates/core/examples/` | 使用示例 |
| `crates/core/tests/` | rstest/proptest 集成测试 |
| `apps/tui/build.rs` | built crate 编译期元数据 |

**workspace dev-deps**:
```toml
[workspace.dev-dependencies]
criterion = "0.5"
rstest = "0.25"
proptest = "1"
```

---

### Item 7：CLI 模块化拆分

**现状**：`main.rs` (~1357 行) 包含所有子命令定义和处理逻辑。

**目标**：按子命令拆分为独立模块：
```
crates/tokenless-cli/src/
├── main.rs            # CLI 定义 + 派发（~200 行）
├── commands/
│   ├── mod.rs
│   ├── compress.rs    # compress-schema / compress-response / compress-auto
│   ├── toon.rs        # compress-toon / decompress-toon
│   ├── stats.rs       # stats summary/list/show/clear/rewrites/status/diff
│   ├── hook.rs        # hook rewrite/compress/diff
│   ├── init.rs        # init
│   ├── env_check.rs   # env-check
│   ├── mcp.rs         # mcp start
│   ├── demo.rs        # demo
│   └── tui.rs         # tui
├── cache.rs
├── util.rs
└── ...
```

---

## 实施检查清单

| # | 项目 | 阶段 | 状态 |
|:---:|------|:---:|:---:|
| 1 | 新增 `crates/core/` | 阶段三 | ✅ |
| 2 | Workspace members 改为 `["crates/*", "apps/*"]` | 阶段三 | ✅ |
| 3 | 统一依赖到 `[workspace.dependencies]` | 阶段三 | ✅ |
| 4 | 锁定松弛补丁版本 | 阶段三 | ✅ |
| 5 | 升级 lint 规则（9 项） | 阶段四 | ✅ |
| 6 | 添加 crate-level `#![forbid(unsafe_code)]` | 阶段四 | ✅ |
| 7 | 移除全局 `allow(unwrap_used)` 抑制 | 阶段五 | ✅ |
| 8 | 消除 unwrap/expect（逐 crate） | 阶段五 | ⚠️ 20 warn 剩余 |
| 9 | TUI 目录重组（方案 A） | 阶段三 | ✅ |
| 10 | 新增 `build.rs` (built crate) | 阶段三 | ✅ |
| 11 | 新增 benches/examples/tests 目录 | 阶段三 | ✅ |
| 12 | CLI 模块化拆分 | 阶段五 | ⬜ 延后 |
| 13 | 添加 workspace dev-deps | 阶段三 | ✅ |
| 14 | 锁定 `tokio` 1.52.1, `tracing` 0.1.41 | 阶段三 | ✅ |

---

## 依赖版本决策

| 依赖 | tokenless | 模板 | 决策 | 原因 |
|------|:---:|:---:|:---:|------|
| `ratatui` | 0.30 | 0.29 | 保留 0.30 | 更新版本 |
| `crossterm` | 0.29 | 0.28 | 保留 0.29 | 更新版本 |
| `tokio` | 1.52 | 1.52.1 | 锁定 1.52.1 | 可重现构建 |
| `tracing` | 0.1 | 0.1.41 | 锁定 0.1.41 | 可重现构建 |
| `tracing-subscriber` | 0.3 | 0.3.19 | 锁定 0.3.19 | 可重现构建 |
| `anyhow` | 1.0.102 | 1.0.102 | 不变 | 一致 |
| `serde` | 1.0.228 | 1.0.228 | 不变 | 一致 |
| `serde_json` | 1.0.142 | 1.0.142 | 不变 | 一致 |
| `thiserror` | 2.0.18 | 2.0.18 | 不变 | 一致 |
| `dirs` | 6.0 | 6 | 不变 | 等效 |

---

## 风险与缓解

| 风险 | 影响 | 缓解 |
|------|:---:|------|
| `crates/core/` 引入导致循环依赖 | 编译失败 | 严格单向依赖：core ← schema/stats ← cli/tui |
| lint 严格化导致 CI 大量失败 | PR 阻塞 | 逐 crate 渐进修复，先 warn 后 deny |
| TUI 重组破坏 `tokenless tui` 命令 | 功能回归 | 方案 A 保留库接口 |
| 依赖集中化引入版本冲突 | 编译失败 | `cargo tree -d` 检查重复依赖 |
| crates.io 包名冲突 | 发布失败 | 新增 `tokenless-core` 名与现有三包无冲突 |

---

## 验证流程

```bash
# 阶段三验证
cargo check --workspace --all-targets --all-features
cargo test --workspace --all-features
cargo tree -d  # 检查重复依赖

# 阶段四验证
cargo clippy --all-targets --all-features -- -D warnings -W clippy::pedantic

# 阶段五验证
cargo bench --workspace
cargo nextest run --all-features
```

---

## 相关文档

- [0001-architecture.md](./0001-architecture.md) — 现有架构设计
- [0006-error-handling-strategy.md](./0006-error-handling-strategy.md) — 错误处理策略
- [0007-testing-strategy.md](./0007-testing-strategy.md) — 测试策略
- [0015-security-hardening.md](./0015-security-hardening.md) — 安全加固规格
- [Template Migration Analysis](../docs/research/template-migration-analysis.md) — 模板迁移分析
