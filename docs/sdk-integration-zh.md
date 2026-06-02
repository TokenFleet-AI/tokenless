# Tokenless SDK 集成指南

> 版本: 0.4.0 | 语言: 中文

## 概述

Tokenless 提供三个独立的 Rust crate，用于将压缩能力嵌入第三方应用：

| Crate | 用途 | 依赖数 |
|------|------|:---:|
| `tokenless-schema` | Schema 与 Response 压缩引擎 | 3 (serde_json, regex, tracing) |
| `tokenless-stats` | SQLite 压缩指标统计 | 7 (rusqlite, serde 等) |
| `tokenless-semantic` | 上下文感知字段过滤（可选 ONNX） | 3 (启用 `onnx` 后 6 个) |

所有 crate 均强制 `#![forbid(unsafe_code)]`，使用 Rust 2024 edition（MSRV 1.89.0）。

---

## 快速开始

### Cargo.toml

```toml
[dependencies]
tokenless-schema = "0.4.0"
tokenless-stats = "0.4.0"
tokenless-semantic = { version = "0.4.0", features = ["onnx"] }  # 可选
```

### 最小示例

```rust
use tokenless_schema::ResponseCompressor;

fn main() {
    let json = r#"{"name":"张三","debug":{"query_ms":42},"trace":"req-123"}"#;
    let value: serde_json::Value = serde_json::from_str(json).unwrap();

    let compressor = ResponseCompressor::new();
    let compressed = compressor.compress(&value);

    // debug 和 trace 字段自动丢弃
    println!("{}", serde_json::to_string(&compressed).unwrap());
}
```

---

## Crate 参考

### tokenless-schema

#### SchemaCompressor — 工具定义压缩

压缩 OpenAI Function Calling 的工具 Schema 定义。

```rust
use tokenless_schema::SchemaCompressor;

let compressor = SchemaCompressor::new()
    .with_func_desc_max_len(256)      // 函数描述最大字符数
    .with_param_desc_max_len(160)     // 参数描述最大字符数
    .with_func_desc_max_tokens(40)    // Token 感知限制（可选）
    .with_max_enum_items(20)          // Enum 截断（可选）
    .with_drop_titles(true)
    .with_drop_examples(true);

let tool = serde_json::json!({
    "function": {
        "name": "get_weather",
        "description": "获取指定城市的天气状况...",
        "parameters": { /* ... */ }
    }
});
let compressed = compressor.compress(&tool);
// 若无节省则返回原始值（零开销回退）
```

| Builder 方法 | 默认值 | 说明 |
|------|:---:|------|
| `with_func_desc_max_len(n)` | 256 | 函数级描述最大字符数 |
| `with_param_desc_max_len(n)` | 160 | 参数级描述最大字符数 |
| `with_func_desc_max_tokens(n)` | MAX | Token 感知软限制（CJK 自适应） |
| `with_param_desc_max_tokens(n)` | MAX | 参数级 token 感知限制 |
| `with_max_enum_items(n)` | MAX | 截断 enum 数组，标记 `x-tokenless-enum-truncated` |
| `with_drop_titles(b)` | true | 移除所有层级的 `title` |
| `with_drop_examples(b)` | true | 移除所有层级的 `examples` |
| `with_drop_markdown(b)` | true | 剥离 Markdown 格式 |

**零节省回退**：若压缩后与原值相同，返回原始对象引用。序列化比较保证可靠性。

**递归关键词**：支持 `properties`、`items`、`anyOf`、`oneOf`、`allOf`、`additionalProperties`、`patternProperties`、`$defs`、`definitions`。`$ref` 保持引用结构不变。

#### ResponseCompressor — 响应压缩

压缩 JSON API 响应：丢弃调试字段、截断长字符串/数组、移除 null/空值、限制嵌套深度。

```rust
use tokenless_schema::{ResponseCompressor, CompressionProfile};

// 标准模式: 512 字符 / 16 项 / 8 层深度
let standard = ResponseCompressor::new();

// 高保真（Shell 输出）: 4096 字符 / 128 项
let hf = ResponseCompressor::new()
    .with_profile(CompressionProfile::HighFidelity);

// 完全自定义
let custom = ResponseCompressor::new()
    .with_string_truncate(256)
    .with_array_truncate(8)
    .with_max_depth(6)
    .with_drop_nulls(true)
    .with_drop_empty_fields(false)
    .with_drop_field("custom_debug_field");
```

