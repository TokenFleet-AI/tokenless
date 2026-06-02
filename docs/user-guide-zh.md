# Tokenless 设计与使用指南

> 📖 快速入口：[README](../README.md) | English: [User Guide](./user-guide.md) | 只想快速上手？跳到 [快速上手](#快速上手5-分钟)

## 目录

- [快速上手（5 分钟）](#快速上手5-分钟)
- [一、项目概览](#一项目概览)
- [二、安装](#二安装)
- [三、CLI 使用](#三cli-使用)
  - [3.1 Schema 压缩](#31-schema-压缩)
  - [3.2 Response 压缩](#32-response-压缩)
  - [3.3 TOON 编码](#33-toon-编码)
  - [3.4 自动压缩](#34-自动压缩compress-auto)
  - [3.5 命令重写](#35-命令重写)
  - [3.6 Hook 命令](#36-hook-命令)
  - [3.7 环境检查](#37-环境检查)
  - [3.8 统计](#38-统计)
  - [3.9 环境变量参考](#39-环境变量参考)
  - [3.10 TUI 仪表盘](#310-tui-仪表盘)
  - [3.11 MCP](#311-mcp)
- [四、Agent 集成](#四agent-集成)
- [五、OpenClaw 插件](#五openclaw-插件)
- [六、Hermes Agent 插件](#六hermes-agent-插件)
- [七、工作流程比较](#七工作流程比较)
- [八、Crate API（Rust 库）](#八crate-apirust-库)
- [九、Token Proxy 集成](#九token-proxy-集成)
- [十、测试数据](#十测试数据)
- [十一、构建与开发](#十一构建与开发)

## 选择你的路径

| 你是... | 从这开始 | 预计时间 |
|---------|---------|---------|
| 🚀 想在 Agent 中自动省 Token | [快速上手](#快速上手5-分钟) → `tokenless init` | 3 分钟 |
| 🔍 想先手动验证压缩效果 | [CLI 使用](#三cli-使用) → 用 fixture 跑一遍 | 10 分钟 |
| 📊 想直观查看节省效果 | [`tokenless tui`](#38-tui-仪表盘) → 交互式仪表盘 | 1 分钟 |
| 🔧 想给 OpenClaw/Hermes 集成 | [工作流程比较](#七工作流程比较) → 对应插件章 | 15 分钟 |
| 📦 想把压缩嵌入自己的系统 | [Crate API](#八crate-apirust-库) | 5 分钟 |
| 🛠 想贡献代码 | [构建与开发](#十一构建与开发) | — |

---

## 快速上手（5 分钟）

```bash
# 1. 安装
git clone https://github.com/TokenFleet-AI/tokenless && cd tokenless && make setup

# 2. 验证环境（可选但推荐）
tokenless env-check --checklist

# 3. 实际跑一次压缩，亲眼看到效果
echo '{"debug":"removed","data":{"items":[1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20]}}' | tokenless compress-response

# 4. 一键接入 Agent（Claude Code / Cursor / Windsurf ...）
tokenless init

# 5. 完成！之后所有 Shell 命令自动重写，响应自动压缩。
#    过几天回来看省了多少：
tokenless stats summary
```

> `tokenless init` 会在项目的 `.claude/settings.json`（或对应 agent 的配置文件）中安装 hooks，之后无需任何手动操作。支持 **12 个 agent**，详见 [Agent 集成](#四agent-集成)。

---

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
                  init 模块 (12 agents)
```

---

## 二、安装

### Cargo 安装（推荐）

```bash
cargo install tokenless
```

需要 Rust >= 1.85。安装到 `~/.cargo/bin/`。

### 预编译二进制

从 [GitHub Releases](https://github.com/TokenFleet-AI/tokenless/releases) 下载对应平台的二进制：

| 平台 | 包 |
|------|-----|
| macOS (Apple Silicon) | `tokenless-aarch64-apple-darwin.tar.gz` |
| macOS (Intel) | `tokenless-x86_64-apple-darwin.tar.gz` |
| Linux (x86_64) | `tokenless-x86_64-unknown-linux-musl.tar.gz` |
| Windows (x86_64) | `tokenless-x86_64-pc-windows-msvc.zip` |

解压后放入 `PATH` 即可。

### Homebrew

```bash
brew install tokenfleet/tap/tokenless
```

### 源码构建

```bash
git clone https://github.com/TokenFleet-AI/tokenless
cd tokenless
make setup
```

安装到 `~/.local/bin/tokenless`，adapter 文件到 `~/.local/share/anolisa/adapters/tokenless/`。

**开发模式**（安装到 `~/.cargo/bin/`）：

```bash
./scripts/dev-install.sh
```

功能等同于 `make install`，但将二进制文件安装到 `~/.cargo/bin/`，方便本地开发使用。

### 前提条件

- Rust >= 1.85
- 命令重写需要安装 [RTK](https://github.com/TokenFleet-AI/rtk)：`cargo install rtk`（可选——核心压缩无需 RTK）

---

## 三、CLI 使用

### 3.1 Schema 压缩

压缩 OpenAI Function Calling 工具定义：

```bash
# 单文件
tokenless compress-schema -f tool.json

# 输出压缩报告
tokenless compress-schema -f tool.json --report

# 标准输入
cat tool.json | tokenless compress-schema

# 批量（JSON 数组）
tokenless compress-schema -f tools.json --batch
```

> 所有压缩命令支持 `--project`、`--agent-id`、`--session-id`、`--tool-use-id` 选项，用于关联统计记录。

压缩效果：移除 `title`、`examples`、截断 description、去除 markdown 格式。

```json
// 压缩前
{"function": {"name": "get_weather", "description": "Very long description...", "parameters": {"properties": {"loc": {"description": "...", "examples": ["Beijing"]}}}}}

// 压缩后：description 截断到 256/160 字符，examples 移除
```

### 3.2 Response 压缩

压缩 API 返回的 JSON：

```bash
# 从文件压缩
tokenless compress-response -f response.json

# 输出压缩报告
tokenless compress-response -f response.json --report

# 带上下文的语义压缩
tokenless compress-response -f response.json --semantic --context "API response"

# 标准输入
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

### 3.4 自动压缩（compress-auto）

整合 schema + response 压缩，一步完成：

```bash
# 从文件自动检测并压缩
tokenless compress-auto -f response.json

# 输出报告
tokenless compress-auto -f response.json --report

# 关联统计
tokenless compress-auto -f response.json --project my-project --agent-id claude --session-id abc --tool-use-id 123
```

| 选项 | 说明 |
|------|------|
| `-f` / `--file` | 输入文件 |
| `--report` | 输出压缩报告 |
| `--project` | 项目名称 |
| `--agent-id` | Agent ID |
| `--session-id` | 会话 ID |
| `--tool-use-id` | 工具调用 ID |

### 3.5 命令重写

```bash
tokenless rewrite "git status"
# → rtk git status

tokenless rewrite "cargo test && git push"
# → rtk cargo test && rtk git push

# 排除特定命令
tokenless rewrite "git status && npm test" --exclude "npm"

# 透明前缀模式
tokenless rewrite "git status" --transparent-prefix "rtk"
```

RTK 没装时回退原始命令并提示安装。

### 3.6 Hook 命令

Hook 命令通过 stdin/stdout JSON 协议与 Claude Code 交互，由 Agent 的 hooks 配置自动调用。

```bash
# 命令重写 hook
tokenless hook rewrite --target claude --project <项目名>

# 响应压缩 hook
tokenless hook compress --semantic --target claude --project <项目名>

# 差分输出 hook
tokenless hook diff --target claude --project <项目名>
```

| 选项 | 说明 |
|------|------|
| `--target` | 目标 agent（claude, cursor, windsurf 等） |
| `--project` | 项目名称 |

### 3.7 环境检查

```bash
# 检查单个工具
tokenless env-check --tool Shell

# JSON 输出格式
tokenless env-check --tool Shell --json

# 检查全部
tokenless env-check --all

# 输出清单
tokenless env-check --checklist

# 自动修复
tokenless env-check --tool Shell --fix
```

依赖声明在 `adapters/tokenless/common/tool-ready-spec.json`，支持 6 个工具类别：Shell、WebFetch、Read、Write、Git、Python。

### 3.8 统计

```bash
tokenless stats summary                    # 汇总
tokenless stats summary --project my-proj  # 按项目汇总
tokenless stats summary --limit 50         # 限制条数
tokenless stats list                       # 最近记录
tokenless stats list --project my-proj --namespace default --limit 20
tokenless stats show <ID>                  # 详情
tokenless stats enable/disable             # 开关
tokenless stats clear                      # 清除所有记录
tokenless stats rewrites                   # 查看重写记录
tokenless stats status                     # 统计状态
tokenless stats diff                       # 差分对比
tokenless stats experimental-on            # 启用实验功能
tokenless stats experimental-off           # 关闭实验功能
```

### 3.9 环境变量参考

| 变量 | 默认值 | 说明 |
|------|--------|------|
| `TOKENLESS_CACHE_SIZE` | 512 | 预测缓存容量（设为 0 禁用缓存） |
| `TOKENLESS_DIFF_THRESHOLD` | 0.7 | 差分响应阈值，diff 大小超过此比例时回退到全量输出 |
| `TOKENLESS_STATS_DB` | `~/.tokenless/stats.db` | 统计数据库路径 |
| `TOKENLESS_STATS_ENABLED` | — | 环境变量方式禁用统计（设为 `0` 或 `false`） |
| `TOKENLESS_LANG` | `zh` | TUI 界面语言（`zh` 或 `en`），也会从 `LANG` 环境变量解析 |

### 3.10 TUI 仪表盘

启动交互式终端仪表盘，实时查看压缩统计数据：

```bash
tokenless tui                     # 默认：中文，1 秒刷新
tokenless tui --lang en           # 英文界面
tokenless tui --refresh 3         # 3 秒刷新间隔
```

仪表盘有 **4 个标签页**，用 `h` / `Tab`（下一个）或 `Shift+Tab`（上一个）切换：

| 标签 | 说明 |
|------|------|
| **仪表盘** | 总 Token/字符节省、各操作分类统计、最近活动 |
| **记录** | 可滚动记录列表：编号、时间、操作、代理、压缩前/后、节省量 |
| **代理** | 各 Agent 汇总：记录数、字符/Token 节省；按 `Enter` 查看操作明细 |
| **趋势** | 每日字符和 Token 节省的柱状图趋势 |

**全局快捷键：**

| 按键 | 功能 |
|------|------|
| `h` / `Tab`（下一个）, `Shift+Tab`（上一个） | 切换标签页 |
| `↑↓` / `j``k` | 滚动 / 导航 |
| `Enter` | 查看详情（记录详情或代理操作明细） |
| `d` | 从详情视图返回 |
| `/` | 搜索 / 过滤记录 |
| `t` | 切换时间范围：今天 → 本周 → 全部 |
| `p` | 切换项目过滤器 |
| `e` | 导出过滤后的记录到 JSON 文件 |
| `c` | 切换配置面板（统计、缓存、阈值、实验模式） |
| `e`（配置面板中） | 切换实验模式 |
| `?` | 切换帮助面板 |
| `q` / `Esc` | 退出 |

> TUI 与 `tokenless stats` 共享同一个 SQLite 数据库。如果看不到数据，用 `tokenless stats enable` 确认统计记录已开启。

> **前置条件**: 先运行 `tokenless stats experimental-on` 启用实验功能。

### 3.11 MCP

Tokenless 提供 MCP（Model Context Protocol）服务，供兼容的 Agent 使用。

```bash
# 启动 MCP 服务
tokenless mcp start --port 3000
```

| 选项 | 说明 |
|------|------|
| `--port` | 监听端口（默认 3456） |

> **前置条件**: 先运行 `tokenless stats experimental-on` 启用实验功能。

---

### 3.12 多项目支持

所有压缩和重写命令都支持 `--project <名称>` 标志，用于给统计记录打上项目标签。这样你就可以在同一个 SQLite 数据库中分别追踪不同项目、仓库或团队的 token 节省情况。

**按项目记录数据：**

```bash
# 给压缩操作打上项目标签
tokenless compress-schema -f tool.json --project my-api
tokenless compress-response -f resp.json --project frontend
tokenless rewrite "git push" --project devops
```

**按项目查询：**

```bash
# 按项目筛选统计
tokenless stats summary --project my-api
tokenless stats list --project my-api --limit 10
```

**TUI 项目选择器：** 在 TUI 仪表盘中按 `p` 打开项目选择器弹窗。用 `↑` `↓` 选择项目，`Enter` 应用筛选。选择"所有项目"清除筛选。选择器自动列出数据库中已有的所有项目名，无需手动注册。

**工作原理：**

- `--project` 始终是可选的。不传则记录不带项目关联。
- 项目名从已有数据中自动发现——不需要预先创建。
- TUI 状态栏显示当前项目筛选状态：筛选时显示 `[p:项目名]`，未筛选时显示 `[p:所有项目]`。
- `--namespace` 标志提供第二个分组维度（如 "production" vs "staging"）。

### 3.13 实验功能

部分功能受**实验模式**开关控制，以保持默认安装稳定轻量。关闭时，tokenless 仅使用核心压缩（Level 1 语义规则，无 format router，无 ONNX 模型）。

**受控功能：**

| 功能 | 需要实验模式 |
|------|:---:|
| TUI 仪表盘 | ✅ |
| MCP 服务器 | ✅ |
| 语义压缩 Level 2（ONNX） | ✅ |
| Format router（自动压缩） | ✅ |
| 增强 TOON 编码 | ✅ |
| `hook diff`（差异响应） | ✅ |
| 核心压缩（schema/response） | — 始终可用 |
| 命令重写 | — 始终可用 |
| 统计记录 | — 始终可用 |

**启用 / 禁用：**

```bash
# 启用所有实验功能
tokenless stats experimental-on

# 禁用（回到纯核心模式）
tokenless stats experimental-off

# 查看当前状态
tokenless stats status
# → Stats recording: enabled | Experimental mode: on
```

**TUI 切换：** 按 `c` 打开配置面板，再按 `e` 切换实验模式。修改即时生效，跨会话持久保留。

**持久化：** 实验模式设置存储在 `~/.tokenless/config.json` 中，重启和升级后保持。与统计录制开关独立——你可以录制统计而不启用实验功能。

**注意：** TUI 运行时禁用实验模式，退出后将无法重新启动。如需恢复，运行 `tokenless stats experimental-on`。

---

## 四、Agent 集成

### 4.1 快速安装

```bash
# 当前项目安装 Claude Code hooks
tokenless init

# 调试模式
tokenless init --debug

# 全局安装到 ~/.claude/settings.json
tokenless init --global

# 其他 agent
tokenless init --global --agent cursor
tokenless init --agent windsurf
```

支持的 12 个 agent：

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
| GitHub Copilot | `.github/hooks/rtk-rewrite.json` | `tokenless init --agent copilot` |

### 4.2 手动配置

Claude Code 的完整 hooks 配置（由 `tokenless init` 自动生成）：

```json
{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Bash",
        "hooks": [
          {
            "type": "command",
            "command": "tokenless hook rewrite --target claude --project <项目名>"
          }
        ]
      }
    ],
    "PostToolUse": [
      {
        "matcher": "^(?!Bash$).*",
        "hooks": [
          {
            "type": "command",
            "command": "tokenless hook compress --semantic --target claude --project <项目名>"
          }
        ]
      }
    ]
  }
}
```

> **注意**：hooks 使用的是 `tokenless hook` 子命令，不是直接调用 `tokenless rewrite` 或 `tokenless compress-response`。`hook` 子命令通过 stdin/stdout 的 JSON 协议与 Claude Code 交互，自动从 hook 输入中读取命令并输出重写后的结果。手动配置时应使用上述 `tokenless hook rewrite --target claude --project <项目名>` 和 `tokenless hook compress --semantic --target claude --project <项目名>` 命令。

---

## 五、OpenClaw 插件

> 💡 **不确定该选哪种集成方式？** 建议先阅读 [七、工作流程比较](#七工作流程比较)，了解 Claude Code hooks / OpenClaw / Hermes 三种方式的差异和适用场景。

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

> **推荐阅读顺序**：先看本章对比三种集成方式，再决定深入哪一章插件细节。

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
