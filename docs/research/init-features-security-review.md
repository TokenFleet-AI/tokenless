# `tokenless init` 新功能安全审查报告

> 审查日期：2026-06-04 | 审查者：安全审计 | 审查对象：init-features-architecture.md v1

---

## 执行摘要

本次审查覆盖 `tokenless init` 三个新功能（压缩日志、用户检测、配置存储）的完整设计方案。共发现 **4 个 High**、**5 个 Medium**、**4 个 Low** 级别风险。

关键发现：
1. **配置文件原子写入缺失** -- `config.save()` 直接覆盖，崩溃时可导致配置丢失或损坏
2. **压缩 debug 日志可能泄露敏感信息** -- before/after 文本内容（含命令输出）写入明文日志
3. **所有数据文件缺少文件权限控制** -- 未设置 `0o600`，可能被其他用户读取
4. **用户 email 以明文存储且无用户同意机制** -- 违反隐私最小化原则

所有风险均可修复，绝大多数修复成本低。建议在实施前完成安全修复。

---

## 1. 压缩日志安全性

### 1.1 [Medium] 压缩 debug 日志可泄露敏感信息

**描述**：现有 `compress-debug.log`（`write_debug_log` 函数）将 before/after 文本内容（截断到 4096 字符）写入 `~/.tokenfleet-ai/tokenless/compress-debug.log`。该日志仅在 `--debug` 模式下启用。

新增的 `compress.log` 仅存储字节数/字符数等元数据，不会泄露内容。但 `compress-debug.log` 仍然存在，可能记录：
- Bash 命令输出中的 API keys、密码、token
- 文件读取内容中的源代码（可能含内嵌密钥）
- 数据库连接字符串
- 个人身份信息（PII）

**严重程度**：Medium（仅在 `--debug` 模式下启用，且数据存储在本地）

**修复建议**：
```rust
/// High-risk secret patterns to scan before writing debug log.
const SECRET_PATTERNS: &[(&str, &str)] = &[
    ("API Key", r"(?i)(api[_-]?key|apikey|api_secret)\s*[:=]\s*\S+"),
    ("Token", r"(?i)(token|secret|password|passwd)\s*[:=]\s*\S+"),
    ("AWS Key", r"(?i)(AKIA[0-9A-Z]{16})"),
    ("Private Key", r"-----BEGIN (RSA|EC|DSA|OPENSSH) PRIVATE KEY-----"),
    ("JWT", r"eyJ[A-Za-z0-9_-]*\.[A-Za-z0-9_-]*\.[A-Za-z0-9_-]*"),
    ("Connection String", r"(?i)(mongodb|mysql|postgres|redis)://[^@]+@"),
];

fn redact_secrets(text: &str) -> String {
    let mut result = text.to_string();
    for (label, pattern) in SECRET_PATTERNS {
        if let Ok(re) = regex::Regex::new(pattern) {
            result = re.replace_all(&result, |_: &regex::Captures| {
                format!("[REDACTED_{label}]")
            }).to_string();
        }
    }
    result
}
```

**建议测试**：
- `test_should_redact_api_key_in_debug_log`
- `test_should_redact_bearer_token_in_debug_log`
- `test_should_redact_aws_access_key_in_debug_log`

---

### 1.2 [High] 日志文件无权限控制

**描述**：`compress.log`、`compress-debug.log`、`config.json` 均通过 `std::fs::OpenOptions::new().create(true).append(true).open(...)` 或 `std::fs::write(...)` 创建，**未显式设置文件权限**。

在 Unix 系统上，新文件的权限由 umask 决定。如果用户 umask 为 `000`（或过于宽松），这些文件将对所有用户可读（`0o666`），导致：
- `config.json` 中的 `userEmail` 被其他用户读取
- `compress.log` 中的项目名称、session ID 等元数据被其他用户读取
- `compress-debug.log` 中的命令输��文本被其他用户读取

**严重程度**：High（多用户系统上可直接泄露 PII）

**修复建议**：

