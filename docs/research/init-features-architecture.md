# `tokenless init` 新功能架构设计

> 设计日期：2026-06-04 | 状态：设计阶段 | 涉及 crates：tokenless-cli, tokenless-stats

## 概述

本文档为 `tokenless init` 的三个新功能提供完整架构设计：

1. **`--compress`/`--no-compress` 开关** -- 控制压缩 hook 安装，启用时输出结构化压缩日志
2. **人员配置初始化** -- 自动检测用户身份（git config user.name > user.email > OS 用户名）
3. **压缩日志记录** -- JSONL 格式结构化日志，独立于现有 debug 日志

---

## 1. TokenlessConfig 扩展

### 1.1 文件位置

`crates/tokenless-stats/src/config.rs` -- 现有实现，追加字段。

### 1.2 字段设计

```rust
/// Persistent configuration for tokenless stats recording.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenlessConfig {
    // ── Existing fields ──

    /// Whether stats recording is enabled.
    #[serde(default = "default_true")]
    pub stats_enabled: bool,

    /// Whether experimental mode is enabled.
    #[serde(default = "default_true")]
    pub experimental_mode: bool,

    // ── New fields (all Optional for backward compatibility) ──

    /// Detected user name for attribution.
    /// Priority: git config user.name > OS username.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_name: Option<String>,

    /// Detected user email for attribution.
    /// Priority: git config user.email.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_email: Option<String>,

    /// Whether compress hook is enabled.
    /// `None` = not explicitly configured (defaults to `true` at runtime).
    /// `Some(true)` = enabled, `Some(false)` = disabled.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compress_enabled: Option<bool>,

    /// Timestamp of last `tokenless init` run (ISO 8601).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_init_at: Option<String>,
}
```

### 1.3 Default 实现

```rust
impl Default for TokenlessConfig {
    fn default() -> Self {
        Self {
            stats_enabled: true,
            experimental_mode: true,
            user_name: None,
            user_email: None,
            compress_enabled: None,
            last_init_at: None,
        }
    }
}
```

### 1.4 向后兼容性分析

| 场景 | 旧 config.json（无新字段） | 新 config.json（有新字段） |
|------|--------------------------|--------------------------|
| 反序列化 | `#[serde(default)]` 将缺失字段置为 `None` | 正常读取 |
| 旧版 tokenless 读新版 config | 忽略未知字段（serde 默认行为） | -- |
| `compress_enabled` 为 `None` | 运行时视为 `true`（默认启用压缩） | -- |
| `user_name` 为 `None` | 压缩日志中 `user_name` 回退到 `"unknown"` | -- |

### 1.5 新增辅助方法

```rust
impl TokenlessConfig {
    /// Check if compress hook should be installed.
    /// Returns `true` when `compress_enabled` is `None` (default) or `Some(true)`.
    #[must_use]
    pub fn is_compress_enabled(&self) -> bool {
        self.compress_enabled.unwrap_or(true)
    }

    /// Get the effective user name, falling back to `"unknown"`.
    #[must_use]
    pub fn effective_user_name(&self) -> &str {
        self.user_name.as_deref().unwrap_or("unknown")
    }

    /// Update user identity fields in-place (for init flow).
    pub fn set_user_identity(&mut self, name: Option<String>, email: Option<String>) {
        self.user_name = name;
        self.user_email = email;
    }
}
```

### 1.6 测试覆盖

- `test_deserialize_old_config_no_new_fields` -- 旧 JSON 反序列化后新字段为 `None`
- `test_serialize_skips_none_fields` -- 序列化时省略值为 `None` 的新字段
- `test_compress_enabled_defaults_true` -- `None` 返回 `true`
- `test_compress_enabled_explicit_false` -- `Some(false)` 返回 `false`

---

## 2. InitConfig 扩展

### 2.1 文件位置

`crates/tokenless-cli/src/init/mod.rs` -- `InitConfig` 结构体。

### 2.2 字段设计

