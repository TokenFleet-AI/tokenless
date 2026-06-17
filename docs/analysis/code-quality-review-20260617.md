# tokenless 代码质量审查报告

- 审查日期：2026-06-17
- 审查范围：workspace、核心 Rust crate、测试体系、文档、CI/CD
- 审查基准：项目 `CLAUDE.md`、`specs/0007-testing-strategy.md`、`specs/0016-architecture-alignment.md`

## 一、总体结论

### 总体评分：7/10

### 评分理由

`tokenless` 当前已经具备中上水平的工程质量基础，尤其在以下方面表现较好：

- workspace 级 lint 基线已建立，且显式要求 `clippy::pedantic`
- 核心压缩库 `tokenless-schema` 已形成 golden snapshot、determinism、stress、round-trip 等多层测试
- `tokenless-stats` 的单元测试密度较高
- CI 已覆盖三平台 build/test/clippy/fmt，并包含 `cargo audit`、`cargo deny`、MSRV、gitleaks
- 生产代码层面已基本摆脱早期大规模 `unwrap/expect` 依赖，工程纪律较规格早期状态明显提升

但距离“高质量 Rust workspace 模板”的目标仍有明显差距，主要短板在：

- CLI 与 TUI 测试覆盖率偏低，且不少测试仍是“近似覆盖”而非真实端到端覆盖
- 公共 API 文档覆盖率明显不足，与项目规则不一致
- 部分核心文件存在长函数、重复逻辑和局部复杂度偏高问题
- `allow(...)` 抑制在 CLI 层分布较多，说明 lint 债务尚未清理完毕
- 当前工作区实际状态下，`cargo clippy --all-targets --all-features -- -D warnings -W clippy::pedantic` 不通过，说明 lint 纪律尚未真正闭环

综合来看，项目已经脱离“原型期”，进入“可维护但尚未 fully hardened”的阶段，适合下一步将重点从“功能推进”转向“质量门与可演化性建设”。

---

## 二、分析方法与样本说明

本次审查读取了以下关键输入：

- `specs/0007-testing-strategy.md`
- `specs/0016-architecture-alignment.md`
- `Cargo.toml`
- `.github/workflows/ci.yml`
- `crates/tokenless-schema/src/`
- `crates/tokenless-cli/src/commands/`
- `crates/tokenless-cli/tests/`
- `crates/tokenless-schema/tests/`

并补充执行了静态统计与 lint 检查，包括：

- 生产 Rust 文件中 `#[cfg(test)]` 分布统计
- `pub` 项文档覆盖率粗略统计
- `#[allow(...)]` 抑制点扫描
- 长函数/大块实现分布扫描
- `cargo clippy --all-targets --all-features -- -D warnings -W clippy::pedantic`

说明：用户提示中提到的 `crates/tokenless-schema/tests/golden/` 目录当前并不存在。实际项目结构为：

- `crates/tokenless-schema/tests/golden_snapshots.rs`
- `crates/tokenless-schema/tests/determinism_tests.rs`
- `crates/tokenless-schema/tests/stress.rs`
- `crates/tokenless-schema/tests/fixtures/`
- `insta` snapshots

因此，以下分析以实际仓库状态为准。

---

## 三、测试覆盖率分析

## 3.1 按模块的测试覆盖现状

以下数据基于“生产文件是否包含同文件单元测试 `#[cfg(test)]`”进行统计。

| 模块 | 生产文件数 | 含单元测试文件数 | 文件级覆盖率 |
|---|---:|---:|---:|
| `tokenless-schema` | 9 | 7 | 77.8% |
| `tokenless-cli` | 22 | 6 | 27.3% |
| `tokenless-stats` | 7 | 6 | 85.7% |
| `tokenless-tui` | 13 | 3 | 23.1% |
| `tokenless-semantic` | 3 | 2 | 66.7% |

### 观察

- `tokenless-schema` 与 `tokenless-stats` 是当前测试质量最好的两个核心模块。
- `tokenless-cli` 的命令层与共享层覆盖明显不足。
- `tokenless-tui` 作为用户可见界面层，测试覆盖最低之一，属于明显空白区。
- `tokenless-semantic` 体量较小，覆盖尚可，但仍非完备。

## 3.2 已有测试资产

### `tokenless-schema`

已有三类集成测试：

