//! Shared helpers for hook subcommands: text cleanup, output extraction,
//! debug logging, and compress-log entry construction.

use std::io::Write;

use tokenless_stats::compress_log::{CompressLogEntry, HookType, OpCategory, append_compress_log};
use tokenless_stats::{TokenlessConfig, estimate_tokens_from_bytes};

use crate::shared::get_home_dir;

/// Write original/compressed output to a debug log file for inspection.
pub(crate) fn write_debug_log(
    tool_name: &str,
    project: Option<&str>,
    before: &str,
    after: &str,
    action: &str,
) {
    let log_path = std::path::PathBuf::from(get_home_dir())
        .join(".tokenfleet-ai")
        .join("tokenless")
        .join("compress-debug.log");

    let truncate = |s: &str, max: usize| {
        if s.len() <= max {
            s.to_string()
        } else {
            let cut = floor_char_boundary(s, max);
            format!("{}…[truncated {}→{}]", &s[..cut], s.len(), max)
        }
    };

    let entry = serde_json::json!({
        "ts": chrono::Local::now().format("%Y-%m-%dT%H:%M:%S%.3f").to_string(),
        "tool": tool_name,
        "project": project,
        "action": action,
        "before_chars": before.len(),
        "after_chars": after.len(),
        "before": truncate(before, 4096),
        "after": truncate(after, 4096),
    });

    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
    {
        let _ = writeln!(f, "{}", serde_json::to_string(&entry).unwrap_or_default());
    }
}

/// Return the largest `char` boundary at or before byte position `index`.
#[must_use]
pub(crate) fn floor_char_boundary(s: &str, index: usize) -> usize {
    assert!(
        index <= s.len(),
        "floor_char_boundary: index {index} exceeds length {}",
        s.len()
    );
    if s.is_char_boundary(index) {
        return index;
    }
    let mut i = index;
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

/// Compress plain text output by stripping ANSI and truncating long content.
pub(crate) fn compress_plain_text(text: &str) -> String {
    const MAX_CHARS: usize = 8192;
    let cleaned = strip_ansi(text);
    if cleaned.len() > MAX_CHARS {
        let cut = floor_char_boundary(&cleaned, MAX_CHARS);
        let truncated = &cleaned[..cut];
        let lines_omitted = cleaned
            .lines()
            .count()
            .saturating_sub(truncated.lines().count());
        format!(
            "{truncated}\n…[truncated: {} → {} chars, {lines_omitted} lines omitted]",
            text.len(),
            cleaned.len(),
        )
    } else {
        cleaned.clone()
    }
}

/// Remove basic ANSI CSI sequences from text, preserving multi-byte UTF-8.
pub(crate) fn strip_ansi(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut chars = text.char_indices();
    while let Some((_byte_idx, ch)) = chars.next() {
        if ch == '\x1b' {
            if let Some((_idx, next_ch)) = chars.clone().next()
                && next_ch == '['
            {
                chars.next();
                for (_idx, c) in chars.by_ref() {
                    if is_ansi_char_terminator(c) {
                        break;
                    }
                }
                continue;
            }
            result.push(ch);
        } else {
            result.push(ch);
        }
    }
    result
}

const fn is_ansi_char_terminator(c: char) -> bool {
    matches!(c, '\x40'..='\x7e')
}

/// Infer a semantic context string from the hook payload.
pub(crate) fn infer_context(val: &serde_json::Value, tool_name: &str) -> Option<String> {
    let command = val
        .pointer("/tool_input/command")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if !command.is_empty() {
        return Some(command.to_string());
    }
    if !tool_name.is_empty() {
        return Some(tool_name.to_string());
    }
    None
}

/// Extract the actual tool output text from a `PostToolUse` hook payload.
pub(crate) fn extract_tool_output(val: &serde_json::Value) -> &str {
    // 1. Legacy: top-level "output" field
    if let Some(s) = val.get("output").and_then(|v| v.as_str())
        && !s.is_empty()
    {
        return s;
    }
    // 2. Bash: tool_response.stdout
    if let Some(s) = val
        .pointer("/tool_response/stdout")
        .and_then(|v| v.as_str())
        && !s.is_empty()
    {
        return s;
    }
    // 3. Read: tool_response.file.content
    if let Some(s) = val
        .pointer("/tool_response/file/content")
        .and_then(|v| v.as_str())
        && !s.is_empty()
    {
        return s;
    }
    // 4. Write/Edit: tool_response.content
    if let Some(s) = val
        .pointer("/tool_response/content")
        .and_then(|v| v.as_str())
        && !s.is_empty()
    {
        return s;
    }
    // 5. Fallback: stringify tool_response
    if let Some(v) = val.get("tool_response")
        && !v.is_null()
        && let Some(s) = v.as_str()
        && !s.is_empty()
    {
        return s;
    }
    ""
}

/// Append a structured compress log entry (fire-and-forget).
#[allow(clippy::cast_precision_loss)]
pub(crate) fn append_compress_log_entry(
    target: &str,
    before: &str,
    after: &str,
    project: Option<&str>,
    user_name: Option<&str>,
) {
    let config = TokenlessConfig::load();
    if !config.is_compress_enabled() && !config.is_passthrough_enabled() {
        return;
    }
    let project_name = project.map_or_else(|| "(unclassified)".to_string(), ToString::to_string);
    let effective_user = user_name.map_or_else(
        || config.effective_user_name().to_string(),
        ToString::to_string,
    );
    let before_bytes = before.len();
    let after_bytes = after.len();
    let saved_tokens = estimate_tokens_from_bytes(before_bytes)
        .saturating_sub(estimate_tokens_from_bytes(after_bytes));
    let compression_pct = if before_bytes > 0 {
        (before_bytes.saturating_sub(after_bytes)) as f64 / before_bytes as f64 * 100.0
    } else {
        0.0
    };
    let log_entry = CompressLogEntry::builder()
        .timestamp(chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true))
        .user_name(effective_user)
        .project(project_name)
        .agent(target.to_string())
        .hook_type(HookType::Compress)
        .before_bytes(before_bytes)
        .after_bytes(after_bytes)
        .saved_tokens(saved_tokens)
        .compression_pct(compression_pct)
        .op_type(OpCategory::ResponseCompression)
        .build();
    append_compress_log(&log_entry);
}