```rust
/// Tokenless hook configuration.
pub struct InitConfig {
    /// Install globally vs project-local.
    pub global: bool,
    /// Enable debug logging for compress hook.
    pub debug: bool,
    /// If `Some(true)`, install compress hook.
    /// If `Some(false)`, skip compress hook installation.
    /// If `None`, use `TokenlessConfig::is_compress_enabled()` to decide.
    pub compress: Option<bool>,
}
```

`compress` 为 `Option<bool>` 而非 `bool` 的原因：
- `None` = CLI 未传 `--compress` 或 `--no-compress`，使用配置文件中的 `compress_enabled` 值
- `Some(true)` = 显式 `--compress`
- `Some(false)` = 显式 `--no-compress`

### 2.3 使用方式

在 `init::run()` 及各 agent `init_*` 函数中：

```rust
fn init_claude(config: &InitConfig) -> Result<(), String> {
    let tokenless_config = TokenlessConfig::load();
    let enable_compress = config.compress.unwrap_or_else(|| tokenless_config.is_compress_enabled());

    // ... settings merge ...

    // Only insert PostToolUse compress hook when compress is enabled
    if enable_compress {
        // write compress hook
    } else {
        // write only PreToolUse rewrite hook
    }
}
```

---

## 3. CLI 参数设计

### 3.1 文件位置

`crates/tokenless-cli/src/main.rs` -- `Commands::Init` variant。

### 3.2 参数变更

```rust
Init {
    /// Install hooks globally (for all projects).
    #[arg(long)]
    global: bool,

    /// Agent name (claude, cursor, windsurf, cline, copilot, gemini, etc.).
    #[arg(short, long, default_value = "claude")]
    agent: String,

    /// Enable debug logging for compress hook.
    #[arg(long)]
    debug: bool,

    // ── New ──

    /// Enable compress hook installation (default: use config or true).
    #[arg(long, conflicts_with = "no_compress")]
    compress: bool,

    /// Disable compress hook installation.
    #[arg(long, conflicts_with = "compress")]
    no_compress: bool,
},
```

### 3.3 参数解析

```rust
// In commands/init_cmd.rs
pub(crate) fn handle(
    global: bool,
    agent: String,
    debug: bool,
    compress: bool,      // new
    no_compress: bool,   // new
) -> Result<(), (String, i32)> {
    let compress_flag = if no_compress {
        Some(false)
    } else if compress {
        Some(true)
    } else {
        None // use config default
    };

    let config = init::InitConfig { global, debug, compress: compress_flag };
    // ...
}
```

### 3.4 参数语义

| 调用方式 | `compress_flag` | 行为 |
|---------|----------------|------|
| `tokenless init` | `None` | 使用 `TokenlessConfig.compress_enabled`（默认 true） |
| `tokenless init --compress` | `Some(true)` | 强制安装压缩 hook |
| `tokenless init --no-compress` | `Some(false)` | 不安装压缩 hook |
| `tokenless init --compress --no-compress` | clap 拒绝 | `conflicts_with` 互斥 |

---

## 4. 压缩日志设计

### 4.1 文件位置

`~/.tokenfleet-ai/tokenless/compress.log`

与现有 debug 日志分离：
| 日志 | 路径 | 格式 | 用途 |
|------|------|------|------|
| 运行日志 | `tokenless.log` | tracing 文本 | 程序运行/错误诊断 |
| Debug 日志 | `compress-debug.log` | 自由文本 | before/after 原始文本，调试压缩效果 |
| **压缩日志（新）** | `compress.log` | JSONL | 结构化压缩指标，统计分析 |

### 4.2 日志结构体

新建 `crates/tokenless-stats/src/compress_log.rs`：

