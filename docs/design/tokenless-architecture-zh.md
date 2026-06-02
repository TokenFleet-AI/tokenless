# Tokenless 架构设计

## 概述

Tokenless 是一个 LLM token 优化工具包，通过 schema 压缩、response 压缩、TOON 编码、命令重写和工具环境就绪检查来降低 token 消耗。二进制名称为 `tokenless`。

参考实现：https://github.com/alibaba/anolisa/tree/main/src/tokenless

## 目标架构

```
tokenless/
├── Cargo.toml                          # Workspace 根配置
├── Makefile                            # 构建自动化
├── crates/
│   ├── tokenless-schema/               # 核心库：SchemaCompressor + ResponseCompressor
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── format_router.rs        # 格式检测与路由
│   │       ├── shape_analyzer.rs        # Schema 形状分析
│   │       ├── schema_compressor.rs     # OpenAI Function Calling schema 压缩
│   │       ├── response_compressor.rs   # JSON response 压缩
│   │       └── encoding/
│   │           ├── mod.rs
│   │           ├── cjson_compact.rs     # CJSON 紧凑编码
│   │           ├── enhanced_toon.rs     # 增强 TOON 编码
│   │           └── toon_hrv.rs          # TOON HRV 变体编码
│   ├── tokenless-stats/                # SQLite 指标追踪
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── record.rs               # StatsRecord, OperationType
│   │       ├── recorder.rs             # SQLite 存储
│   │       ├── config.rs               # TokenlessConfig
│   │       ├── query.rs                # 格式化辅助
│   │       └── tokenizer.rs            # Token 估算
│   └── tokenless-cli/                  # CLI 二进制：`tokenless` 命令
│       └── src/
│           ├── main.rs                 # CLI 入口 + 子命令
│           ├── cache.rs                # 缓存管理
│           ├── shared.rs               # 共享工具和类型
│           ├── mcp.rs                  # MCP 协议支持
│           ├── commands/
│           │   ├── mod.rs
│           │   ├── compress.rs         # Schema/Response/TOON 压缩
│           │   ├── hook.rs             # Hook 命令（Rewrite/Compress/Diff）
│           │   ├── init_cmd.rs         # 初始化命令
│           │   ├── rewrite.rs          # 命令重写
│           │   ├── stats.rs            # 统计命令
│           │   ├── toon.rs             # TOON 编解码
│           │   ├── demo.rs             # 演示模式
│           │   ├── tui.rs              # TUI 启动
│           │   ├── mcp_cmd.rs          # MCP 命令
│           │   └── config.rs           # 配置命令
│           ├── init/
│           │   └── mod.rs              # 初始化模块
│           └── env_check/
│               ├── mod.rs
│               ├── check.rs            # 环境检查逻辑
│               ├── fix.rs              # 自动修复逻辑
│               └── spec.rs             # 工具就绪规范
├── apps/
│   └── tui/                             # TUI 二进制 crate
│       └── src/
│           └── main.rs                  # TUI 应用入口
├── adapters/                           # 适配器包（已实现）
│   └── tokenless/
│       ├── common/                     # 共享 hooks、spec、commands
│       ├── openclaw/                   # OpenClaw 插件
│       └── hermes/                     # Hermes Agent 插件
```

## Workspace Cargo.toml 变更

### 新增 workspace 依赖

```toml
regex = "1.10"
clap = { version = "4", features = ["derive"] }
chrono = "0.4"
toon-format = { version = "0.5", default-features = false }
rusqlite = { version = "0.32", features = ["bundled"] }
dirs = "6.0"
libc = "0.2"

# 命令重写引擎（路径：/Users/byx/Documents/workspace/github.com/TokenFleet-AI/rtk/crates/rtk-registry）
rtk-registry = { version = "0.1.0" }
```

### 新增 workspace members

```toml
members = ["crates/*", "apps/*"]
# apps/* 已存在，新增 tokenless-schema、tokenless-stats、tokenless-cli
```

## Crate 规格

### 1. tokenless-schema

**职责**：压缩 OpenAI Function Calling 工具 schema 和 JSON API response。

