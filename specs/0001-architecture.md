# 0001 — Architecture Design

## 1. System Context

Tokenless 在 LLM Agent 生态中的位置：

```
┌──────────────────────────────────────────────────────────────────┐
│                       LLM Provider (Anthropic / OpenAI)           │
│                              ▲                                    │
│                              │ API Calls (tools + prompts)        │
│                              │                                    │
│  ┌───────────────────────────┴────────────────────────────────┐  │
│  │                   AI Coding Agent Runtime                    │  │
│  │  ┌──────────────────────────────────────────────────────┐  │  │
│  │  │ Claude Code / Cursor / Windsurf / Copilot / Gemini... │  │  │
│  │  └──────┬──────────┬──────────┬────────────┬────────────┘  │  │
│  │         │          │          │            │                │  │
│  │    ┌────┴────┐┌───┴────┐┌───┴─────┐┌────┴─────┐          │  │
│  │    │PreToolUse││ Tool   ││PostToolUse││ Session  │          │  │
│  │    │  Hook    ││ Execute││   Hook    ││  Lifecycle│         │  │
│  │    └────┬─────┘└───┬────┘└────┬─────┘└────┬─────┘          │  │
│  └─────────┼──────────┼──────────┼───────────┼────────────────┘  │
│            │          │          │           │                    │
│            ▼          ▼          ▼           ▼                    │
│  ┌──────────────────────────────────────────────────────────┐    │
│  │                     Tokenless CLI                          │    │
│  │  ┌───────────┐ ┌────────────┐ ┌────────┐ ┌────────────┐  │    │
│  │  │Schema     │ │Response    │ │Command │ │Tool Ready  │  │    │
│  │  │Compressor │ │Compressor  │ │Rewrite │ │env-check   │  │    │
│  │  └─────┬─────┘ └─────┬──────┘ └───┬────┘ └─────┬──────┘  │    │
│  │        │             │            │            │          │    │
│  │        ▼             ▼            ▼            ▼          │    │
│  │  ┌──────────────────────────────────────────────────┐    │    │
│  │  │              StatsRecorder (SQLite)                │    │    │
│  │  └──────────────────────────────────────────────────┘    │    │
│  └──────────────────────────────────────────────────────────┘    │
│                                                                   │
│  ┌──────────────────────────────────────────────────────────┐    │
│  │                RuFlo Orchestration Layer                   │    │
│  │  ┌────────┐ ┌──────────┐ ┌────────┐ ┌─────────────────┐  │    │
│  │  │Daemon  │ │Swarm     │ │Memory  │ │Neural/Learning  │  │    │
│  │  │(12     │ │(hierarch-│ │(AgentDB│ │Bridge (SONA +   │  │    │
│  │  │workers)│ │ical-mesh)│ │+HNSW)  │ │ReasoningBank)   │  │    │
│  │  └────────┘ └──────────┘ └────────┘ └─────────────────┘  │    │
│  └──────────────────────────────────────────────────────────┘    │
└──────────────────────────────────────────────────────────────────┘
```

## 2. Layered Architecture

