# Tokenless 设计与使用指南

## 一、项目概览

Tokenless 是一个 LLM Token 优化工具包，通过四种互补策略降低 Token 消耗：

| 策略 | 节省 | 说明 |
|------|------|------|
| Schema 压缩 | ~57% | 压缩 OpenAI Function Calling 工具定义 |
| Response 压缩 | ~26-78% | 去除 debug/null/空字段，截断字符串/数组 |
| TOON 编码 | 15-40% | JSON 转 TOON 格式 |
| 命令重写 | 60-90% | 通过 RTK 过滤 CLI 输出 |

### 架构

```
tokenless/
├── crates/
│   ├── tokenless-schema/   SchemaCompressor + ResponseCompressor
│   ├── tokenless-stats/    SQLite 指标追踪
│   └── tokenless-cli/      CLI 二进制
├── adapters/               FHS 适配器包
│   └── tokenless/
│       ├── common/                工具依赖 + env-fix 脚本
│       ├── openclaw/              OpenClaw v5 插件
│       └── hermes/                Hermes Agent 插件
├── docs/
│   ├── design/             架构设计文档
│   └── user-guide.md       本文件
└── tests/fixtures/         测试数据
```

### 依赖关系

```
rtk-registry (外部 crate)
    ↕ 命令文本改写
tokenless-schema ← tokenless-cli → tokenless-stats
    ↕ 压缩逻辑           ↕ CLI 入口    ↕ SQLite 存储
                  init 模块 (11 agents)
```

---

## 二、安装

### 源码构建

```bash
git clone <repo>
cd tokenless

# 构建 + 安装
make setup

# 或手动
make install           # 安装 binary
make adapter-install   # 安装 adapter 文件
```

安装到 `~/.local/bin/tokenless`，adapter 文件到 `~/.local/share/anolisa/adapters/tokenless/`。

### 前提条件