**SchemaCompressor** — builder 模式结构体：
- `with_func_desc_max_len(usize)` — 默认 256
- `with_param_desc_max_len(usize)` — 默认 160
- `with_drop_examples(bool)` — 默认 true
- `with_drop_titles(bool)` — 默认 true
- `with_drop_markdown(bool)` — 默认 true
- `compress(&Value) -> Value` — 压缩工具 schema
- `compress_json_schema(&mut Value, depth)` — 递归 JSON Schema 压缩
- `truncate_description(&str, usize) -> String` — 句子边界感知的截断

**ResponseCompressor** — builder 模式结构体：
- `with_truncate_strings_at(usize)` — 默认 512
- `with_truncate_arrays_at(usize)` — 默认 16
- `with_drop_nulls(bool)` — 默认 true
- `with_drop_empty_fields(bool)` — 默认 true
- `with_max_depth(usize)` — 默认 8
- `with_add_truncation_marker(bool)` — 默认 true
- `add_drop_field(&str)` — 自定义排除字段
- `compress(&Value) -> Value` — 压缩 JSON response

**依赖**：`serde_json`, `regex`, `tracing`

### 2. tokenless-stats

**职责**：基于 SQLite 的压缩效果指标追踪。

**核心类型**：
- `OperationType` 枚举：`CompressSchema`、`CompressResponse`、`RewriteCommand`、`CompressToon`
- `StatsRecord` — 完整记录，包含压缩前后字符数、token 数、文本内容、输出对比
- `StatsRecorder` — 线程安全 SQLite 连接，支持 schema 迁移
- `StatsSummary` — 聚合指标
- `TokenlessConfig` — 持久化配置（启用/禁用统计）
- `estimate_tokens_from_bytes(usize) -> usize` — 快速 token 估算

**数据库表结构**：
```sql
CREATE TABLE stats (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp TEXT NOT NULL,
    operation TEXT NOT NULL,
    agent_id TEXT NOT NULL,
    source_pid INTEGER,
    session_id TEXT,
    tool_use_id TEXT,
    before_chars INTEGER NOT NULL,
    before_tokens INTEGER NOT NULL,
    after_chars INTEGER NOT NULL,
    after_tokens INTEGER NOT NULL,
    before_text TEXT,
    after_text TEXT,
    before_output TEXT,
    after_output TEXT
);
```

**依赖**：`serde`, `serde_json`, `chrono`, `rusqlite`, `thiserror`, `dirs`, `tracing`

### 3. tokenless-core

**职责**：共享核心类型和工具函数，供其他 crate 使用。

**核心类型**：
- 统一的错误类型和结果别名
- 跨 crate 共享的配置类型
- 公共工具函数（文本处理、文件 I/O 辅助等）

**依赖**：`serde`, `serde_json`, `thiserror`, `tracing`

### 4. tokenless-semantic

**职责**：语义分析和代码理解，为压缩和重写提供上下文感知能力。

**核心功能**：
- 代码语义分析
- 上下文感知的 token 优化
- 与压缩引擎集成

**依赖**：`tokenless-core`, `tokenless-schema`, `serde`, `serde_json`, `tracing`

### 5. tokenless-tui

**职责**：终端用户界面（TUI），提供交互式的统计查看和管理功能。

**核心功能**：
- 交互式仪表盘（dashboard）
- 统计趋势图（trends）
- 记录列表（records）
- 详细视图（detail）
- 帮助页面（help）
- 配置管理（config）
- 项目选择器（project_picker）
- 多语言支持（lang）

**依赖**：`tokenless-core`, `tokenless-stats`, `ratatui`, `crossterm`, `tracing`

### 6. tokenless-cli

**职责**：CLI 二进制，提供所有子命令。

**子命令**：
- `compress-schema [-f FILE] [--batch] [--agent-id] [--session-id] [--tool-use-id]`
- `compress-response [-f FILE] [--agent-id] [--session-id] [--tool-use-id]`
- `compress-toon [-f FILE] [--agent-id] [--session-id] [--tool-use-id]`
- `compress-auto [-f FILE] [--agent-id] [--session-id] [--tool-use-id]` — 自动检测格式并选择最佳压缩模式
- `decompress-toon [-f FILE]`
- `hook rewrite <CMD>` — 通过 rtk-registry 重写单个命令
- `hook compress <CMD>` — 压缩命令中的 token 消耗
- `hook diff <CMD>` — 对比重写前后的命令差异
- `init [--force]` — 初始化 tokenless 项目配置
- `mcp [--port PORT]` — 启动 MCP 服务器
- `demo` — 演示模式
- `tui` — 启动终端交互界面
- `env-check [--tool NAME|--all] [--fix] [--checklist] [--json]`
- `stats summary [--limit N]`
- `stats list [-l N]`
- `stats show <ID>`
- `stats clear [--yes]`
- `stats status`
- `stats enable`
- `stats disable`