```
Layer 5: Agent Hooks
  ├── Claude Code (PreToolUse + PostToolUse)     ← 零 round-trip 改写
  ├── Cursor (preToolUse)                        ← 零 round-trip 改写
  ├── Gemini CLI (BeforeTool)                    ← 零 round-trip 改写
  ├── Copilot (VS Code / CLI dual protocol)      ← VS Code 零 round-trip
  ├── OpenClaw (TypeScript plugin)               ← 3 事件点
  ├── Hermes (Python plugin)                     ← 3 hooks (含 block)
  ├── OpenCode (JSON plugin manifest)            ← exec-based
  └── Windsurf / Cline / KiloCode / Antigravity  ← rules 文件（行为引导）
       / Augment / Pi

Layer 4: CLI Binary (tokenless-cli)
  ├── compress-schema     → SchemaCompressor
  ├── compress-response   → ResponseCompressor
  ├── compress-toon       → TOON encode
  ├── decompress-toon     → TOON decode
  ├── rewrite             → rtk-registry
  ├── env-check           → tool-ready spec + env-fix
  ├── init                → 11-agent hook installer
  ├── hook rewrite/compress → stdin→stdout 协议适配
  └── stats *             → StatsRecorder query

Layer 3: Core Libraries
  ├── tokenless-schema    → SchemaCompressor + ResponseCompressor + FormatRouter + ShapeAnalyzer
  │   └── encoding/       → TOON HRV, Enhanced TOON, CJSON Compact strategies
  ├── tokenless-stats     → SQLite metrics + config + token estimation + query layer
  └── rtk-registry        → external: command classification + rewriting rules

Layer 2: Runtime Infrastructure
  ├── RuFlo Daemon        → 12 background workers (map/audit/optimize/...)
  ├── RuFlo Swarm         → hierarchical-mesh, 8-15 agents
  ├── RuFlo Memory        → AgentDB hybrid backend, HNSW indexing
  ├── RuVector            → Vector embeddings + Flash Attention
  └── MCP Server          → Model Context Protocol (stdio mode, 7 tools)

Layer 1: Persistence
  ├── ~/.tokenless/stats.db              → SQLite (WAL, indexed)
  ├── ~/.tokenless/config.json           → Stats enable/disable
  ├── ruvector.db                        → Vector storage
  ├── .swarm/memory.db                   → Swarm coordination state
  ├── .claude-flow/daemon-state.json     → Worker scheduling + metrics
  ├── .claude-flow/config.yaml           → Swarm topology + memory config
  └── .claude-flow/data/                 → Sessions + hooks + learning

Layer 0: User Interface
  └── tokenless-tui       → ratatui 终端仪表盘（agents/records/trends/config/help）
```

## 3. Crate Dependency Graph

```
rtk-registry (external)
    │
    │ lib dependency (no subprocess)
    ▼
tokenless-cli
    ├──▶ tokenless-schema (compression logic)
    │   ├── SchemaCompressor + ResponseCompressor
    │   ├── format_router (Strategy auto-select)
    │   ├── shape_analyzer (JsonShape, TopType)
    │   ├── encoding/ (TOON HRV, Enhanced TOON, CJSON Compact)
    │   ├── serde_json
    │   └── regex
    │
    ├──▶ tokenless-stats (metrics tracking)
    │   ├── StatsRecorder + StatsSummary + TokenlessConfig
    │   ├── query layer (rewrites/operation/trends)
    │   ├── tokenizer (estimate_tokens_from_bytes)
    │   ├── config manager
    │   ├── rusqlite (bundled SQLite)
    │   ├── chrono
    │   ├── serde / serde_json
    │   ├── thiserror
    │   └── dirs
    │
    ├──▶ rtk-registry (command rewriting)
    │
    ├── clap (CLI parsing)
    ├── toon-format (TOON encode/decode)
    ├── blake3 (cache fingerprinting)
    ├── lru (predictive cache)
    ├── libc (getuid, env_check only)
    └── tracing (structured logging)

tokenless-tui (dashboard binary)
    ├──▶ tokenless-stats (read-only query)
    ├── ratatui (terminal UI framework)
    ├── crossterm (terminal control)
    ├── compact_str (UI text)
    └── tracing (structured logging)
```

**编译产物**：
- `tokenless` — CLI 二进制（~8-12 MB release, stripped）
- `tokenless-tui` — TUI 仪表盘（ratatui 终端仪表盘）

## 4. RuFlo Cluster Integration

### 4.1 为什么集成 RuFlo

Tokenless 是 LLM Agent 工具链中的 Token 优化层，RuFlo 提供 Agent 编排和智能运维能力。二者协同：

