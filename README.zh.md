# Token-Less

**LLM Token 优化工具包** — Schema/Response 压缩 + TOON 编码 + 命令重写 + 工具环境就绪检查。

Token-Less 通过多种互补策略最大程度降低 LLM 的 Token 消耗：

- **Schema 压缩** — 压缩 OpenAI Function Calling 工具定义，在 token 进入上下文窗口前减少约 57% 的结构性开销。
- **Response 压缩** — 移除调试字段、截断字符串、限制数组大小、去除 null/空值来压缩 API/工具响应（节省约 26–78%）。
- **TOON 上下文压缩** — 将 JSON 响应编码为 TOON（面向 Token 的对象表示法）格式，结构化数据 token 减少 15–40%。
- **命令重写** — 通过 `rtk-registry` crate 委托给 [RTK](https://github.com/TokenFleet-AI/rtk) 进行命令输出过滤（70+ 命令，节省 60–90%）。
- **工具就绪检查** — 预检工具执行环境（二进制、配置、权限、网络），自动修复缺失依赖，将执行失败区分为环境问题与逻辑错误。

## Token 节省对比

| 策略 | 节省 | 说明 |
|---|---|---|
| Schema 压缩 | ~57% | 压缩 OpenAI Function Calling 工具 schema |
| Response 压缩 | ~26–78% | 压缩 API/工具响应（按内容类型而异） |
| TOON 上下文压缩 | 15–40% | 将 JSON 编码为 TOON 格式供 LLM 使用 |
| 命令重写 | 60–90% | 通过 RTK 过滤 CLI 输出（支持 70+ 命令）|
| 工具就绪检查 | 减少重试浪费 | 预检环境、自动修复依赖、失败归因 |
| 零运行时依赖 | — | 纯 Rust，单个静态二进制 |

## 架构

```
tokenless/
├── crates/tokenless-schema/        # 核心库：SchemaCompressor + ResponseCompressor
├── crates/tokenless-stats/         # 基于 SQLite 的压缩指标追踪
├── crates/tokenless-cli/           # CLI 二进制：`tokenless` 命令
├── adapters/tokenless/             # FHS 适配器包（未来：hooks、插件）
│   ├── manifest.json
│   ├── common/
│   │   ├── hooks/                  # copilot-shell / hermes hooks
│   │   ├── tool-ready-spec.json    # 工具依赖规范（4 类）
│   │   └── tokenless-env-fix.sh    # 缺失依赖自动修复脚本
│   ├── openclaw/                   # OpenClaw 插件（未来）
│   └── hermes/                     # Hermes Agent 插件（未来）
```

**命令重写** 由 [`rtk-registry`](https://github.com/TokenFleet-AI/rtk/tree/main/crates/rtk-registry) crate 在库层面完成（无需调用 RTK 二进制）：

```rust
use rtk_registry::rewrite_command;

// "git status" → Some("rtk git status")
let rewritten = rewrite_command("git status", &[], &[]);
```

RTK 二进制在运行时仍用于输出过滤——registry 只负责命令转换。

## CLI 使用

### compress-schema

压缩 OpenAI Function Calling 工具 schema：

```bash
# 从文件
tokenless compress-schema -f tool.json

# 从标准输入（单个 schema）
cat tool.json | tokenless compress-schema

# 批量模式（JSON 数组）
tokenless compress-schema -f tools.json --batch
```

### compress-response

压缩 API/工具响应：

```bash
# 从文件
tokenless compress-response -f response.json

# 从标准输入
curl -s https://api.example.com/data | tokenless compress-response
```

### compress-toon / decompress-toon

JSON 与 TOON 格式互转：

```bash
# JSON 编码为 TOON
echo '{"name":"Alice","age":30}' | tokenless compress-toon
# name: Alice
# age: 30

# TOON 解码为 JSON
echo 'name: Alice\nage: 30' | tokenless decompress-toon
# {"name":"Alice","age":30}
```

### env-check

检查工具环境就绪状态：

```bash
# 检查特定工具
tokenless env-check --tool Shell

# 检查全部工具
tokenless env-check --all

# 输出检查清单
tokenless env-check --checklist

# 检查并自动修复缺失依赖
tokenless env-check --tool Shell --fix
```

### stats

查看压缩统计：

```bash
# 汇总
tokenless stats summary

# 最近记录
tokenless stats list --limit 20

# 查看记录详情
tokenless stats show 5

# 启用/禁用记录
tokenless stats enable
tokenless stats disable
```

## 构建

| 目标 | 描述 |
|---|---|
| `make build` | 构建 `tokenless`（release 模式） |
| `make test` | 运行所有测试 |
| `make lint` | 运行 clippy 检查 |
| `make fmt` | 格式化代码 |
| `make clean` | 清理构建产物 |

## 环境要求

- **Rust** 工具链 >= 1.85（Rust 2024 版）
- **RTK** 二进制 — 命令重写输出过滤所需

## 许可证

Apache License 2.0 — 详见 [LICENSE](LICENSE.md)。
