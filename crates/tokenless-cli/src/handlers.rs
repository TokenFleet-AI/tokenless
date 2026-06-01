//! Extracted command handlers for the tokenless CLI.
//!
//! Each function in this module handles one CLI subcommand or group of
//! subcommands. They receive fully-parsed arguments and return
//! `Result<(), (String, i32)>` so the caller drives output/exit.

#![allow(
    clippy::needless_pass_by_value,
    clippy::unnecessary_wraps
)]

use std::io;

use rtk_registry::{Classification, classify_command, rewrite_command};
use tokenless_schema::{compress_auto as schema_compress_auto, strategy_name};
use tokenless_stats::{
    estimate_tokens_from_bytes, format_list, format_rewrites, format_show, format_summary,
    OperationType, TokenlessConfig,
};

use crate::cache;
use crate::env_check;
use crate::init;
use crate::mcp;
use crate::{
    compressor_for_tool, open_recorder, read_input, record_compression_stats, rtk_available,
    strip_leading_bom, HookCommands, McpAction, RewriteTarget, SCHEMA_COMPRESSOR,
    StatsCommands, RESPONSE_COMPRESSOR,
};

// ── Schema compression ─────────────────────────────────────────────────────

/// Handle `tokenless compress-schema`.
#[allow(clippy::too_many_lines)]
pub(crate) fn compress_schema(
    file: Option<String>,
    batch: bool,
    agent_id: Option<String>,
    session_id: Option<String>,
    tool_use_id: Option<String>,
) -> Result<(), (String, i32)> {
    let input = read_input(&file).map_err(|e| (e, 2))?;
    if let Some(cached) = cache::cache_get(&input) {
        println!("{cached}");
        return Ok(());
    }
    let value: serde_json::Value =
        serde_json::from_str(&input).map_err(|e| (format!("JSON parse error: {e}"), 2))?;

    let compressor = &*SCHEMA_COMPRESSOR;

    let (after_compact, result_json) = if batch {
        let arr = value
            .as_array()
            .ok_or_else(|| ("Expected a JSON array for --batch mode".to_string(), 1))?;
        let results: Vec<serde_json::Value> =
            arr.iter().map(|item| compressor.compress(item)).collect();
        let compact = serde_json::to_string(&results).unwrap_or_default();
        let pretty = serde_json::to_string_pretty(&results)
            .map_err(|e| (format!("Serialization error: {e}"), 2))?;
        (compact, pretty)
    } else {
        let result = compressor.compress(&value);
        let compact = serde_json::to_string(&result).unwrap_or_default();
        let pretty = serde_json::to_string_pretty(&result)
            .map_err(|e| (format!("Serialization error: {e}"), 2))?;
        (compact, pretty)
    };

    let before_tokens = estimate_tokens_from_bytes(input.len());
    let after_tokens = estimate_tokens_from_bytes(after_compact.len());
    let output_text = if after_tokens >= before_tokens {
        input.clone()
    } else {
        result_json
    };

    cache::cache_insert(&input, &output_text);
    println!("{output_text}");

    record_compression_stats(
        OperationType::CompressSchema,
        agent_id,
        session_id,
        tool_use_id,
        input,
        output_text,
    );
    Ok(())
}

// ── Response compression ───────────────────────────────────────────────────

/// Handle `tokenless compress-response`.
pub(crate) fn compress_response(
    file: Option<String>,
    agent_id: Option<String>,
    session_id: Option<String>,
    tool_use_id: Option<String>,
) -> Result<(), (String, i32)> {
    let input = read_input(&file).map_err(|e| (e, 2))?;
    if let Some(cached) = cache::cache_get(&input) {
        println!("{cached}");
        return Ok(());
    }
    let value: serde_json::Value =
        serde_json::from_str(&input).map_err(|e| (format!("JSON parse error: {e}"), 2))?;

    let compressor = &*RESPONSE_COMPRESSOR;
    let result = compressor.compress(&value);
    let after_compact = serde_json::to_string(&result).unwrap_or_default();
    let result_json = serde_json::to_string_pretty(&result)
        .map_err(|e| (format!("Serialization error: {e}"), 2))?;

    let before_tokens = estimate_tokens_from_bytes(input.len());
    let after_tokens = estimate_tokens_from_bytes(after_compact.len());
    let output_text = if after_tokens >= before_tokens {
        input.clone()
    } else {
        result_json
    };

    cache::cache_insert(&input, &output_text);
    println!("{output_text}");

    record_compression_stats(
        OperationType::CompressResponse,
        agent_id,
        session_id,
        tool_use_id,
        input,
        output_text,
    );
    Ok(())
}

