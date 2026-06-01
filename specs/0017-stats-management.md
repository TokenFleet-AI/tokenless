# 0017 — Stats History Management

> 统计数据历史管理：删除、导出、维护。Spec 完成，待实施。

---

## 1. 背景

当前 `tokenless-stats` 提供基础的写入和查询 API，但缺少日常管理能力：

| 功能 | 当前 | 目标 |
|------|:---:|:---:|
| 全量清空 | `clear()` ✅ | 保留 |
| 精确删除 | ❌ | `delete_by_id(id)` |
| 按条件删除 | ❌ | `delete_by_agent()` / `delete_before()` |
| 导出 | ❌ | JSON / CSV |
| 数据库维护 | ❌ | `vacuum()` / `db_size_bytes()` |
| 数据库概览 | ❌ | 记录数、大小、时间范围 |

---

## 2. 数据模型

```
StatsRecord (已有，不变):
├── id: i64                    自增主键
├── timestamp: DateTime        创建时间（索引）
├── operation: OperationType   操作类型
├── agent_id: String           Agent 标识（索引）
├── session_id: Option<String> 会话 ID
├── tool_use_id: Option<String>
├── before_chars / before_tokens
├── after_chars / after_tokens
├── before_text / after_text   （可选字段，可能为 NULL）
└── before_output / after_output
```

引擎: SQLite (WAL 模式)，嵌入 `rusqlite` (bundled 编译)。

---

## 3. API 设计

### 3.1 库层（`tokenless-stats`）

```rust
/// 条件删除的筛选参数
#[derive(Debug, Default)]
pub struct DeleteFilter {
    /// Agent 标识
    pub agent_id: Option<String>,
    /// 操作类型
    pub operation: Option<OperationType>,
    /// 删除此日期之前的所有记录 (ISO 8601)
    pub before: Option<String>,
    /// 会话 ID
    pub session_id: Option<String>,
}

impl StatsRecorder {
    // ── 删除 ──────────────────────────────────────────────────

    /// 按 ID 删除单条记录。返回 `true` 表示找到并删除。
    pub fn delete_by_id(id: i64) -> StatsResult<bool>;

    /// 删除指定 Agent 的所有记录，返回删除条数。
    pub fn delete_by_agent(agent_id: &str) -> StatsResult<usize>;

    /// 删除指定日期之前的所有记录，返回删除条数。
    /// `date` 格式: ISO 8601（如 `"2026-05-01"`）或相对（如 `"30d"`）。
    pub fn delete_before(date: &str) -> StatsResult<usize>;

    /// 条件删除，返回删除条数和筛选说明。
    pub fn delete_where(filter: &DeleteFilter) -> StatsResult<DeleteResult>;

    // ── 导出 ──────────────────────────────────────────────────

    /// 导出所有记录为 JSON 文件，返回导出条数。
    pub fn export_json(&self, path: &Path) -> StatsResult<usize>;

    /// 导出所有记录为 CSV 文件，返回导出条数。
    pub fn export_csv(&self, path: &Path) -> StatsResult<usize>;

    // ── 维护 ──────────────────────────────────────────────────

    /// 获取数据库文件大小（字节）。
    pub fn db_size_bytes(&self) -> StatsResult<u64>;

    /// 获取总记录数。
    pub fn record_count(&self) -> StatsResult<usize>;

    /// 获取最早和最晚记录的时间戳。
    pub fn time_range(&self) -> StatsResult<(String, String)>;

    /// 执行 VACUUM 回收空间。
    pub fn vacuum(&self) -> StatsResult<()>;
}

/// 删除操作结果
#[derive(Debug)]
pub struct DeleteResult {
    /// 删除的记录数
    pub deleted: usize,
    /// 删除前记录总数
    pub before_count: usize,
    /// 释放的磁盘空间估算（字节）
    pub freed_bytes: u64,
}

/// 数据库概览信息
#[derive(Debug, Serialize)]
pub struct DbInfo {
    pub path: String,
    pub size_bytes: u64,
    pub record_count: usize,
    pub earliest_ts: String,
    pub latest_ts: String,
    pub agent_count: usize,
}
```

### 3.2 CLI 层

```bash
tokenless stats info
    → 显示数据库路径、大小、记录数、时间范围

tokenless stats delete --id 42
tokenless stats delete --agent copilot-shell
tokenless stats delete --before "2026-05-01"
tokenless stats delete --before "30d"
    → 所有删除命令支持 --dry-run 预览模式
    → 支持 --yes 跳过确认

tokenless stats export --format json --output /tmp/stats.json
tokenless stats export --format csv  --output /tmp/stats.csv

tokenless stats vacuum
    → 压缩数据库文件，回收已删除空间
```

---

## 4. 安全设计

| 原则 | 实施 |
|------|------|
| **预览优先** | 所有删除命令默认 `--dry-run`，只显示影响范围不执行 |
| **确认门禁** | 非 dry-run 模式需要 `--yes` 或交互式确认 |
| **删除前备份** | 自动导出到 `~/.tokenless/backups/<timestamp>.json`（`export_on_delete` 选项） |
| **最小权限** | 所有方法 `&self`（只读借用），无全局副作用 |
| **事务性** | 删除操作在单个 SQLite 事务内执行 |

---

## 5. 实现计划