- `crates/tokenless-schema/tests/golden_snapshots.rs`
- `crates/tokenless-schema/tests/determinism_tests.rs`
- `crates/tokenless-schema/tests/stress.rs`

覆盖特点：

- golden snapshots：验证压缩输出稳定性与回归控制
- determinism：100 次重复执行一致性
- idempotency：重复压缩行为一致性
- stress：大输入下不崩溃、不 OOM、输出仍合法
- format router：策略选择逻辑稳定性

这说明核心压缩路径已经具备“功能 + 回归 + 稳定性”三层测试意识，是项目质量最成熟的区域。

### `tokenless-cli`

已有四个集成测试文件：

- `crates/tokenless-cli/tests/handler_integration.rs`
- `crates/tokenless-cli/tests/hook_protocol.rs`
- `crates/tokenless-cli/tests/serialization_regression.rs`
- `crates/tokenless-cli/tests/toon_roundtrip.rs`

覆盖特点：

- handler 逻辑的近似验证
- hook 协议 JSON 结构验证
- 序列化回归验证
- TOON round-trip 验证

但这些测试多数没有直接驱动 CLI 二进制，也没有真实验证 `clap` 参数解析、stdin/stdout、退出码和 hook 协议端到端行为，属于“中层测试”而非“完整 CLI E2E 测试”。

### `tokenless-stats`

单元测试覆盖主要集中在：

- `record.rs`
- `recorder.rs`
- `tokenizer.rs`
- `config.rs`
- `query.rs`
- `compress_log.rs`

其优势是模型稳定、行为边界清晰，适合继续扩展并发和迁移场景测试。

## 3.3 缺少测试的关键文件

### `tokenless-cli` 中未发现单元测试的关键文件

- `crates/tokenless-cli/src/commands/compress.rs`
- `crates/tokenless-cli/src/commands/demo.rs`
- `crates/tokenless-cli/src/commands/env_check_cmd.rs`
- `crates/tokenless-cli/src/commands/init_cmd.rs`
- `crates/tokenless-cli/src/commands/mcp_cmd.rs`
- `crates/tokenless-cli/src/commands/rewrite.rs`
- `crates/tokenless-cli/src/commands/stats.rs`
- `crates/tokenless-cli/src/commands/toon.rs`
- `crates/tokenless-cli/src/commands/tui.rs`
- `crates/tokenless-cli/src/main.rs`
- `crates/tokenless-cli/src/shared.rs`
- `crates/tokenless-cli/src/env_check/checker.rs`
- `crates/tokenless-cli/src/env_check/fixer.rs`
- `crates/tokenless-cli/src/env_check/spec.rs`

### `tokenless-tui` 中未发现单元测试的主要文件

- `crates/tokenless-tui/src/lang.rs`
- `crates/tokenless-tui/src/lib.rs`
- `crates/tokenless-tui/src/ui/agent_detail.rs`
- `crates/tokenless-tui/src/ui/agents.rs`
- `crates/tokenless-tui/src/ui/config.rs`
- `crates/tokenless-tui/src/ui/dashboard.rs`
- `crates/tokenless-tui/src/ui/detail.rs`
- `crates/tokenless-tui/src/ui/help.rs`
- `crates/tokenless-tui/src/ui/records.rs`
- `crates/tokenless-tui/src/ui/trends.rs`

### 结论

当前项目的测试重心明显集中在“核心库”，而非“用户交互层”和“命令编排层”。这对库质量有利，但对发布质量和使用体验仍存在风险。

---

## 四、测试质量分析

## 4.1 做得好的部分

### 1. 核心压缩路径不仅测 happy path，也测稳定性

`tokenless-schema` 的测试已经超出基础功能验证，进入以下层级：

- golden snapshots：防止输出格式悄然漂移
- determinism 100x：保证多次执行结果稳定
- idempotency：验证重复执行性质
- stress：验证较大输入下的健壮性
- route selection：验证策略选择一致性

这是比较成熟的测试设计。

### 2. TOON round-trip 覆盖了多语言和特殊字符

`crates/tokenless-cli/tests/toon_roundtrip.rs` 已覆盖：

- 嵌套对象
- 数组
- CJK 文本
- emoji
- 特殊字符
- mixed CJK + ASCII + emoji
- RTL placeholder
- zero-width joiner
- 数值场景
- 空值场景

这类测试对于“透明压缩/编码工具”非常重要，因为编码边界常常出现在 Unicode 和结构交互处。