// ── Auto compression ───────────────────────────────────────────────────────

/// Handle `tokenless compress-auto`.
pub(crate) fn compress_auto(
    file: Option<String>,
    json_output: bool,
    agent_id: Option<String>,
    session_id: Option<String>,
    tool_use_id: Option<String>,
) -> Result<(), (String, i32)> {
    let input = read_input(&file).map_err(|e| (e, 2))?;
    if let Some(cached) = cache::cache_get(&input) {
        println!("{cached}");
        return Ok(());
    }
    let value: serde_json::Value =
        serde_json::from_str(&input).map_err(|e| (format!("JSON parse error: {e}"), 2))?;

    let (strategy, compressed) = schema_compress_auto(&value, &input);

    let before_tokens = estimate_tokens_from_bytes(input.len());
    let after_tokens = estimate_tokens_from_bytes(compressed.len());

    let output_text = if after_tokens >= before_tokens {
        input.clone()
    } else if json_output {
        let savings_pct = if input.is_empty() {
            0.0
        } else {
            ((input.len() - compressed.len()) as f64 / input.len() as f64) * 100.0
        };
        serde_json::to_string_pretty(&serde_json::json!({
            "strategy": strategy_name(&strategy),
            "compressed": compressed,
            "savings": {
                "chars_before": input.len(),
                "chars_after": compressed.len(),
                "pct": (savings_pct * 10.0).round() / 10.0
            }
        }))
        .unwrap_or_default()
    } else {
        compressed
    };

    cache::cache_insert(&input, &output_text);
    println!("{output_text}");

    record_compression_stats(
        OperationType::CompressResponse,
        agent_id,
        session_id,
        tool_use_id,
        input,
        output_text,
    );
    Ok(())
}

// ── TOON compression ───────────────────────────────────────────────────────

/// Handle `tokenless compress-toon`.
pub(crate) fn compress_toon(
    file: Option<String>,
    agent_id: Option<String>,
    session_id: Option<String>,
    tool_use_id: Option<String>,
) -> Result<(), (String, i32)> {
    let input = read_input(&file).map_err(|e| (e, 2))?;
    if let Some(cached) = cache::cache_get(&input) {
        println!("{cached}");
        return Ok(());
    }
    let value: serde_json::Value =
        serde_json::from_str(&input).map_err(|e| (format!("JSON parse error: {e}"), 2))?;
    let output = toon_format::encode_default(&value)
        .map_err(|e| (format!("toon encode failed: {e}"), 2))?;
    let output = output.trim_end().to_string();

    let before_tokens = estimate_tokens_from_bytes(input.len());
    let after_tokens = estimate_tokens_from_bytes(output.len());
    let display = if output.is_empty() || after_tokens >= before_tokens {
        input.clone()
    } else {
        output
    };
    cache::cache_insert(&input, &display);
    println!("{display}");

    record_compression_stats(
        OperationType::CompressToon,
        agent_id,
        session_id,
        tool_use_id,
        input,
        display,
    );
    Ok(())
}

// ── TOON decompression ─────────────────────────────────────────────────────

/// Handle `tokenless decompress-toon`.
pub(crate) fn decompress_toon(file: Option<String>) -> Result<(), (String, i32)> {
    let input = read_input(&file).map_err(|e| (e, 2))?;
    let value: serde_json::Value = toon_format::decode_default(&input)
        .map_err(|e| (format!("toon decode failed: {e}"), 2))?;
    let output = serde_json::to_string_pretty(&value)
        .map_err(|e| (format!("Serialization error: {e}"), 2))?;
    let output = output.trim_end().to_string();
    if !output.is_empty() {
        println!("{output}");
    }
    Ok(())
}