```rust
use serde::Serialize;

/// Serde default for `user_name` when field is missing during deserialization.
fn default_user_name() -> String {
    "unknown".to_string()
}

/// A single compress log entry, written as one JSON line.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompressLogEntry {
    /// ISO 8601 timestamp with millisecond precision.
    pub timestamp: String,

    /// User name (from TokenlessConfig.user_name or "unknown").
    #[serde(default = "default_user_name")]
    pub user_name: String,

    /// User email (from TokenlessConfig.user_email, optional).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,

    /// Project name (detected from git remote / manifest / dirname).
    pub project: String,

    /// Agent name (claude, cursor, etc.).
    pub agent: String,

    /// Hook type: "rewrite" | "compress".
    pub hook_type: String,

    /// Original input size in bytes.
    pub before_bytes: usize,

    /// Compressed output size in bytes.
    pub after_bytes: usize,

    /// Estimated token savings.
    pub saved_tokens: usize,

    /// Compression ratio as percentage (e.g., 45.2 means 45.2% reduction).
    pub compression_pct: f64,

    /// Session ID from hook payload (for correlation).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,

    /// Tool use ID from hook payload.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_use_id: Option<String>,

    /// Operation type: "SchemaCompression" | "ResponseCompression" | ...
    pub op_type: String,
}
```

### 4.3 日志写入函数

```rust
/// Append a compress log entry to `~/.tokenfleet-ai/tokenless/compress.log`.
///
/// This is fire-and-forget: failures are traced but never block compression.
pub fn append_compress_log(entry: &CompressLogEntry) {
    let log_path = get_compress_log_path();
    // same pattern as append_report_to_file in shared.rs:
    // create_dir_all, OpenOptions::create(true).append(true), writeln! with JSON
}
```

### 4.4 记录时机

压缩日志在 `tokenless hook compress` 完成时记录，具体位置在 `commands::hook::hook_compress()` 函数末尾：

```rust
pub(crate) fn hook_compress(
    semantic: bool,
    target: &str,
    project: Option<String>,
    debug: bool,
    compress_log_enabled: bool,  // new param
) -> Result<(), (String, i32)> {
    // ... existing compression logic ...

    // After successful compression
    if compress_log_enabled {
        let config = TokenlessConfig::load();
        let entry = CompressLogEntry {
            timestamp: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            user_name: config.effective_user_name().to_string(),
            email: config.user_email.clone(),
            project: project.unwrap_or_else(|| "(unknown)".to_string()),
            agent: target.to_string(),
            hook_type: "compress".to_string(),
            before_bytes: original.len(),
            after_bytes: compressed.len(),
            saved_tokens: before_tokens.saturating_sub(after_tokens),
            compression_pct: /* calculate */,
            session_id,
            tool_use_id,
            op_type: "ResponseCompression".to_string(),
        };
        append_compress_log(&entry);
    }
}
```

### 4.5 是否记录压缩日志的控制

压缩日志开关由 `TokenlessConfig::is_compress_enabled()` 控制。当用户执行 `tokenless init --no-compress` 时，`compress_enabled` 设为 `Some(false)`，后续压缩操作同时跳过 hook 和日志记录。

### 4.6 日志轮转策略

- 不做自动轮转（JSONL 每行约 300 bytes，每 100 万条约 300 MB）
- `tokenless init` 执行时如果日志文件超过 100 MB，打印警告提示用户手动清理
- 未来可扩展 `tokenless logs rotate --max-size 100M` 子命令

---

## 5. 用户检测模块设计

### 5.1 模块位置

新建 `crates/tokenless-cli/src/init/user_detect.rs`

### 5.2 数据结构

```rust
/// Detected user identity for attribution.
#[derive(Debug, Clone, Default)]
pub struct UserIdentity {
    /// User's display name (highest priority source available).
    pub name: Option<String>,
    /// User's email address.
    pub email: Option<String>,
    /// Source of the detected name (for diagnostics).
    pub name_source: IdentitySource,
    /// Source of the detected email.
    pub email_source: IdentitySource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdentitySource {
    /// Detected from `git config user.name` / `git config user.email`.
    GitConfig,
    /// Detected from OS environment ($USER / $LOGNAME / whoami).
    OsUser,
    /// Not detected — fallback.
    Unknown,
}
```

### 5.3 检测优先级