新建 `crates/tokenless-stats/src/fs_util.rs`（或放入现有 `shared.rs`）：
```rust
use std::fs::{self, File, OpenOptions};
use std::io;
use std::path::Path;

#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

/// Create a file with owner-only permissions (0o600).
/// On non-Unix platforms, this is equivalent to `File::create`.
pub(crate) fn create_private_file(path: &Path) -> io::Result<File> {
    let mut opts = OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    #[cfg(unix)]
    opts.mode(0o600);
    opts.open(path)
}

/// Open a file for append with owner-only permissions.
pub(crate) fn open_private_append(path: &Path) -> io::Result<File> {
    let mut opts = OpenOptions::new();
    opts.write(true).create(true).append(true);
    #[cfg(unix)]
    opts.mode(0o600);
    opts.open(path)
}

/// Write content to a file with owner-only permissions.
pub(crate) fn write_private(path: &Path, content: &str) -> io::Result<()> {
    // Write to temp file, then rename — ensures atomicity and
    // prevents partial reads by other processes.
    let tmp_path = path.with_extension("tmp");
    fs::write(&tmp_path, content)?;
    #[cfg(unix)]
    fs::set_permissions(&tmp_path, fs::Permissions::from_mode(0o600))?;
    fs::rename(&tmp_path, path)?;
    Ok(())
}

/// Create a directory with owner-only permissions (0o700).
pub(crate) fn create_private_dir(path: &Path) -> io::Result<()> {
    fs::create_dir_all(path)?;
    #[cfg(unix)]
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    Ok(())
}
```

同时，在 `get_tokenless_dir()` 创建目录时确保权限：
```rust
pub(crate) fn get_tokenless_dir() -> std::path::PathBuf {
    let dir = std::path::PathBuf::from(get_home_dir())
        .join(".tokenfleet-ai")
        .join("tokenless");
    if let Err(e) = fs::create_dir_all(&dir) {
        tracing::warn!(error = %e, path = %dir.display(), "failed to create tokenless dir");
    }
    // Ensure directory permissions are owner-only
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = fs::metadata(&dir) {
            let mut perms = meta.permissions();
            if perms.mode() & 0o077 != 0 {
                perms.set_mode(0o700);
                let _ = fs::set_permissions(&dir, perms);
            }
        }
    }
    dir
}
```

**建议测试**：
- `test_should_create_config_with_owner_only_perms`（Unix only）
- `test_should_create_log_with_owner_only_perms`（Unix only）
- `test_should_fix_perms_on_existing_tokenless_dir`（Unix only）

---

### 1.3 [Low] 100MB 日志大小检查存在 TOCTOU 竞态

**描述**：设计文档提到 "`tokenless init` 执行时如果日志文件超过 100 MB，打印警告"。此检查使用 `fs::metadata()` 读取文件大小，与后续可能的用户操作之间存在时间窗口。但由于当前设计仅"打印警告"而不自动执行任何文件操作（如轮转/截断），TOCTOU 不构成实际安全风险。

如果未来实现自动日志轮转（rename + create new），需要注意：
1. 使用 `OpenOptions::create_new(true)` 创建新文件（O_CREAT | O_EXCL）防止覆盖
2. 在 rename 前检查目标文件不存在
3. 考虑使用文件锁（`fs2` crate 或 `flock`）

**严重程度**：Low（当前设计仅打印警告，不执行文件操作）

**修复建议**：在设计文档的"未来轮转"部分添加注释，提醒使用原子重命名 + O_EXCL 创建。

---

### 1.4 [Medium] session_id 在 compress.log 中未做输入验证

**描述**：现有代码中 `append_report_to_file` 对 `session_id` 做了严格的 sanitize（仅保留 `[a-zA-Z0-9_-]`，限制 128 字符）。但设计文档中的 `CompressLogEntry` 直接使用原始 `session_id` 和 `tool_use_id`，未做任何过滤。

**严重程度**：Medium（session_id 来自外部 hook payload，攻击者可能注入包含换行符或非 ASCII 字符的值，破坏 JSONL 格式）