// ── Command rewriting ──────────────────────────────────────────────────────

/// Handle `tokenless rewrite`.
pub(crate) fn rewrite(
    command: Option<String>,
    exclude: Vec<String>,
    transparent_prefix: Vec<String>,
    agent_id: Option<String>,
    session_id: Option<String>,
    tool_use_id: Option<String>,
) -> Result<(), (String, i32)> {
    let cmd = match command {
        Some(c) => c,
        None => read_input(&None).map_err(|e| (e, 2))?,
    };
    let cmd = cmd.trim().to_string();

    if let Some(cached) = cache::cache_get(&cmd) {
        println!("{cached}");
        return Ok(());
    }

    if !rtk_available() {
        eprintln!("[tokenless] RTK is not installed — using original command.");
        eprintln!("  Install: cargo install rtk  or  brew install TokenFleet-AI/rtk/rtk");
        println!("{cmd}");
        return Ok(());
    }

    match rewrite_command(&cmd, &exclude, &transparent_prefix) {
        Some(rewritten) => {
            cache::cache_insert(&cmd, &rewritten);
            record_compression_stats(
                OperationType::RewriteCommand,
                agent_id,
                session_id,
                tool_use_id,
                cmd,
                rewritten.clone(),
            );
            println!("{rewritten}");
        }
        None => {
            eprintln!("[tokenless] No rewrite available — passing through original.");
            println!("{cmd}");
        }
    }
    Ok(())
}

// ── Hook handlers ──────────────────────────────────────────────────────────

/// Handle `tokenless hook <subcommand>`.
pub(crate) fn handle_hook(hook: HookCommands) -> Result<(), (String, i32)> {
    match hook {
        HookCommands::Rewrite(target) => handle_hook_rewrite(target),
        HookCommands::Compress => handle_hook_compress(),
        HookCommands::Diff => handle_hook_diff(),
    }
}