### 3. hook 协议已具备基础结构回归测试

`hook_protocol.rs` 验证了 Claude / Cursor / Gemini / Copilot 等不同协议 JSON 形状的稳定性，说明项目已意识到多 agent 兼容性的回归风险。

## 4.2 质量不足的部分

### 1. CLI 集成测试仍偏“近似覆盖”

`handler_integration.rs` 文件头已经明确说明：

- 由于 `tokenless-cli` 仍是 binary crate
- handler 无法被 integration test 直接 import
- 所以测试转而复用 `tokenless_schema` API 模拟 handler 行为

这意味着：

- 测到的是“相似逻辑”
- 不一定测到真实 handler 中的参数组合、输出格式、stdin/stdout、错误码
- 一旦 CLI handler 与 schema API 使用方式出现偏差，当前测试不一定能及时发现

### 2. hook 协议测试主要是 JSON shape 测试，不是 E2E

`hook_protocol.rs` 当前主要做了：

- 结构字段存在性断言
- JSON 可解析性断言
- BOM 处理的静态字符串测试

但没有直接运行：

- `tokenless hook rewrite`
- `tokenless hook compress`
- `tokenless hook diff`

也没有真实验证 stdin 输入到 stdout 输出的协议往返。这意味着协议层风险仍未完全封住。

### 3. stress 测试规模仍偏小

`stress.rs` 当前主要测试：

- 1000 keys 的对象
- 大约 100KB 级别输入
- 中等深度嵌套
- 大数组长字符串

这足以验证“不会立刻崩”，但仍明显低于 `specs/0007-testing-strategy.md` 中提到的：

- 100MB+ 大输入
- streaming/stress 场景
- 更接近真实 agent 工具输出的超大 JSON

### 4. 并发与迁移路径测试不足

根据现有文件，以下测试仍明显不足或缺失：

- `tokenless-stats` 并发 record/query 访问
- SQLite schema 升级/迁移回放
- env-check auto-fix 的真实执行与失败路径
- Windows 路径分隔符、CRLF、BOM、PowerShell 输出格式
- `mcp` 服务层协议回归

---

## 五、测试覆盖缺口矩阵

| 区域 | 当前状态 | 风险等级 | 缺口描述 | 建议优先级 |
|---|---|---|---|---|
| Schema core | 较强 | 低 | 已有 golden/determinism/stress，但超大输入仍不足 | P2 |
| Response core | 较强 | 低 | 大体完善，但极限规模与 profile 组合仍可补充 | P2 |
| CLI command handlers | 偏弱 | 高 | 多数命令模块无单测，无真实二进制 E2E | P0 |
| Hook protocol | 中等 | 高 | 主要是结构测试，不是真正 stdin→stdout 回归 | P0 |
| Env check | 偏弱 | 高 | checker/fixer/spec 缺少成体系测试，特别是 auto-fix | P0 |
| MCP server | 偏弱 | 高 | 缺少 JSON-RPC 协议级测试 | P1 |
| Stats concurrency | 偏弱 | 中 | 缺少并发写读与锁竞争测试 | P1 |
| Stats migration | 偏弱 | 中 | 缺少旧 DB 升级验证 | P1 |
| TUI UI 层 | 很弱 | 中 | 页面/组件几乎无快照或交互测试 | P1 |
| Windows/BOM/CRLF | 局部 | 中 | 有部分 BOM 测试，但非真实平台回归 | P1 |
| Very large input | 局部 | 中 | 仅 100KB 级，未覆盖更大规模 | P2 |

---

## 六、文档覆盖率分析

## 6.1 公共 API 文档覆盖率

基于静态扫描“`pub` 项前是否紧邻 `///` 文档注释”的粗略统计：

| 模块 | `pub` 项总数 | 带直接文档的 `pub` 项 | 粗略覆盖率 |
|---|---:|---:|---:|
| `tokenless-schema` | 44 | 1 | 2.3% |
| `tokenless-cli` | 18 | 9 | 50.0% |
| `tokenless-stats` | 71 | 19 | 26.8% |

### 说明

这个统计方法偏保守，但足以说明趋势：

- 公共 API 的逐项 rustdoc 覆盖明显不足
- 与项目规则“所有 public items require documentation”不一致
- `missing_docs = "warn"` 已配置，但显然尚未被当作强约束执行

## 6.2 模块级文档现状

### 做得较好的部分

