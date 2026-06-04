//! Structured compression log in JSONL format.
//!
//! Each compression operation appends one line to
//! `~/.tokenfleet-ai/tokenless/compress.log`.

use serde::Serialize;
use typed_builder::TypedBuilder;

/// Default user name when none is configured.
fn default_user_name() -> String {
    "unknown".to_string()
}

/// Type of hook that generated this log entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum HookType {
    /// PreToolUse: command rewriting hook.
    Rewrite,
    /// PostToolUse: response compression hook.
    Compress,
}

/// Category of compression operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "PascalCase")]
pub enum OpCategory {
    /// Schema compression operation.
    SchemaCompression,
    /// Response compression operation.
    ResponseCompression,
    /// Command rewriting operation.
    RewriteCommand,
    /// Toon format encoding operation.
    ToonEncoding,
}

/// A single compress log entry, written as one JSON line in `compress.log`.
#[derive(Debug, Clone, Serialize, TypedBuilder)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct CompressLogEntry {
    /// ISO 8601 timestamp.
    #[builder(setter(into))]
    pub timestamp: String,

    /// User name for attribution. Defaults to "unknown".
    #[builder(default = default_user_name(), setter(into))]
    pub user_name: String,

    /// User email (optional).
    #[builder(default, setter(strip_option, into))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,

    /// Project name.
    #[builder(setter(into))]
    pub project: String,

    /// Agent name (claude, cursor, etc.).
    #[builder(setter(into))]
    pub agent: String,

    /// Hook type: "rewrite" or "compress".
    #[builder(setter(into))]
    pub hook_type: HookType,

    /// Original input size in bytes.
    pub before_bytes: usize,

    /// Compressed output size in bytes.
    pub after_bytes: usize,

    /// Estimated token savings.
    pub saved_tokens: usize,

    /// Compression ratio as percentage (e.g. 45.2 = 45.2%).
    pub compression_pct: f64,

    /// Session ID from hook payload.
    #[builder(default, setter(strip_option, into))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,

    /// Tool use ID from hook payload.
    #[builder(default, setter(strip_option, into))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_use_id: Option<String>,

    /// Operation category.
    #[builder(setter(into))]
    pub op_type: OpCategory,
}

/// Append a compress log entry to `~/.tokenfleet-ai/tokenless/compress.log`.
///
/// This is fire-and-forget: failures are traced via `tracing::warn!` but
/// never block compression. This function never writes to stdout.
pub fn append_compress_log(entry: &CompressLogEntry) {
    let log_path = get_compress_log_path();
    append_compress_log_to(entry, &log_path);
}

/// Append a compress log entry to a specific path (for testing).
#[allow(clippy::disallowed_methods, clippy::disallowed_types)]
fn append_compress_log_to(entry: &CompressLogEntry, path: &std::path::Path) {
    use std::io::Write;

    let line = match serde_json::to_string(entry) {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(error = %e, "compress log serialize failed");
            return;
        }
    };

    if let Some(parent) = path.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            tracing::warn!(error = %e, path = %parent.display(), "compress log parent dir failed");
            return;
        }
    }

    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }

    let mut f = match opts.open(path) {
        Ok(f) => f,
        Err(e) => {
            tracing::warn!(error = %e, path = %path.display(), "compress log open failed");
            return;
        }
    };

    if let Err(e) = writeln!(f, "{line}") {
        tracing::warn!(error = %e, "compress log write failed");
    }
}

/// Get the default compress log path.
#[must_use]
fn get_compress_log_path() -> std::path::PathBuf {
    let home = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
    home.join(".tokenfleet-ai")
        .join("tokenless")
        .join("compress.log")
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::disallowed_methods)]
mod tests {
    use super::*;