```
detect_user_identity(cwd: &Path) -> UserIdentity

1. git config user.name (highest priority)
   - Runs: git config --global user.name
   - If fails, tries: git config user.name (current repo scope)
   - Sets name_source = GitConfig, name = output

2. git config user.email
   - Runs: git config --global user.email
   - If fails, tries: git config user.email (current repo scope)
   - Sets email_source = GitConfig, email = output

3. Operating system username (fallback for name only)
   - Reads env: $USER, then $LOGNAME
   - If both empty, runs: whoami
   - Only used when name is still None after step 1
   - Sets name_source = OsUser
```

### 5.4 核心函数签名

```rust
/// Detect user identity using git config and OS environment.
///
/// # Arguments
/// * `cwd` - Current working directory for repo-scoped git config lookup.
///
/// # Returns
/// `UserIdentity` with the best available information. Fields are `None`
/// only when all detection methods fail.
#[must_use]
pub fn detect_user_identity(cwd: &std::path::Path) -> UserIdentity {
    // implementation
}
```

### 5.5 辅助函数

```rust
/// Run `git config <key>` and return trimmed stdout, or `None` on failure.
fn git_config(key: &str, global: bool) -> Option<String> {
    let mut cmd = std::process::Command::new("git");
    cmd.args(["config"]);
    if global {
        cmd.arg("--global");
    }
    cmd.arg(key);

    cmd.output().ok().and_then(|output| {
        if output.status.success() {
            let value = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if value.is_empty() { None } else { Some(value) }
        } else {
            None
        }
    })
}

/// Get the OS username from environment or `whoami`.
fn os_username() -> Option<String> {
    std::env::var("USER")
        .ok()
        .filter(|s| !s.is_empty())
        .or_else(|| std::env::var("LOGNAME").ok().filter(|s| !s.is_empty()))
        .or_else(|| {
            std::process::Command::new("whoami")
                .output()
                .ok()
                .and_then(|o| {
                    if o.status.success() {
                        let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
                        if s.is_empty() { None } else { Some(s) }
                    } else {
                        None
                    }
                })
        })
}
```

### 5.6 安全验证

对检测到的值进行边界验证：

```rust
/// Sanitize a detected identity string: trim, bound length, reject control chars.
fn sanitize_identity(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    // Reject strings with control characters (except whitespace, already trimmed)
    if trimmed.chars().any(|c| c.is_control() && c != '\t') {
        return None;
    }
    // Bound to 256 bytes (reasonable name/email length)
    if trimmed.len() > 256 {
        return Some(trimmed[..256].to_string());
    }
    Some(trimmed.to_string())
}
```

### 5.7 错误回退策略

| 检测方法 | 失败处理 |
|---------|---------|
| `git config --global user.name` | 静默回退到 `git config user.name` (repo scope) |
| `git config user.name` (repo) | 静默回退到 OS 用户名 |
| OS 用户名 (`$USER`, `$LOGNAME`, `whoami`) | `name` 保持 `None`，日志写 `"unknown"` |
| `git config --global user.email` | 静默回退到 `git config user.email` (repo scope) |
| `git config user.email` (repo) | `email` 保持 `None` |

**重要**：用户检测失败不阻塞 `tokenless init`。检测是尽力而为的辅助功能，`init` 在任何情况下都要成功完成 hook 安装。