**修复建议**：
```rust
impl CompressLogEntry {
    /// Sanitize a correlation ID for log output:
    /// only allow `[a-zA-Z0-9_-]`, truncate to 128 chars.
    fn sanitize_id(raw: Option<&str>) -> Option<String> {
        raw.filter(|s| !s.is_empty())
            .map(|s| {
                s.chars()
                    .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
                    .take(128)
                    .collect::<String>()
            })
            .filter(|s| !s.is_empty())
    }
}
```

---

## 2. 用户检测安全性

### 2.1 [Low] `git config` 命令执行安全

**描述**：设计使用 `std::process::Command::new("git").args(["config", "--global", "user.name"])` 调用 git。通过以下分析确认安全：

- `Command::new("git")` 直接执行二进制，不经过 shell 解析
- `.arg(key)` 传递的参数不会触发 shell 展开，**不存在命令注入**
- 各个参数皆为此硬编码值，不存在用户输入拼接

`whoami` 命令同理，无参数传递更无注入风险。

**严重程度**：Low（使用 argv 执行，已安全）

**修复建议**：无需修改。可在注释中标注安全性理由。

---

### 2.2 [Medium] `sanitize_identity` 对 Unicode 处理不完整

**描述**：设计中的 `sanitize_identity` 函数：
```rust
fn sanitize_identity(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() { return None; }
    // Reject control chars (except tab)
    if trimmed.chars().any(|c| c.is_control() && c != '\t') { return None; }
    // Bound to 256 bytes
    if trimmed.len() > 256 { return Some(trimmed[..256].to_string()); }
    Some(trimmed.to_string())
}
```

存在以下问题：
1. **允许 `\t` (tab)** -- 可能在日志输出中对齐混乱，但不会造成注入
2. **未过滤 Unicode 双向控制字符** -- `\u{202A}`–`\u{202E}`、`\u{2066}`–`\u{2069}` 可能被用于文件名欺骗攻击（与 `SafePath` 的防御一致）
3. **未过滤零宽字符** -- `\u{200B}`（零宽空格）等可能造成显示欺骗
4. **未做 Unicode 规范化** -- NFC/NFD 不一致可能导致同一用户名在不同表示下重复存储
5. **按字节截断可能截断多字节 UTF-8 字符** -- `trimmed[..256]` 可能在多字节字符中间切割，产生无效 UTF-8

**严重程度**：Medium（用户名被注入日志文件和 JSON 输出，虽不直接导致 RCE，但可能破坏日志格式或造成显示欺骗）

**修复建议**：
```rust
/// Bidirectional control characters (same list as `SafePath`).
const BIDI_CONTROLS: &[char] = &[
    '\u{202A}', '\u{202B}', '\u{202C}', '\u{202D}', '\u{202E}',
    '\u{2066}', '\u{2067}', '\u{2068}', '\u{2069}',
];

/// Zero-width and invisible characters.
const ZERO_WIDTH_CHARS: &[char] = &[
    '\u{200B}', // ZERO WIDTH SPACE
    '\u{200C}', // ZERO WIDTH NON-JOINER
    '\u{200D}', // ZERO WIDTH JOINER
    '\u{FEFF}', // BOM / ZERO WIDTH NO-BREAK SPACE
    '\u{00AD}', // SOFT HYPHEN
];

/// Maximum allowed byte length for sanitized identity values.
const MAX_IDENTITY_BYTES: usize = 256;

/// Sanitize a detected identity string for safe storage and display.
///
/// 1. Trim whitespace (including Unicode whitespace via NFC normalization).
/// 2. Reject empty strings.
/// 3. Reject C0 control characters (except HTAB).
/// 4. Reject Unicode bidirectional control characters.
/// 5. Reject zero-width/invisible characters.
/// 6. Apply Unicode NFC normalization for consistency.
/// 7. Truncate to MAX_IDENTITY_BYTES bytes on a valid UTF-8 boundary.
#[must_use]
fn sanitize_identity(raw: &str) -> Option<String> {
    // Normalize first (NFC) so trimming catches decomposed whitespace
    let normalized = unicode_normalization::UnicodeNormalization::nfc(raw)
        .collect::<String>();
    let trimmed = normalized.trim();
    if trimmed.is_empty() {
        return None;
    }

    // Reject C0 controls except horizontal tab
    if trimmed.chars().any(|c| c.is_control() && c != '\t') {
        return None;
    }

    // Reject bidirectional control characters
    if trimmed.chars().any(|c| BIDI_CONTROLS.contains(&c)) {
        return None;
    }

    // Strip zero-width/invisible characters rather than rejecting
    let cleaned: String = trimmed
        .chars()
        .filter(|c| !ZERO_WIDTH_CHARS.contains(c))
        .collect();

    if cleaned.is_empty() {
        return None;
    }

    // Truncate on valid UTF-8 boundary
    let bytes = cleaned.as_bytes();
    if bytes.len() > MAX_IDENTITY_BYTES {
        // Find the last valid UTF-8 boundary at or before MAX_IDENTITY_BYTES
        let mut end = MAX_IDENTITY_BYTES;
        while end > 0 && (bytes[end] & 0xC0) == 0x80 {
            end -= 1;
        }
        Some(String::from_utf8_lossy(&bytes[..end]).to_string())
    } else {
        Some(cleaned)
    }
}
```

