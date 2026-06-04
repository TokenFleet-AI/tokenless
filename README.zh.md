[![CI](https://github.com/TokenFleet-AI/tokenless/actions/workflows/ci.yml/badge.svg)](https://github.com/TokenFleet-AI/tokenless/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/TokenFleet-AI/tokenless)](https://github.com/TokenFleet-AI/tokenless/releases)
[![Rust 2024](https://img.shields.io/badge/Rust-2024-orange?logo=rust)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/License-Apache%202.0-blue)](LICENSE.md)

<p align="center">
  <img src="./assets/tokenless.svg" alt="tokenless" width="520">
</p>

# Tokenless

> LLM Token 优化工具包 — Schema/Response 压缩 + 智能格式路由 + 差分响应 + 预测缓存 + TOON 编码 + 命令重写 + MCP Server + 工具环境就绪检查。

English docs: [README.md](README.md). 详细设计文档见 [specs/index.md](specs/index.md) 和 [docs/index.md](docs/index.md).

**快速导航:** [环境要求](#环境要求) · [快速开始](#快速开始) · [Token 节省对比](#token-节省对比) · [CLI 使用](#cli-使用) · [架构](#架构) · [构建](#构建) · [参与贡献](#参与贡献)

Tokenless 通过多种互补策略最大限度地降低 LLM 的 Token 消耗：

- **Schema 压缩** — 压缩 OpenAI Function Calling 工具定义，在 token 进入上下文窗口前减少约 57% 的结构性开销。
- **Response 压缩** — 移除调试字段、截断字符串、限制数组大小、去除 null/空值来压缩 API/工具响应（节省约 26–78%）。
- **智能格式路由** — 根据 JSON 结构自动选择最优编码：均匀数组用 TOON HRV（50-60%）、Schema 用 Enhanced TOON（40-55%）、不规则结构用 CJSON（30-40%）。
- **差分响应** — 对重复工具调用只发送 unified diff（轮询场景如 `git status` 最多省 95%）。
- **预测缓存** — LRU + blake3 哈希，命中时跳过整个压缩计算（重复操作近乎零延迟）。
- **TOON 上下文压缩** — 将 JSON 响应编码为 TOON（面向 Token 的对象表示法）格式，结构化数据 token 减少 15–40%。
- **命令重写** — 通过 `rtk-registry` crate 委托给 [RTK](https://github.com/TokenFleet-AI/rtk) 进行命令输出过滤（70+ 命令，节省 60–90%）。
- **工具就绪检查** — 预检工具执行环境（二进制、配置、权限、网络），自动修复缺失依赖。
- **MCP Server** — JSON-RPC 2.0 over stdio，提供 7 个工具，兼容任意 MCP Agent。

## 环境要求

- **Rust** 工具链 >= 1.89（Rust 2024 版）— `cargo install` 或源码构建所需
- **RTK** 二进制 — 可选，仅命令重写所需（`cargo install rtk`），核心压缩功能无需 RTK

## 快速开始

```bash
# 1. 安装
git clone https://github.com/TokenFleet-AI/tokenless && cd tokenless && make setup

# 确保 ~/.local/bin 在 PATH 中（建议写入 ~/.bashrc 或 ~/.zshrc 持久化）
export PATH="$HOME/.local/bin:$PATH"

# 2. 一键接入 Agent（支持 Claude Code、Cursor、Windsurf 等）
tokenless init

# 3. 完成！所有 Shell 命令自动重写，响应自动压缩。
#    稍后查看节省统计：
tokenless stats summary
```

**安装方式:** `cargo install tokenless`、从 [GitHub Releases](https://github.com/TokenFleet-AI/tokenless/releases) 下载预编译二进制、或 `brew install tokenfleet/tap/tokenless`。

> 支持 **12 种 Agent**：Claude Code、Cursor、Windsurf、Cline、Kilo Code、Antigravity、Augment、Hermes CLI、Pi、Gemini CLI、OpenCode、GitHub Copilot。
> `tokenless init` 自动安装 hooks。详见 [用户指南 §4](./docs/user-guide.md#4-agent-integration)。

## Token 节省对比

| 策略 | 节省 | 说明 |
|---|---|---|
| Schema 压缩 | ~57% | 压缩 OpenAI Function Calling 工具 schema |
| Response 压缩 | ~26–78% | 压缩 API/工具响应（按内容类型而异） |
| 格式路由 | 30–60% | 自动选择 TOON HRV / Enhanced TOON / CJSON |
| TOON 上下文压缩 | 15–40% | 将 JSON 编码为 TOON 格式供 LLM 使用 |
| 差分响应 | 最多 95% | 轮询场景 unified diff 替代全量输出 |
| 预测缓存 | 近乎零延迟 | LRU + blake3 跳过重复压缩 |
| 命令重写 | 60–90% | 通过 RTK 过滤 CLI 输出（支持 70+ 命令）|
| MCP Server | 7 个工具 | JSON-RPC over stdio，任意 MCP Agent 可用 |
| 工具就绪检查 | 减少重试浪费 | 预检环境、自动修复依赖、失败归因 |
| 零运行时依赖 | — | 纯 Rust，单个静态二进制 |

## CLI 使用

### init（Agent 接入）

```bash
tokenless init                  # 安装 Claude Code hooks（项目级）
tokenless init --global         # 全局安装（所有项目生效）
tokenless init --agent cursor   # 为 Cursor 编辑器安装
```

自动将 hooks 写入 `.claude/settings.json`（或其他 Agent 的等效配置文件）。安装完成后，所有 Shell 命令自动重写、响应自动压缩。

> 详见 [用户指南 §4](./docs/user-guide.md#4-agent-integration) 了解全部 12 种 Agent 及手动配置方式。

### compress-schema / compress-response

```bash
tokenless compress-schema -f tool.json       # 压缩工具 schema
tokenless compress-response -f response.json  # 压缩 API 响应
cat tool.json | tokenless compress-schema --batch  # 批量模式
```

### compress-auto（智能格式路由）

根据 JSON 结构自动选择最优编码策略：

```bash
tokenless compress-auto -f response.json       # 自动：TOON HRV / Enhanced TOON / CJSON
```

### compress-toon / decompress-toon

```bash
echo '{"name":"Alice","age":30}' | tokenless compress-toon    # JSON → TOON
echo 'name: Alice\nage: 30' | tokenless decompress-toon       # TOON → JSON
```

### hook diff（差分响应）

```bash
# PostToolUse hook：重复工具调用时只发送 unified diff
echo '{"command":"git status","output":"M src/main.rs\n"}' | tokenless hook diff
# 阈值可配置：TOKENLESS_DIFF_THRESHOLD=0.7（默认）
```

### mcp start（MCP Server）

> 需先执行 `tokenless stats experimental-on` 启用实验性功能。

```bash
tokenless mcp start    # 启动 JSON-RPC 2.0 server over stdin/stdout
# 提供 7 个工具：compress_schema, compress_response, rewrite_command,
# compress_toon, decompress_toon, env_check, stats_summary
```

### demo

```bash
tokenless demo    # 运行 4 个压缩策略演示（内嵌测试数据）
```

### env-check

```bash
tokenless env-check --tool Shell         # 检查特定工具
tokenless env-check --all                # 检查全部工具
tokenless env-check --tool Shell --fix   # 自动修复缺失依赖
```

### stats

```bash
tokenless stats summary              # 汇总统计
tokenless stats list --limit 20      # 最近记录
tokenless stats show 5               # 记录详情
```

### tui（交互式仪表盘）

> 需先执行 `tokenless stats experimental-on` 启用实验性功能。

```bash
tokenless tui                        # 启动 TUI 仪表盘（中文，1s 刷新）
tokenless tui --lang en              # 英文界面
tokenless tui --refresh 3            # 3 秒刷新间隔
```

4 页终端仪表盘：Dashboard · Records · Agents · Trends。支持键盘操作、搜索、导出、时间范围筛选。详见 [用户指南 §3.11](./docs/user-guide.md#311-tui-dashboard)。

## 架构

```
tokenless/
├── crates/tokenless-schema/        # 核心库
│   ├── schema_compressor.rs        # SchemaCompressor（P1/P2/P3 增强）
│   ├── response_compressor.rs      # ResponseCompressor（6 项修复 + 广度限制）
│   ├── shape_analyzer.rs           # JSON 结构分析器
│   ├── format_router.rs            # 智能编码策略选择器
│   └── encoding/                   # 编码策略
│       ├── enhanced_toon.rs        # Enhanced TOON（类型缩写 + 约束内联）
│       ├── toon_hrv.rs             # TOON HRV（均匀数组表格式）
│       └── cjson_compact.rs        # CJSON 兜底编码
├── crates/tokenless-stats/         # 基于 SQLite 的压缩指标追踪
├── crates/tokenless-cli/           # CLI 二进制：`tokenless` 命令
│   ├── cache.rs                    # 预测缓存（LRU + blake3）+ 差分响应
│   ├── mcp.rs                      # MCP JSON-RPC Server（7 个工具）
│   └── env_check/                   # 工具环境就绪检查（并行检测）
├── adapters/tokenless/             # FHS 适配器包
├── specs/                          # 设计规格（17+ 份文档）
└── docs/                           # 用户文档
```

**命令重写** 由 [`rtk-registry`](https://github.com/TokenFleet-AI/rtk/tree/master/crates/rtk-registry) crate 在库层面完成（无需调用 RTK 二进制）：

```rust
use rtk_registry::rewrite_command;

// "git status" → Some("rtk git status")
let rewritten = rewrite_command("git status", &[], &[]);
```

RTK 二进制在运行时仍用于输出过滤——registry 只负责命令转换。

## 构建

| 目标 | 描述 |
|---|---|
| `make build` | 构建 `tokenless`（release 模式） |
| `make test` | 运行全部测试 |
| `make lint` | fmt + clippy + cargo-audit |
| `make fmt` | 格式化代码 |
| `make clean` | 清理构建产物 |

## 进一步阅读

| 内容 | 链接 |
|---|---|
| 完整使用指南（安装、CLI、插件、API） | [docs/user-guide.md](./docs/user-guide.md) |
| 设计规格（17+ 份文档）— 架构、数据流、Hook 协议、安全、测试等 | [specs/](./specs/) |
| 贡献指南 | [CONTRIBUTING.md](CONTRIBUTING.md) |

## 参与贡献

参见 [CONTRIBUTING.md](CONTRIBUTING.md) 了解开发流程、编码规范和测试指南。

## 开发者社区

- [GitHub Issues](https://github.com/TokenFleet-AI/tokenless/issues) — Bug 报告和功能请求
- [GitHub Discussions](https://github.com/TokenFleet-AI/tokenless/discussions) — 问答和想法交流

**微信开发者群：**

<p align="center">
  <img src="assets/wechat-dev-group.png" alt="微信开发者群" width="200">
</p>

<p align="center"><strong>扫码加入微信开发者群</strong></p>

<p align="center">欢迎在群内交流使用心得、反馈问题、参与功能讨论</p>

## 常见问题排查

- **`tokenless: command not found`** — 请确保 `~/.local/bin/` 在 `PATH` 中（见快速开始）。
- **TUI/MCP 提示"experimental feature"错误** — 请先执行 `tokenless stats experimental-on`。
- **Hooks 未生效** — 重新执行 `tokenless init` 并重启 Agent。
- **Stats 无数据** — 请确保统计记录已启用：`tokenless stats enable`。
- 详见 [用户指南](./docs/user-guide.md) 获取更多排查信息。

## 许可证

Apache License 2.0 — 详见 [LICENSE](LICENSE.md)。
