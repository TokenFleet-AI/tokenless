# 0018 — Compression Stats Reporting (tokenless ↔ agent-proxy)

> tokenless hook 通过文件队列上报压缩统计，agent-proxy 通过 rename-then-read 消费，统一写入 CostRecord。

---

## 1. 背景

当前两套统计系统各管各的：

```
tokenless stats (SQLite)          agent-proxy cost_records (SQLite)
├─ hook 压缩统计                  ├─ 上游 API token 消耗
├─ rewrite 统计                   ├─ 实际花费
└─ 实验模式效果                   └─ 代理层压缩统计（CompressMiddleware）
```

**目标**：每个 API 请求的压缩效果（tokenless）和实际消耗（agent-proxy）关联到同一条 CostRecord。

---

## 2. 数据模型

### 2.1 传输格式 `ProxyReport`

Tokenless hook 在压缩/改写完成后，追加一行 JSON 到报告文件：

```
~/.tokenfleet-ai/tokenless/reports/{session_id}.jsonl
```

每行一条（JSONL 格式）：

```json
{
  "sessionId": "sess_abc123",
  "agentId": "claude",
  "projectPath": null,
  "opType": "CompressSchema",
  "method": "ToonHrv",
  "beforeTokens": 1500,
  "afterTokens": 700,
  "savedTokens": 800,
  "beforeBytes": 6000,
  "afterBytes": 2800,
  "savedBytes": 3200,
  "timestamp": "2026-06-03T10:30:00Z"
}
```

| 字段 | 类型 | 说明 |
|------|------|------|
| `sessionId` | string | Claude Code 会话 ID |
| `agentId` | string | 代理标识（如 "claude"） |
| `projectPath` | string\|null | 项目路径 |
| `opType` | string | 操作类型（kebab-case）：`compress-schema` / `compress-response` / `rewrite-command` / `compress-toon` |
| `method` | string\|null | 压缩策略，见下表 |
| `beforeTokens` | u64 | 压缩前估算 token |
| `afterTokens` | u64 | 压缩后估算 token |
| `savedTokens` | u64 | 节省 token |
| `beforeBytes` | u64 | 压缩前字节数 |
| `afterBytes` | u64 | 压缩后字节数 |
| `savedBytes` | u64 | 节省字节数 |
| `timestamp` | string | RFC 3339 时间戳 |

#### method 枚举值

**CompressSchema / compress-schema**

| method | 触发条件 |
|--------|---------|
| `CompressorOnly` | 基础 schema 压缩器（截断描述、移除 title/example） |
| `ToonHrv` | 统一对象数组 >= 5 项 |
| `EnhancedToon` | schema 含枚举/约束，或深层嵌套 > 3 层 |
| `CjsonCompact` | 其他情况 |
| `CompressorOnly` | 输入 < 200 字符，跳过编码 |

**CompressResponse / compress-response**

| method | 触发条件 |
|--------|---------|
| `Standard` | 标准响应压缩（截断字符串/数组、删除 null） |
| `HighFidelity` | Bash 输出压缩（更宽松的截断限制） |
| `Semantic` | 语义感知字段过滤（`--semantic` 标志） |

**RewriteCommand / rewrite-command**

| method | 触发条件 |
|--------|---------|
| `RtkStandard` | RTK 命令改写 |

**CompressToon / compress-toon**

| method | 触发条件 |
|--------|---------|
| `ToonDefault` | 基础 TOON 编码 |

### 2.2 消费端 `CostRecord`（agent-proxy 侧）

agent-proxy 消费报告文件后，写入统一的 CostRecord：

```rust
struct CostRecord {
    // ── 关联 ──
    session_id: Option<String>,              // X-Claude-Code-Session-Id
    channel_id: String,                      // 渠道 ID
    project: String,                         // 项目路径
    user_id: String,                         // 用户
    agent_type: String,                      // ClaudeCode / Codex / ...

    // ── 上游消耗 ──
    input_tokens: i64,
    output_tokens: i64,
    cache_write_tokens: i64,
    cache_read_tokens: i64,
    thinking_tokens: i64,
    cost: f64,
    unit: String,                            // USD / CNY / credits

    // ── 压缩统计 ──
    schema_saved_tokens: i64,                // agent-proxy 侧 schema 压缩
    response_saved_tokens: i64,              // agent-proxy 侧 response 压缩
    rtk_saved_tokens: i64,                   // RTK 命令改写
    pre_compress_tokens: i64,
    post_compress_tokens: i64,
    compression_tokens_saved: i64,           // agent-proxy 侧总节省

    // ── 计费关联（来自 tokenless） ──
    before_tokens: i64,                      // 压缩前估算（≈ after + saved）
    after_tokens: i64,                       // 实际消耗（input + output）
    tokens_saved: i64,                       // 总节省
    compression_breakdown_json: String,      // JSON 数组明细

    // ── 审计 ──
    pricing_snapshot_json: String,
    timestamp: String,                       // RFC 3339
}
```