| 关注点 | Tokenless | RuFlo |
|--------|-----------|-------|
| Token 优化 | Schema/Response 压缩、命令重写 | — |
| Agent 编排 | 11 种 Agent Hook 集成 | Swarm 拓扑、任务分配 |
| 运维监控 | StatsRecorder (SQLite) | Daemon Workers (12 个后台任务) |
| 智能学习 | 压缩策略调优（规划中） | SONA 神经学习 + HNSW 检索 |
| 安全 | 输入验证、信任边界 | Security Worker (audit) |
| 代码分析 | — | CodeGraph + map worker |

### 4.2 集群拓扑

```
┌──────────────────────────────────────────────────────────┐
│                   RuFlo Swarm: hierarchical-mesh          │
│                                                          │
│  ┌──────────────────┐                                    │
│  │  Strategic Queen  │──── 长期规划 + 任务分解             │
│  └────────┬─────────┘                                    │
│           │                                              │
│     ┌─────┴─────┐                                        │
│     │  Tactical  │──── 执行协调 + 资源调度                  │
│     │   Queen    │                                        │
│     └─────┬─────┘                                        │
│           │                                              │
│  ┌────────┼────────┬──────────┬──────────┐              │
│  ▼        ▼        ▼          ▼          ▼              │
│ coder  reviewer  tester    researcher  architect         │
│  │        │        │          │          │               │
│  └────────┴───────┼──────────┴──────────┘               │
│                   │ (mesh: peer-to-peer for specific tasks)│
└──────────────────────────────────────────────────────────┘
```

**当前配置** (`.claude-flow/config.yaml`):
```yaml
swarm:
  topology: hierarchical-mesh
  maxAgents: 15
  autoScale: true
  coordinationStrategy: consensus

memory:
  backend: hybrid        # AgentDB + RuVector
  enableHNSW: true       # 150x-12,500x 快速检索
  learningBridge:        # ADR-049: 自学习记忆
    enabled: true
    sonaMode: balanced
  memoryGraph:
    enabled: true        # PageRank 知识图谱
  agentScopes:
    enabled: true        # project / local / user 三级
```

### 4.3 Daemon Workers（12 个后台任务）

| Worker | 优先级 | 间隔 | 用途 | 与 Tokenless 的关联 |
|--------|--------|------|------|-------------------|
| `map` | normal | 15min | 代码库结构映射 | 识别可优化的工具调用模式 |
| `audit` | critical | 10min | 安全分析 | 检测 Stats DB 中的敏感数据 |
| `optimize` | high | 15min | 性能优化建议 | 分析压缩率，推荐参数调优 |
| `consolidate` | low | 30min | 记忆整理 | 压缩历史数据、去重 |
| `testgaps` | normal | 20min | 测试覆盖分析 | 发现压缩器未覆盖的边界情况 |
| `predict` | low | 10min | 预加载 | 缓存热点 schema/command |
| `document` | low | 60min | 自动文档 | 生成压缩统计报告 |
| `ultralearn` | normal | — | 深度学习 | 模式识别 |
| `deepdive` | normal | — | 深度分析 | 复杂调用链分析 |
| `refactor` | normal | — | 重构建议 | 代码改进 |
| `benchmark` | normal | — | 基准测试 | 压缩性能基准 |
| `preload` | low | — | 资源预载 | 提前加载依赖 |

### 4.4 MCP Server 配置

```json
// .mcp.json
{
  "mcpServers": {
    "ruflo": {
      "command": "npx",
      "args": ["-y", "ruflo@latest", "mcp", "start"],
      "env": {
        "CLAUDE_FLOW_MODE": "v3",
        "CLAUDE_FLOW_TOPOLOGY": "hierarchical-mesh",
        "CLAUDE_FLOW_MAX_AGENTS": "15",
        "CLAUDE_FLOW_MEMORY_BACKEND": "hybrid",
        "CLAUDE_FLOW_HOOKS_ENABLED": "true"
      },
      "autoStart": false
    }
  }
}
```

**注意**：当前环境使用 DeepSeek 代理（ANTHROPIC_BASE_URL），`agent_execute` 不可用。采用分离模式：
- RuFlo MCP 工具 → 管理（swarm_init, agent_spawn, task_create）
- Claude Code 原生 Agent → 执行（利用当前 Session 的模型路由）