| Builder 方法 | 默认值 | 说明 |
|------|:---:|------|
| `with_string_truncate(n)` | 512 | 最大字符串长度（UTF-8 安全截断） |
| `with_array_truncate(n)` | 16 | 最大数组元素数 |
| `with_max_depth(n)` | 8 | 最大嵌套深度 |
| `with_drop_nulls(b)` | true | 移除 `null` 值 |
| `with_drop_empty_fields(b)` | true | 移除 `""`、`[]`、`{}` |
| `with_drop_field(name)` | — | 添加自定义丢弃字段名 |
| `with_profile(profile)` | Standard | 预设: `Standard` 或 `HighFidelity` |

**默认丢弃字段**：`debug`、`trace`、`traces`、`stack`、`stacktrace`、`logs`、`logging`

**压缩规则优先级**：调试字段清除 → 深度保护 → 数组截断 → 字符串截断 → null 清除 → 空字段清除

#### Format Router — 智能路由

根据 JSON 结构自动选择最优编码策略：

```rust
use tokenless_schema::{compress_auto, strategy_name, Strategy};

let (strategy, result) = compress_auto(&value, &original_json);
// Strategy: ToonHrv | EnhancedToon | CjsonCompact | CompressorOnly
```

| 策略 | 适用场景 | 节省率 |
|------|------|:---:|
| `ToonHrv` | 均匀对象数组 | 50-60% |
| `EnhancedToon` | 带约束的对象 | 40-55% |
| `CjsonCompact` | 不规则结构 | 30-40% |
| `CompressorOnly` | 无法编码 | 使用 ResponseCompressor |

#### 实验模式 API

控制实验性功能的全局开关（格式路由器、增强 TOON、语义压缩、TUI、MCP、缓存）：

```rust
use tokenless_schema::{set_experimental_mode, is_experimental_mode};

// 启用实验性功能
set_experimental_mode(true);
assert!(is_experimental_mode());

// 禁用实验性功能（仅保留核心压缩：schema + response + 基础 TOON）
set_experimental_mode(false);
assert!(!is_experimental_mode());
```

