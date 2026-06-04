[![CI](https://github.com/TokenFleet-AI/tokenless/actions/workflows/ci.yml/badge.svg)](https://github.com/TokenFleet-AI/tokenless/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/TokenFleet-AI/tokenless)](https://github.com/TokenFleet-AI/tokenless/releases)
[![Rust 2024](https://img.shields.io/badge/Rust-2024-orange?logo=rust)](https://www.rust-lang.org/)
[![License](https://img.shields.io/github/license/TokenFleet-AI/tokenless)](LICENSE.md)

<p align="center">
  <img src="./assets/tokenless.svg" alt="tokenless" width="520">
</p>

# Tokenless

> LLM Token 优化工具包 — Schema/Response 压缩 + 智能格式路由 + 差分响应 + 预测缓存 + TOON 编码 + 命令重写 + MCP Server + 工具环境就绪检查。

English docs: [README.md](README.md). 详细设计文档见 [specs/index.md](specs/index.md) 和 [docs/index.md](docs/index.md).

**快速导航:** [Token 节省对比](#token-节省对比) · [快速开始](#快速开始) · [架构](#架构) · [CLI 使用](#cli-使用) · [构建](#构建) · [参与贡献](#参与贡献)

Tokenless 通过多种互补策略最大程度降低 LLM 的 Token 消耗：

- **Schema 压缩** — 压缩 OpenAI Function Calling 工具定义，在 token 进入上下文窗口前减少约 57% 的结构性开销。
- **Response 压缩** — 移除调试字段、截断字符串、限制数组大小、去除 null/空值来压缩 API/工具响应（节省约 26–78%）。
- **智能格式路由** — 根据 JSON 结构自动选择最优编码：均匀数组用 TOON HRV（50-60%）、Schema 用 Enhanced TOON（40-55%）、不规则结构用 CJSON（30-40%）。
- **差分响应** — 对重复工具调用只发送 unified diff（轮询场景如 `git status` 最多省 95%）。
- **预测缓存** — LRU + blake3 哈希，命中时跳过整个压缩计算（重复操作近乎零延迟）。
- **TOON 上下文压缩** — 将 JSON 响应编码为 TOON（面向 Token 的对象表示法）格式，结构化数据 token 减少 15–40%。
- **命令重写** — 通过 `rtk-registry` crate 委托给 [RTK](https://github.com/TokenFleet-AI/rtk) 进行命令输出过滤（70+ 命令，节省 60–90%）。
- **工具就绪检查** — 预检工具执行环境（二进制、配置、权限、网络），自动修复缺失依赖。
- **MCP Server** — JSON-RPC 2.0 over stdio，暴露 7 个 Tool，兼容任意 MCP Agent。

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
│   ├── mcp.rs                      # MCP JSON-RPC Server（7 个 Tool）
│   └── env_check.rs                # 工具环境就绪检查（并行检测）
├── adapters/tokenless/             # FHS 适配器包
├── specs/                          # 设计规格（14 份文档）
└── docs/                           # 用户文档
```

**命令重写** 由 [`rtk-registry`](https://github.com/TokenFleet-AI/rtk/tree/master/crates/rtk-registry) crate 在库层面完成（无需调用 RTK 二进制）：

```rust
use rtk_registry::rewrite_command;

// "git status" → Some("rtk git status")
let rewritten = rewrite_command("git status", &[], &[]);
```

RTK 二进制在运行时仍用于输出过滤——registry 只负责命令转换。

## CLI 使用

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
tokenless compress-auto -f response.json --json # 输出含策略信息
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

```bash
tokenless mcp start    # 启动 JSON-RPC 2.0 server over stdin/stdout
# 暴露 7 个 Tool：compress_schema, compress_response, rewrite_command,
# compress_toon, decompress_toon, env_check, stats_summary
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

## 构建

| 目标 | 描述 |
|---|---|
| `make build` | 构建 `tokenless`（release 模式） |
| `make test` | 运行所有测试（257 passing） |
| `make lint` | fmt + clippy + cargo-audit |
| `make fmt` | 格式化代码 |
| `make clean` | 清理构建产物 |

## 环境要求

- **Rust** 工具链 >= 1.85（Rust 2024 版）
- **RTK** 二进制 — 命令重写输出过滤所需

## 设计规格

详见 [specs/](./specs/) 目录下的 14 份设计文档，覆盖架构设计、数据流、Hook 协议、安全模型、错误处理、测试策略、部署架构、优化分析和创新路线图。

## 参与贡献

参见 [CONTRIBUTING.md](CONTRIBUTING.md) 了解开发流程、编码规范和测试指南。

## 开发者社区

<p align="center">
  <img src="assets/wechat-dev-group.png" alt="微信开发者群" width="200">
</p>

<p align="center"><strong>扫码加入微信开发者群</strong></p>

<p align="center">使用问题、功能建议、Bug 反馈 — 直接群里聊</p>

## 许可证

Apache License 2.0 — 详见 [LICENSE](LICENSE.md)。