    #[test]
    fn test_compress_log_entry_serializes_camelcase() {
        let entry = CompressLogEntry::builder()
            .timestamp("2026-06-04T10:00:00Z".to_string())
            .user_name("alice".to_string())
            .project("test-project".to_string())
            .agent("claude".to_string())
            .hook_type(HookType::Compress)
            .before_bytes(1000)
            .after_bytes(400)
            .saved_tokens(150)
            .compression_pct(60.0)
            .op_type(OpCategory::ResponseCompression)
            .build();
        let json = serde_json::to_string(&entry).expect("serialize");
        assert!(json.contains("userName"));
        assert!(json.contains("alice"));
        assert!(json.contains("hookType"));
        assert!(json.contains("compress"));
        assert!(json.contains("opType"));
        assert!(json.contains("ResponseCompression"));
        assert!(json.contains("beforeBytes"));
        assert!(json.contains("compressionPct"));
    }

    #[test]
    fn test_compress_log_entry_omits_none_optionals() {
        let entry = CompressLogEntry::builder()
            .timestamp("2026-06-04T10:00:00Z".to_string())
            .user_name("alice".to_string())
            .project("test".to_string())
            .agent("claude".to_string())
            .hook_type(HookType::Rewrite)
            .before_bytes(100)
            .after_bytes(50)
            .saved_tokens(10)
            .compression_pct(50.0)
            .op_type(OpCategory::RewriteCommand)
            .build();
        let json = serde_json::to_string(&entry).expect("serialize");
        assert!(!json.contains("email"));
        assert!(!json.contains("sessionId"));
        assert!(!json.contains("toolUseId"));
    }

    #[test]
    fn test_compress_log_entry_user_name_serializes_value() {
        let entry = CompressLogEntry::builder()
            .timestamp("2026-06-04T10:00:00Z".to_string())
            .user_name("bob".to_string())
            .project("test".to_string())
            .agent("cursor".to_string())
            .hook_type(HookType::Compress)
            .before_bytes(100)
            .after_bytes(50)
            .saved_tokens(10)
            .compression_pct(50.0)
            .op_type(OpCategory::ResponseCompression)
            .build();
        let json = serde_json::to_string(&entry).expect("serialize");
        assert!(json.contains("\"userName\":\"bob\""));
    }

    #[test]
    fn test_append_compress_log_creates_file() {
        let tmp = std::env::temp_dir().join("tokenless-test-compress.log");
        let _ = std::fs::remove_file(&tmp);
        let entry = CompressLogEntry::builder()
            .timestamp("2026-06-04T10:00:00Z".to_string())
            .user_name("test".to_string())
            .project("p".to_string())
            .agent("a".to_string())
            .hook_type(HookType::Compress)
            .before_bytes(10)
            .after_bytes(5)
            .saved_tokens(1)
            .compression_pct(50.0)
            .op_type(OpCategory::ResponseCompression)
            .build();
        append_compress_log_to(&entry, &tmp);
        assert!(tmp.exists());
        let content = std::fs::read_to_string(&tmp).expect("read");
        assert!(content.contains("test"));
        std::fs::remove_file(&tmp).ok();
    }

    #[test]
    fn test_append_compress_log_appends_jsonl() {
        let tmp = std::env::temp_dir().join("tokenless-test-jsonl.log");
        let _ = std::fs::remove_file(&tmp);
        let e1 = CompressLogEntry::builder()
            .timestamp("2026-06-04T10:00:00Z".to_string())
            .user_name("a".to_string())
            .project("p1".to_string())
            .agent("a1".to_string())
            .hook_type(HookType::Compress)
            .before_bytes(10)
            .after_bytes(5)
            .saved_tokens(1)
            .compression_pct(50.0)
            .op_type(OpCategory::ResponseCompression)
            .build();
        let e2 = CompressLogEntry::builder()
            .timestamp("2026-06-04T11:00:00Z".to_string())
            .user_name("b".to_string())
            .project("p2".to_string())
            .agent("a2".to_string())
            .hook_type(HookType::Rewrite)
            .before_bytes(20)
            .after_bytes(10)
            .saved_tokens(2)
            .compression_pct(50.0)
            .op_type(OpCategory::RewriteCommand)
            .build();
        append_compress_log_to(&e1, &tmp);
        append_compress_log_to(&e2, &tmp);
        let content = std::fs::read_to_string(&tmp).expect("read");
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("p1"));
        assert!(lines[1].contains("p2"));
        std::fs::remove_file(&tmp).ok();
    }
}