**依赖变更**：新增 `unicode-normalization` crate（纯 Rust，轻量级）。

**建议测试**：
- `test_should_reject_bidi_control_chars`
- `test_should_strip_zero_width_chars`
- `test_should_normalize_nfc`
- `test_should_truncate_on_utf8_boundary`
- `test_should_truncate_multibyte_char_cleanly`
- `test_should_reject_only_invisible_chars`

---

### 2.3 [High] 用户 email 明文存储且无同意机制

**描述**：设计在 `tokenless init` 时自动采集 `git config user.email` 并持久化到 `~/.tokenfleet-ai/tokenless/config.json`。虽然此文件仅存储在本地，但：

1. **无用户同意步骤** -- 用户执行 `tokenless init` 时，email 被静默采集存储
2. **email 以明文 JSON 存储** -- 任何可读取该文件的进程均可获取
3. **email 会被写入 compress.log** -- 每行 JSONL 日志包含 `email` 字段（虽然标记为 `skip_serializing_if = "Option::is_none"`）
4. **CLAUDE.md 安全规范要求使用 `secrecy` crate 包裹敏感信息** -- 该规范在此处未被执行

**严重程度**：High（PII 未受保护存储，不符合隐私最佳实践及项目安全规范）

**修复建议**：

**(a) 添加用户同意确认（推荐为默认方案）**：
```rust
/// Prompt user for consent before collecting identity information.
fn prompt_identity_consent() -> bool {
    use std::io::{self, Write};
    print!("[tokenless] Detect user identity (git config user.name/email) for attribution? [Y/n] ");
    let _ = io::stdout().flush();
    let mut input = String::new();
    if io::stdin().read_line(&mut input).is_err() {
        return false;
    }
    let input = input.trim().to_lowercase();
    input.is_empty() || input == "y" || input == "yes"
}
```

**(b) 对 email 字段使用 `secrecy::SecretString`**：
```rust
use secrecy::{ExposeSecret, SecretString};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenlessConfig {
    // ...
    /// User email (serialized as redacted, deserialized via SecretString).
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_email_redacted",
        deserialize_with = "deserialize_email_secret"
    )]
    pub user_email: Option<SecretString>,
}

fn serialize_email_redacted<S>(email: &Option<SecretString>, s: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    match email {
        Some(e) => {
            // Store only a hash prefix for verification, not the actual email
            let hash = sha256_prefix(e.expose_secret());
            s.serialize_some(&hash)
        }
        None => s.serialize_none(),
    }
}

fn sha256_prefix(input: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    format!("sha256:{:x}", &hasher.finalize()[..8])
}
```

