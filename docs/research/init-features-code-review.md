# `tokenless init` 新功能代码质量审查报告

> 审查日期：2026-06-04 | 审查文档：`init-features-architecture.md`
> 涉及 crates：`tokenless-stats`, `tokenless-cli`
> 审查维度：类型设计、错误处理、API 设计、测试设计、与现有代码一致性

---

## 执行摘要

设计方案整体合理，数据流清晰，向后兼容性分析充分。发现 **3 个高严重度问题**、**8 个中严重度问题**、**7 个低严重度问题**。高严重度问题集中在 PII 敏感数据的 Debug 实现、`#[non_exhaustive]` 缺失、以及 `typed-builder` 未应用于大结构体。所有问题均在实现前可修复，无架构性阻塞。

---

## 1. 类型设计

### 1.1 [HIGH] TokenlessConfig 需手动实现 Debug 以 redact PII 字段

**问题描述：** 设计方案为 `TokenlessConfig` 新增了 `user_name` 和 `user_email` 两个字段，这是用户身份信息（PII）。当前 `TokenlessConfig` 使用 `#[derive(Debug)]`，新的 PII 字段将在任何 `{:#?}` 调试输出中完全暴露。

**违反规则：** CLAUDE.md 要求 "Derive or implement Debug for all types; redact sensitive fields manually."

**严重程度：** HIGH -- 默认 Debug 输出会在 tracing 日志、错误信息、测试输出中暴露用户真实姓名和邮箱。

**建议修改：**

```rust
use std::fmt;

impl fmt::Debug for TokenlessConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TokenlessConfig")
            .field("stats_enabled", &self.stats_enabled)
            .field("experimental_mode", &self.experimental_mode)
            .field("user_name", &self.user_name.as_ref().map(|_| "[redacted]"))
            .field("user_email", &self.user_email.as_ref().map(|_| "[redacted]"))
            .field("compress_enabled", &self.compress_enabled)
            .field("last_init_at", &self.last_init_at)
            .finish()
    }
}
```

**测试验证：**

```rust
#[test]
fn test_debug_redacts_user_info() {
    let config = TokenlessConfig {
        user_name: Some("Alice".into()),
        user_email: Some("alice@example.com".into()),
        ..Default::default()
    };
    let debug_str = format!("{config:?}");
    assert!(!debug_str.contains("Alice"), "user_name must be redacted");
    assert!(!debug_str.contains("alice@example.com"), "user_email must be redacted");
    assert!(debug_str.contains("[redacted]"), "should show redacted marker");
}
```

---

### 1.2 [HIGH] TokenlessConfig 和 CompressLogEntry 缺少 #[non_exhaustive]

**问题描述：** `TokenlessConfig` 和 `CompressLogEntry` 均为 `tokenless-stats` 库 crate 中的公开 struct。设计方案已经计划在 v1 中新增 4 个字段，未来极有可能继续扩展。若不标记 `#[non_exhaustive]`，下游 crate（`tokenless-cli`、外部消费者）在使用结构体字面量构造时，新增字段将导致编译错误。

**违反规则：** CLAUDE.md 要求 "Mark library-facing structs `#[non_exhaustive]` when future fields are likely."

**严重程度：** HIGH -- 缺少此标记将锁定 API，未来任何字段新增都是破坏性变更。