### 5.8 测试设计

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_valid_name() {
        assert_eq!(sanitize_identity("John Doe"), Some("John Doe".to_string()));
    }

    #[test]
    fn test_sanitize_empty() {
        assert_eq!(sanitize_identity("   "), None);
    }

    #[test]
    fn test_sanitize_control_chars() {
        assert_eq!(sanitize_identity("john\x00doe"), None);
    }

    #[test]
    fn test_sanitize_truncates_long() {
        let long = "a".repeat(300);
        let result = sanitize_identity(&long);
        assert!(result.is_some());
        assert_eq!(result.unwrap().len(), 256);
    }

    #[test]
    fn test_detect_user_identity_fallback_chain() {
        // Tests require mocking git and env — use integration tests
        // or test sanitize_identity and git_config separately.
    }
}
```

---

## 6. 集成流程

### 6.1 完整 `tokenless init` 执行顺序

```
tokenless init [--global] [--compress|--no-compress] [--debug] [--agent claude]
│
├─ 1. Parse CLI args → InitConfig { global, debug, compress }
│
├─ 2. Load existing TokenlessConfig from ~/.tokenfleet-ai/tokenless/config.json
│      (If file does not exist, use TokenlessConfig::default())
│
├─ 3. User identity detection (always runs, even on re-init)
│      ├─ detect_user_identity(cwd) → UserIdentity { name, email, sources }
│      └─ Update TokenlessConfig.user_name, .user_email with detected values
│
├─ 4. Resolve compress flag
│      ├─ If CLI passed --compress   → compress = true
│      ├─ If CLI passed --no-compress → compress = false
│      └─ Otherwise                   → compress = config.is_compress_enabled() (default: true)
│      └─ Update TokenlessConfig.compress_enabled with resolved value
│
├─ 5. Set TokenlessConfig.last_init_at = Utc::now().to_rfc3339()
│
├─ 6. Save TokenlessConfig to disk (create parent dirs if needed)
│
├─ 7. Delegate to agent-specific init function
│      init::run(agent, &InitConfig { global, debug, compress: resolved })
│      ├─ init_claude, init_cursor, ...
│      └─ Each function checks config.compress to decide whether to install compress hook
│
├─ 8. Print summary
│      [tokenless] Installed hooks for Claude Code (project)
│        project: my-project
│        user: John Doe <john@example.com>
│        compress: enabled
│        debug: ~/.tokenfleet-ai/tokenless/compress-debug.log
│        compress log: ~/.tokenfleet-ai/tokenless/compress.log
│        .claude/settings.json
│
└─ 9. Compress log check
       If compress.log exists and size > 100 MB, print warning:
       [tokenless] Warning: compress.log is 156 MB. Consider archiving:
         mv ~/.tokenfleet-ai/tokenless/compress.log ~/.tokenfleet-ai/tokenless/compress.log.old
```

### 6.2 `commands/init_cmd.rs` 修改

```rust
use tokenless_stats::TokenlessConfig;

pub(crate) fn handle(
    global: bool,
    agent: String,
    debug: bool,
    compress: bool,
    no_compress: bool,
) -> Result<(), (String, i32)> {
    let agent = parse_agent(&agent); // existing logic

    // Resolve compress flag
    let compress_flag = if no_compress {
        Some(false)
    } else if compress {
        Some(true)
    } else {
        None
    };

    // Load and update config
    let mut config = TokenlessConfig::load();

    // Detect user identity
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let identity = crate::init::user_detect::detect_user_identity(&cwd);
    config.set_user_identity(identity.name.clone(), identity.email.clone());

    // Resolve and persist compress preference
    let resolved_compress = compress_flag.unwrap_or_else(|| config.is_compress_enabled());
    config.compress_enabled = Some(resolved_compress);

    // Record init timestamp
    config.last_init_at = Some(chrono::Utc::now().to_rfc3339());

    // Persist config
    config.save().map_err(|e| (format!("Failed to save config: {e}"), 1))?;

    let init_config = init::InitConfig {
        global,
        debug,
        compress: Some(resolved_compress),
    };

    init::run(agent, &init_config).map_err(|e| (e, 1))
}
```

### 6.3 现有 hook_compress 集成点

`commands::hook::hook_compress()` 需要新增参数以控制压缩日志记录。日志是否写入取决于 `TokenlessConfig::is_compress_enabled()` 在运行时的值。

```rust
// In hook_compress(), after compression is done:
let config = TokenlessConfig::load();
if config.is_compress_enabled() {
    let entry = CompressLogEntry {
        timestamp: chrono::Utc::now()
            .to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
        user_name: config.effective_user_name().to_string(),
        email: config.user_email.clone(),
        project: project.clone().unwrap_or_else(|| "(unknown)".to_string()),
        agent: target.to_string(),
        hook_type: "compress".to_string(),
        before_bytes: original.len(),
        after_bytes: compressed.len(),
        saved_tokens: bt.saturating_sub(at),
        compression_pct: if bt > 0 {
            ((bt.saturating_sub(at)) as f64 / bt as f64) * 100.0
        } else {
            0.0
        },
        session_id: session_id.clone(),
        tool_use_id: tool_use_id.clone(),
        op_type: "ResponseCompression".to_string(),
    };
    let _ = compress_log::append_entry(&entry);
}
```

---

## 7. 数据流图

### 7.1 `tokenless init` 数据流

```
                    ┌──────────────────┐
                    │   User invokes   │
                    │ tokenless init   │
                    │ --compress ...   │
                    └────────┬─────────┘
                             │
                             ▼
                    ┌──────────────────┐
                    │  CLI Parser      │
                    │  (clap)          │
                    └────────┬─────────┘
                             │ InitConfig { global, debug, compress }
                             ▼
               ┌─────────────────────────┐
               │   init_cmd::handle()    │
               └────────┬────────────────┘
                        │
          ┌─────────────┼──────────────┐
          ▼             ▼              ▼
   ┌────────────┐ ┌───────────┐ ┌────────────────┐
   │ Config     │ │ User      │ │ Agent          │
   │ Load/Save  │ │ Detection │ │ Dispatch       │
   └─────┬──────┘ └─────┬─────┘ └───────┬────────┘
         │              │               │
         │              │               ▼
         │              │    ┌──────────────────┐
         │              │    │ init_claude()    │
         │              │    │ init_cursor()    │
         │              │    │ ...              │
         │              │    │ (check compress) │
         │              │    └───────┬──────────┘
         │              │            │
         ▼              ▼            ▼
