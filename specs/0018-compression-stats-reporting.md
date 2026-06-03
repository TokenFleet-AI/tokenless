# 0018 — Compression Stats Reporting (agent-proxy → tokenless)

> 每个 API 请求完成后，agent-proxy-rust 将消耗统计和压缩数据回传给 tokenless，统一存储和展示。

---

## 1. 背景

当前两套统计系统各管各的：

```
tokenless stats (SQLite)          agent-proxy cost_records (SQLite)
├─ hook 压缩统计                  ├─ 上游 API token 消耗
├─ rewrite 统计                   ├─ 实际花费
└─ 实验模式效果                   └─ 代理层压缩统计（CompressMiddleware）
```

**问题**：同一个请求的压缩效果（tokenless）和实际消耗（agent-proxy）分散在两个数据库，无法关联查询。

**目标**：每个请求结束后，agent-proxy 将数据回传给 tokenless，由 tokenless 统一存储，提供完整的"该请求省了多少钱"视图。

---

## 2. 数据模型

### 2.1 回传数据格式 `RequestReport`

```json
{
  "session_id": "sess_abc123",
  "project": "/Users/baoyx/my-project",
  "model": "deepseek-v4-flash",
  "channel": "deepseek",
  "timestamp": "2026-06-02T10:30:00Z",

  "usage": {
    "input_tokens": 500,
    "output_tokens": 200,
    "cache_read_tokens": 50,
    "cache_write_tokens": 0,
    "thinking_tokens": 100
  },

  "pricing": {
    "type": "per_token",
    "input_per_mtok": 1.0,
    "output_per_mtok": 2.0,
    "unit": "CNY"
  },

  "actual_cost": 0.0012,

  "compression": {
    "proxy_schema_pre": 300,
    "proxy_schema_post": 220,
    "proxy_response_pre": 0,
    "proxy_response_post": 0,
    "total_saved": 80
  },

  "saved_cost": 0.00008
}
```

### 2.2 字段说明

| 字段 | 类型 | 来源 | 说明 |
|------|------|------|------|
| `session_id` | string | Claude Code `x-session-id` header | 会话 ID，关联多次请求 |
| `project` | string | Claude Code `x-project-path` header | 项目路径 |
| `model` | string | SelectedMappingInfo.upstream_name | 上游实际使用的模型名 |
| `channel` | string | SelectedMappingInfo.channel_id | 选中的渠道 ID |
| `timestamp` | string | 请求完成时间 | RFC 3339 |
| `usage` | object | 上游 API 响应 `usage` 字段 | 实际 token 消耗 |
| `pricing` | object | SelectedMappingInfo.pricing | 定价快照 |
| `actual_cost` | f64 | calc_cost(usage, pricing) | 实际花费 |
| `compression` | object | CompressMiddleware | 代理层压缩统计 |
| `compression.total_saved` | u64 | proxy_schema_saved + proxy_response_saved | 代理层共省 token |
| `saved_cost` | f64 | total_saved × pricing | 压缩节省金额 |

### 2.3 Session ID 和 Project 来源

agent-proxy 需要新增对两个 header 的提取：

```
x-session-id:  sess_abc123    → ConnectionContext.session_id
x-project-path:  /Users/baoyx/my-project  → ConnectionContext.project
```

这两个 header 由 Claude Code 在上层配置的自定义 API endpoint 请求中携带。如果未携带，使用默认值（`session_id = ""`，`project` 从环境变量或当前目录推断）。

---

## 3. 传输机制

agent-proxy 在 `CostMiddleware::record()` 中，写完自己的 cost_records 后，调用 tokenless CLI 子进程回传数据。

### 3.1 CLI 命令

```
tokenless stats report --json '<RequestReport>'
```

### 3.2 agent-proxy 调用方式

```rust
// CostMiddleware::record() 末尾
let report = serde_json::to_string(&report).unwrap_or_default();
let result = std::process::Command::new("tokenless")
    .args(["stats", "report", "--json", &report])
    .stdout(std::process::Stdio::null())
    .stderr(std::process::Stdio::null())
    .spawn(); // fire-and-forget，不阻塞主线程
```

**设计决策**：
- `spawn()` 而非 `output()`，不阻塞代理请求处理
- 失败不影响代理正常运行（静默丢弃，记录 tracing::debug）
- 如果 tokenless 不在 PATH，使用环境变量 `TOKENLESS_BIN` 指定路径