## 5. Crate Specifications

### 5.1 tokenless-schema

**职责**：OpenAI Function Calling schema 和 JSON API response 压缩。

**SchemaCompressor** — builder 模式：

| 方法 | 默认值 | 说明 |
|------|--------|------|
| `with_func_desc_max_len(usize)` | 256 | 函数级描述最大字符数 |
| `with_param_desc_max_len(usize)` | 160 | 参数级描述最大字符数 |
| `with_drop_examples(bool)` | true | 移除 examples 字段 |
| `with_drop_titles(bool)` | true | 移除 title 字段 |
| `with_drop_markdown(bool)` | true | 移除描述中的 Markdown 格式 |
| `compress(&Value) -> Value` | — | 执行压缩，零节省时返回原值不变 |

**ResponseCompressor** — builder 模式：

| 方法 | 默认值 | 说明 |
|------|--------|------|
| `with_truncate_strings_at(usize)` | 512 | 字符串截断长度（UTF-8 安全） |
| `with_truncate_arrays_at(usize)` | 16 | 数组截断长度 |
| `with_drop_nulls(bool)` | true | 移除 null 值 |
| `with_drop_empty_fields(bool)` | true | 移除空字符串/数组/对象 |
| `with_max_depth(usize)` | 8 | 最大嵌套深度 |
| `add_drop_field(&str)` | — | 自定义排除字段（默认含 debug/trace/stack/logs 等 8 个） |
| `compress(&Value) -> Value` | — | 执行压缩，零节省时返回原值不变 |

**依赖**：`serde_json`, `regex`

**ShapeAnalyzer** — JSON 结构形状检测：

| 类型 | 说明 |
|------|------|
| `JsonShape` | 检测结果：uniform_array / flat_object / nested_object / mixed |
| `TopType` | 主导数据类型：string / number / boolean / object / array / null |
| `analyze(&Value) -> JsonShape` | 检测数组均匀性、嵌套深度、键基数 |

**FormatRouter** — 智能编码策略路由：

| 方法 | 说明 |
|------|------|
| `select_strategy(shape, top_type, size) -> Strategy` | 基于 JSON 形状/类型/大小自动选择 |
| `compress_auto(value) -> (Value, Strategy)` | 自动选择 + 执行压缩 |
| `strategy_name(Strategy) -> &str` | 策略名称映射 |

**Strategy 枚举**：
| 策略 | 适用场景 |
|------|---------|
| `SchemaCompressor` | 工具定义 / function calling schema |
| `ResponseCompressor` | API 响应 JSON |
| `ToonHrv` | 高可读性 TOON 编码 |
| `EnhancedToon` | 结构感知增强 TOON |
| `CjsonCompact` | 最小化紧凑格式 |

**Encoding 模块**：
| 模块 | 说明 |
|------|------|
| `toon_hrv` | 高可读性变体（保留人类可读的缩进） |
| `enhanced_toon` | 结构感知增强（type-prefix + value） |
| `cjson_compact` | 紧凑格式（最小化 whitespace） |

### 5.2 tokenless-stats

**职责**：SQLite 持久化的压缩指标追踪，静默失败不影响压缩管线。

**核心类型**：

| 类型 | 说明 |
|------|------|
| `OperationType` | `CompressSchema` / `CompressResponse` / `RewriteCommand` / `CompressToon` |
| `StatsRecord` | 完整记录：before/after chars、tokens、text、output |
| `StatsRecorder` | 线程安全 SQLite（Mutex<Connection>，WAL 模式，5s 超时） |
| `StatsSummary` | 跨记录聚合：总节省字符/token 数及百分比 |
| `TokenlessConfig` | 持久化配置（stats_enabled 开关） |
| `estimate_tokens_from_bytes` | 快速 token 估算（4 bytes/token） |

**Query 层**（新增）：

| 方法 | 说明 |
|------|------|
| `query_rewrites(limit, offset)` | 分页查询重写记录 |
| `query_by_operation(op, limit)` | 按操作类型查询 |
| `query_trend_data(hours)` | 时间序列趋势数据 |