┌──────────────────────────────────────────┐
│  ~/.tokenfleet-ai/tokenless/config.json  │
│  {                                       │
│    "statsEnabled": true,                 │
│    "experimentalMode": true,             │
│    "userName": "John Doe",      ◄── new  │
│    "userEmail": "john@ex.com",  ◄── new  │
│    "compressEnabled": true,     ◄── new  │
│    "lastInitAt": "2026-06-04T..."  ◄── new │
│  }                                       │
└──────────────────────────────────────────┘
```

### 7.2 压缩日志数据流

```
┌──────────────────────────────────────────────────────┐
│                  Hook Payload (stdin)                 │
│  { tool_name, tool_input, tool_result, session_id }  │
└────────────────────────┬─────────────────────────────┘
                         │
                         ▼
              ┌─────────────────────┐
              │ hook_compress()     │
              │ 1. Parse payload    │
              │ 2. Compress content │
              │ 3. Output result    │
              └──────────┬──────────┘
                         │
           ┌─────────────┴────────────┐
           │ (after compression done) │
           └─────────────┬────────────┘
                         │
                 ┌───────▼────────┐
                 │ is_compress_   │
                 │ enabled()?     │
                 └───┬────────┬───┘
                     │ No     │ Yes
                     ▼        ▼
                  (skip)  ┌─────────────────────────┐
                          │ CompressLogEntry::new() │
                          │ + append_compress_log() │
                          └───────────┬─────────────┘
                                      │
                                      ▼
                          ┌─────────────────────────┐
                          │ ~/.tokenfleet-ai/       │
                          │   tokenless/            │
                          │   compress.log (JSONL)  │
                          └─────────────────────────┘
```

### 7.3 用户检测数据流

```
detect_user_identity(cwd: &Path)
│
├─ 1. git config --global user.name
│     ├─ Success → name = output, name_source = GitConfig
│     └─ Failure
│        └─ git config user.name (repo scope)
│           ├─ Success → name = output, name_source = GitConfig
│           └─ Failure → continue to step 3
│
├─ 2. git config --global user.email
│     (same fallback pattern as step 1)
│
├─ 3. (only if name is still None) OS username
│     ├─ $USER env → Success
│     ├─ $LOGNAME env → Success
│     └─ whoami command → Success/Failure
│
└─ 4. Return UserIdentity { name, email, name_source, email_source }
         │
         ▼
   TokenlessConfig.user_name = identity.name
   TokenlessConfig.user_email = identity.email
         │
         ▼
   TokenlessConfig::save()
         │
         ▼
   CompressLogEntry.user_name = config.effective_user_name()
   CompressLogEntry.email = config.user_email