- `crates/tokenless-schema/src/lib.rs` 具有较完整 crate-level 文档
- `format_router.rs` 有模块级 `//!` 文档
- `encoding/` 子模块大多也有模块级说明
- `tokenless-cli/src/main.rs` 与部分 `commands/*.rs` 带有模块说明

### 不足部分

以下核心模块缺少模块级 `//!` 文档或不够完整：

- `crates/tokenless-schema/src/response_compressor.rs`
- `crates/tokenless-schema/src/schema_compressor.rs`
- `crates/tokenless-stats` 中部分文件虽有项级文档，但模块级叙事仍偏弱

## 6.3 文档示例与可运行性

现有可见示例主要分布在：

- `crates/tokenless-schema/src/lib.rs`
- `crates/tokenless-schema/src/format_router.rs`

优点：

- 已经开始使用 rustdoc example，而不只是自然语言描述

不足：

- CI 中未看到单独的 `cargo doc --no-deps` 或 doctest 验证步骤
- 大量 builder/API 缺少 `# Errors` / `# Panics` / 使用示例
- 对用户最常用的 public builder 组合缺少“推荐用法”文档

### 结论

文档覆盖率是当前最明显的质量短板之一，尤其是库导向的 `tokenless-schema` 和 `tokenless-stats`。如果项目继续定位为“可复用 workspace 模板 + 发布 crate”，文档质量必须尽快补齐。

---

## 七、代码复杂度分析

## 7.1 过长函数与大文件热点

静态扫描显示以下实现或测试块明显偏长：

| 文件 | 热点 | 观察 |
|---|---|---|
| `crates/tokenless-schema/src/schema_compressor.rs` | 最长匹配块约 913 行；核心函数块约 416 行 | 明显复杂，且存在 `too_many_lines` 抑制 |
| `crates/tokenless-schema/src/response_compressor.rs` | `new` 相关匹配块约 248 行 | builder/default 配置与逻辑仍偏集中 |
| `crates/tokenless-schema/src/format_router.rs` | `make_shape` 测试辅助约 247 行 | 测试体量较大，辅助构造偏重 |
| `crates/tokenless-cli/src/commands/hook.rs` | 约 302 行大块实现/测试 | hook 压缩逻辑较重，职责较多 |
| `crates/tokenless-cli/src/main.rs` | `run` 约 207 行 | CLI dispatch 仍偏重，虽然已有模块拆分 |
| `crates/tokenless-stats/src/recorder.rs` | 出现 599 行、901 行大块匹配 | recorder 可能承担过多职责 |

## 7.2 重复代码与结构耦合

`schema_compressor.rs` 中已有注释直接承认：

- function-wrapper 路径
- bare-schema 路径

共享了大约 80% 逻辑，但目前尚未去重。这类重复存在以下问题：

- 修改规则时容易双处同步失误
- 测试需要重复覆盖两个近似分支
- 后续加入新 schema 规则时更容易产生行为漂移

## 7.3 圈复杂度与职责边界

虽然本次未使用正式圈复杂度工具，但从结构上可见以下热点：

- `hook_compress` 同时负责：输入读取、JSON 解析、协议分支、semantic 压缩、普通压缩、日志写入、debug dump、统计记录
- `main.rs::run` 仍承担较重命令分发职责
- `recorder.rs` 很可能混合：连接管理、统计查询、格式逻辑与测试辅助

### 结论

当前项目的复杂度问题不是“到处都复杂”，而是“少量核心文件过于集中”。这类问题适合通过小步提炼和职责下沉解决，而不是整体重写。

---

## 八、Lint 纪律分析

## 8.1 Workspace lint 策略现状

`Cargo.toml` 当前 workspace lint 关键项包括：

### Rust lint

- `unsafe_code = "forbid"`
- `missing_docs = "warn"`
- `missing_debug_implementations = "warn"`

### Clippy lint

- `unwrap_used = "warn"`
- `expect_used = "warn"`
- `unwrap_in_result = "warn"`
- `panic = "warn"`
- `todo = "deny"`
- `dbg_macro = "deny"`
- `missing_errors_doc = "warn"`
- `missing_panics_doc = "warn"`

这是一个不错的起点，但与 `specs/0016-architecture-alignment.md` 目标相比仍不完整。

## 8.2 与规格目标的差距

规格中提到希望统一到 workspace 的 lint 包括但不限于：