- Rust >= 1.85
- 命令重写需要安装 [RTK](https://github.com/TokenFleet-AI/rtk)：`cargo install rtk`

---

## 三、CLI 使用

### 3.1 Schema 压缩

压缩 OpenAI Function Calling 工具定义：

```bash
# 单文件
tokenless compress-schema -f tool.json

# 标准输入
cat tool.json | tokenless compress-schema

# 批量（JSON 数组）
tokenless compress-schema -f tools.json --batch
```

压缩效果：移除 `title`、`examples`、截断 description、去除 markdown 格式。

```json
// 压缩前
{"function": {"name": "get_weather", "description": "Very long description...", "parameters": {"properties": {"loc": {"description": "...", "examples": ["Beijing"]}}}}}

// 压缩后：description 截断到 256/160 字符，examples 移除
```

### 3.2 Response 压缩

压缩 API 返回的 JSON：

```bash
tokenless compress-response -f response.json
curl -s https://api.example.com/data | tokenless compress-response
```

压缩规则：
- 丢弃 `debug`、`trace`、`stacktrace`、`logs` 等字段
- 去除 `null` 值
- 去除空字符串 `""`、空数组 `[]`、空对象 `{}`
- 字符串截断到 512 字符
- 数组截断到 16 项
- 嵌套深度超过 8 层时截断

### 3.3 TOON 编码

```bash
echo '{"name":"Alice","age":30}' | tokenless compress-toon
# → name: Alice
# → age: 30

echo 'name: Alice\nage: 30' | tokenless decompress-toon
# → {"name":"Alice","age":30}
```

### 3.4 命令重写

```bash
tokenless rewrite "git status"
# → rtk git status

tokenless rewrite "cargo test && git push"
# → rtk cargo test && rtk git push
```

RTK 没装时回退原始命令并提示安装。

### 3.5 环境检查

```bash
# 检查单个工具
tokenless env-check --tool Shell

# 检查全部
tokenless env-check --all

# 输出清单
tokenless env-check --checklist

# 自动修复
tokenless env-check --tool Shell --fix
```

依赖声明在 `adapters/tokenless/common/tool-ready-spec.json`，支持 6 个工具类别：Shell、WebFetch、Read、Write、Git、Python。

### 3.6 统计

```bash
tokenless stats summary           # 汇总
tokenless stats list              # 最近记录
tokenless stats show <ID>         # 详情
tokenless stats enable/disable    # 开关
```

---

## 四、Agent 集成

### 4.1 快速安装

```bash
# 当前项目安装 Claude Code hooks
tokenless init

# 全局安装到 ~/.claude/settings.json
tokenless init --global

# 其他 agent
tokenless init --global --agent cursor
tokenless init --agent windsurf
```

支持的 11 个 agent：

| Agent | 配置文件路径 | 安装命令 |
|-------|-------------|---------|
| Claude Code | `.claude/settings.json` | `tokenless init` |
| Cursor | `.cursor/hooks/` | `tokenless init --global --agent cursor` |
| Windsurf | `.windsurf/settings.json` | `tokenless init --agent windsurf` |
| Cline | VS Code globalStorage | `tokenless init --agent cline` |
| Kilo Code | `.kilocode/settings.json` | `tokenless init --agent kilocode` |
| Antigravity | `.antigravity/settings.json` | `tokenless init --agent antigravity` |
| Augment | `.augment/settings.json` | `tokenless init --agent augment` |
| Hermes CLI | `.hermes/plugins/tokenless-rewrite/` | `tokenless init --agent hermes` |
| Pi | `.pi/agent/extensions/` | `tokenless init --agent pi` |
| Gemini CLI | `.gemini/` | `tokenless init --agent gemini` |
| OpenCode | `~/.opencode/plugins/tokenless/` | `tokenless init --global --agent opencode` |

### 4.2 手动配置

Claude Code 的完整 hooks 配置：

```json
{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Bash",
        "hooks": [
          {
            "type": "command",
            "command": "tokenless rewrite {{input}}"
          }
        ]
      }
    ],
    "PostToolUse": [
      {
        "matcher": "*",
        "hooks": [
          {
            "type": "command",
            "command": "tokenless compress-response"
          }
        ]
      }
    ]
  }
}
```

---

## 五、OpenClaw 插件

### 5.1 概述

OpenClaw 插件是一个 TypeScript 插件，在 **before_tool_call** 和 **tool_result_persist** 两个事件点集成 tokenless 功能。

```
OpenClaw 会话
    ↓
session_start  → 记录 sessionId 映射
    ↓
before_tool_call (priority 5)  → Tool Ready 环境预检
before_tool_call (priority 10) → RTK 命令重写（仅 exec 工具）
    ↓
工具执行
    ↓
tool_result_persist → Response 压缩 → TOON 编码
```

### 5.2 文件结构

```
adapters/tokenless/openclaw/
├── index.ts               # 插件主逻辑（TypeScript）
├── openclaw.plugin.json   # 插件清单（配置 schema）
├── package.json           # NPM 包
└── scripts/
    ├── install.sh         # 安装到 OpenClaw
    └── uninstall.sh       # 从 OpenClaw 移除
```

### 5.3 配置选项

`openclaw.plugin.json` 中定义的可配置字段：

| 选项 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `rtk_enabled` | boolean | true | 启用 RTK 命令重写 |
| `response_compression_enabled` | boolean | true | 启用 response 压缩 |
| `tool_ready_enabled` | boolean | true | 启用环境预检 |
| `toon_compression_enabled` | boolean | false | 启用 TOON 编码（需手动开启） |
| `skip_tools` | string[] | ["Read","read_file","Glob",...] | 不压缩的工具列表 |
| `verbose` | boolean | false | 详细日志 |

### 5.4 安装

```bash
# 方式一：openclaw CLI 安装
make openclaw-install

# 方式二：手动注册
openclaw plugins install adapters/tokenless/openclaw --force
openclaw gateway restart
```

### 5.5 事件处理

**before_tool_call（priority 5）— Tool Ready：**
- 调用 `tokenless env-check --tool {name} --json`
- UNKNOWN/READY → 跳过
- NOT_READY → 自动修复（fix）→ 失败时注入 `contextPrefix` 跳过重试

**before_tool_call（priority 10）— RTK 重写：**
- 仅处理 `exec` 工具
- 调用 `rtk rewrite {command}` 文本替换
- 成功 → 用重写后的 command 替换原参数（`updatedInput`）
- 失败/不支持 → 透传

**tool_result_persist — 响应压缩：**
- 跳过小于 200 字符的响应
- 跳过 `skip_tools` 列表中的工具
- 跳过 skill 文件（YAML 格式）
- 第一步：Response 压缩（去 debug/null/空，截断）
- 第二步：TOON 编码（需配置启用）
- 返回压缩后的 `{ message }`

### 5.6 行为说明

- 所有功能**优雅降级**：binary（tokenless/rtk）未安装时对应功能自动跳过
- Binary 检测结果缓存，同次会话不重复检测
- TOON 编码时保持 `toolResult` 消息结构，避免会话修复注入错误
- sessionId 通过 `session_start` 事件构建 `sessionKey → sessionId` 映射

---

## 六、Hermes Agent 插件

### 6.1 概述

Hermes 插件是一个 Python 插件，注册三个 hooks：

```
Hermes 会话
    ↓
on_session_start        → 记录 sessionId 到环境变量
    ↓
pre_tool_call           → Tool Ready 预检 → RTK 命令重写
    ↓
工具执行
    ↓
transform_tool_result   → Response 压缩 → TOON 编码
```

### 6.2 文件结构

```
adapters/tokenless/hermes/
├── __init__.py           # 插件主逻辑（Python）
├── plugin.yaml           # 插件清单
└── scripts/
    ├── install.sh        # 安装到 Hermes
    └── uninstall.sh      # 从 Hermes 移除
```

### 6.3 安装

```bash
# 自动安装
make hermes-install

# 验证
hermes plugins list
# → tokenless    enabled

# 如果未启用
hermes plugins enable tokenless
```

### 6.4 Hook 说明

**on_session_start：**
- 记录 sessionId 到环境变量 `TOKENLESS_SESSION_ID`
- 供后续 stats 记录使用

**pre_tool_call：**
- **Step 1 — Tool Ready：** 调用 `tokenless env-check --tool {name} --json`
  - UNKNOWN/READY → 跳过
  - NOT_READY → 自动修复 → 仍失败则返回 `{action: "block"}` 跳过重试
- **Step 2 — RTK 重写（仅 terminal）：**
  - 调用 `rtk rewrite {command}`
  - 版本检查 >= 0.35.0
  - 成功 → 返回 `{action: "block", message: "建议使用重写命令"}` 供 agent 重新执行

**transform_tool_result：**
- 跳过 content-retrieval 工具
- 跳过 skill 文件、非 JSON、小于 200 字符的响应
- Step 1: Response 压缩（`tokenless compress-response`）
- Step 2: TOON 编码（`tokenless compress-toon`）
- 无压缩效果时返回 `None`

### 6.5 命令重写的工作方式

Hermes 的 `pre_tool_call` 不能修改命令参数，只能 block + 建议。因此 RTK 重写需要多一次 round-trip：

```
Agent 执行: kubectl get pods
    ↓
pre_tool_call hook: rtk rewrite "kubectl get pods" → "rtk kubectl get pods"
    ↓
返回 {action: "block", message: "建议使用 rtk kubectl get pods"}
    ↓
Agent 看到提示，重新执行: rtk kubectl get pods
    ↓
RTK 过滤输出，省 85% token
```

这种 limitation 是 Hermes hook 系统的限制，不影响最终的 token 节省效果。

### 6.6 优雅降级

- `tokenless` 未安装 → 跳过所有 compression/toon/tool-ready
- `rtk` 未安装 → 跳过 rewrite
- 版本过低 → 跳过 rewrite 并记录 warning

---

## 七、工作流程比较

### Claude Code hooks（推荐）

```
Agent 执行 git status
    ↓
PreToolUse hook → tokenless rewrite → "rtk git status"
    ↓ 直接修改命令参数，零额外 round-trip
Agent 执行 rtk git status
    ↓
PostToolUse hook → tokenless compress-response
```

### OpenClaw 插件

```
Agent 执行 exec("git status")
    ↓
before_tool_call → rtk rewrite → 替换 command 参数
    ↓ 直接修改参数，零额外 round-trip
Agent 执行 rtk git status
    ↓
tool_result_persist → compress → TOON 编码
```

### Hermes 插件

```
Agent 执行 kubectl get pods
    ↓
pre_tool_call → rtk rewrite → block + 建议
    ↓ 一次额外 round-trip（Hermes hook 限制）
Agent 重新执行 rtk kubectl get pods
    ↓
transform_tool_result → compress → TOON
```

---

## 八、Crate API（Rust 库）

### tokenless-schema

```rust
use tokenless_schema::{SchemaCompressor, ResponseCompressor};

// 压缩 schema
let compressed = SchemaCompressor::new()
    .with_func_desc_max_len(200)
    .compress(&tool_json);

// 压缩 response
let compressed = ResponseCompressor::new()
    .with_truncate_arrays_at(10)
    .with_drop_nulls(true)
    .compress(&response_json);
```

### tokenless-stats

```rust
use tokenless_stats::{StatsRecorder, StatsRecord, OperationType};

let recorder = StatsRecorder::new(":memory:")?;
let record = StatsRecord::new(
    OperationType::CompressResponse,
    "my-agent".into(),
    1000,  // before_chars
    250,   // before_tokens
    500,   // after_chars
    125,   // after_tokens
);
recorder.record(&record)?;
```

---

## 九、Token Proxy 集成

作为 LLM API 中转 proxy 时：

```rust
use tokenless_schema::{SchemaCompressor, ResponseCompressor};
use tokenless_stats::{StatsRecorder, StatsRecord, OperationType};

// 请求阶段：压缩 tool schemas
let compressed_schemas = SchemaCompressor::new().compress(&tools);

// 响应阶段：压缩 response
let compressed = ResponseCompressor::new().compress(&response);

// 记录
recorder.record(&StatsRecord::new(
    OperationType::CompressResponse, "proxy", before, bt, after, at
));
```

不需要安装 RTK 或 tokenless CLI，直接用 Rust crate 依赖。

---

## 十、测试数据

```bash
cd tokenless

# Schema 压缩
tokenless compress-schema -f tests/fixtures/tool-schema.json

# Response 压缩
tokenless compress-response -f tests/fixtures/response.json
tokenless compress-response -f tests/fixtures/response-large.json

# TOON 编码
tokenless compress-toon -f tests/fixtures/response.json

# 命令重写
tokenless rewrite "git log --oneline -10"
tokenless rewrite "docker ps && cargo test"
```

`tests/fixtures/tool-schema.json` — 包含长描述的 OpenAI Function Calling schema
`tests/fixtures/response.json` — 含 debug/logs 字段的 API 响应
`tests/fixtures/response-large.json` — 含 null/空/数组的大型响应

---

## 十一、构建与开发

```bash
make build     # release 构建
make test      # 运行全部测试
make lint      # fmt + clippy
make install   # 安装到 ~/.local/bin
make setup     # 构建 + 安装 + adapter
```