**数据库索引**：`timestamp`, `operation`, `agent_id`, `session_id`

**迁移策略**：`ALTER TABLE ADD COLUMN IF NOT EXISTS`（捕获 "duplicate column" 错误）。

**依赖**：`serde`, `serde_json`, `chrono`, `rusqlite`, `thiserror`, `dirs`

### 5.3 tokenless-cli

**职责**：CLI 二进制入口 + 11 Agent Hook 协议适配 + 环境检查 + 初始化安装 + MCP Server + 预测缓存。

**子命令矩阵**：

| 命令 | 输入 | 输出 | 退出码 |
|------|------|------|--------|
| `compress-schema` | file/stdin JSON | stdout 压缩后 JSON | 0/1/2 |
| `compress-response` | file/stdin JSON | stdout 压缩后 JSON | 0/1/2 |
| `compress-toon` | file/stdin JSON | stdout TOON 文本 | 0/1/2 |
| `decompress-toon` | file/stdin TOON | stdout JSON | 0/1/2 |
| `rewrite <cmd>` | CLI arg/stdin | stdout 改写后命令 | 0/1/2 |
| `hook rewrite <agent>` | stdin JSON (hook协议) | stdout JSON (hook响应) | 0 |
| `hook compress` | stdin JSON | stdout 压缩后 JSON | 0 |
| `env-check` | tool-ready-spec.json | stdout text/json | 0/1 |
| `init` | — | 写入配置文件 | 1 |
| `stats *` | stats.db | stdout 表格 | 0/1 |

**关键设计决策**：
- 零节省 → 输出原始内容（不输出"压缩了但没效果"的结果）
- Stats 静默失败 → DB 错误不阻塞压缩输出（`let _ = recorder.record()`）
- Hook 命令永远不返回非零 → 代理工具循环不会被中断
- RTK 未安装 → 输出原始命令 + stderr 安装提示

**init 模块**：支持 11 种 Agent 的 Hook 自动安装，含 settings.json 合并逻辑（保留已有配置）。

**env_check 模块**：
- 6 个工具类别：Shell、WebFetch、Read、Write、Git、Python
- 5 项检查：binary、version、config、permission、network
- 自动修复：`tokenless-env-fix.sh`（配置驱动的安装引擎）

**cache 模块**（新增）：
- `PredictCache`: LRU 缓存 + blake3 哈希指纹
- 默认 512 条目，`TOKENLESS_CACHE_SIZE=0` 禁用
- 纯函数操作 → 相同输入产生相同输出 → 安全缓存
- 缓存命中跳过压缩/重写/编码计算

**mcp 模块**（新增）：
- JSON-RPC 2.0 协议实现（stdio 传输）
- 7 个 MCP tools: `compress_schema`, `compress_response`, `rewrite_command`, `compress_toon`, `decompress_toon`, `env_check`, `stats_summary`
- 通过 `tokenless mcp` 子命令启动

### 5.4 tokenless-tui（新增）

**职责**：ratatui 终端仪表盘，可视化压缩指标和系统状态。

**UI 模块**：

| 面板 | 说明 |
|------|------|
| `dashboard` | 总览：总节省、今日活动、热门操作 |
| `agents` | Agent 列表：按 agent_id 分组统计 |
| `agent_detail` | 单个 Agent 的详细记录和趋势 |
| `records` | 原始记录分页浏览 |
| `trends` | 时间序列趋势图 |
| `config` | 配置查看/编辑 |
| `help` | 帮助/快捷键 |

**语言支持**：`lang` 模块 — 多语言国际化（i18n）

**依赖**：`ratatui`, `crossterm`, `compact_str`, `tokenless-stats`（只读查询）

## 6. Data Flow