/// Handle hook rewrite for a specific agent target.
fn handle_hook_rewrite(target: RewriteTarget) -> Result<(), (String, i32)> {
    match target {
        RewriteTarget::Claude => {
            let input = read_input(&None).map_err(|e| (e, 2))?;
            let hook_input: serde_json::Value = serde_json::from_str(&input)
                .map_err(|e| (format!("JSON parse error: {e}"), 2))?;
            let cmd = hook_input
                .pointer("/tool_input/command")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if cmd.is_empty() || !rtk_available() {
                return Ok(());
            }
            match rewrite_command(cmd, &[], &[]) {
                Some(rewritten) if rewritten != cmd => {
                    record_compression_stats(
                        OperationType::RewriteCommand,
                        None,
                        None,
                        None,
                        cmd.to_string(),
                        rewritten.clone(),
                    );
                    let response = serde_json::json!({
                        "hookSpecificOutput": {
                            "hookEventName": "PreToolUse",
                            "permissionDecision": "allow",
                            "permissionDecisionReason": "tokenless auto-rewrite",
                            "updatedInput": {
                                "command": rewritten
                            }
                        }
                    });
                    println!("{}", serde_json::to_string(&response).unwrap_or_default());
                }
                _ => {}
            }
        }
        RewriteTarget::Cursor => {
            let input = read_input(&None).map_err(|e| (e, 2))?;
            let input = strip_leading_bom(&input);
            let hook_input: serde_json::Value = serde_json::from_str(&input)
                .map_err(|e| (format!("JSON parse error: {e}"), 2))?;
            let cmd = hook_input
                .pointer("/tool_input/command")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if cmd.is_empty() || !rtk_available() {
                println!("{{}}");
                return Ok(());
            }
            match rewrite_command(cmd, &[], &[]) {
                Some(rewritten) if rewritten != cmd => {
                    record_compression_stats(
                        OperationType::RewriteCommand,
                        None,
                        None,
                        None,
                        cmd.to_string(),
                        rewritten.clone(),
                    );
                    let response = serde_json::json!({
                        "continue": true,
                        "permission": "allow",
                        "updated_input": {
                            "command": rewritten
                        }
                    });
                    println!("{}", serde_json::to_string(&response).unwrap_or_default());
                }
                _ => println!("{{}}"),
            }
        }
        RewriteTarget::Gemini => {
            let input = read_input(&None).map_err(|e| (e, 2))?;
            let hook_input: serde_json::Value = serde_json::from_str(&input)
                .map_err(|e| (format!("JSON parse error: {e}"), 2))?;
            let cmd = hook_input
                .pointer("/tool_input/command")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if cmd.is_empty() || !rtk_available() {
                println!("{{\"decision\":\"allow\"}}");
                return Ok(());
            }
            match rewrite_command(cmd, &[], &[]) {
                Some(rewritten) if rewritten != cmd => {
                    record_compression_stats(
                        OperationType::RewriteCommand,
                        None,
                        None,
                        None,
                        cmd.to_string(),
                        rewritten.clone(),
                    );
                    let response = serde_json::json!({
                        "decision": "allow",
                        "hookSpecificOutput": {
                            "tool_input": {
                                "command": rewritten
                            }
                        }
                    });
                    println!("{}", serde_json::to_string(&response).unwrap_or_default());
                }
                _ => println!("{{\"decision\":\"allow\"}}"),
            }
        }
        RewriteTarget::Copilot => {
            let input = read_input(&None).map_err(|e| (e, 2))?;
            let hook_input: serde_json::Value = serde_json::from_str(&input)
                .map_err(|e| (format!("JSON parse error: {e}"), 2))?;
            // Detect format: snake_case tool_name = VS Code, camelCase toolName = CLI
            let is_cli = hook_input.get("toolName").is_some();
            if is_cli {
                let tool_args_str = hook_input
                    .pointer("/toolArgs")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let args: serde_json::Value =
                    serde_json::from_str(tool_args_str).unwrap_or_default();
                let cmd = args.get("command").and_then(|v| v.as_str()).unwrap_or("");
                if cmd.is_empty() || !rtk_available() {
                    return Ok(());
                }
                if let Some(rewritten) = rewrite_command(cmd, &[], &[]) {
                    if rewritten != cmd {
                        record_compression_stats(
                            OperationType::RewriteCommand,
                            None,
                            None,
                            None,
                            cmd.to_string(),
                            rewritten.clone(),
                        );
                        println!(
                            "{}",
                            serde_json::to_string(&serde_json::json!({
                                "permissionDecision": "deny",
                                "permissionDecisionReason": format!(
                                    "Token savings: use `{rewritten}` instead (rtk saves 60-90% tokens)"
                                )
                            }))
                            .unwrap_or_default()
                        );
                    }
                }
            } else {
                // VS Code Copilot Chat — same protocol as Claude
                let cmd = hook_input
                    .pointer("/tool_input/command")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                if cmd.is_empty() || !rtk_available() {
                    return Ok(());
                }
                match rewrite_command(cmd, &[], &[]) {
                    Some(rewritten) if rewritten != cmd => {
                        record_compression_stats(
                            OperationType::RewriteCommand,
                            None,
                            None,
                            None,
                            cmd.to_string(),
                            rewritten.clone(),
                        );
                        let response = serde_json::json!({
                            "hookSpecificOutput": {
                                "hookEventName": "PreToolUse",
                                "permissionDecision": "allow",
                                "permissionDecisionReason": "tokenless auto-rewrite",
                                "updatedInput": {
                                    "command": rewritten
                                }
                            }
                        });
                        println!(
                            "{}",
                            serde_json::to_string(&response).unwrap_or_default()
                        );
                    }
                    _ => {}
                }
            }
        }
    }
    Ok(())
}