- `indexing_slicing`
- `cargo`
- `allow_attributes_without_reason`
- `rust_2024_compatibility`

当前根 `Cargo.toml` 中尚未看到这些规则被统一纳入 workspace 基线。

## 8.3 当前 clippy pedantic 实际状态

本次执行：

```bash
cargo clippy --all-targets --all-features -- -D warnings -W clippy::pedantic
```

结果：失败。

这点非常关键，因为它表明：

- 项目目标规则是“要求 pedantic 通过”
- 但当前真实工作树并未满足
- 这属于质量门未闭合，而非仅仅“还可以优化”

### 本次捕获到的代表性失败

- `crates/tokenless-cli/src/cache.rs:167`：`cast_precision_loss`
- `crates/tokenless-cli/src/commands/compress.rs`：多处 `needless_pass_by_value`
- `crates/tokenless-cli/src/commands/demo.rs`：多处 `write_with_newline`
- `crates/tokenless-cli/src/mcp.rs`：多处 `cast_possible_truncation`
- `crates/tokenless-cli/src/shared.rs`：`single_match_else`、`map_unwrap_or`、`needless_pass_by_value`

### 结论

当前 lint 问题的中心不在 `schema`，而主要集中在 `tokenless-cli`。

## 8.4 `allow(...)` 抑制使用情况

### 相对合理的抑制

- 测试中的 `unwrap_used` / `expect_used`
- 带 `reason = ...` 的局部 allow
- 某些静态 regex 初始化场景中的 `expect_used`

### 风险较高或需要清理的抑制

#### `tokenless-cli`

- `cache.rs`
  - `dead_code`
  - `unwrap_used`
  - `cast_possible_truncation`
  - `cast_sign_loss`
  - `expect_used`
- `commands/hook.rs`
  - `too_many_lines`
  - `needless_pass_by_value`
  - `cast_precision_loss`
  - `unwrap_used`
- `env_check/*`
  - `disallowed_methods`
  - `too_many_lines`
  - `expect_used`
- `shared.rs`
  - `ref_option`
  - `too_many_arguments`
  - 多个 `cast_*`
  - 多个 `disallowed_methods`
- `main.rs`
  - `too_many_lines`，虽然带 reason，但仍表明 dispatch 层偏重

#### `tokenless-schema`

- `schema_compressor.rs`
  - `struct_excessive_bools`
  - `too_many_lines`
  - `cast_*`
  - `double_ended_iterator_last`
- `response_compressor.rs`、`shape_analyzer.rs`、`format_router.rs`
  - 测试中局部 `unwrap_used`/`expect_used`

### 总评

- `schema` 侧的 allow 以“测试便利性”或“带原因的局部豁免”为主，整体可接受。
- `cli` 侧的 allow 更像“待偿还 lint 债务”，需要作为专项清理。

---

## 九、CI/CD 健康度分析

## 9.1 当前 CI 已具备的优点

`.github/workflows/ci.yml` 当前已覆盖：

### 多平台基础检查

- Ubuntu
- macOS
- Windows

并在三平台执行：

- `cargo build --release --workspace`
- `cargo test --workspace`
- `cargo clippy --all-targets --all-features -- -D warnings -W clippy::pedantic`
- `cargo fmt --check`

### 安全与供应链

独立 `audit` job 包括：

- `cargo deny check`
- `cargo audit`
- `gitleaks`

### 兼容性

独立 `msrv` job：

- 从 `Cargo.toml` 提取 `rust-version`
- 用该版本运行 `cargo check --workspace --all-targets --all-features`

### 工程一致性

- `make check-agent-sync`
- Linux-only `typos`

这套 CI 相比一般中小型 Rust 项目已算比较完整，尤其在安全和 MSRV 意识上明显优于平均水平。

## 9.2 当前 CI 的缺失项

### 1. 覆盖率门禁缺失

当前没有：

- `cargo llvm-cov`
- `tarpaulin`
- diff coverage
- crate-level coverage trend

这导致测试“有多少”只能靠感知，不能靠门槛控制。

### 2. 文档验证缺失

当前未看到：

- `cargo doc --no-deps`
- doctest 显式验证

对于一个含公开 crate 的 workspace，这是明显空白。

### 3. nextest 缺失

当前仍使用 `cargo test --workspace`，没有：

- `cargo nextest run`

当测试规模继续增长时，`nextest` 将显著改善稳定性和执行效率。