```
Agent 发起工具调用
    │
    ▼
PreToolUse Hook ──────────────────────────────────────────┐
    │                                                     │
    ├─▶ Schema Compression (BeforeModel, future)          │
    │   ├─▶ PredictCache lookup (blake3 + LRU)            │
    │   ├─▶ ShapeAnalyzer → detect JsonShape/TopType      │
    │   ├─▶ FormatRouter → select optimal Strategy        │
    │   │   ├─ auto (TOON HRV / Enhanced TOON / CJSON)    │
    │   │   └─ manual (SchemaCompressor / ResponseCompressor)
    │   └─▶ SchemaCompressor.compress() → ~57% savings    │
    │                                                     │
    ├─▶ Command Rewriting (Bash/Shell only)               │
    │   ├─▶ PredictCache lookup                           │
    │   └─▶ rtk_registry::rewrite_command()               │
    │       ├─ "git status" → "rtk git status"            │
    │       └─ RTK 未安装 → 透传 + 提示                    │
    │                                                     │
    └─▶ Environment Pre-Check (optional, --fix)           │
        └─▶ env_check::run() → READY/PARTIAL/NOT_READY    │
            └─▶ auto_fix() → tokenless-env-fix.sh         │
    │                                                     │
    ▼                                                     │
Tool Execution (rewritten command, if applicable)          │
    │                                                     │
    ▼                                                     │
PostToolUse Hook ─────────────────────────────────────────┘
    │
    ├─▶ Response Compression
    │   ├─▶ PredictCache lookup                           │
    │   ├─▶ ShapeAnalyzer → detect JsonShape/TopType      │
    │   ├─▶ FormatRouter → select optimal Strategy        │
    │   ├─▶ ResponseCompressor.compress() → ~26-78% savings
    │   │   ├─ Drop: debug/trace/stack/logs/null/empty    │
    │   │   ├─ Truncate: strings>512, arrays>16, depth>8  │
    │   │   └─ Zero-savings guard → return original       │
    │   │                                                 │
    │   ├─▶ TOON Encoding (optional, opt-in)              │
    │   │   ├─ toon_format::encode_default() → +15-40%    │
    │   │   ├─ TOON HRV (high-readability variant)        │
    │   │   ├─ Enhanced TOON (结构感知增强)               │
    │   │   ├─ CJSON Compact (最小化紧凑格式)             │
    │   │   └─ Zero-savings guard → return pre-TOON       │
    │   └─▶ PredictCache store (on miss)                  │
    │                                                     │
    └─▶ Stats Recording (fail-silent)
        └─▶ StatsRecorder.record() → SQLite
            └─ DB error → silently ignored

MCP Server Mode (stdin/stdout JSON-RPC 2.0):
    │
    ├─▶ compress_schema   → SchemaCompressor + FormatRouter
    ├─▶ compress_response → ResponseCompressor + FormatRouter
    ├─▶ rewrite_command   → rtk_registry rewrite
    ├─▶ compress_toon     → TOON encode
    ├─▶ decompress_toon   → TOON decode
    ├─▶ env_check         → environment validation
    └─▶ stats_summary     → StatsRecorder query + summary
```

## 7. Agent Integration Matrix

| Agent | 类型 | Install 路径 | 改写方式 | Round-trip | PostToolUse |
|-------|------|-------------|---------|-----------|-------------|
| Claude Code | Hook | `.claude/settings.json` | updatedInput | 0 | compress |
| Cursor | Hook | `.cursor/hooks.json` | updated_input | 0 | — |
| Gemini CLI | Hook | `.gemini/settings.json` + `.sh` | tool_input | 0 | — |
| Copilot (VS Code) | Hook | `.github/hooks/rtk-rewrite.json` | updatedInput | 0 | — |
| Copilot (CLI) | Hook | `.github/hooks/rtk-rewrite.json` | block+suggest | 1 | — |
| OpenClaw | Plugin | `openclaw plugins install` | command replace | 0 | compress + TOON |
| Hermes | Plugin | `.hermes/plugins/tokenless/` | block+suggest | 1 | compress + TOON |
| OpenCode | Plugin | `~/.opencode/plugins/tokenless/` | exec rewrite | 0 | compress |
| Pi | Extension | `.pi/agent/extensions/` | exec rewrite | 0 | — |
| Windsurf | Rules | `.windsurfrules` | 行为引导 | N/A | — |
| Cline/Roo | Rules | `.clinerules` | 行为引导 | N/A | — |
| Kilo Code | Rules | `.kilocode/rules/` | 行为引导 | N/A | — |
| Antigravity | Rules | `.agents/rules/` | 行为引导 | N/A | — |
| Augment | Rules | `.augment/rules/` | 行为引导 | N/A | — |