| 优先级 | 功能 | API | CLI | 预估 |
|:---:|------|:---:|:---:|:---:|
| P0 | 概览信息 | `record_count()` + `db_size_bytes()` + `time_range()` | `stats info` | ~30 行 Rust |
| P0 | 按时间清理 | `delete_before(date)` | `stats delete --before` | ~40 行 Rust |
| P1 | 按 Agent 删除 | `delete_by_agent(id)` | `stats delete --agent` | ~30 行 Rust |
| P1 | JSON 导出 | `export_json(path)` | `stats export --format json` | ~30 行 Rust |
| P2 | 精确删除 | `delete_by_id(id)` | `stats delete --id` | ~15 行 Rust |
| P2 | VACUUM | `vacuum()` | `stats vacuum` | ~10 行 Rust |
| P3 | 条件删除 | `delete_where(filter)` | — | ~20 行 Rust |
| P3 | CSV 导出 | `export_csv(path)` | `stats export --format csv` | ~25 行 Rust |
| P3 | 自动清理策略 | 写入时 1% 概率检查 | `retention_days` 配置 | ~30 行 Rust |

---

## 6. 配置格式

```json
// ~/.tokenless/config.json
{
  "stats_enabled": true,
  "retention": {
    "max_days": 90,              // 保留最近 90 天
    "max_records": 10000,        // 或最多 1 万条
    "auto_cleanup": false,       // 默认关闭自动清理
    "export_on_delete": true     // 删除前自动导出备份
  }
}
```

---

## 7. SDK 集成示例

```rust
use tokenless_stats::StatsRecorder;

let recorder = StatsRecorder::new("/var/data/stats.db")?;

// 查看概览
let info = recorder.db_info()?;
println!("Records: {}, Size: {} MB, Range: {} ~ {}",
    info.record_count,
    info.size_bytes / 1024 / 1024,
    info.earliest_ts, info.latest_ts
);

// 预览要删除的数据
let filter = DeleteFilter {
    before: Some("2026-03-01".into()),
    ..Default::default()
};
let result = recorder.delete_where(&filter)?;
println!("Would delete {} records, freeing ~{} KB",
    result.deleted, result.freed_bytes / 1024
);

// 导出备份后删除
recorder.export_json("backup.json".as_ref())?;
recorder.delete_before("90d")?;

// 回收空间
recorder.vacuum()?;
```

---

## 8. TUI 界面设计

### 8.1 当前 TUI 标签

```
tokenless tui 已有标签:
┌────────┬──────────┬─────────┬────────┐
│ 仪表盘  │  记录    │  智能体  │  趋势  │
└────────┴──────────┴─────────┴────────┘
```

### 8.2 新增"管理"标签

```
┌────────┬──────────┬─────────┬────────┬────────┐
│ 仪表盘  │  记录    │  智能体  │  趋势  │  管理  │  ← 新增
└────────┴──────────┴─────────┴────────┴────────┘
```

### 8.3 管理标签页布局

```
┌─ Stats Management ───────────────────────────────┐
│                                                    │
│  Database Info                                     │
│  ┌──────────────────────────────────────────────┐ │
│  │ Path:    ~/.tokenless/stats.db                │ │
│  │ Size:    12.3 MB                              │ │
│  │ Records: 4,821                                │ │
│  │ Range:   2026-03-15 ~ 2026-06-01              │ │
│  │ Agents:  3 (claude-code, copilot, cursor)     │ │
│  └──────────────────────────────────────────────┘ │
│                                                    │
│  Cleanup                                           │
│  ┌──────────────────────────────────────────────┐ │
│  │ [1] Delete by Agent:  [copilot-shell    ▼]   │ │
│  │ [2] Keep recent:      [90] days              │ │
│  │ [3] Delete before:    [2026-03-01]           │ │
│  │                                                │ │
│  │ Preview: 1,200 records will be deleted        │ │
│  │          ~2.1 MB will be freed                │ │
│  │                                                │ │
│  │ [ Enter ] Execute    [ Esc ] Cancel           │ │
│  └──────────────────────────────────────────────┘ │
│                                                    │
│  [E] Export JSON    [C] Export CSV    [V] Vacuum   │
│                                                    │
└────────────────────────────────────────────────────┘
```

### 8.4 交互设计

| 按键 | 功能 |
|------|------|
| `1` | 选择 Agent 下拉列表 |
| `2` | 输入保留天数 |
| `3` | 输入截止日期 |
| `Enter` | 执行删除（弹出确认对话框） |
| `E` | 导出 JSON 到 `~/tokenless-export-<date>.json` |
| `C` | 导出 CSV |
| `V` | 执行 VACUUM |
| `Esc` / `q` | 返回 |

### 8.5 确认对话框

```
┌─ Confirm Deletion ───────────────────────────────┐
│                                                    │
│  ⚠ This will permanently delete 1,200 records    │
│    from before 2026-03-01.                        │
│                                                    │
│  A backup will be saved to:                       │
│  ~/.tokenless/backups/2026-06-01T12:00:00.json    │
│                                                    │
│  [ Enter ] Confirm    [ Esc ] Cancel              │
│                                                    │
└────────────────────────────────────────────────────┘
```

### 8.6 实现清单

| 文件 | 改动 | 预估 |
|------|------|:---:|
| `crates/tokenless-tui/src/ui/manage.rs` | 新文件: 管理面板渲染 | ~100 行 |
| `crates/tokenless-tui/src/app.rs` | 新增 `Tab::Manage` + 事件处理 | ~40 行 |
| `crates/tokenless-tui/src/lang.rs` | 管理面板 i18n (中/英) | ~30 行 |
| `crates/tokenless-stats/src/recorder.rs` | P0 管理 API (db_info, delete_before 等) | ~80 行 |
| `crates/tokenless-stats/src/config.rs` | retention 配置字段 | ~10 行 |

---

## 9. 相关文档

- [SDK Integration (EN)](../docs/sdk-integration.md)
- [SDK Integration (中文)](../docs/sdk-integration-zh.md)
- [TUI Patterns](../docs/tui-patterns.md)
- [Testing Strategy](./0007-testing-strategy.md)
- [Error Handling](./0006-error-handling-strategy.md)