/// Handle `tokenless hook compress` — response compression via hook protocol.
fn handle_hook_compress() -> Result<(), (String, i32)> {
    let input = read_input(&None).map_err(|e| (e, 2))?;
    let value: serde_json::Value =
        serde_json::from_str(&input).map_err(|e| (format!("JSON parse error: {e}"), 2))?;
    let tool_name = value
        .get("tool_name")
        .or_else(|| value.get("toolName"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let compressor = compressor_for_tool(tool_name);
    let result = compressor.compress(&value);
    let after_compact = serde_json::to_string(&result).unwrap_or_default();
    let result_json = serde_json::to_string_pretty(&result)
        .map_err(|e| (format!("Serialization error: {e}"), 2))?;
    let before_tokens = estimate_tokens_from_bytes(input.len());
    let after_tokens = estimate_tokens_from_bytes(after_compact.len());
    let output_text = if after_tokens >= before_tokens {
        input.clone()
    } else {
        result_json
    };
    println!("{output_text}");
    record_compression_stats(
        OperationType::CompressResponse,
        None,
        None,
        None,
        input,
        output_text,
    );
    Ok(())
}

/// Handle `tokenless hook diff` — differential response compression.
fn handle_hook_diff() -> Result<(), (String, i32)> {
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

// ── Init ───────────────────────────────────────────────────────────────────

/// Handle `tokenless init`.
pub(crate) fn handle_init(global: bool, agent: String) -> Result<(), (String, i32)> {
    let agent_enum = match agent.as_str() {
        "cursor" => init::Agent::Cursor,
        "windsurf" => init::Agent::Windsurf,
        "cline" => init::Agent::Cline,
        "kilocode" => init::Agent::Kilocode,
        "antigravity" => init::Agent::Antigravity,
        "augment" => init::Agent::Augment,
        "hermes" => init::Agent::Hermes,
        "pi" => init::Agent::Pi,
        "gemini" => init::Agent::Gemini,
        "opencode" => init::Agent::Opencode,
        "copilot" => init::Agent::Copilot,
        _ => init::Agent::Claude,
    };
    let config = init::InitConfig { global };
    init::run(agent_enum, &config).map_err(|e| (e, 1))
}

// ── EnvCheck ───────────────────────────────────────────────────────────────

/// Handle `tokenless env-check`.
pub(crate) fn handle_env_check(
    tool: Option<String>,
    all: bool,
    fix: bool,
    checklist: bool,
    json: bool,
) -> Result<(), (String, i32)> {
    env_check::run(tool.as_deref(), all, fix, checklist, json)
}

// ── MCP ────────────────────────────────────────────────────────────────────

/// Handle `tokenless mcp start`.
#[allow(clippy::unnecessary_wraps)]
pub(crate) fn handle_mcp(action: McpAction) -> Result<(), (String, i32)> {
    match action {
        McpAction::Start => {
            mcp::run_mcp();
            Ok(())
        }
    }
}

// ── Stats ──────────────────────────────────────────────────────────────────

/// Handle all `tokenless stats` subcommands.
#[allow(clippy::too_many_lines)]
pub(crate) fn handle_stats(stats_cmd: StatsCommands) -> Result<(), (String, i32)> {
    let recorder = open_recorder()?;

    match stats_cmd {
        StatsCommands::Summary { limit } => {
            let records = recorder
                .all_records(limit)
                .map_err(|e| (format!("Failed to query records: {e}"), 1))?;
            println!(
                "{}",
                format_summary(&records, Some("Tokenless Statistics Summary"))
            );
        }
        StatsCommands::Rewrites { limit, offset } => {
            let all = recorder
                .all_records(None)
                .map_err(|e| (format!("Failed to query records: {e}"), 1))?;
            let rewrites: Vec<_> = all
                .iter()
                .filter(|r| r.operation == OperationType::RewriteCommand)
                .collect();
            let mut by_cmd: std::collections::BTreeMap<&str, usize> =
                std::collections::BTreeMap::new();
            for r in &rewrites {
                if let Some(ref before) = r.before_text {
                    *by_cmd.entry(before.as_str()).or_default() += 1;
                }
            }
            let mut entries: Vec<_> = by_cmd
                .into_iter()
                .map(|(cmd, count)| {
                    let savings = match classify_command(cmd) {
                        Classification::Supported {
                            estimated_savings_pct,
                            ..
                        } => Some(estimated_savings_pct),
                        _ => None,
                    };
                    (cmd, count, savings)
                })
                .collect();
            entries.sort_by(|a, b| b.1.cmp(&a.1));
            let slice: Vec<(&str, usize, Option<f64>)> =
                entries.iter().map(|(c, n, s)| (*c, *n, *s)).collect();
            println!("{}", format_rewrites(&slice, limit, offset));
        }
        StatsCommands::List { limit } => {
            let records = recorder
                .all_records(Some(limit))
                .map_err(|e| (format!("Failed to query records: {e}"), 1))?;
            println!("{}", format_list(&records, limit));
        }
        StatsCommands::Show { id } => {
            let record = recorder
                .record_by_id(id)
                .map_err(|e| (format!("Failed to query record: {e}"), 1))?
                .ok_or_else(|| (format!("Record not found: {id}"), 1))?;
            println!("{}", format_show(&record));
        }
        StatsCommands::Clear { yes } => {
            if !yes {
                print!("Are you sure you want to clear all statistics? [y/N] ");
                use std::io::Write;
                let _ = io::stdout().flush();
                let mut input = String::new();
                if io::stdin().read_line(&mut input).unwrap_or(0) == 0 {
                    println!("Cancelled.");
                    return Ok(());
                }
                if !input.trim().eq_ignore_ascii_case("y") {
                    println!("Cancelled.");
                    return Ok(());
                }
            }
            recorder
                .clear()
                .map_err(|e| (format!("Failed to clear: {e}"), 1))?;
            println!("Statistics cleared.");
        }
        StatsCommands::Status => {
            let config = TokenlessConfig::load();
            let source = if std::env::var("TOKENLESS_STATS_ENABLED").is_ok() {
                "env override"
            } else if TokenlessConfig::config_file_exists() {
                "config file"
            } else {
                "default"
            };
            let state = if config.is_stats_enabled() {
                "ENABLED"
            } else {
                "DISABLED"
            };
            println!("Stats recording: {state} (via {source})");
        }
        StatsCommands::Enable => {
            let mut config = TokenlessConfig::load();
            config.stats_enabled = true;
            config
                .save()
                .map_err(|e| (format!("Failed to save config: {e}"), 1))?;
            println!("Stats recording enabled.");
        }
        StatsCommands::Disable => {
            let mut config = TokenlessConfig::load();
            config.stats_enabled = false;
            config
                .save()
                .map_err(|e| (format!("Failed to save config: {e}"), 1))?;
            println!("Stats recording disabled.");
        }
    }
    Ok(())
}

// ── TUI ──────────────────────────────────────────────────────────────────────

/// Handle `tokenless tui` — launch interactive dashboard.
pub(crate) fn handle_tui(refresh_secs: u64, lang_str: &str) -> Result<(), (String, i32)> {
    let lang = match lang_str {
        "en" => tokenless_tui::Lang::En,
        _ => tokenless_tui::Lang::Zh,
    };
    let recorder = open_recorder()?;
    tokenless_tui::run_tui(recorder, refresh_secs, lang).map_err(|e| (e, 1))
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    // ── compress_schema handler tests ───────────────────────────────

    #[test]
    fn test_compress_schema_handler_with_temp_file() {
        let dir = std::env::temp_dir().join("tokenless-test-handlers");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("test_schema.json");
        let content = r#"{"function":{"name":"test_fn","description":"a test function","parameters":{"type":"object","properties":{"x":{"type":"string"}}}}}"#;
        std::fs::write(&path, content).expect("write temp file");

        let result = compress_schema(Some(path.to_string_lossy().to_string()), false, None, None, None);
        assert!(result.is_ok(), "compress_schema should succeed with valid JSON file");

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_compress_schema_handler_batch_with_temp_file() {
        let dir = std::env::temp_dir().join("tokenless-test-handlers");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("test_schemas_batch.json");
        let content = r#"[{"function":{"name":"fn_a","description":"A"}},{"function":{"name":"fn_b","description":"B"}}]"#;
        std::fs::write(&path, content).expect("write temp file");

        let result = compress_schema(Some(path.to_string_lossy().to_string()), true, None, None, None);
        assert!(result.is_ok(), "compress_schema --batch should succeed");

        let _ = std::fs::remove_file(&path);
    }

    // ── compress_response handler tests ──────────────────────────────

    #[test]
    fn test_compress_response_handler_with_temp_file() {
        let dir = std::env::temp_dir().join("tokenless-test-handlers");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("test_response.json");
        let content = r#"{"data":"important","debug":"should-drop","null_field":null}"#;
        std::fs::write(&path, content).expect("write temp file");

        let result = compress_response(Some(path.to_string_lossy().to_string()), None, None, None);
        assert!(result.is_ok(), "compress_response should succeed");

        let _ = std::fs::remove_file(&path);
    }

    // ── compress_toon handler tests ──────────────────────────────────

    #[test]
    fn test_compress_toon_handler_with_temp_file() {
        let dir = std::env::temp_dir().join("tokenless-test-handlers");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("test_toon_in.json");
        let content = r#"{"a":1,"b":[2,3]}"#;
        std::fs::write(&path, content).expect("write temp file");

        let result = compress_toon(Some(path.to_string_lossy().to_string()), None, None, None);
        assert!(result.is_ok(), "compress_toon should succeed");

        let _ = std::fs::remove_file(&path);
    }

    // ── decompress_toon handler tests ────────────────────────────────

    #[test]
    fn test_decompress_toon_handler_with_temp_file() {
        // First compress, then decompress
        let dir = std::env::temp_dir().join("tokenless-test-handlers");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("test_toon_roundtrip.json");

        let input_json = serde_json::json!({"hello": "world", "items": [1, 2, 3]});
        let encoded = toon_format::encode_default(&input_json).expect("toon encode");
        std::fs::write(&path, &encoded).expect("write toon file");

        let result = decompress_toon(Some(path.to_string_lossy().to_string()));
        assert!(result.is_ok(), "decompress_toon should succeed on valid toon");

        let _ = std::fs::remove_file(&path);
    }

    // ── Error handling tests ──────────────────────────────────────────

    #[test]
    fn test_compress_schema_handler_invalid_json() {
        let dir = std::env::temp_dir().join("tokenless-test-handlers");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("test_invalid.json");
        std::fs::write(&path, "not valid json").expect("write temp file");

        let result = compress_schema(Some(path.to_string_lossy().to_string()), false, None, None, None);
        assert!(result.is_err(), "should fail on invalid JSON");

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_compress_schema_handler_nonexistent_file() {
        let result = compress_schema(Some("/nonexistent/path.json".to_string()), false, None, None, None);
        assert!(result.is_err(), "should fail on nonexistent file");
    }

    #[test]
    fn test_decompress_toon_handler_invalid_toon() {
        // Use malformed TOON: starting with `{` creates an invalid JSON-like
        // document that the TOON parser rejects.
        let dir = std::env::temp_dir().join("tokenless-test-handlers");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("test_invalid_toon.json");
        std::fs::write(&path, "{{{{invalid toon!!!!").expect("write temp file");

        let result = decompress_toon(Some(path.to_string_lossy().to_string()));
        assert!(result.is_err(), "should fail on invalid TOON input");

        let _ = std::fs::remove_file(&path);
    }

    // ── handle_stats: Enable/Disable are safe (no real DB) ──────────

    #[test]
    fn test_batch_mode_error_on_non_array() {
        let dir = std::env::temp_dir().join("tokenless-test-handlers");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("test_non_array.json");
        std::fs::write(&path, r#"{"not":"an array"}"#).expect("write temp file");

        let result = compress_schema(Some(path.to_string_lossy().to_string()), true, None, None, None);
        assert!(result.is_err(), "batch mode should fail on non-array input");

        let _ = std::fs::remove_file(&path);
    }
}