**关键设计决策**：
- 零节省时输出原始内容 — 如果压缩后 token 数没有减少，输出原始不变
- Stats 静默失败 — 数据库错误不会阻断压缩输出
- Exit 码语义化 — 0=成功, 1=配置/用法错误, 2=解析/序列化错误

**env_check 模块**：
- 加载 `tool-ready-spec.json`（支持字符串或对象格式的依赖声明）
- 检查 binary 可用性、版本约束、配置文件、权限、网络连通性
- 通过 `tokenless-env-fix.sh` 脚本自动修复缺失依赖（配置驱动的安装引擎）
- 支持别名解析和大小写不敏感的工具名称
- 自动检测原生包管理器（dnf/yum/apt/apk）
- 版本比较：类 semver 格式，支持 `v` 前缀和构建后缀

### rtk-registry 集成

**源码路径**：`/Users/byx/Documents/workspace/github.com/TokenFleet-AI/rtk/crates/rtk-registry`
**依赖**：`regex 1`, `lazy_static 1.4`, `serde 1`, `which 8`

命令重写通过 `rtk-registry` 作为库依赖完成（无需 shell 调用 `rtk` 二进制）。通过 path 依赖加入 workspace：

```toml
rtk-registry = { version = "0.1.0" }
```

**tokenless-cli 使用的公开 API**：

| 函数 | 用途 |
|---|---|
| `rewrite_command(cmd, excluded, transparent_prefixes) -> Option<String>` | 将 shell 命令重写为 RTK 等效命令 |
| `classify_command(cmd) -> Classification` | 将命令分类为 Supported/Unsupported/Ignored |
| `is_rtk_installed() -> RtkInstallStatus` | 检查 RTK binary 是否可用 |

**错误处理**：`rtk-registry` 对不支持/忽略的命令返回 `None` — tokenless 应在重写结果为 `None` 时保持原始命令不变。

**依赖**：`tokenless-schema`, `tokenless-stats`, `tokenless-semantic`, `tokenless-tui`, `rtk-registry`, `dirs`, `clap`, `serde_json`, `toon-format`, `chrono`, `rusqlite`, `tracing`, `blake3`, `lru`

## 实现顺序

1. **tokenless-schema** — schema_compressor.rs + response_compressor.rs（从参考实现迁移）
2. **tokenless-stats** — record、recorder、config、query、tokenizer 模块
3. **tokenless-cli** — main.rs + env_check.rs
4. **Cargo.toml** — 更新 workspace 依赖 + 新增 crate 配置
5. **Makefile** — 构建/安装目标（未来：adapters）

## Release Profile（从参考实现复制）

```toml
[profile.release]
opt-level = 3
lto = true
codegen-units = 1
panic = "abort"
strip = true
```

## CLAUDE.md 合规说明

- 所有代码必须通过 `clippy::pedantic` 检查（`-D warnings`）
- 生产代码中禁止使用 `unwrap()`/`expect()`
- 使用 `tracing` 替代 `println!`/`dbg!`
- 所有公共项必须有文档注释
- 错误处理：library 使用 `thiserror`，应用层使用 `anyhow`
- Rust 2024 版本，`#![forbid(unsafe_code)]`（env_check 中 `libc::getuid` 需要 `unsafe` 块除外）

## 相关 Specs

- [0001 架构设计](../../specs/0001-architecture.md) — 项目架构总览（英文）
- [0002 Schema Compressor 增强方案](../../specs/0002-schema-compressor-enhancements.md)
- [0003 数据流与管道设计](../../specs/0003-data-flow-pipeline-design.md)
- [0004 Hook 协议规范](../../specs/0004-hook-protocol-spec.md)
- [0005 安全模型设计](../../specs/0005-security-model-design.md)
- [0006 错误处理策略](../../specs/0006-error-handling-strategy.md)
- [0007 测试策略](../../specs/0007-testing-strategy.md)
- [0008 部署架构](../../specs/0008-deployment-architecture.md)
- [0009 优化分析](../../specs/0009-optimization-analysis.md)
- [0010 创新路线图](../../specs/0010-innovation-roadmap.md)
