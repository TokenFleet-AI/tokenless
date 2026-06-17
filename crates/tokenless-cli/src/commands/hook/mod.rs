//! Handlers for `tokenless hook` subcommands.
//!
//! Three entry points for hook protocol: `rewrite` (PreToolUse),
//! `compress` (PostToolUse), and `diff` (differential response).
//! Shared helpers live in the [`helpers`] module.

mod helpers;

use crate::{
    cache,
    shared::{
        SEMANTIC_COMPRESSOR, compressor_for_tool, is_experimental_enabled, read_input,
        record_compression_stats, strip_leading_bom,
    },
};

use helpers::{
    append_compress_log_entry, compress_plain_text, extract_tool_output, infer_context,
    write_debug_log,
};

/// Handle `tokenless hook rewrite` — PreToolUse command rewriting via RTK.
pub(crate) fn hook_rewrite(
    target: &str,
    project: Option<String>,
    cli_user_name: Option<String>,
) -> Result<(), (String, i32)> {
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
        .map(std::string::ToString::to_string);

    let config = tokenless_stats::TokenlessConfig::load();
    let user_name = cli_user_name.unwrap_or_else(|| config.effective_user_name().to_string());
    if config.is_passthrough_enabled() {
        println!("{input}");
        record_compression_stats(
            tokenless_stats::OperationType::RewriteCommand,
            Some(target.to_string()),
            session_id,
            None,
            project.clone(),
            Some(user_name),
            input.clone(),
            input,
            false,
            Some("Passthrough".into()),
        );
        return Ok(());
    }

    let rewritten = rtk_registry::rewrite_command(command, &[], &[]);
    if let Some(ref rw) = rewritten
        && let Some(obj) = val.as_object()
    {
        let mut new_val = obj.clone();
        if let Some(tool_input) = new_val.get_mut("tool_input")
            && let Some(obj) = tool_input.as_object_mut()
        {
            obj.insert("command".to_string(), serde_json::Value::String(rw.clone()));
        }
        let output = serde_json::to_string(&new_val).unwrap_or_default();
        println!("{output}");
        record_compression_stats(
            tokenless_stats::OperationType::RewriteCommand,
            Some(target.to_string()),
            session_id,
            None,
            project,
            Some(user_name),
            input,
            output,
            false,
            Some("RtkStandard".into()),
        );
        return Ok(());
    }
    println!("{input}");
    Ok(())
}

/// Handle `tokenless hook compress` — PostToolUse response compression.
#[allow(clippy::too_many_lines, clippy::needless_pass_by_value)]
pub(crate) fn hook_compress(
    semantic: bool,
    target: &str,
    project: Option<String>,
    cli_user_name: Option<String>,
    debug: bool,
) -> Result<(), (String, i32)> {
    use tokenless_stats::OperationType;

    let input = read_input(&None).map_err(|e| (e, 2))?;
    let input = strip_leading_bom(&input);
    let val: serde_json::Value =
        serde_json::from_str(&input).map_err(|e| (format!("JSON parse error: {e}"), 2))?;
    let tool_name = val.get("tool_name").and_then(|v| v.as_str()).unwrap_or("");
    let output = extract_tool_output(&val);
    let session_id = val
        .get("session_id")
        .and_then(|v| v.as_str())
        .map(std::string::ToString::to_string);

    let config = tokenless_stats::TokenlessConfig::load();
    let user_name = cli_user_name.unwrap_or_else(|| config.effective_user_name().to_string());
    if config.is_passthrough_enabled() {
        println!("{input}");
        if !output.is_empty() {
            append_compress_log_entry(target, output, output, project.as_deref(), Some(&user_name));
        }
        return Ok(());
    }

    if output.is_empty() {
        println!("{input}");
        if debug {
            write_debug_log(tool_name, &project, &input, "", "empty");
        }
        return Ok(());
    }
    if let Ok(mut output_val) = serde_json::from_str::<serde_json::Value>(output) {
        let use_semantic = semantic && is_experimental_enabled();
        if use_semantic && let Some(context) = infer_context(&val, tool_name) {
            let mut sc = SEMANTIC_COMPRESSOR
                .lock()
                .map_err(|e| (format!("Semantic compressor lock error: {e}"), 1))?;
            let _ = sc.load_onnx();
            output_val = sc.compress(&output_val, &context);
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
            let method = if use_semantic {
                "Semantic"
            } else if tool_name.eq_ignore_ascii_case("Bash") {
                "HighFidelity"
            } else {
                "Standard"
            };
            record_compression_stats(
                OperationType::CompressResponse,
                Some(target.to_string()),
                session_id,
                None,
                project.clone(),
                Some(user_name.clone()),
                output.to_string(),
                compressed_str.clone(),
                use_semantic,
                Some(method.into()),
            );
            if debug {
                write_debug_log(tool_name, &project, output, &compressed_str, "compressed");
            }
            append_compress_log_entry(
                target,
                output,
                &compressed_str,
                project.as_deref(),
                Some(&user_name),
            );
        }
    } else {
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
                    OperationType::CompressResponse,
                    Some(target.to_string()),
                    session_id,
                    None,
                    project.clone(),
                    Some(user_name.clone()),
                    output.to_string(),
                    cleaned.clone(),
                    false,
                    Some("Standard".into()),
                );
                if debug {
                    write_debug_log(tool_name, &project, output, &cleaned, "compressed");
                }
                append_compress_log_entry(
                    target,
                    output,
                    &cleaned,
                    project.as_deref(),
                    Some(&user_name),
                );
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

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::needless_pass_by_value,
    reason = "Test code uses unwrap/expect idiomatically"
)]
mod tests {
    use super::helpers::*;
    use super::*;