### 4. cross-compile / release-smoke 缺失

虽然 CI 在三平台本机 build，但未看到：

- Linux musl 目标
- `aarch64-unknown-linux-gnu`
- release artifact smoke test

对 CLI 工具而言，这会影响发布前信心。

### 5. 性能回归门禁缺失

当前没有：

- benchmark job
- golden performance baseline
- 压缩率/吞吐回归阈值

对于“透明压缩工具”来说，压缩率和吞吐本身就是产品质量的一部分。

## 9.3 CI 健康度总结

### 现状评价：8/10

优点：

- 三平台、MSRV、安全、格式、lint、测试齐全

短板：

- 没有 coverage
- 没有 docs gate
- 没有 nextest
- 没有性能回归基线

整体属于“成熟基础 CI”，但还不是“质量驱动型 CI”。

---

## 十、各维度综合发现摘要

| 维度 | 评分 | 结论 |
|---|---:|---|
| 测试覆盖率 | 7/10 | 核心库较强，CLI/TUI 明显不足 |
| 测试质量 | 7/10 | schema 测试成熟，CLI 多为近似覆盖 |
| 文档覆盖率 | 4/10 | 公共 API 文档明显不足 |
| 代码复杂度 | 6/10 | 少数热点文件复杂度过高 |
| Lint 纪律 | 5/10 | 有基线，但 pedantic 现实中未通过 |
| CI/CD 健康度 | 8/10 | 基础完整，缺 coverage/docs/perf gate |

---

## 十一、质量改进与新功能规划

以下建议按优先级、复杂度、预期影响进行规划。

## 11.1 建议一：建立真实 CLI 端到端测试层

### 内容

引入基于二进制的测试体系，例如：

- `assert_cmd`
- `predicates`
- `insta`

直接测试以下命令：

- `tokenless compress-schema`
- `tokenless compress-response`
- `tokenless compress-auto`
- `tokenless hook rewrite`
- `tokenless hook compress`
- `tokenless env-check`
- `tokenless mcp start`

### 优先级

- P0

### 实现复杂度

- 中

### 预期影响

- 高

### 价值

这项改进能直接补齐目前最大的测试真实性缺口，尤其是：

- `clap` 参数解析
- stdin/stdout
- 错误码
- hook 协议往返
- 平台差异

---

## 11.2 建议二：清零 CLI pedantic debt

### 内容

以 `tokenless-cli` 为中心，分批修复当前 clippy 失败：

- `needless_pass_by_value`
- `write_with_newline`
- `cast_possible_truncation`
- `cast_precision_loss`
- `single_match_else`
- `map_unwrap_or`

并同步减少 `allow(...)` 抑制点。

### 优先级

- P0

### 实现复杂度

- 中

### 预期影响

- 高

### 价值

这是让“代码质量要求”真正从文档落到现实状态的关键步骤。否则 CI 中的 pedantic 检查会持续成为“名义门槛”。

---

## 11.3 建议三：补齐公共 API rustdoc 与 doctest

### 内容

优先补齐以下 crate：

- `tokenless-schema`
- `tokenless-stats`

覆盖内容包括：

- 所有 `pub fn`、`pub struct`、`pub enum`
- builder 方法说明
- 关键类型示例
- `# Errors` / `# Panics`
- 推荐使用方式

并在 CI 中加入：

- `cargo doc --no-deps`
- doctest 执行

### 优先级

- P0

### 实现复杂度

- 中

### 预期影响

- 高

### 价值

该项目本身定位为模板/workspace/crate 组合体，公共 API 文档质量直接决定复用价值和维护成本。

---

## 11.4 建议四：新增覆盖率基线与 diff coverage 门禁

### 内容

引入：

- `cargo llvm-cov`
- 行覆盖率、文件覆盖率、crate 覆盖率统计
- PR diff coverage 门槛

建议先从阈值较低的基线开始，例如：

- 总体覆盖率 ≥ 60%
- `tokenless-schema` ≥ 80%
- `tokenless-stats` ≥ 75%
- 新增/修改文件 diff coverage ≥ 80%

### 优先级

- P1

### 实现复杂度

- 中

### 预期影响

- 高

### 价值

将“测试强弱”从主观判断变为客观数据，避免质量随功能开发而回落。

---

## 11.5 建议五：引入 TUI 快照测试与 stats 并发测试工具

### 内容

拆成两个质量增强工具：