`compression_breakdown_json` 示例：

```json
[
  {"op": "CompressSchema", "method": "ToonHrv", "beforeTokens": 1500, "afterTokens": 700, "savedTokens": 800},
  {"op": "RewriteCommand", "method": "RtkStandard", "beforeTokens": 200, "afterTokens": 50, "savedTokens": 150}
]
```

### 2.3 Session ID 来源

agent-proxy 从 Claude Code HTTP 请求的 header 中提取：

```
X-Claude-Code-Session-Id: sess_abc123  → ConnectionContext.session_id
```

---

## 3. 传输机制：文件队列 + rename-then-read

### 3.1 整体流程

```
tokenless hook                     agent-proxy
─────────────                      ───────────
1. 压缩/改写
2. 写 stats.db（现有逻辑不变）
3. append 一行 JSONL 到
   ~/.tokenfleet-ai/tokenless/
   reports/{session_id}.jsonl
                                   4. 收到 Claude Code API 请求
                                   5. 读 X-Claude-Code-Session-Id header
                                   6. rename reports/{id}.jsonl
                                      → reports/{id}.processing（原子操作）
                                   7. 解析 processing 文件
                                   8. 累积 saved_tokens + breakdown
                                   9. 删除 processing 文件
                                   10. 注入 ctx（tokenless_saved_tokens 等）
                                   11. 正常处理请求（压缩、转发）
                                   12. CostRecorder.record() 写 CostRecord
```

### 3.2 为什么用文件而非 HTTP

- tokenless hook 是**短生命周期 CLI 进程**，启动 HTTP server 太重
- 文件 append 是 O(1)，不阻塞 hook
- `rename` 同文件系统内是**原子操作**，天然处理并发写入
- agent-proxy 已经在处理请求时做文件 I/O，无额外开销

### 3.3 agent-proxy 消费代码

```rust
// crates/core/src/report.rs
pub(crate) fn consume_report(session_id: &str) -> Option<TokenlessAccumulator> {
    let source = reports_dir.join(format!("{safe_sid}.jsonl"));
    let target = reports_dir.join(format!("{safe_sid}.processing"));

    // 原子 rename 认领文件
    fs::rename(&source, &target).ok()?;

    // 解析
    let result = parse_report_file(&target);

    // 清理
    let _ = fs::remove_file(&target);

    result
}
```

### 3.4 tokenless 写入代码

```rust
// crates/tokenless-cli/src/shared.rs
fn append_report_to_file(report: ProxyReport) -> Result<(), ()> {
    let file_path = get_reports_dir().join(format!("{safe_sid}.jsonl"));
    let line = serde_json::to_string(&report)?;
    let mut f = fs::OpenOptions::new()
        .create(true).append(true)
        .open(&file_path)?;
    writeln!(f, "{line}")
}
```

---

## 4. CostRecorder Trait（agent-proxy 侧）

Cost recording 不属于 `ProxyMiddleware` 链。核心 crate 定义独立 trait：

```rust
#[async_trait]
pub trait CostRecorder: Send + Sync + std::fmt::Debug {
    async fn record(
        &self,
        ctx: &ConnectionContext,
        response_body: &serde_json::Value,
    ) -> Result<(), ProxyError>;
}
```

成本 crate (`agent-proxy-rust-cost`) 实现该 trait，通过 `AgentProxyBuilder::cost_recorder()` 注册。服务器引擎在 `on_response` 链完成后调用。

---

## 5. 错误处理

| 情况 | 处理 |
|------|------|
| tokenless 写入报告失败 | `tracing::warn!`，不阻塞 hook 输出 |
| agent-proxy rename 失败（文件不存在） | 静默跳过，本次请求不计 before |
| 报告文件解析失败 | `tracing::warn!`，删除 processing 文件 |
| CostRecord 写入失败 | `tracing::warn!`，不影响响应返回 |
| session_id 为空 | 跳过整个关联流程 |

所有错误日志通过 `tracing` 同时输出到 stderr 和 `~/.tokenfleet-ai/tokenless/tokenless.log`。

---

## 6. 展示层（未来）

从 CostRecord 直接查询：

```sql
-- 按会话汇总
SELECT session_id,
       COUNT(*) as requests,
       SUM(cost) as total_cost,
       SUM(tokens_saved) as total_saved,
       SUM(before_tokens) as before,
       SUM(after_tokens) as after
FROM cost_records
WHERE session_id IS NOT NULL
GROUP BY session_id;

-- 压缩明细
SELECT session_id, compression_breakdown_json
FROM cost_records
WHERE compression_breakdown_json != '[]';
```

---

> Owner: baoyx · 版本：v2.0 · 更新：2026-06-03（反映文件队列 + CostRecorder 实际实现）