**或者更简单的方案**（如果 email 仅用于显示，不需要可逆性）：
- 取消存储 email 明文
- 若必须用于 agent-proxy 上报，在写入日志时实时从 `git config` 读取而不持久化
- 在 compress.log 的 `email` 字段中使用 SHA-256 前 8 字节的哈希代替明文

**(c) 文件权限保护**：结合 1.2 节的 `write_private` 修复，确保 config.json 权限为 `0o600`。

**建议测试**：
- `test_should_prompt_before_collecting_email`
- `test_should_store_email_hash_not_plaintext`
- `test_should_not_include_email_in_compress_log_when_none`

---

## 3. TokenlessConfig 存储安全性

### 3.1 [High] `config.save()` 静默覆盖导致数据丢失

**描述**：现有 `TokenlessConfig::save()` 实现：
```rust
pub fn save(&self) -> Result<(), std::io::Error> {
    let path = Self::config_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let content = serde_json::to_string_pretty(self).unwrap_or_default();
    std::fs::write(&path, content)
}
```

存在两个严重问题：

1. **`unwrap_or_default()` 在序列化失败时返回空字符串**，然后 `std::fs::write` 会将空字符串写入配置文件，**彻底破坏 config.json**（原内容丢失，且无法被后续 `serde_json::from_str` 成功解析为 `TokenlessConfig`）

2. **`std::fs::write` 非原子操作** -- 如果进程在写入中途崩溃（如断电、OOM kill），配置文件将处于损坏状态（部分写入）

**严重程度**：High（可导致用户配置永久丢失，无法恢复）

**修复建议**：

```rust
/// Save configuration atomically using write-to-temp + rename.
///
/// # Errors
///
/// Returns an I/O error if:
/// - The configuration cannot be serialized to JSON
/// - The parent directory cannot be created
/// - The temp file cannot be written
/// - The rename operation fails
pub fn save(&self) -> Result<(), std::io::Error> {
    let path = Self::config_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    // Serialize first — fail loudly if config can't be serialized
    let content = serde_json::to_string_pretty(self).map_err(|e| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("failed to serialize config: {e}"),
        )
    })?;

    // Write to temp file first, then atomically rename
    let tmp_path = path.with_extension("tmp");
    fs::write(&tmp_path, content)?;

    // Ensure temp file has correct permissions
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&tmp_path, fs::Permissions::from_mode(0o600))?;
    }

    // Atomic rename (on most filesystems)
    fs::rename(&tmp_path, &path)?;

    Ok(())
}
```

**建议测试**：
- `test_should_return_error_on_serialize_failure_not_write_empty` -- 模拟序列化失败
- `test_should_preserve_existing_config_on_write_failure` -- 写失败时原文件不受影响
- `test_should_atomically_replace_config` -- 验证 rename 模式
- `test_should_not_create_partial_config_file` -- 中途崩溃不会产生残缺文件

---

### 3.2 [Medium] `config.load()` 返回默认值掩盖错误

**描述**：现有 `load()` 实现：
```rust
pub fn load() -> Self {
    let path = Self::config_path();
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}
```

问题：
- 文件存在但 JSON 无效（损坏）时，静默回退到默认值，用户不知道配置文件损坏
- 文件存在但权限不足无法读取时，同样静默回退
- 无法区分"文件不存在"和"文件损坏"两种情况

**严重程度**：Medium（信息丢失而非安全漏洞，但可能导致用户困惑）

**修复建议**：
```rust
pub fn load() -> Self {
    let path = Self::config_path();
    match std::fs::read_to_string(&path) {
        Ok(content) => {
            match serde_json::from_str(&content) {
                Ok(config) => config,
                Err(e) => {
                    tracing::warn!(
                        error = %e,
                        path = %path.display(),
                        "failed to parse config.json, using defaults"
                    );
                    Self::default()
                }
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            // File doesn't exist yet — expected on first run
            Self::default()
        }
        Err(e) => {
            tracing::warn!(
                error = %e,
                path = %path.display(),
                "failed to read config.json, using defaults"
            );
            Self::default()
        }
    }
}
```

---