实验模式也通过 `TokenlessConfig` 和 `TOKENLESS_EXPERIMENTAL` 环境变量控制（见下方 [tokenless-stats 配置](#配置)）。

---

### tokenless-stats — 统计存储

SQLite 持久化的压缩指标记录与查询。

#### 记录

```rust
use tokenless_stats::{StatsRecorder, StatsRecord, OperationType};
use tokenless_stats::estimate_tokens_from_bytes;

let recorder = StatsRecorder::new("/path/to/stats.db")?;

let record = StatsRecord::new(
    OperationType::CompressResponse,  // 操作类型
    "my-agent".into(),                // Agent 标识
    before_chars,                     // 压缩前字节数
    before_tokens,                    // 压缩前 token 估算
    after_chars,                      // 压缩后字节数
    after_tokens,                     // 压缩后 token 估算
)
.with_session_id("session-abc")       // 会话 ID
.with_tool_use_id("call_xyz")         // Tool Call ID
.with_source_pid(12345)               // 源进程 ID
.with_project("my-project")           // 项目名称（多项目统计过滤）
.with_namespace("default")            // 命名空间（项目内细粒度分组）
.with_experimental_mode(true)         // 实验模式标记
.with_before_text(before_json)        // 压缩前文本
.with_after_text(after_json)          // 压缩后文本
.with_output(before_cmd, after_cmd);  // 命令输出对比（RewriteCommand）

recorder.record(&record)?;
```

#### 操作类型

```rust
pub enum OperationType {
    CompressSchema,      // Schema 压缩
    CompressResponse,    // Response 压缩
    CompressToon,        // TOON 编码
    RewriteCommand,      // RTK 命令重写
}
```

#### 查询

```rust
// 带操作类型细分的摘要
let records = recorder.all_records(Some(100))?;
println!("{}", format_summary(&records, Some("每日报告")));

// 按 ID 查询单条
let record = recorder.record_by_id(42)?.unwrap();

// 时间范围查询
let records = recorder.records_since(Some("2026-05-01"), Some("2026-06-01"))?;

// Agent 列表
let agents = recorder.all_agents()?;

// 项目列表
let projects = recorder.all_projects()?;

// 条件过滤查询
let records = recorder.records_filtered(
    Some("my-agent"),     // agent_id (可选)
    Some("compress"),     // 文本搜索 (可选)
    Some("my-project"),   // project (可选)
    Some("default"),      // namespace (可选)
    Some(50),             // limit (可选)
)?;

// 便捷封装: 按项目过滤
let records = recorder.all_records_filtered(Some("my-project"), Some(100))?;

// 时间范围 + 项目过滤
let records = recorder.records_since_filtered(
    Some("2026-05-01"), Some("2026-06-01"), Some("my-project"),
)?;

// Agent 聚合摘要
let agent_summary = recorder.agent_summary("my-agent")?;

// 项目聚合摘要
let proj = recorder.project_summary("my-project")?;

// 所有项目摘要
let all = recorder.projects_summary()?;

// 每日趋势数据
let trends = recorder.project_daily_trends(
    Some("my-project"),   // 项目名 (None = 全部)
    Some("2026-05-01"),   // 起始日期 (可选)
    Some("2026-06-01"),   // 截至日期 (可选)
)?;

// 格式化输出函数
println!("{}", format_list(&records, 20));                            // 列表视图
println!("{}", format_show(&records[0]));                             // 单条详情
println!("{}", format_diff(&records, "2026-05-01", "2026-06-01"));   // 差异报告

// 时间范围解析（支持 "today", "7d", "2026-05-01" 等）
let ts = parse_time_range("7d").unwrap();
```

#### 配置

```rust
use tokenless_stats::TokenlessConfig;

// 加载配置（~/.tokenless/config.json），不存在则用默认值
let mut config = TokenlessConfig::load();

// 控制是否启用统计记录
config.stats_enabled = true;

// 控制是否启用实验模式
// 也可通过环境变量 TOKENLESS_EXPERIMENTAL=0 禁用
config.experimental_mode = true;

// 运行时检查（会同时检查环境变量覆盖）
if config.is_experimental_enabled() {
    // 使用格式路由器、增强 TOON、语义压缩等
}

config.save()?;

// 检查配置文件是否存在
if TokenlessConfig::config_file_exists() {
    println!("使用持久化配置");
}
```

#### 公开类型

```rust
// StatsSummary — 聚合摘要
let summary = StatsSummary::from_records(&records);
println!("共 {} 条记录, 节省 {} 字符 ({} tokens)",
    summary.total_records,
    summary.chars_saved(),
    summary.tokens_saved(),
);

// AgentSummaryRow — Agent 聚合统计（由 agent_summary() 返回）
// 字段: agent_id, record_count, total_before_chars, total_after_chars,
//       total_before_tokens, total_after_tokens

// ProjectSummaryRow — 项目聚合统计（由 project_summary() / projects_summary() 返回）
// 字段: project, record_count, total_before_chars, total_after_chars,
//       total_before_tokens, total_after_tokens

// ProjectDaily — 每日趋势数据点（由 project_daily_trends() 返回）
// 字段: date, chars_saved, tokens_saved, record_count

// StatsError — 统计操作错误类型
// 变体: Database(rusqlite::Error), Io(std::io::Error)

// sanitize_stats_text() — 检查文本是否包含敏感内容（API Key 等）
// 返回 None 表示检测到敏感内容，应跳过记录
```

#### Token 估算

```rust
// 方法 1: 从 &str 文本估算（ASCII ~4 字符/token）
let tokens = estimate_tokens(text);

// 方法 2: 从字节数估算（与 estimate_tokens 等效于纯 ASCII 场景）
let tokens = estimate_tokens_from_bytes(json_string.len());

// 方法 3: CJK 感知估算（中/日/韩 1 字符 = 1 token）
let tokens = estimate_tokens_cjk_aware(text);
```

---

### tokenless-semantic — 语义感知

上下文感知的字段过滤。Level 1（关键词规则）零额外依赖；Level 2 可选 ONNX 模型。

#### Level 1: 规则匹配（默认，零依赖）

```rust
use tokenless_semantic::SemanticCompressor;

let compressor = SemanticCompressor::new();

// 根据用户上下文压缩 JSON
let result = compressor.compress(&json_value, "今天天气怎么样");
// weather 规则: 保留 temp*, wind*; 丢弃 station_id, sensor_*

// 检测上下文分类
let category = compressor.detect_category("kubectl get pods");
// → "devops"

// 检查单个字段是否保留
let kept = compressor.is_field_kept("temperature", "天气怎么样");
// → true
```

#### Level 2: ONNX 模型（可选启用）

```rust
let mut compressor = SemanticCompressor::new();
compressor.load_onnx()?;  // 首次调用自动下载 ~86MB 模型

let result = compressor.compress(&json_value, "排查线上故障");
// 计算字段名与上下文的 embedding 余弦相似度
// 相似度 < 0.3 的字段自动丢弃
// 模型不可用时自动降级到 Level 1
```

#### 内置规则领域

| 领域 | 触发词 | 保留 | 丢弃 |
|------|------|------|------|
| `weather` | weather, 天气, temperature | temp*, wind*, humid* | station_id, sensor_* |
| `devops` | k8s, kubectl, deploy, 集群 | pod*, cpu*, status* | uid, self_link, owner_ref* |
| `database` | sql, query, 查询 | query*, table*, result* | internal_* |
| `git` | git, commit, branch | branch*, status, diff* | author_*, committer_* |
| `default` | 其他 | — | debug, trace, logs, stack* |

自定义规则文件 `~/.tokenless/context_rules.toml`：

```toml
[my_domain]
keep = ["user_*", "order_*"]
drop = ["internal_*", "debug_*"]
```

---

### tokenless-stats: 历史数据管理（规划中）

> 状态: 📝 Spec 完成，待实施（含 TUI 管理面板）。详见 [0017-stats-management](../specs/0017-stats-management.md)。

```rust
use tokenless_stats::{StatsRecorder, DeleteFilter};

let recorder = StatsRecorder::new("/path/to/stats.db")?;

// 概览信息
let count = recorder.record_count()?;
let size = recorder.db_size_bytes()?;
let (earliest, latest) = recorder.time_range()?;

// 精确删除
recorder.delete_by_id(42)?;                          // 删除单条
recorder.delete_by_agent("copilot-shell")?;           // 按 Agent 删除
recorder.delete_before("90d")?;                       // 保留最近 90 天
recorder.delete_before("2026-03-01")?;                // 删除此日期之前

// 删除前备份
recorder.export_json("backup.json".as_ref())?;
recorder.export_csv("backup.csv".as_ref())?;

// 数据库维护
recorder.vacuum()?;  // 压缩数据库文件，回收空间

// 条件删除（带预览）
let filter = DeleteFilter {
    agent_id: Some("old-agent".into()),
    before: Some("2026-01-01".into()),
    ..Default::default()
};
let result = recorder.delete_where(&filter)?;
println!("删除 {} 条，释放 ~{} KB",
    result.deleted, result.freed_bytes / 1024);
```

---

## 集成模式

### 1. Agent 平台（Hook 协议）

```rust
// PostToolUse 钩子调用：每次工具执行后触发
fn handle_post_tool_use(payload: &str) -> String {
    let val: serde_json::Value = serde_json::from_str(payload).unwrap();
    let tool_name = val["tool_name"].as_str().unwrap_or("");
    let output = val["output"].as_str().unwrap_or("");
    let command = val.pointer("/tool_input/command")
        .and_then(|v| v.as_str()).unwrap_or("");

    let mut output_val: serde_json::Value = serde_json::from_str(output).unwrap();

    // 1. 语义过滤: 根据命令上下文丢弃无关字段
    let semantic = SemanticCompressor::new();
    let _ = semantic.load_onnx();
    output_val = semantic.compress(&output_val, command);

    // 2. 结构压缩
    let compressor = ResponseCompressor::new();
    output_val = compressor.compress(&output_val);

    // 3. 记录统计
    let recorder = StatsRecorder::new("stats.db").unwrap();
    let record = StatsRecord::new(
        OperationType::CompressResponse, "agent".into(),
        output.len(), est_tokens(output.len()),
        output_val.to_string().len(), est_tokens(output_val.to_string().len()),
    );
    recorder.record(&record).ok();

    serde_json::to_string(&output_val).unwrap()
}
```

### 2. API 网关 / 代理

```rust
// 拦截 Agent 与 LLM 之间的 API 响应
fn proxy_response(api_response: &str, user_query: &str) -> String {
    let value = serde_json::from_str(api_response).unwrap();

    let semantic = SemanticCompressor::new();
    let value = semantic.compress(&value, user_query);

    let compressor = ResponseCompressor::new();
    let compressed = compressor.compress(&value);

    serde_json::to_string(&compressed).unwrap()
}
```

### 3. MCP Server

```rust
fn tool_call(name: &str, args: serde_json::Value) -> String {
    match name {
        "compress_schema" => {
            let compressor = SchemaCompressor::new();
            let result = compressor.compress(&args);
            serde_json::to_string(&result).unwrap()
        }
        "compress_response" => {
            let compressor = ResponseCompressor::new();
            let result = compressor.compress(&args);
            serde_json::to_string(&result).unwrap()
        }
        _ => "{}".into(),
    }
}
```

---

## 安装

SDK crates 已发布到 [crates.io](https://crates.io)：

```bash
cargo add tokenless-schema tokenless-stats tokenless-semantic
```

启用 ONNX Level 2：

```bash
cargo add tokenless-semantic --features onnx
```

ONNX 模型文件首次运行时自动下载，或通过本地安装：

```bash
make models-install  # 复制模型文件到 ~/.tokenless/models/
```

---

## 许可证

Apache 2.0。详见 [LICENSE.md](../LICENSE.md)。

---

## 相关文档

- [用户指南](./user-guide-zh.md) — 完整 CLI 使用教程
- [架构设计](../specs/0001-architecture.md) — Crate 依赖关系图
- [安全模型](../specs/0005-security-model-design.md) — 威胁模型与输入验证
- [错误处理策略](../specs/0006-error-handling-strategy.md) — 错误类型与传播