### 3.3 tokenless 接收端

```
crates/tokenless-cli/src/commands/stats.rs  (或新建)

fn cmd_stats_report(json: &str) -> Result<()> {
    let report: RequestReport = serde_json::from_str(json)?;
    stats::insert_request_report(&report)?;
    Ok(())
}
```

tokenless 在自己的数据库中新增表 `request_reports`（或合并到现有 `stats_records`），存储完整报告。

---

## 4. agent-proxy 需要补充的实现

### 4.1 ConnectionContext 扩展

```rust
// crates/core/src/types.rs — ConnectionContext 新增字段
pub struct ConnectionContext {
    // ... 现有字段 ...
    /// Session ID from x-session-id header
    pub session_id: Option<String>,
    /// Project path from x-project-path header
    pub project: Option<String>,
}
```

### 4.2 Header 提取

```rust
// crates/core/src/server.rs — handle_proxy_request 中提取
let session_id = parts.headers
    .get("x-session-id")
    .and_then(|v| v.to_str().ok())
    .map(|s| s.to_string());
let project = parts.headers
    .get("x-project-path")
    .and_then(|v| v.to_str().ok())
    .map(|s| s.to_string());
```

### 4.3 CostMiddleware 集成

在 `record()` 中构造 `RequestReport`，序列化为 JSON，调用 tokenless CLI。

---

## 5. tokenless 需要补充的实现

### 5.1 新增 `stats report` 子命令

```
crates/tokenless-cli/src/commands/stats.rs

tokenless stats report --json '...'
```

接收 `RequestReport` JSON，写入 stats 数据库。

### 5.2 数据库表设计

```sql
CREATE TABLE IF NOT EXISTS request_reports (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id TEXT NOT NULL DEFAULT '',
    project TEXT NOT NULL DEFAULT '',
    model TEXT NOT NULL DEFAULT '',
    channel TEXT NOT NULL DEFAULT '',
    timestamp TEXT NOT NULL,
    report_json TEXT NOT NULL,      -- 完整 RequestReport JSON
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_reports_session ON request_reports(session_id);
CREATE INDEX IF NOT EXISTS idx_reports_project ON request_reports(project);
CREATE INDEX IF NOT EXISTS idx_reports_timestamp ON request_reports(timestamp);
```

`report_json` 存储完整 JSON，方便查询和导出。

---

## 6. 展示层

tokenless 侧新增查询命令：

```
tokenless stats summary --project /Users/baoyx/my-project --days 7

输出:
┌────────────────────────────────────────────────────────┐
│  项目: /Users/baoyx/my-project (最近 7 天)             │
├─────────────┬──────────┬──────────┬──────────┬─────────┤
│ 日期         │ 请求数    │ 实际花费  │ 节省金额  │ 节省率  │
├─────────────┼──────────┼──────────┼──────────┼─────────┤
│ 2026-06-02  │    45    │ ¥0.23   │ ¥0.08   │ 25.8%  │
│ 2026-06-01  │    38    │ ¥0.19   │ ¥0.06   │ 24.0%  │
│ ...         │    ...   │ ...     │ ...     │ ...    │
├─────────────┼──────────┼──────────┼──────────┼─────────┤
│ 合计         │   283    │ ¥1.42   │ ¥0.52   │ 26.8%  │
└─────────────┴──────────┴──────────┴──────────┴─────────┘
```

---

## 7. 错误处理

| 情况 | 处理 |
|------|------|
| tokenless 二进制不在 PATH | agent-proxy 静默跳过（非关键路径） |
| JSON 解析失败 | 记录 tracing::debug，不重试 |
| 子进程启动失败 | spawn() 失败不阻塞代理 |
| 数据库写入失败 | tokenless 侧记录错误日志 |

---

## 8. 实施计划

```
Phase 1: agent-proxy 侧
├── ConnectionContext 加 session_id + project
├── server.rs 提取 x-session-id / x-project-path header
├── CostRecord 已有完整字段，无需改动
└── CostMiddleware::record() 调用 tokenless stats report

Phase 2: tokenless 侧
├── 新增 stats report 子命令
├── 创建 request_reports 表
├── 实现 stats summary 查询命令
└── 单元测试
```

---

> Owner: baoyx · 版本：v1.0 · 生效日期：2026-06-02