## 8. Configuration & State

```
优先级（从高到低）：
  1. 环境变量
     ├── TOKENLESS_STATS_DB          → 覆盖 stats DB 路径
     ├── TOKENLESS_STATS_ENABLED     → 强制开启/关闭
     ├── TOKENLESS_TOOL_READY_SPEC   → 覆盖 spec 文件路径
     ├── TOKENLESS_PACKAGE_MANAGER   → 覆盖包管理器
     └── TOKENLESS_ENV_FIX_SCRIPT    → 覆盖修复脚本路径
  2. 配置文件 (~/.tokenless/config.json)
  3. CLI flags（每次调用指定）
  4. 硬编码默认值
```

**持久化存储布局**：
```
~/.tokenless/
├── stats.db              # SQLite 压缩指标（WAL 模式）
└── config.json           # { stats_enabled: true/false }

{project}/.claude-flow/
├── config.yaml           # RuFlo 运行时配置
├── daemon-state.json     # Worker 调度状态
├── data/                 # Memory 持久化
├── logs/                 # 操作日志
├── sessions/             # 会话状态
├── agents/               # Agent 配置
└── hooks/                # 自定义 Hooks

{project}/
├── .swarm/memory.db      # Swarm 协调状态
├── ruvector.db           # 向量嵌入存储
└── .mcp.json             # MCP Server 配置
```

## 9. Security Boundaries

```
┌── Untrusted Zone ──────────────────────────┐
│  LLM output (hallucination possible)        │
│  User stdin / CLI args                      │
│  External JSON files                        │
│  Shell command text (from agent)             │
└────────────────────────────────────────────┘
                    │
            ┌───────┴───────┐
            │  Tokenless     │  ← Trust Boundary
            │  Validation:   │
            │  • serde_json  │     strict parse
            │  • depth limit │     8 levels max
            │  • str limit   │     512 chars
            │  • array limit │     16 items
            │  • no shell    │     never executes
            │  • param SQL   │     rusqlite params
            └───────┬───────┘
                    │
┌── Trusted Zone ────────────────────────────┐
│  stdout (compressed, validated output)      │
│  ~/.tokenless/stats.db (only tokenless writes)│
│  Config files (user-managed)                │
└─────────────────────────────────────────────┘
```

## 10. Architecture Optimization Opportunities

基于当前代码审查和架构分析，以下优化建议：

### P0 — 架构层面

| # | 优化项 | 收益 | 说明 |
|---|--------|------|------|
| A1 | **env_check 并行化** | 10x 速度提升 | 依赖检查串行 spawn shell → 改为 `thread::scope` 并行 |
| A2 | **消除重复序列化** | -40% 序列化开销 | `compress-schema/response` 内两次 `to_string` + 一次 `from_str` → 一次 compact |
| A3 | **CI 加入 cargo-audit** | 供应链安全 | `make lint` 目标增加 `cargo audit` |

### P1 — 代码层面

| # | 优化项 | 收益 | 说明 |
|---|--------|------|------|
| B1 | **Schema 压缩减少 JSON round-trip** | 20-30% 更快 | `.clone()` → 先检测变化再克隆，避免无效 clone |
| B2 | **LazyLock 默认 drop_fields** | 零分配默认值 | `ResponseCompressor::new()` 的 HashSet 分配改为 `LazyLock` |
| B3 | **提取 `lock_conn()` helper** | DRY + 可读性 | StatsRecorder 中 5 个方法重复 poison 处理 |
| B4 | **压缩器实例复用** | 微小但免费 | `LazyLock<SchemaCompressor>` 替代每次 `::new()` |

