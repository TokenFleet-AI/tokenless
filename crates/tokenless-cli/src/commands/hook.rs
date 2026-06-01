//! Handlers for `tokenless hook` subcommands.

use crate::{
    cache,
    shared::{
        SEMANTIC_COMPRESSOR, compressor_for_tool, read_input, record_compression_stats,
        strip_leading_bom,
    },
};

/// Handle `tokenless hook rewrite` for a specific agent target.
pub(crate) fn hook_rewrite(target: &str) -> Result<(), (String, i32)> {
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
            return Ok(());
        }
    }
    println!("{input}");
    Ok(())
}

/// Handle `tokenless hook compress` — response compression via hook protocol.
pub(crate) fn hook_compress(semantic: bool) -> Result<(), (String, i32)> {
    let input = read_input(&None).map_err(|e| (e, 2))?;
    let input = strip_leading_bom(&input);
    let val: serde_json::Value =
        serde_json::from_str(&input).map_err(|e| (format!("JSON parse error: {e}"), 2))?;
    let tool_name = val.get("tool_name").and_then(|v| v.as_str()).unwrap_or("");
    let output = val.get("output").and_then(|v| v.as_str()).unwrap_or("");
    if output.is_empty() {
        println!("{input}");
        return Ok(());
    }
    if let Ok(mut output_val) = serde_json::from_str::<serde_json::Value>(output) {
        // Semantic-aware field filtering: infer context from tool input.
        if semantic {
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
            record_compression_stats(
                tokenless_stats::OperationType::CompressResponse,
                None,
                None,
                None,
                input,
                output_text,
            );
        }
    } else {
        println!("{input}");
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
    if let Some(diff) = cache::compute_diff(cmd, output) {
        println!("{diff}");
    } else {
        println!("{output}");
    }
    Ok(())
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