### 3.3 [Low] serde JSON 反序列化无输入大小限制

**描述**：`serde_json::from_str` 对输入大小无限制。虽然 config.json 本身由程序写入（可控），但如果文件被外部篡改（恶意用户或恶意进程），可能注入超大型 JSON 导致内存耗尽。

**严重程度**：Low（config.json 仅本地进程写入，攻击面极小）

**修复建议**：在 `load()` 中读取文件前检查文件大小：
```rust
const MAX_CONFIG_SIZE: u64 = 64 * 1024; // 64 KiB — config should be tiny

pub fn load() -> Self {
    let path = Self::config_path();
    // Check file size before reading
    if let Ok(meta) = std::fs::metadata(&path) {
        if meta.len() > MAX_CONFIG_SIZE {
            tracing::warn!(
                size = meta.len(),
                path = %path.display(),
                "config.json exceeds size limit, using defaults"
            );
            return Self::default();
        }
    }
    // ... rest of load logic
}
```

---

### 3.4 [Low] 配置文件并发读写无锁保护

**描述**：设计文档在"风险与注意事项"第 2 点自述了此问题，并建议 "config 只在 init 写入，运行时只读"。这是一个合理的设计约束但需要强制执行。

当前：
- `tokenless init` 写入 config
- `tokenless hook compress` 读取 config（通过 `TokenlessConfig::load()` 的 `is_compress_enabled()` / `is_stats_enabled()` 调用）
- 如果用户同时运行 `tokenless init` 和 agent（触发 hook），存在并发读写

**严重程度**：Low（竞态窗口极小，且 3.1 节的原子写入已缓解数据损坏问题）

**修复建议**：
1. 在 `TokenlessConfig` 文档注释中明确标注 "只应在 `tokenless init` 时写入，运行时只读"
2. 将 `save()` 标记为 `#[doc(hidden)]` 或 `pub(crate)` 限制可见性
3. hook 路径中使用缓存的 `TokenlessConfig` 而非每次重新加载（可用 `OnceLock` 实现）：

```rust
/// Cached config loaded once per process lifetime. Hook invocations
/// are short-lived subprocesses, so each gets a fresh load.
/// For long-running daemons, reload would be needed.
static CONFIG: LazyLock<TokenlessConfig> = LazyLock::new(TokenlessConfig::load);
```

---

## 4. 输入验证

### 4.1 [Low] `--compress` / `--no-compress` 参数

**描述**：这两个参数为布尔标记（flag），由 clap 解析为 `bool` 或存在性检测。用户不能注入任意值。`conflicts_with` 确保互斥。

**严重程度**：Low（无注入向量）

**修复建议**：无需修改。

---

### 4.2 [Medium] 项目名称在 hook 命令中未转义

**描述**：在 `init_claude` 中，项目名称通过 `format!` 插入到 hook JSON 中：
```rust
let hooks_json = format!(
    r#"..."command": "tokenless hook compress --semantic --target claude --project {project}"..."#,
    project = project_name,
);
```

`project_name` 来自 `detect_project_name()`，其值为：
- git remote URL 解析结果（`extract_repo_name`）-- 仅包含 `/` 和 `:` 分割后的最后一段，相对可控
- `Cargo.toml` 的 `package.name` -- 使用简单的 `trim_matches('"')` 解析，可能包含 shell 元字符（如 `$()`、反引号等）
- `package.json` 的 `name` 字段 -- 同上
- 目录名 -- 可包含任意字符

如果项目名包含双引号 `"`，会破坏 JSON 结构导致 hook 配置无效。如果包含反斜杠或控制字符，也可能破坏 JSON 解析。

**注意**：此处注入发生在 JSON 字符串值内部（`"command": "...{project}..."`），最终由 agent 的 JSON 解析器读取并以 argv 形式执行（不经过 shell），因此 shell 注入风险较低。但 JSON 结构破坏会导致 hook 失效。

**严重程度**：Medium（可能导致 hook 配置无效或 agent 设置解析失败）