1. TUI snapshot harness
   - 基于 `ratatui` 输出快照
   - 覆盖 dashboard、records、trends、detail 等主视图

2. stats 并发/迁移验证套件
   - 多线程并发写入
   - schema 升级回放
   - 锁竞争与恢复

### 优先级

- P1

### 实现复杂度

- 中到高

### 预期影响

- 中到高

### 价值

这两项能补齐当前“界面层”和“持久化层”最主要的质量盲区。

---

## 十二、CI/CD 增强计划

## 12.1 短期增强（1 个迭代）

### 增加覆盖率 job

新增 `coverage` job：

- 安装 `cargo-llvm-cov`
- 运行 workspace coverage
- 产出 summary 和 artifact

### 增加 docs 验证 job

新增 `docs` job：

- `cargo doc --workspace --no-deps`
- doctest 验证

### 增加 nextest job

新增 `nextest` job：

- `cargo nextest run --workspace --all-features`

## 12.2 中期增强（2-3 个迭代）

### 增加 cross-compile smoke

建议至少验证：

- `x86_64-unknown-linux-musl`
- `aarch64-unknown-linux-gnu`

### 增加 benchmark/perf baseline

对关键压缩路径建立：

- 吞吐基线
- 压缩率基线
- 大输入延迟阈值

### 增加 PR quality summary

在 CI 输出中汇总：

- clippy 状态
- 覆盖率变化
- docs 变化
- 测试运行时长变化

## 12.3 长期增强

### 质量趋势化

将以下指标持续化：

- 覆盖率趋势
- lint debt 数量
- `allow(...)` 数量
- 平均测试时长
- 压缩率回归

使其从“单次检查”升级为“质量看板”。

---

## 十三、质量改进路线图

## Phase 1：补齐质量门（P0，立即执行）

### 目标

让“规则”与“现实”一致。

### 内容

1. 修复 `tokenless-cli` 当前 pedantic 失败项
2. 对 `compress` / `hook` / `env-check` 引入真实 CLI E2E 测试
3. 补齐 `tokenless-schema`、`tokenless-stats` 公共 API 文档
4. 将 docs/doctest 加入 CI

### 预期结果

- pedantic 通过
- CLI 关键路径具备真实回归保护
- 文档不再明显落后于代码

## Phase 2：补齐结构性盲区（P1）

### 目标

扩展测试与质量门覆盖面。

### 内容

1. 引入 coverage 指标与 diff coverage
2. 为 `stats` 增加并发与迁移测试
3. 为 `mcp` 增加协议级测试
4. 为 TUI 引入 snapshot 测试
5. 增加 Windows/BOM/CRLF 更真实场景回归

### 预期结果

- CLI、TUI、持久化层不再是弱覆盖区
- 质量评估有统一数据口径

## Phase 3：提升长期可维护性（P2）

### 目标

从“通过检查”升级到“可持续维护”。

### 内容

1. 拆分 `schema_compressor.rs`、`hook.rs`、`recorder.rs` 热点大文件
2. 去除 `schema_compressor` 中 80% 重复逻辑
3. 建立性能基线测试
4. 引入 cross-compile / release-smoke
5. 持续清理零散 `allow(...)`

### 预期结果

- 热点复杂度下降
- 代码演进风险降低
- 发布信心提升

---

## 十四、最终建议

如果只能优先做三件事，建议按以下顺序推进：

1. 先修 `tokenless-cli` 的 pedantic 问题，让质量门真正可用
2. 再补 CLI 端到端测试，堵住当前最大的真实回归缺口
3. 随后补齐 `tokenless-schema` 与 `tokenless-stats` 的公共 API 文档，并把 docs 检查纳入 CI

这三步完成后，`tokenless` 的整体质量将从“中上”明显跃迁到“可靠可维护”，也更符合其作为 Rust workspace 模板项目的定位。

---

## 附录：本次审查中的关键事实

- 当前工作区 `clippy pedantic` 实际不通过
- `tokenless-schema` 是测试最成熟的模块之一
- `tokenless-cli` 是 lint debt 与测试缺口最集中的模块
- `tokenless-tui` 是覆盖率最低的区域之一
- `tokenless-stats` 基础较好，但并发与迁移测试仍不足
- 用户提示中的 `crates/tokenless-schema/tests/golden/` 目录与实际仓库状态不一致，应以现有 `golden_snapshots.rs` 结构为准