```

### 7.4 文件关系图

```
crates/
├── tokenless-stats/
│   ├── src/
│   │   ├── config.rs          ◄── TokenlessConfig 扩展 (user_name, email, compress_enabled, last_init_at)
│   │   ├── compress_log.rs    ◄── NEW: CompressLogEntry, append_compress_log()
│   │   ├── lib.rs             ◄── 导出新模块
│   │   ├── record.rs          (无变更)
│   │   └── recorder.rs        (无变更)
│   └── Cargo.toml             ◄── 可能新增 chrono 依赖
│
└── tokenless-cli/
    ├── src/
    │   ├── main.rs             ◄── Commands::Init 新增 --compress/--no-compress
    │   ├── commands/
    │   │   ├── init_cmd.rs     ◄── handle() 新增参数, config 保存逻辑
    │   │   └── hook.rs         ◄── hook_compress() 追加 compress_log 写入
    │   ├── init/
    │   │   ├── mod.rs          ◄── InitConfig 新增 compress 字段, init_claude 条件安装
    │   │   └── user_detect.rs  ◄── NEW: detect_user_identity(), sanitize_identity()
    │   └── shared.rs           ◄── 可能提取 compress_log 路径工具函数
    └── Cargo.toml              (无变更)
```

---

## 8. 实施检查清单

### Phase 1: TokenlessConfig 扩展 (tokenless-stats)

- [ ] `config.rs`: 添加 `user_name`, `user_email`, `compress_enabled`, `last_init_at` 字段
- [ ] `config.rs`: 添加 `is_compress_enabled()`, `effective_user_name()`, `set_user_identity()` 方法
- [ ] `config.rs`: 添加向后兼容性测试
- [ ] `compress_log.rs`: 新建文件，定义 `CompressLogEntry` 和 `append_compress_log()`
- [ ] `lib.rs`: 导出新模块

### Phase 2: 用户检测模块 (tokenless-cli)

- [ ] `init/user_detect.rs`: 实现 `detect_user_identity()`, `git_config()`, `os_username()`, `sanitize_identity()`
- [ ] 添加单元测试
- [ ] `init/mod.rs`: 导出新模块

### Phase 3: InitConfig + CLI 参数 (tokenless-cli)

- [ ] `init/mod.rs`: `InitConfig` 添加 `compress: Option<bool>` 字段
- [ ] `main.rs`: `Commands::Init` 添加 `--compress` 和 `--no-compress`
- [ ] `commands/init_cmd.rs`: 完整集成流程（config 加载、用户检测、保存、dispatch）

### Phase 4: 压缩日志写入 (tokenless-cli)

- [ ] `commands/hook.rs`: `hook_compress()` 追加压缩日志记录
- [ ] `init/mod.rs`: 各 agent init 函数检查 `config.compress` 条件安装

### Phase 5: 验证

- [ ] `cargo fmt --all`
- [ ] `cargo clippy --all-targets --all-features -- -D warnings -W clippy::pedantic`
- [ ] `cargo test --all-targets`
- [ ] 手动测试: `tokenless init --no-compress` 验证 hook 无 compress
- [ ] 手动测试: `tokenless init` 验证 config.json 正确写入用户信息
- [ ] 手动测试: 验证 compress.log JSONL 格式

---

## 9. 风险与注意事项

1. **git 不可用**：用户检测依赖 `git` 命令。在无 git 环境中回退到 OS 用户名，不影响 init 流程。
2. **config.json 并发写入**：当前无锁保护。`tokenless init` 和 `tokenless hook compress` 可能同时读写 config。建议 config 只在 init 写入，运行时只读。
3. **compress.log 无限增长**：当前不做自动轮转。建议在 v1 中打印警告，v2 中添加轮转子命令。
4. **隐私**：用户检测自动采集 `git config user.name/email` 并持久化到本地文件。所有数据仅存储本地 `~/.tokenfleet-ai/tokenless/`，不上传。
5. **chrono 依赖**：`tokenless-stats` 当前无 chrono 依赖。如果不想新增依赖，可以在 CLI 层生成时间戳后传入。