**建议修改：**

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct TokenlessConfig {
    // ... fields
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct CompressLogEntry {
    // ... fields
}
```

注意：添加 `#[non_exhaustive]` 后，外部 crate 不能使用结构体字面量语法（`TokenlessConfig { ... }`），必须通过构造函数或 builder 创建。`Default::default()` 不受影响。需要检查 `init_cmd.rs` 和现有测试中是否有跨 crate 的结构体字面量构造。

---

### 1.3 [HIGH] CompressLogEntry 字段过多，应使用 typed-builder

**问题描述：** `CompressLogEntry` 有 12 个字段（timestamp, uid, email, project, agent, hook_type, before_bytes, after_bytes, saved_tokens, compression_pct, session_id, tool_use_id, op_type），远超 5 个字段的阈值。

**违反规则：** CLAUDE.md 要求 "Use `typed-builder` for structs with more than five fields."

**严重程度：** HIGH -- 12 字段的结构体字面量构造极易出错（参数顺序混淆），且添加新字段时所有构造点都需要修改。

**建议修改：**

```rust
use typed_builder::TypedBuilder;

#[derive(Debug, Clone, Serialize, TypedBuilder)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct CompressLogEntry {
    #[builder(setter(into))]
    pub timestamp: chrono::DateTime<chrono::Utc>,

    #[builder(setter(into))]
    pub uid: String,

    #[builder(default, setter(strip_option, into))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,

    #[builder(setter(into))]
    pub project: String,

    #[builder(setter(into))]
    pub agent: String,

    #[builder(setter(into))]
    pub hook_type: HookType,  // See 1.6 below

    pub before_bytes: usize,
    pub after_bytes: usize,
    pub saved_tokens: usize,
    pub compression_pct: f64,

    #[builder(default, setter(strip_option, into))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,

    #[builder(default, setter(strip_option, into))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_use_id: Option<String>,

    #[builder(setter(into))]
    pub op_type: OpCategory,  // See 1.6 below
}
```

注意：`typed-builder` 需要添加到 `tokenless-stats/Cargo.toml` 的依赖中。

---

### 1.4 [MEDIUM] last_init_at 应使用 chrono 类型而非 String

**问题描述：** 设计方案将 `last_init_at` 定义为 `Option<String>`。然而 `tokenless-stats` 已经依赖 `chrono`（见 `Cargo.toml`），且 `StatsRecord.timestamp` 已经使用 `chrono::DateTime<chrono::Local>`。使用 `String` 存储时间戳放弃了类型安全：无法进行时间比较、格式验证在序列化时才能发现错误。

**严重程度：** MEDIUM -- 功能上可工作，但降低了类型安全性，与 crate 内已有实践不一致。

**建议修改：**

```rust
/// Timestamp of last `tokenless init` run.
#[serde(default, skip_serializing_if = "Option::is_none")]
pub last_init_at: Option<chrono::DateTime<chrono::Utc>>,
```

或者保持更简单的字符串格式，但使用 newtype 包装：

```rust
/// ISO 8601 timestamp wrapper for config serialization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Iso8601(String);

impl Iso8601 {
    #[must_use]
    pub fn now() -> Self {
        Self(chrono::Utc::now().to_rfc3339())
    }
}
```

**推荐：** 使用 `chrono::DateTime<chrono::Utc>` 配合 serde 的 `chrono` feature（已在依赖中启用 `chrono = { features = ["serde"] }`）。

---

### 1.5 [MEDIUM] hook_type 和 op_type 应使用 enum 而非 String

**问题描述：** `CompressLogEntry` 中 `hook_type: String` 和 `op_type: String` 使用字符串表示有限集合的值。这违反了 "Make illegal states unrepresentable" 原则。如果某处将 `"compress"` 误拼为其他字符串，编译期无法发现。

**严重程度：** MEDIUM -- 当前值集合小，但扩展时会累积技术债务。参考已有模式：`OperationType` 在 `record.rs` 中就是 enum。

**建议修改：**

```rust
/// Type of hook that generated this log entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum HookType {
    /// PreToolUse: command rewriting hook.
    Rewrite,
    /// PostToolUse: response compression hook.
    Compress,
}

/// Category of compression operation for analytics grouping.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "PascalCase")]
pub enum OpCategory {
    SchemaCompression,
    ResponseCompression,
    RewriteCommand,
    ToonEncoding,
}
```

注意：`OpCategory` 与已有 `OperationType` 类似但更泛化。如果两者语义完全重叠，应直接复用 `OperationType`（需添加 `Copy` derive）。

---

### 1.6 [LOW] Option<bool> tri-state 模式可接受但不够自文档化

**问题描述：** `compress_enabled: Option<bool>` 和 `InitConfig::compress: Option<bool>` 实现三态逻辑：
- `None` = 未明确配置，使用默认值
- `Some(true)` = 强制启用
- `Some(false)` = 强制禁用

这是有效的设计模式（clap 内部也使用 `Option<bool>` 处理 `--flag` / `--no-flag`），但阅读代码时需要额外心智负担理解 `None` 语义。

**严重程度：** LOW -- 功能正确，注释已说明语义。但可考虑更明确的表示。

**可选的替代方案（非强制）：**

```rust
/// Compress hook installation preference.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompressPreference {
    /// Use the configuration file's compress_enabled value.
    UseConfig,
    /// Explicitly enable compress hook.
    Enable,
    /// Explicitly disable compress hook.
    Disable,
}
```

**判断：** 当前 `Option<bool>` 方案简单、与 clap 参数解析直接对应，不需要修改。在 `InitConfig` 文档注释中明确说明 `None` 语义即可。

---

### 1.7 [LOW] UserIdentity 应实现 PartialEq（含 IdentitySource）

**问题描述：** `IdentitySource` 已 derive `PartialEq`，但 `UserIdentity` 未显示 derive `PartialEq`。对于值对象类型，`PartialEq` 是基本需求（测试断言、去重比较）。

**严重程度：** LOW -- 不影响功能，但增加测试复杂度。

**建议：** `UserIdentity` 添加 `PartialEq` derive。

---

## 2. 错误处理

### 2.1 [MEDIUM] append_compress_log 需明确 fail-silent 行为

**问题描述：** 设计方案描述 `append_compress_log` 为 "fire-and-forget: failures are traced but never block compression"，但未展示具体的错误处理代码。需要与现有 `append_report_to_file` 模式完全对齐。

**参考模式（`shared.rs` 中的 `append_report_to_file`）：**
- 序列化失败：`tracing::warn!` + 静默返回
- 文件打开失败：`tracing::warn!` + 静默返回
- 写入失败：`tracing::warn!` + 静默返回

**严重程度：** MEDIUM -- 如果实现不一致（例如使用 `eprintln!` 或返回错误），将破坏 hook 协议的输出流。

**建议实现：**

```rust
/// Append a compress log entry to `~/.tokenfleet-ai/tokenless/compress.log`.
///
/// This is fire-and-forget: failures are traced but never block compression.
/// The function must never write to stdout, as stdout is the hook protocol output.
pub fn append_compress_log(entry: &CompressLogEntry) {
    let log_path = get_tokenless_dir().join("compress.log");

    let line = match serde_json::to_string(entry) {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(error = %e, "compress log serialize failed");
            return;
        }
    };

    let mut f = match std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
    {
        Ok(f) => f,
        Err(e) => {
            tracing::warn!(error = %e, path = %log_path.display(), "compress log open failed");
            return;
        }
    };

    use std::io::Write;
    if let Err(e) = writeln!(f, "{line}") {
        tracing::warn!(error = %e, "compress log write failed");
    }
}
```

关键约束：
1. 绝不写入 stdout（会破坏 hook 协议）
2. 不返回 `Result`（调用方不应感知日志写入失败）
3. 使用 `tracing::warn!` 而非 `eprintln!`（`eprintln!` 在某些 hook 场景可能混入 stderr 流）

---

### 2.2 [MEDIUM] save() 中的 unwrap_or_default 会静默丢失配置

**问题描述：** 现有代码 `config.rs` 中：
```rust
let content = serde_json::to_string_pretty(self).unwrap_or_default();
```
序列化失败时静默写入空字符串，导致 config.json 被清空。虽然不是设计方案引入的新问题，但在新增 4 个字段后序列化失败概率增加。

**严重程度：** MEDIUM -- 现有问题，但在本次变更中被放大。

**建议修改（可在本次 PR 中顺带修复）：**

```rust
pub fn save(&self) -> Result<(), std::io::Error> {
    let path = Self::config_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let content = serde_json::to_string_pretty(self)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;
    std::fs::write(&path, content)
}
```

---

### 2.3 [LOW] init_cmd.rs 中的 current_dir fallback 行为不一致

**问题描述：** 设计方案 6.2 中：
```rust
let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
```
这个 fallback 路径 `.` 在 chdir 失败时可能导致 `detect_user_identity` 在一个无意义的目录运行 `git config`。

**严重程度：** LOW -- chdir 失败是极端边缘情况，且用户检测失败不阻塞 init。

**建议：** 如果 `current_dir()` 失败，跳过用户检测而不是用 `.` 作为回退路径。

```rust
let identity = std::env::current_dir()
    .map(|cwd| crate::init::user_detect::detect_user_identity(&cwd))
    .unwrap_or_default(); // UserIdentity::default() — all None
```

---

### 2.4 [LOW] sanitize_identity 需要处理 tab 字符

**问题描述：** `sanitize_identity` 过滤控制字符但允许 `\t`：
```rust
if trimmed.chars().any(|c| c.is_control() && c != '\t') {
    return None;
}
```
`git config user.name` 实际可以包含 tab，但 config.json 中存储 tab 可能引起格式问题。

**严重程度：** LOW -- 实际触发概率极低。

**建议：** 将 tab 也纳入过滤范围，或统一转义处理。

---

## 3. API 设计

### 3.1 [MEDIUM] 新增公开 API 文档不完整

**问题描述：** 设计方案未展示以下公开 API 的完整文档注释（含 `# Errors` / `# Panics` 部分）：

| API | 缺失 |
|-----|------|
| `CompressLogEntry` struct | 缺少 struct-level doc（`//!` 风格模块文档已覆盖） |
| `append_compress_log()` | 缺少 `# Panics` 声明（应注明 "This function does not panic"） |
| `sanitize_identity()` | 缺少文档注释 |
| `set_user_identity()` | 缺少 `# Panics` |
| `IdentitySource` enum | 缺少每个 variant 的文档 |
| `UserIdentity::default()` 行为 | 应文档化 "All fields are None, all sources are Unknown" |

**违反规则：** CLAUDE.md 要求 "Write doc comments for all public items."

**严重程度：** MEDIUM -- 功能正确但不符合项目文档标准。

**建议示例：**

```rust
/// Detected user identity for attribution in compression logs.
///
/// All fields are optional — they are `None` only when every detection
/// method fails. The `*_source` fields record which method succeeded,
/// useful for debugging detection issues.
///
/// Use [`UserIdentity::default()`] to get an empty identity (all `None`,
/// all sources [`IdentitySource::Unknown`]).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct UserIdentity {
    /// User's display name from the highest-priority available source.
    pub name: Option<String>,
    /// User's email address (typically from git config).
    pub email: Option<String>,
    /// Which method produced the [`name`](Self::name) field.
    pub name_source: IdentitySource,
    /// Which method produced the [`email`](Self::email) field.
    pub email_source: IdentitySource,
}
```

---

### 3.2 [MEDIUM] user_detect 模块放置位置需要确认

**问题描述：** 设计方案将 `user_detect` 放在 `tokenless-cli/src/init/user_detect.rs`。

**分析：**

| 放在 `tokenless-stats` | 放在 `tokenless-cli` |
|------------------------|----------------------|
| 优点：库级可复用 | 优点：git 命令/环境读取属于 I/O 层 |
| 缺点：库 crate 不应执行外部命令 | 缺点：其他二进制无法复用 |
| 违反关注点分离 | 符合整洁架构（adapter 层）|

**结论：** 正确选择。`git_config()` 和 `os_username()` 执行外部命令和读取环境变量，属于基础设施/适配器层。`tokenless-stats` 作为库 crate 应该只接受已验证的数据。

**但有一个改进点：** `sanitize_identity()` 是纯数据验证函数，可以提取到 `tokenless-stats` 中以便复用：

```rust
// tokenless-stats/src/identity.rs
/// Sanitize a raw identity string detected from the environment.
///
/// Applies trimming, control character rejection, and 256-byte length bounding.
/// Returns `None` if the input is empty or contains only control characters.
#[must_use]
pub fn sanitize_identity(raw: &str) -> Option<String> {
    // ... pure validation logic
}
```

---

### 3.3 [LOW] 缺少 derive 推荐列表

**问题描述：** 设计方案中部分类型 derive 了部分 trait。建议对所有公开数据类型完成以下 derive（或手动实现）：

| 类型 | 推荐 derive |
|------|------------|
| `UserIdentity` | `Debug, Clone, Default, PartialEq, Eq` |
| `IdentitySource` | `Debug, Clone, Copy, PartialEq, Eq, Hash` |
| `CompressLogEntry` | `Debug, Clone, Serialize` (已满足) + builder |
| `TokenlessConfig` | 手动 `Debug` + `Clone, Serialize, Deserialize` + `#[non_exhaustive]` |

**严重程度：** LOW -- 功能性无影响，但影响可用性（测试断言等）。

---

### 3.4 [LOW] hook_compress 新增参数破坏了二进制接口

**问题描述：** 设计方案在 4.4 节给 `hook_compress()` 新增了一个 `compress_log_enabled: bool` 参数。当前该函数只在 `main.rs` 的 dispatch 中调用一次，所以实际影响范围小。

**严重程度：** LOW -- 当前只有一个调用点。

**建议：** 不在函数签名中新增参数，而是在函数内部直接读取 `TokenlessConfig`：

```rust
// Inside hook_compress(), after compression:
if TokenlessConfig::load().is_compress_enabled() {
    // write compress log
}
```

这样保持 `hook_compress` 签名不变，调用方无需感知日志逻辑。与现有 `record_compression_stats` 模式一致（该函数内部调用 `TokenlessConfig::load().is_stats_enabled()`）。

---

## 4. 测试设计

### 4.1 [HIGH] 测试覆盖不完整

**问题描述：** 设计方案中仅包含 config 和 sanitize 的单元测试计划，缺乏以下关键领域的测试覆盖：

| 缺失的测试领域 | 风险 |
|---------------|------|
| `CompressLogEntry` JSON 序列化格式验证 | JSONL 格式错误导致下游解析失败 |
| `append_compress_log` 写入/轮转行为 | 磁盘满、权限错误时 hook 阻塞 |
| `detect_user_identity` 完整流程集成测试 | 用户检测逻辑回归 |
| `InitConfig` compress 三态解析 | CLI 参数组合逻辑错误 |
| `TokenlessConfig` save/load roundtrip（含新字段） | 配置持久化回归 |
| `init_claude` 条件安装（compress 开关） | hook 安装/卸载逻辑错误 |

**严重程度：** HIGH -- 缺少这些测试会导致回归风险集中在手动测试阶段。

**推荐的测试用例清单（按模块）：**

#### tokenless-stats/src/config.rs

```
test_deserialize_old_config_no_new_fields
  旧 config.json (只有 stats_enabled 和 experimental_mode)
  → 反序列化后 user_name, user_email, compress_enabled, last_init_at 均为 None

test_serialize_skips_none_fields
  TokenlessConfig::default()
  → 序列化后的 JSON 不含 user_name/user_email/compress_enabled/last_init_at 键

test_compress_enabled_defaults_true
  compress_enabled: None → is_compress_enabled() 返回 true

test_compress_enabled_explicit_false
  compress_enabled: Some(false) → is_compress_enabled() 返回 false

test_effective_user_name_fallback
  user_name: None → effective_user_name() 返回 "unknown"

test_debug_redacts_user_info
  format!("{:?}", config) 不包含 user_name/user_email 原始值

test_config_roundtrip_with_all_new_fields
  构造 → save → load → 所有字段值相等
```

#### tokenless-stats/src/compress_log.rs (新)

```
test_compress_log_entry_serializes_camelcase
  CompressLogEntry 序列化为 camelCase JSON，验证关键字段名

test_compress_log_entry_omits_none_optionals
  session_id: None, tool_use_id: None, email: None
  → 序列化后的 JSON 不含这些键

test_append_compress_log_creates_file
  临时目录中写入一条日志 → 文件存在且内容为合法 JSON

test_append_compress_log_appends_jsonl
  写入两条日志 → 文件有两行，每行均为独立合法 JSON

test_compress_log_entry_builder_required_fields
  使用 typed-builder 构造，缺少必填字段时编译失败
```

#### tokenless-cli/src/init/user_detect.rs (新)

```
test_sanitize_valid_name
  "John Doe" → Some("John Doe")

test_sanitize_empty_string
  "   " → None

test_sanitize_control_chars
  "john\x00doe" → None
  "name\x1b" → None

test_sanitize_truncates_long_string
  300 字符输入 → Some(256 字符)

test_sanitize_tab_handling
  "name\twith\ttabs" → 按设计预期行为

test_user_identity_default_all_none
  UserIdentity::default() → name/email 均为 None, sources 均为 Unknown

test_git_config_returns_none_when_git_not_installed
  (integration) 验证无 git 环境优雅降级

test_detect_user_identity_os_fallback
  (integration) 无 git config 时回退到 $USER 环境变量
```

#### tokenless-cli/src/init/mod.rs

```
test_init_config_compress_none_means_use_config
  InitConfig { compress: None } + config.is_compress_enabled()=true
  → 解析后 compress 为 true

test_init_config_compress_some_true_overrides
  InitConfig { compress: Some(false) } + config.is_compress_enabled()=true
  → 解析后 compress 为 false

test_init_claude_includes_compress_hook_when_enabled
  InitConfig { compress: Some(true) }
  → settings.json 中包含 PostToolUse compress hook

test_init_claude_excludes_compress_hook_when_disabled
  InitConfig { compress: Some(false) }
  → settings.json 中不包含 PostToolUse hook (仅 PreToolUse rewrite hook)
```

#### tokenless-cli/src/commands/init_cmd.rs

```
test_handle_resolves_compress_flag_from_cli
  --compress 传入 → TokenlessConfig.compress_enabled = Some(true)

test_handle_no_compress_sets_false
  --no-compress 传入 → TokenlessConfig.compress_enabled = Some(false)

test_handle_without_compress_flag_preserves_config
  不传 --compress/--no-compress → TokenlessConfig.compress_enabled 保持原值
```

---

### 4.2 [MEDIUM] 缺少 git_config 函数可测试性设计

**问题描述：** `git_config()` 直接调用 `std::process::Command::new("git")`，在单元测试中无法 mock。当前已有的 `detect_project_name` 中 `git remote get-url` 面临同样问题，但设计方案未解决。

**严重程度：** MEDIUM -- 如果希望单元测试覆盖 fallback 链，需要可注入的命令执行器。

**建议方案（三种选择）：**

**方案 A：环境变量覆盖（最小改动，推荐）**

```rust
fn git_config(key: &str, global: bool) -> Option<String> {
    // Allow test override via env var
    if let Ok(val) = std::env::var(format!("TEST_GIT_CONFIG_{}", key.to_uppercase().replace('.', "_"))) {
        return if val.is_empty() { None } else { Some(val) };
    }
    // ... real git execution
}
```

**方案 B：提取 CommandRunner trait（更通用但改动大）**

```rust
trait CommandRunner {
    fn run(&self, cmd: &str, args: &[&str]) -> Option<String>;
}
```

**方案 C：纯集成测试（当前方案）**

仅在安装了 git 的环境中运行，标记 `#[ignore]` 在普通 `cargo test` 中跳过。

**推荐：** 方案 A，改动最小且覆盖 fallback 链的关键路径。环境变量名用 `TEST_` 前缀明确测试用途。

---

### 4.3 [LOW] rstest 参数化测试适用场景

**问题描述：** 设计方案未使用 `rstest`。以下场景非常适合参数化测试：

**严重程度：** LOW -- 不影响功能，但可提高测试可维护性。

**建议使用场景：**

```rust
#[rstest]
#[case("John Doe", Some("John Doe"))]
#[case("   ", None)]
#[case("", None)]
#[case("john\x00doe", None)]
#[case("name\x1b[0m", None)]
fn test_sanitize_identity(#[case] input: &str, #[case] expected: Option<&str>) {
    assert_eq!(sanitize_identity(input), expected.map(String::from));
}

#[rstest]
#[case("compressEnabled", true, "enabled")]
#[case("compressEnabled", false, "disabled")]
#[case("noConfig", None, "default")]
fn test_compress_resolution(
    #[case] _desc: &str,
    #[case] config_value: Option<bool>,
    #[case] _expected_behavior: &str,
) {
    // verify TokenlessConfig::is_compress_enabled()
}
```

---

## 5. 与现有代码一致性

### 5.1 [MEDIUM] TokenlessConfig::save() 模式不一致

**问题描述：** 现有 `TokenlessConfig::save()` 返回 `Result<(), std::io::Error>`（库级别错误），而 CLI 层使用 `Result<(), (String, i32)>`（应用级别错误）。设计方案在 `init_cmd.rs` 中：
```rust
config.save().map_err(|e| (format!("Failed to save config: {e}"), 1))?;
```
这是正确的转换方式。但需要注意：如果 `save()` 按 2.2 节的建议改为不吞没序列化错误，这个 `map_err` 映射仍然正确。

**严重程度：** MEDIUM -- 需要与其他状态修改命令（如 `stats_enable`、`stats_disable`、`experimental_on/off`）的 save 模式对齐。建议检查这些命令是否也使用了 `TokenlessConfig::save()`。

**建议：** 在 `commands/stats.rs` 中检查 `stats_enable`/`stats_disable` 等函数的 config 保存模式，确保一致。

---

### 5.2 [LOW] 导入顺序规范

**问题描述：** 设计方案未展示模块导入部分。根据 CLAUDE.md 规范，导入顺序必须为：`std` → external → local（`crate::`）。

**严重程度：** LOW -- 实现时遵循已有模式即可。

**示例（`init_cmd.rs` 修改后）：**

```rust
//! Handler for `tokenless init`.

use std::path::PathBuf;

use crate::init::{self, user_detect};
use tokenless_stats::TokenlessConfig;
```

---

### 5.3 [LOW] 函数大小检查

**问题描述：** 设计方案中的 `handle()` 函数（`init_cmd.rs`）新增了用户检测、config 更新、config 保存的逻辑。修改后函数可能超过 30 行。

**严重程度：** LOW -- 建议提取辅助函数。

**建议提取：**

```rust
fn resolve_and_persist_config(
    compress_flag: Option<bool>,
) -> Result<TokenlessConfig, (String, i32)> {
    let mut config = TokenlessConfig::load();

    // Detect user identity
    if let Ok(cwd) = std::env::current_dir() {
        let identity = user_detect::detect_user_identity(&cwd);
        config.set_user_identity(identity.name, identity.email);
    }

    // Resolve compress preference
    let resolved = compress_flag.unwrap_or_else(|| config.is_compress_enabled());
    config.compress_enabled = Some(resolved);

    // Timestamp
    config.last_init_at = Some(chrono::Utc::now().to_rfc3339());

    config.save().map_err(|e| (format!("Failed to save config: {e}"), 1))?;
    Ok(config)
}
```

---

### 5.4 [LOW] 并发 config 写入的警告不足

**问题描述：** 设计方案第 9 节提到 "config.json 并发写入：当前无锁保护"，但归类为风险而非待解决项。对于 `tokenless init` 和 `tokenless hook compress` 可能同时运行的场景，config.json 损坏的风险需要更明确的处理。

**严重程度：** LOW -- 当前触发概率低（`hook compress` 只读 config，`init` 才是唯一写入者），但应该在 `save()` 中增加原子写入。

**建议（可选，后续 PR）：**

```rust
pub fn save(&self) -> Result<(), std::io::Error> {
    let path = Self::config_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let content = serde_json::to_string_pretty(self)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;
    // Atomic write: write to temp file, then rename
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, &content)?;
    std::fs::rename(&tmp, &path)?;
    Ok(())
}
```

---

### 5.5 [LOW] 压缩日志路径工具函数应统一

**问题描述：** 多个模块都需要访问 `~/.tokenfleet-ai/tokenless/` 路径。当前有：
- `config.rs`: `config_path()` (private)
- `shared.rs`: `get_tokenless_dir()`, `get_reports_dir()`, `get_db_path()`
- `hook.rs`: 内联构造 `compress-debug.log` 路径

新增 `compress.log` 路径后，建议在 `shared.rs` 中添加 `get_compress_log_path()` 统一管理。

**严重程度：** LOW -- 后续清理即可。

---

## 6. 额外发现

### 6.1 chrono 依赖无需新增

设计方案第 9 节提到 "tokenless-stats 当前无 chrono 依赖" -- 这不准确。检查 `crates/tokenless-stats/Cargo.toml`：
```toml
chrono = { workspace = true, features = ["serde"] }
```
`tokenless-stats` 已经依赖 `chrono`（用于 `StatsRecord.timestamp`），所以新增 `CompressLogEntry` 使用 chrono 类型不需要引入新依赖。

### 6.2 日志轮转触发时机

设计方案中压缩日志大小检查在 `init` 流程的第 9 步。但在 hook 运行期间日志会增长，如果用户从不运行 `init`，警告永远不会触发。建议在 `append_compress_log` 中每次写入后异步检查文件大小（代价很低，只是 `metadata()` 调用）。

---

## 7. 检查清单总结

### 实施前必须修复（Blocking）

- [ ] **1.1** TokenlessConfig 手动实现 Debug，redact user_name/user_email
- [ ] **1.2** TokenlessConfig 和 CompressLogEntry 添加 `#[non_exhaustive]`
- [ ] **1.3** CompressLogEntry 使用 typed-builder（需添加依赖）
- [ ] **4.1** 编写完整的测试用例（按 4.1 节清单）

### 实施前建议修复（Strongly Recommended）

- [ ] **1.4** last_init_at 改为 `chrono::DateTime<chrono::Utc>`
- [ ] **1.5** hook_type 和 op_type 改为 enum
- [ ] **2.1** append_compress_log 严格遵循 fire-and-forget 模式
- [ ] **3.1** 所有公开 API 添加完整文档注释
- [ ] **4.2** git_config 添加测试环境变量覆盖

### 实施中注意（Best Practice）

- [ ] **1.7** UserIdentity derive PartialEq, Eq
- [ ] **2.2** config::save() 序列化错误传播（顺带修复）
- [ ] **2.3** current_dir 失败时跳过用户检测而非 fallback 到 "."
- [ ] **3.2** sanitize_identity 考虑提取到 tokenless-stats
- [ ] **3.4** hook_compress 内部读取 TokenlessConfig 而非新增参数
- [ ] **5.1** 检查 stats 命令的 config 保存模式一致性
- [ ] **5.3** 提取 resolve_and_persist_config 辅助函数
- [ ] **5.5** shared.rs 添加 get_compress_log_path()

### 后续优化（Future PR）

- [ ] **1.6** 可选：CompressPreference enum 替代 Option<bool>
- [ ] **5.4** config 原子写入（write-tmp + rename）
- [ ] **4.3** rstest 参数化测试迁移

---

## 附录 A：完整类型修改建议（签名汇总）

```rust
// ── tokenless-stats/src/config.rs ──

/// Persistent configuration for tokenless stats recording.
#[derive(Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct TokenlessConfig {
    #[serde(default = "default_true")]
    pub stats_enabled: bool,

    #[serde(default = "default_true")]
    pub experimental_mode: bool,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_name: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_email: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compress_enabled: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_init_at: Option<chrono::DateTime<chrono::Utc>>,
}

// Manual Debug for PII redaction
impl fmt::Debug for TokenlessConfig { /* ... */ }

// ── tokenless-stats/src/compress_log.rs (NEW) ──

#[derive(Debug, Clone, Serialize, TypedBuilder)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct CompressLogEntry {
    #[builder(setter(into))]
    pub timestamp: chrono::DateTime<chrono::Utc>,

    #[builder(setter(into))]
    pub uid: String,

    #[builder(default, setter(strip_option, into))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,

    #[builder(setter(into))]
    pub project: String,

    #[builder(setter(into))]
    pub agent: String,

    #[builder(setter(into))]
    pub hook_type: HookType,

    pub before_bytes: usize,
    pub after_bytes: usize,
    pub saved_tokens: usize,
    pub compression_pct: f64,

    #[builder(default, setter(strip_option, into))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,

    #[builder(default, setter(strip_option, into))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_use_id: Option<String>,

    #[builder(setter(into))]
    pub op_type: OpCategory,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum HookType { Rewrite, Compress }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "PascalCase")]
pub enum OpCategory {
    SchemaCompression,
    ResponseCompression,
    RewriteCommand,
    ToonEncoding,
}

// ── tokenless-cli/src/init/user_detect.rs (NEW) ──

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UserIdentity {
    pub name: Option<String>,
    pub email: Option<String>,
    pub name_source: IdentitySource,
    pub email_source: IdentitySource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IdentitySource { GitConfig, OsUser, Unknown }
```

---

## 附录 B：参考的现有代码模式

| 模式 | 位置 | 用途 |
|------|------|------|
| fail-silent log writing | `shared.rs::append_report_to_file()` | compress_log 的目标模式 |
| trait-based error handling | `recorder.rs::{StatsError, StatsResult}` | tokenless-stats 错误模式 |
| CLI error tuple | `main.rs`: `Result<(), (String, i32)>` | CLI 层错误模式 |
| enum for bounded values | `record.rs::OperationType` | hook_type/op_type 的参考模式 |
| `#[serde(skip_serializing_if)]` | `ProxyReport` struct in `shared.rs` | 序列化省略模式 |
| `#[must_use]` on config helpers | `config.rs::is_stats_enabled()` | config 方法的标注模式 |
| `#[non_exhaustive]` example | 当前无使用（需新增） | 未来 API 扩展保护 |
| typed-builder example | 当前无使用（需新增） | 大结构体构造模式 |
