//! Handlers for `tokenless hook` subcommands.

use std::io::Write;

use tokenless_stats::compress_log::{CompressLogEntry, HookType, OpCategory, append_compress_log};
use tokenless_stats::{OperationType, TokenlessConfig, estimate_tokens_from_bytes};

use crate::{
    cache,
    shared::{
        SEMANTIC_COMPRESSOR, compressor_for_tool, get_home_dir, is_experimental_enabled,
        read_input, record_compression_stats, strip_leading_bom,
    },
};

/// Handle `tokenless hook rewrite` for a specific agent target.
pub(crate) fn hook_rewrite(target: &str, project: Option<String>) -> Result<(), (String, i32)> {
    if target != "claude" {
        return Err((
            format!("Hook rewrite not yet implemented for agent: {target}"),
            1,
        ));
    }
    let input = read_input(&None).map_err(|e| (e, 2))?;
    let input = strip_leading_bom(&input);
    let val: serde_json::Value =
        serde_json::from_str(&input).map_err(|e| (format!("JSON parse error: {e}"), 2))?;
    let cmd = val.get("tool_name").and_then(|v| v.as_str()).unwrap_or("");
    if cmd != "Bash" {
        println!("{input}");
        return Ok(());
    }
    let command = val
        .pointer("/tool_input/command")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if command.is_empty() {
        println!("{input}");
        return Ok(());
    }
    let session_id = val
        .get("session_id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    // Passthrough mode: pass through original, log with zero savings
    let config = TokenlessConfig::load();
    if config.is_passthrough_enabled() {
        println!("{input}");
        record_compression_stats(
            OperationType::RewriteCommand,
            Some(target.to_string()),
            session_id,
            None,
            project.clone(),
            input.clone(),
            input,
            false,
            Some("Passthrough".into()),
        );
        return Ok(());
    }

    let rewritten = rtk_registry::rewrite_command(command, &[], &[]);
    if let Some(ref rw) = rewritten {
        if let Some(obj) = val.as_object() {
            let mut new_val = obj.clone();
            if let Some(tool_input) = new_val.get_mut("tool_input") {
                if let Some(obj) = tool_input.as_object_mut() {
                    obj.insert("command".to_string(), serde_json::Value::String(rw.clone()));
                }
            }
            let output = serde_json::to_string(&new_val).unwrap_or_default();
            println!("{output}");
            record_compression_stats(
                OperationType::RewriteCommand,
                Some(target.to_string()),
                session_id,
                None,
                project,
                input,
                output,
                false,                      // RTK rewrite is always core
                Some("RtkStandard".into()), // method
            );
            return Ok(());
        }
    }
    println!("{input}");
    Ok(())
}

/// Handle `tokenless hook compress` — response compression via hook protocol.
pub(crate) fn hook_compress(
    semantic: bool,
    target: &str,
    project: Option<String>,
    debug: bool,
) -> Result<(), (String, i32)> {
    let input = read_input(&None).map_err(|e| (e, 2))?;
    let input = strip_leading_bom(&input);
    let val: serde_json::Value =
        serde_json::from_str(&input).map_err(|e| (format!("JSON parse error: {e}"), 2))?;
    let tool_name = val.get("tool_name").and_then(|v| v.as_str()).unwrap_or("");
    let output = extract_tool_output(&val);
    let session_id = val
        .get("session_id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    // Passthrough mode: pass through original, log with zero savings
    let config = TokenlessConfig::load();
    if config.is_passthrough_enabled() {
        println!("{input}");
        if !output.is_empty() {
            append_compress_log_entry(target, output, output, project.as_deref());
        }
        return Ok(());
    }

    if output.is_empty() {
        println!("{input}");
        if debug {
            // Dump raw payload to diagnose missing field names
            write_debug_log(tool_name, &project, &input, "", "empty");
        }
        return Ok(());
    }
    if let Ok(mut output_val) = serde_json::from_str::<serde_json::Value>(output) {
        // Semantic-aware field filtering: infer context from tool input.
        let use_semantic = semantic && is_experimental_enabled();
        if use_semantic {
            if let Some(context) = infer_context(&val, tool_name) {
                let mut sc = SEMANTIC_COMPRESSOR
                    .lock()
                    .map_err(|e| (format!("Semantic compressor lock error: {e}"), 1))?;
                let _ = sc.load_onnx(); // no-op after first success, degrades to Level 1
                output_val = sc.compress(&output_val, &context);
            }
        }

        let compressor = compressor_for_tool(tool_name);
        let compressed = compressor.compress(&output_val);
        let compressed_str = serde_json::to_string(&compressed).unwrap_or_default();
        if let Some(obj) = val.as_object() {
            let mut new_val = obj.clone();
            new_val.insert(
                "output".to_string(),
                serde_json::Value::String(compressed_str.clone()),
            );
            let output_text = serde_json::to_string(&new_val).unwrap_or_default();
            println!("{output_text}");
            // Use the actual tool output text for before/after comparison,
            // not the entire hook payload (which may contain tool_response
            // fields that inflate the after-text size).
            let method = if use_semantic {
                "Semantic"
            } else if tool_name.eq_ignore_ascii_case("Bash") {
                "HighFidelity"
            } else {
                "Standard"
            };
            record_compression_stats(
                tokenless_stats::OperationType::CompressResponse,
                Some(target.to_string()),
                session_id,
                None,
                project.clone(),
                output.to_string(),
                compressed_str.clone(),
                use_semantic, // experimental only when semantic is active
                Some(method.into()),
            );

            // Debug log: write original + compressed to a file for inspection.
            if debug {
                write_debug_log(tool_name, &project, output, &compressed_str, "compressed");
            }

            // Append structured compress log (fire-and-forget).
            append_compress_log_entry(target, output, &compressed_str, project.as_deref());
        }
    } else {
        // Plain text output: strip ANSI and truncate if too long.
        let cleaned = compress_plain_text(output);
        if cleaned.len() < output.len() {
            if let Some(obj) = val.as_object() {
                let mut new_val = obj.clone();
                new_val.insert(
                    "output".to_string(),
                    serde_json::Value::String(cleaned.clone()),
                );
                let output_text = serde_json::to_string(&new_val).unwrap_or_default();
                println!("{output_text}");
                record_compression_stats(
                    tokenless_stats::OperationType::CompressResponse,
                    Some(target.to_string()),
                    session_id,
                    None,
                    project.clone(),
                    output.to_string(),
                    cleaned.clone(),
                    false,                   // plain text compression is always core
                    Some("Standard".into()), // method
                );
                if debug {
                    write_debug_log(tool_name, &project, output, &cleaned, "compressed");
                }

                // Append structured compress log (fire-and-forget).
                append_compress_log_entry(target, output, &cleaned, project.as_deref());
            }
        } else {
            println!("{input}");
            if debug {
                write_debug_log(tool_name, &project, output, output, "passthrough");
            }
        }
    }
    Ok(())
}

/// Handle `tokenless hook diff` — differential response compression.
pub(crate) fn hook_diff() -> Result<(), (String, i32)> {
    let input = read_input(&None).map_err(|e| (e, 2))?;
    let input = strip_leading_bom(&input);
    let val: serde_json::Value =
        serde_json::from_str(&input).map_err(|e| (format!("JSON parse error: {e}"), 2))?;
    let cmd = val.get("command").and_then(|v| v.as_str()).unwrap_or("");
    let output = val.get("output").and_then(|v| v.as_str()).unwrap_or("");

    if !is_experimental_enabled() {
        // Passthrough: diff compression is experimental
        println!("{output}");
        return Ok(());
    }

    if let Some(diff) = cache::compute_diff(cmd, output) {
        println!("{diff}");
    } else {
        println!("{output}");
    }
    Ok(())
}

/// Write original/compressed output to a debug log file for inspection.
///
/// Log file: `~/.tokenless/compress-debug.log` (JSON Lines, one entry per line).
/// Truncates text to 4096 chars each to keep the file manageable.
///
/// `action`: `"compressed"` (JSON, stats recorded), `"passthrough"` (non-JSON,
/// skipped), or `"empty"` (no output found).
fn write_debug_log(
    tool_name: &str,
    project: &Option<String>,
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
            format!("{}…[truncated {}→{}]", &s[..max], s.len(), max)
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

/// Compress plain text output by stripping ANSI escape codes and truncating
/// excessively long content.
///
/// Returns the compressed text (or original if no compression possible).
fn compress_plain_text(text: &str) -> String {
    // Max chars to keep — beyond this the LLM gets diminishing returns.
    const MAX_CHARS: usize = 8192;

    // Strip ANSI escape sequences (color codes, cursor movement, etc.).
    let cleaned = strip_ansi(text);

    // Truncate if still too long.
    if cleaned.len() > MAX_CHARS {
        let truncated = &cleaned[..MAX_CHARS];
        format!(
            "{truncated}\n…[truncated: {} → {} chars, {} lines omitted]",
            text.len(),
            cleaned.len(),
            cleaned
                .lines()
                .count()
                .saturating_sub(truncated.lines().count()),
        )
    } else {
        cleaned.to_string()
    }
}

/// Remove basic ANSI escape sequences (CSI sequences) from text.
///
/// Matches patterns like `\x1b[0m`, `\x1b[32m`, `\x1b[1;32m`, etc.
fn strip_ansi(text: &str) -> String {
    // Simple state-machine: skip everything from ESC (0x1b) through the
    // terminating byte (usually 'm', but also 'A'-'H', 'J', 'K', 's', 'u', etc.).
    let mut result = String::with_capacity(text.len());
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == 0x1b && i + 1 < bytes.len() && bytes[i + 1] == b'[' {
            // Skip ESC[ ... terminating byte
            i += 2;
            while i < bytes.len() && !is_ansi_terminator(bytes[i]) {
                i += 1;
            }
            if i < bytes.len() {
                i += 1; // skip the terminator
            }
        } else {
            result.push(bytes[i] as char);
            i += 1;
        }
    }
    result
}

/// Check if a byte is an ANSI CSI sequence terminator.
const fn is_ansi_terminator(b: u8) -> bool {
    matches!(
        b,
        b'm' | b'A'..=b'H' | b'J' | b'K' | b's' | b'u' | b'h' | b'l'
    )
}

/// Infer a semantic context string from the hook payload.
///
/// Uses `tool_input.command` (Bash) or the `tool_name` itself to guess
/// the user's task domain.
fn infer_context(val: &serde_json::Value, tool_name: &str) -> Option<String> {
    // Bash: use the command itself as context
    let command = val
        .pointer("/tool_input/command")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if !command.is_empty() {
        return Some(command.to_string());
    }
    // Other tools: use tool_name as context
    if !tool_name.is_empty() {
        return Some(tool_name.to_string());
    }
    None
}

/// Extract the actual tool output text from a PostToolUse hook payload.
///
/// Claude Code sends the tool result in `tool_response`, whose shape varies by
/// tool type:
/// - Bash: `tool_response.stdout` (command output)
/// - Read / Write / Edit / MultiEdit / Grep: `tool_response.content`
/// - Other tools / legacy: top-level `output` field
///
/// Returns the output text, or an empty string if nothing is found.
fn extract_tool_output(val: &serde_json::Value) -> &str {
    // 1. Legacy: top-level "output" field
    if let Some(s) = val.get("output").and_then(|v| v.as_str()) {
        if !s.is_empty() {
            return s;
        }
    }
    // 2. Bash: tool_response.stdout
    if let Some(s) = val
        .pointer("/tool_response/stdout")
        .and_then(|v| v.as_str())
    {
        if !s.is_empty() {
            return s;
        }
    }
    // 3. Read: tool_response.file.content
    if let Some(s) = val
        .pointer("/tool_response/file/content")
        .and_then(|v| v.as_str())
    {
        if !s.is_empty() {
            return s;
        }
    }
    // 4. Write / Edit: tool_response.content
    if let Some(s) = val
        .pointer("/tool_response/content")
        .and_then(|v| v.as_str())
    {
        if !s.is_empty() {
            return s;
        }
    }
    // 5. Fallback: stringify entire tool_response object
    if let Some(v) = val.get("tool_response") {
        if !v.is_null() {
            // Only if it's a plain string value
            if let Some(s) = v.as_str() {
                if !s.is_empty() {
                    return s;
                }
            }
        }
    }
    ""
}

/// Append a structured compress log entry after response compression.
///
/// Reads compression preference from `TokenlessConfig` and skips if
/// compress logging is disabled.  This is fire-and-forget: failures
/// are traced but never block the compression pipeline.
fn append_compress_log_entry(target: &str, before: &str, after: &str, project: Option<&str>) {
    let config = TokenlessConfig::load();
    // Skip logging only when compress is disabled AND passthrough is off.
    // In passthrough mode we always log (for baseline measurement).
    if !config.is_compress_enabled() && !config.is_passthrough_enabled() {
        return;
    }
    let project_name = project
        .map(|s| s.to_string())
        .unwrap_or_else(|| "(unclassified)".to_string());
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
        .user_name(config.effective_user_name().to_string())
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

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    // ── extract_tool_output tests ─────────────────────────────────────────

    #[test]
    fn test_should_extract_legacy_output_field() {
        let payload = serde_json::json!({
            "output": "some legacy output text"
        });
        assert_eq!(extract_tool_output(&payload), "some legacy output text");
    }

    #[test]
    fn test_should_extract_bash_stdout_from_tool_response() {
        // Real Claude Code PostToolUse payload for Bash
        let payload = serde_json::json!({
            "session_id": "abc123",
            "hook_event_name": "PostToolUse",
            "tool_name": "Bash",
            "tool_input": {
                "command": "git status",
                "description": "Check git status"
            },
            "tool_response": {
                "stdout": "On branch master\nnothing to commit, working tree clean\n",
                "stderr": "",
                "success": true
            }
        });
        assert_eq!(
            extract_tool_output(&payload),
            "On branch master\nnothing to commit, working tree clean\n"
        );
    }

    #[test]
    fn test_should_extract_read_content_from_tool_response() {
        // Real Claude Code PostToolUse payload for Read tool
        let payload = serde_json::json!({
            "session_id": "abc123",
            "hook_event_name": "PostToolUse",
            "tool_name": "Read",
            "tool_input": {
                "file_path": "/path/to/file.rs"
            },
            "tool_response": {
                "content": "fn main() {\n    println!(\"hello\");\n}\n",
                "success": true
            }
        });
        assert_eq!(
            extract_tool_output(&payload),
            "fn main() {\n    println!(\"hello\");\n}\n"
        );
    }

    #[test]
    fn test_should_extract_write_content_from_tool_response() {
        // PostToolUse payload for Write tool
        let payload = serde_json::json!({
            "session_id": "abc123",
            "hook_event_name": "PostToolUse",
            "tool_name": "Write",
            "tool_input": {
                "file_path": "/path/to/output.txt",
                "content": "some written content"
            },
            "tool_response": {
                "content": "some written content",
                "success": true
            }
        });
        assert_eq!(extract_tool_output(&payload), "some written content");
    }

    #[test]
    fn test_should_prefer_legacy_output_over_tool_response() {
        let payload = serde_json::json!({
            "output": "legacy output text",
            "tool_response": {
                "stdout": "bash output text"
            }
        });
        assert_eq!(extract_tool_output(&payload), "legacy output text");
    }

    #[test]
    fn test_should_return_empty_for_empty_payload() {
        let payload = serde_json::json!({});
        assert_eq!(extract_tool_output(&payload), "");
    }

    #[test]
    fn test_should_return_empty_when_tool_response_has_no_text() {
        let payload = serde_json::json!({
            "tool_response": {
                "success": true
            }
        });
        assert_eq!(extract_tool_output(&payload), "");
    }

    #[test]
    fn test_should_extract_tool_response_as_plain_string() {
        // Some tools may return tool_response as a plain string
        let payload = serde_json::json!({
            "tool_response": "plain string response"
        });
        assert_eq!(extract_tool_output(&payload), "plain string response");
    }

    #[test]
    fn test_should_extract_read_file_content() {
        // Real Claude Code PostToolUse payload for Read tool
        let payload = serde_json::json!({
            "session_id": "abc123",
            "hook_event_name": "PostToolUse",
            "tool_name": "Read",
            "tool_input": {
                "file_path": "/path/to/file.rs"
            },
            "tool_response": {
                "type": "text",
                "file": {
                    "filePath": "/path/to/file.rs",
                    "content": "fn main() {\n    println!(\"hello\");\n}\n",
                    "numLines": 3,
                    "startLine": 1,
                    "totalLines": 100
                }
            }
        });
        assert_eq!(
            extract_tool_output(&payload),
            "fn main() {\n    println!(\"hello\");\n}\n"
        );
    }

    #[test]
    fn test_should_handle_empty_stdout() {
        let payload = serde_json::json!({
            "tool_response": {
                "stdout": "",
                "stderr": "some error",
                "success": false
            }
        });
        // Empty stdout → should try other fields, stderr is NOT extracted
        assert_eq!(extract_tool_output(&payload), "");
    }

    // ── compress_plain_text tests ──────────────────────────────────────────

    #[test]
    fn test_strip_ansi_color_codes() {
        let input = "\x1b[32mgreen text\x1b[0m normal";
        assert_eq!(strip_ansi(input), "green text normal");
    }

    #[test]
    fn test_strip_ansi_bold_reset() {
        let input = "\x1b[1mbold\x1b[0m";
        assert_eq!(strip_ansi(input), "bold");
    }

    #[test]
    fn test_strip_ansi_no_escape() {
        let input = "plain text without escapes";
        assert_eq!(strip_ansi(input), "plain text without escapes");
    }

    #[test]
    fn test_compress_plain_text_truncates_long_output() {
        let long = "x".repeat(9000);
        let result = compress_plain_text(&long);
        assert!(result.len() < 9000, "should be truncated");
        assert!(result.contains("truncated"), "should have truncation note");
    }

    #[test]
    fn test_compress_plain_text_short_output_unchanged() {
        let short = "hello world";
        let result = compress_plain_text(short);
        assert_eq!(result, "hello world");
    }

    #[test]
    fn test_compress_plain_text_strips_ansi_before_truncation() {
        let mut long = String::from("\x1b[32m");
        long.push_str(&"x".repeat(9000));
        long.push_str("\x1b[0m");
        let result = compress_plain_text(&long);
        assert!(!result.contains('\x1b'), "ANSI should be stripped");
        assert!(result.len() <= 8300, "should be truncated after ANSI strip");
    }
}