**修复建议**：
```rust
/// Escape a string for safe embedding in a JSON string value.
/// Escapes: backslash, double quote, and control characters.
fn json_escape(s: &str) -> String {
    let mut escaped = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            c if c.is_control() => {
                escaped.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => escaped.push(c),
        }
    }
    escaped
}
```

在 `init_claude` 中使用：
```rust
let hooks_json = format!(
    r#"..."command": "tokenless hook compress --semantic --target claude --project {project}"..."#,
    project = json_escape(&project_name),
);
```

更好的方案是使用 `serde_json::json!` 宏构造 JSON，避免手动拼接：
```rust
let hooks = serde_json::json!({
    "env": { "RTK_SKIP_HOOK_CHECK": "1" },
    "hooks": {
        "PreToolUse": [{
            "matcher": "Bash",
            "hooks": [{
                "type": "command",
                "command": format!("tokenless hook rewrite --target claude --project {project_name}")
            }]
        }],
        "PostToolUse": [{
            "matcher": "^(?!Bash$).*",
            "hooks": [{
                "type": "command",
                "command": format!("tokenless hook compress --semantic --target claude --project {project_name}{debug_flag}")
            }]
        }]
    }
});
let hooks_json = serde_json::to_string_pretty(&hooks)?;
```

**建议测试**：
- `test_should_escape_double_quote_in_project_name`
- `test_should_handle_project_name_with_backslash`
- `test_should_use_serde_json_for_hook_generation`

---

## 5. 安全测试用例汇总

以下为建议新增的安全测试用例（除文档各节中已列出的之外）：

### 5.1 TokenlessConfig 测试

```rust
#[cfg(test)]
mod security_tests {
    use super::*;

    // ── Atomic save ──

    #[test]
    fn test_save_does_not_corrupt_on_serialize_failure() {
        // If serialization fails, the file should remain unchanged
    }

    #[test]
    fn test_save_uses_atomic_rename() {
        // Verify no .tmp file lingers after successful save
    }

    #[test]
    fn test_old_config_preserved_on_save_crash() {
        // Simulate crash during write: original file intact
    }

    // ── Permission ──

    #[cfg(unix)]
    #[test]
    fn test_config_file_has_owner_only_perms() {
        // config.json should be 0o600
    }

    #[cfg(unix)]
    #[test]
    fn test_compress_log_has_owner_only_perms() {
        // compress.log should be 0o600
    }

    // ── Email protection ──

    #[test]
    fn test_email_not_logged_when_none() {
        // Verify email field skipped when None
    }

    #[test]
    fn test_email_serialized_as_hash_not_plaintext() {
        // If using hashed storage
    }

    // ── Config size limit ──

    #[test]
    fn test_rejects_oversized_config_file() {
        // > 64 KiB config.json returns defaults
    }

    // ── Backward compatibility ──

    #[test]
    fn test_old_config_without_new_fields_still_loads() {
        let json = r#"{"statsEnabled":true,"experimentalMode":false}"#;
        let config: TokenlessConfig = serde_json::from_str(json).unwrap();
        assert!(config.user_name.is_none());
        assert!(config.user_email.is_none());
        assert!(config.compress_enabled.is_none());
    }

    #[test]
    fn test_unknown_fields_ignored() {
        let json = r#"{"statsEnabled":true,"experimentalMode":false,"unknownField":42}"#;
        let config: TokenlessConfig = serde_json::from_str(json).unwrap();
        assert!(config.stats_enabled);
    }
}
```

### 5.2 用户检测测试