    fn read_json_fixture(name: &str) -> serde_json::Value {
        let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures");
        let path = dir.join(name);
        let raw = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("Failed to read fixture {}: {}", path.display(), e));
        serde_json::from_str(&raw)
            .unwrap_or_else(|e| panic!("Failed to parse fixture {}: {}", path.display(), e))
    }

    // ── extract_tool_output ────────────────────────────────────────────

    #[test]
    fn test_should_extract_legacy_output_field() {
        let val = serde_json::json!({"output": "some stdout content"});
        assert_eq!(extract_tool_output(&val), "some stdout content");
    }

    #[test]
    fn test_should_extract_bash_stdout_from_tool_response() {
        let val = serde_json::json!({
            "tool_response": {"stdout": "command output here"}
        });
        assert_eq!(extract_tool_output(&val), "command output here");
    }

    #[test]
    fn test_should_extract_read_content_from_tool_response() {
        let val = serde_json::json!({
            "tool_response": {"file": {"content": "file contents"}}
        });
        assert_eq!(extract_tool_output(&val), "file contents");
    }

    #[test]
    fn test_should_extract_write_content_from_tool_response() {
        let val = serde_json::json!({
            "tool_response": {"content": "edited content"}
        });
        assert_eq!(extract_tool_output(&val), "edited content");
    }

    #[test]
    fn test_should_prefer_legacy_output_over_tool_response() {
        let val = serde_json::json!({
            "output": "top-level output",
            "tool_response": {"stdout": "ignored"}
        });
        assert_eq!(extract_tool_output(&val), "top-level output");
    }

    #[test]
    fn test_should_return_empty_for_empty_payload() {
        let val = serde_json::json!({});
        assert_eq!(extract_tool_output(&val), "");
    }

    #[test]
    fn test_should_return_empty_when_tool_response_has_no_text() {
        let val = serde_json::json!({"tool_response": {"exit_code": 0}});
        assert_eq!(extract_tool_output(&val), "");
    }

    #[test]
    fn test_should_extract_tool_response_as_plain_string() {
        let val = serde_json::json!({"tool_response": "plain string response"});
        assert_eq!(extract_tool_output(&val), "plain string response");
    }

    #[test]
    fn test_should_extract_read_file_content() {
        let val = serde_json::json!({"tool_response": {"file": {"content": "fn main() {}"}}});
        assert_eq!(extract_tool_output(&val), "fn main() {}");
    }

    #[test]
    fn test_should_handle_empty_stdout() {
        let val = serde_json::json!({"tool_response": {"stdout": ""}});
        assert_eq!(extract_tool_output(&val), "");
    }

    // ── strip_ansi ─────────────────────────────────────────────────────

    #[test]
    fn test_strip_ansi_color_codes() {
        assert_eq!(strip_ansi("\x1b[31mred\x1b[0m"), "red");
    }

    #[test]
    fn test_strip_ansi_bold_reset() {
        assert_eq!(strip_ansi("\x1b[1mbold\x1b[0m"), "bold");
    }

    #[test]
    fn test_strip_ansi_no_escape() {
        assert_eq!(strip_ansi("plain"), "plain");
    }

    // ── compress_plain_text ─────────────────────────────────────────────

    #[test]
    fn test_compress_plain_text_truncates_long_output() {
        let long = "x".repeat(9000);
        let result = compress_plain_text(&long);
        assert!(result.len() < 9000);
        assert!(result.contains("[truncated"));
    }

    #[test]
    fn test_compress_plain_text_short_output_unchanged() {
        let result = compress_plain_text("hello");
        assert_eq!(result, "hello");
    }

    #[test]
    fn test_compress_plain_text_strips_ansi_before_truncation() {
        let text = format!("\x1b[1m{}\x1b[0m", "x".repeat(9000));
        let result = compress_plain_text(&text);
        assert!(!result.contains('\x1b'));
        assert!(result.contains("[truncated"));
    }

    // ── edge cases ─────────────────────────────────────────────────────

    #[test]
    fn test_should_strip_ansi_sgr_semicolon_params() {
        assert_eq!(strip_ansi("\x1b[1;31mcolored\x1b[0m"), "colored");
    }

    #[test]
    fn test_should_strip_ansi_256_color() {
        assert_eq!(strip_ansi("\x1b[38;5;196mred\x1b[0m"), "red");
    }

    #[test]
    fn test_should_strip_ansi_24bit_color() {
        assert_eq!(strip_ansi("\x1b[38;2;255;0;0mrgb\x1b[0m"), "rgb");
    }

    #[test]
    fn test_should_strip_ansi_multi_param_sgr() {
        assert_eq!(strip_ansi("\x1b[1;4;31mstyled\x1b[0m"), "styled");
    }

    #[test]
    fn test_should_strip_ansi_mixed_colors_and_styles() {
        let input = "\x1b[32mgreen\x1b[0m plain \x1b[1mbold\x1b[0m";
        assert_eq!(strip_ansi(input), "green plain bold");
    }

    #[test]
    fn test_should_strip_ansi_cursor_erase_and_mode_sequences() {
        assert_eq!(strip_ansi("\x1b[2J\x1b[Hhello\x1b[K"), "hello");
    }

    #[test]
    fn test_should_strip_ansi_preserving_multibyte_utf8() {
        assert_eq!(strip_ansi("\x1b[32m你好\x1b[0m"), "你好");
    }

    #[test]
    fn test_should_strip_ansi_with_chinese_and_bold() {
        assert_eq!(strip_ansi("\x1b[1m中文\x1b[0m"), "中文");
    }

    #[test]
    fn test_should_truncate_long_multibyte_content_without_panic() {
        let input = "中文".repeat(5000);
        let result = compress_plain_text(&input);
        assert!(result.contains("[truncated"));
    }

    #[test]
    fn test_should_truncate_at_char_boundary_not_byte_boundary() {
        let multibyte = "中".repeat(10000);
        let result = compress_plain_text(&multibyte);
        assert!(result.len() < 12000, "should be truncated to char boundary");
        // Verify the truncated part is valid UTF-8 by indexing
        let _ = result.as_bytes();
    }

    #[test]
    fn test_should_handle_truncated_ansi_sequence_gracefully() {
        let input = "hello \x1b[未完成的序列";
        let result = compress_plain_text(input);
        assert!(result.len() > 0);
    }

    #[test]
    fn test_should_handle_emoji_heavy_content_truncation() {
        let input = "🌟".repeat(5000);
        let result = compress_plain_text(&input);
        assert!(!result.is_empty());
    }
}