### P2 — 模块边界

| # | 优化项 | 收益 | 说明 |
|---|--------|------|------|
| C1 | **Schema Compressor if/else 去重** | 代码质量 | `compress()` 中 function-wrapper 和 bare-schema 两条路径近乎重复 |
| C2 | **权限检查枚举化** | Bug 预防 | `check_permission(&str)` → 用 enum 替代字符串，避免 typo 静默通过 |
| C3 | **`#[allow]` 范围缩小** | Lint 精度 | 22 个模块级 allow → 函数/块级，暴露真实问题 |

## 11. Implementation Status

| 组件 | 状态 | 备注 |
|------|------|------|
| tokenless-schema | ✅ 完成 | SchemaCompressor + ResponseCompressor |
| tokenless-schema (shape_analyzer) | ✅ 完成 | JsonShape, TopType 检测 |
| tokenless-schema (format_router) | ✅ 完成 | Strategy auto-select，基于 JSON 形状智能路由 |
| tokenless-schema (encoding) | ✅ 完成 | TOON HRV / Enhanced TOON / CJSON Compact |
| tokenless-stats | ✅ 完成 | SQLite + 迁移 + 配置 + query 层 + tokenizer |
| tokenless-cli (核心命令) | ✅ 完成 | compress/rewrite/env-check/stats |
| tokenless-cli (hook 协议) | ✅ 完成 | 5 种 Agent（Claude/Cursor/Gemini/Copilot ×2） |
| tokenless-cli (init) | ✅ 完成 | 11 Agent 自动安装 |
| tokenless-cli (MCP Server) | ✅ 完成 | 7 tools: compress_schema/response, rewrite, toon, env_check, stats |
| tokenless-cli (predictive cache) | ✅ 完成 | blake3 + LRU, TOKENLESS_CACHE_SIZE 控制 |
| tokenless-tui | ✅ 完成 | ratatui 仪表盘: agents/records/trends/config/help |
| OpenClaw 插件 | ✅ 完成 | TypeScript 插件（3 事件） |
| Hermes 插件 | ✅ 完成 | Python 插件（3 hooks） |
| TOON 编解码 | ✅ 完成 | 往返一致 + TOON HRV + Enhanced TOON + CJSON |
| RuFlo Daemon | ✅ 运行中 | 5 Workers 激活 |
| RuFlo Swarm | ✅ 初始化 | hierarchical-mesh, 8 agents |
| RuFlo Memory | ✅ 初始化 | hybrid backend, HNSW, AgentDB 16 controllers |
| MCP Server 配置 | ✅ 完成 | stdio 模式, autoStart: false |
| 版本 | ✅ v0.3.0 | 最新 release tag |
| CI/CD | ⚠️ 部分 | release-please + git cliff，缺少 cargo-audit |

## 12. Related Specs

- [0002 Schema Compressor Enhancements](./0002-schema-compressor-enhancements.md) — P1 max_enum_items, P2 token-aware truncation, P3 $ref/$defs recursion
- [0003 Data Flow & Pipeline Design](./0003-data-flow-pipeline-design.md) — 多阶段压缩管道、端到端数据流
- [0004 Hook Protocol Specification](./0004-hook-protocol-spec.md) — 11 Agent 协议完整规范
- [0005 Security Model](./0005-security-model-design.md) — 威胁模型、信任边界
- [0006 Error Handling Strategy](./0006-error-handling-strategy.md) — 优雅降级模式
- [0007 Testing Strategy](./0007-testing-strategy.md) — 测试架构与覆盖缺口
- [0008 Deployment Architecture](./0008-deployment-architecture.md) — 构建/安装/CI/CD
- [0009 Optimization Analysis](./0009-optimization-analysis.md) — 14 项优化详细分析
- [0010 Innovation Roadmap](./0010-innovation-roadmap.md) — 12 个创新方向