```rust
// ── sanitize_identity ──

#[test]
fn test_rejects_null_byte() {
    assert_eq!(sanitize_identity("user\x00name"), None);
}

#[test]
fn test_rejects_bidi_override() {
    assert_eq!(sanitize_identity("user\u{202E}malicious"), None);
}

#[test]
fn test_strips_zero_width_space() {
    assert_eq!(sanitize_identity("user\u{200B}name"), Some("username".to_string()));
}

#[test]
fn test_truncate_preserves_utf8_boundary() {
    // 'é' is 2 bytes in UTF-8. Test truncation at 255 bytes doesn't split it.
    let input = "a".repeat(255) + "é";
    let result = sanitize_identity(&input).unwrap();
    assert!(std::str::from_utf8(result.as_bytes()).is_ok());
}

#[test]
fn test_normalize_nfc() {
    // U+0065 (e) + U+0301 (combining acute) should normalize to U+00E9 (é)
    let input = "cafe\u{0301}"; // cafe + combining acute accent
    let result = sanitize_identity(input).unwrap();
    assert_eq!(result, "caf\u{00E9}"); // café as single codepoint
}

#[test]
fn test_reject_empty_after_sanitize() {
    assert_eq!(sanitize_identity("\u{200B}\u{200C}\u{200D}"), None);
}
```

### 5.3 压缩日志测试

```rust
#[test]
fn test_session_id_sanitized_in_log() {
    let raw = "abc\n123\x00def!@#";
    let sanitized = CompressLogEntry::sanitize_id(Some(raw));
    assert_eq!(sanitized, Some("abc123def".to_string()));
}

#[test]
fn test_session_id_truncated_to_128_chars() {
    let raw = "a".repeat(200);
    let sanitized = CompressLogEntry::sanitize_id(Some(&raw));
    assert_eq!(sanitized.unwrap().len(), 128);
}

#[test]
fn test_debug_log_redacts_api_keys() {
    let text = "Authorization: Bearer sk-abc123def456";
    let redacted = redact_secrets(text);
    assert!(!redacted.contains("sk-abc123def456"));
    assert!(redacted.contains("[REDACTED_Token]"));
}
```

### 5.4 集成测试

```rust
#[test]
fn test_init_no_compress_does_not_leak_pii() {
    // End-to-end: run init --no-compress, verify config.json:
    // - has owner-only perms (0o600)
    // - userEmail not stored in plaintext
    // - no compress log created
}

#[test]
fn test_concurrent_init_and_hook_no_config_corruption() {
    // Stress test: run init in one thread while hook reads config
}
```

---

## 6. 风险汇总

| # | 风险 | 分类 | 严重程度 | 修复成本 |
|---|------|------|---------|---------|
| 1 | config.save() 原子写入缺失 | 配置存储 | **High** | 低 |
| 2 | 文件权限未设为 0o600/0o700 | 压缩日志 | **High** | 低 |
| 3 | 用户 email 明文存储无同意 | 用户检测 | **High** | 中 |
| 4 | 项目名未转义破坏 JSON | 输入验证 | **Medium** | 低 |
| 5 | debug 日志泄露文本内容 | 压缩日志 | **Medium** | 中 |
| 6 | session_id 未 sanitize | 压缩日志 | **Medium** | 低 |
| 7 | sanitize_identity Unicode 不完整 | 用户检测 | **Medium** | 中 |
| 8 | config.load() 静默回退默认值 | 配置存储 | **Medium** | 低 |
| 9 | config.load() 无输入大小限制 | 配置存储 | **Low** | 低 |
| 10 | 100MB 日志检查 TOCTOU | 压缩日志 | **Low** | 低 |
| 11 | CLI 参数布尔标记无注入 | 输入验证 | **Low** | -- |
| 12 | git/whoami 命令执行安全 | 用户检测 | **Low** | -- |
| 13 | 配置文件并发无锁 | 配置存储 | **Low** | 中 |

---

## 7. 修复优先级建议

**Phase 1 -- 实施前必须修复：**
1. 配置文件原子写入（3.1）
2. 文件权限设为 0o600/0o700（1.2, 3.1）
3. 项目名 JSON 转义（4.2）

**Phase 2 -- 实施后高优先级：**
4. 用户 email 存储方案评审（2.3）
5. session_id sanitize（1.4）
6. sanitize_identity Unicode 加固（2.2）

**Phase 3 -- 后续迭代：**
7. debug 日志密钥脱敏（1.1）
8. config.load() 错误日志（3.2）
9. config.load() 大小限制（3.3）
10. 并发访问文档化（3.4）
