//! Tokenless CLI — LLM token optimization via schema and response compression.

#![allow(
    clippy::cast_lossless,
    clippy::cast_precision_loss,
    clippy::disallowed_methods,
    clippy::disallowed_types,
    clippy::doc_markdown,
    clippy::fn_params_excessive_bools,
    clippy::format_push_string,
    clippy::items_after_statements,
    clippy::map_unwrap_or,
    clippy::match_single_binding,
    clippy::redundant_closure_for_method_calls,
    clippy::ref_option,
    clippy::similar_names,
    clippy::single_match_else,
    clippy::too_many_lines,
    clippy::unnecessary_map_or,
    clippy::unwrap_used,
    clippy::useless_format
)]

mod env_check;
mod init;

use std::{
    fs,
    io::{self, Read},
    process,
    sync::OnceLock,
};

use clap::{Parser, Subcommand};
use rtk_registry::{
    Classification, RtkInstallStatus, classify_command, is_rtk_installed, rewrite_command,
};
use tokenless_schema::{ResponseCompressor, SchemaCompressor};
use tokenless_stats::{
    OperationType, StatsRecord, StatsRecorder, TokenlessConfig, estimate_tokens_from_bytes,
    format_list, format_rewrites, format_show, format_summary,
};

/// Cached result of `is_rtk_installed()`, checked at most once per process lifetime.
fn rtk_available() -> bool {
    static RTK_OK: OnceLock<bool> = OnceLock::new();
    *RTK_OK.get_or_init(|| matches!(is_rtk_installed(), RtkInstallStatus::Installed { .. }))
}

#[derive(Parser)]
#[command(
    name = "tokenless",
    version,
    about = "LLM token optimization via schema and response compression"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Compress OpenAI Function Calling tool schemas.
    CompressSchema {
        #[arg(short, long)]
        file: Option<String>,
        /// Compress a JSON array of schemas.
        #[arg(long)]
        batch: bool,
        /// Agent ID for stats (e.g., "copilot-shell").
        #[arg(long)]
        agent_id: Option<String>,
        /// Session ID for grouping.
        #[arg(long)]
        session_id: Option<String>,
        /// Tool use ID.
        #[arg(long)]
        tool_use_id: Option<String>,
    },
    /// Compress API responses.
    CompressResponse {
        #[arg(short, long)]
        file: Option<String>,
        /// Agent ID for stats.
        #[arg(long)]
        agent_id: Option<String>,
        /// Session ID for grouping.
        #[arg(long)]
        session_id: Option<String>,
        /// Tool use ID.
        #[arg(long)]
        tool_use_id: Option<String>,
    },
    /// Encode JSON to TOON format.
    CompressToon {
        #[arg(short, long)]
        file: Option<String>,
        /// Agent ID for stats.
        #[arg(long)]
        agent_id: Option<String>,
        /// Session ID for grouping.
        #[arg(long)]
        session_id: Option<String>,
        /// Tool use ID.
        #[arg(long)]
        tool_use_id: Option<String>,
    },
    /// Decode TOON format back to JSON.
    DecompressToon {
        #[arg(short, long)]
        file: Option<String>,
    },
    /// View and manage compression statistics.
    #[command(subcommand)]
    Stats(StatsCommands),
    /// Rewrite a shell command to its RTK equivalent (dry-run).
    Rewrite {
        /// Shell command to rewrite (from stdin if not provided).
        command: Option<String>,
        /// Exclude patterns (can be repeated).
        #[arg(long)]
        exclude: Vec<String>,
        /// Transparent prefixes (can be repeated).
        #[arg(long)]
        transparent_prefix: Vec<String>,
        /// Agent ID for stats.
        #[arg(long)]
        agent_id: Option<String>,
        /// Session ID for grouping.
        #[arg(long)]
        session_id: Option<String>,
        /// Tool use ID.
        #[arg(long)]
        tool_use_id: Option<String>,
    },
    /// Agent hook protocol handlers (stdin JSON → stdout JSON).
    #[command(subcommand)]
    Hook(HookCommands),
    /// Install tokenless hooks for AI coding agents (claude, cursor, windsurf, cline, kilocode,
    /// antigravity, augment, hermes, pi, gemini, opencode).
    Init {
        /// Install globally (shared config) instead of project-local.
        #[arg(short, long)]
        global: bool,
        /// Target agent [default: claude] [possible values: claude, cursor, windsurf, cline,
        /// kilocode, antigravity, augment, hermes, pi, gemini, opencode]
        #[arg(long, default_value = "claude")]
        agent: String,
    },
    /// Check tool environment readiness (binary availability, config, permissions, network).
    EnvCheck {
        /// Check a specific tool.
        #[arg(long)]
        tool: Option<String>,
        /// Check all tools.
        #[arg(long)]
        all: bool,
        /// Auto-fix missing dependencies.
        #[arg(long)]
        fix: bool,
        /// Output full checklist.
        #[arg(long)]
        checklist: bool,
        /// Output machine-readable JSON.
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum HookCommands {
    /// Command rewriting hooks (PreToolUse).
    #[command(subcommand)]
    Rewrite(RewriteTarget),
    /// Response compression hook (PostToolUse, stdin → stdout).
    Compress,
}

#[derive(Debug, Subcommand)]
enum RewriteTarget {
    /// Claude Code PreToolUse hook.
    Claude,
    /// Cursor editor PreToolUse hook (not yet implemented).
    Cursor,
    /// Gemini CLI BeforeTool hook (not yet implemented).
    Gemini,
    /// GitHub Copilot PreToolUse hook (not yet implemented).
    Copilot,
}

#[derive(Subcommand)]
enum StatsCommands {
    /// Show summary statistics with breakdown by operation.
    Summary {
        #[arg(long)]
        limit: Option<usize>,
    },
    /// List recent records.
    List {
        #[arg(short, long, default_value = "20")]
        limit: usize,
    },
    /// Show before/after text content for a specific record.
    Show {
        /// Record database ID.
        id: i64,
    },
    /// Clear all statistics.
    Clear {
        #[arg(long)]
        yes: bool,
    },
    /// Show rewrite-command breakdown by original command.
    Rewrites {
        #[arg(short, long, default_value = "20")]
        limit: usize,
        /// Skip the first N commands (for pagination).
        #[arg(long, default_value = "0")]
        offset: usize,
    },
    /// Show stats recording status.
    Status,
    /// Enable stats recording.
    Enable,
    /// Disable stats recording.
    Disable,
}

/// Read input from a file or stdin.
fn read_input(file: &Option<String>) -> Result<String, String> {
    match file {
        Some(path) => {
            fs::read_to_string(path).map_err(|e| format!("Failed to read file '{path}': {e}"))
        }
        None => {
            let mut buf = String::new();
            io::stdin()
                .read_to_string(&mut buf)
                .map_err(|e| format!("Failed to read stdin: {e}"))?;
            Ok(buf)
        }
    }
}

/// Get the user's home directory path.
#[must_use]
pub fn get_home_dir() -> String {
    dirs::home_dir()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| std::env::var("HOME").unwrap_or_else(|_| ".".to_string()))
}

/// Get the stats database path from env or default.
fn get_db_path() -> String {
    std::env::var("TOKENLESS_STATS_DB")
        .unwrap_or_else(|_| format!("{}/.tokenless/stats.db", get_home_dir()))
}

/// Ensure the stats database directory exists.
fn ensure_db_dir() -> Result<(), (String, i32)> {
    let db_path = get_db_path();
    if let Some(parent) = std::path::Path::new(&db_path).parent() {
        #[allow(clippy::disallowed_methods)]
        fs::create_dir_all(parent)
            .map_err(|e| (format!("Failed to create database directory: {e}"), 1))?;
    }
    Ok(())
}

/// Open the stats recorder, creating the DB directory if needed.
fn open_recorder() -> Result<StatsRecorder, (String, i32)> {
    ensure_db_dir()?;
    StatsRecorder::new(get_db_path()).map_err(|e| (format!("Failed to open database: {e}"), 1))
}

/// Record compression stats — fail-silent so compression output is never blocked.
#[allow(clippy::too_many_lines)]
fn record_compression_stats(
    op: OperationType,
    agent_id: Option<String>,
    session_id: Option<String>,
    tool_use_id: Option<String>,
    before_text: String,
    after_text: String,
) {
    if !TokenlessConfig::load().is_stats_enabled() {
        return;
    }

    let (before_bytes, after_bytes, before_tokens, after_tokens) =
        if op == OperationType::RewriteCommand {
            let n = before_text.len();
            let t = estimate_tokens_from_bytes(n);
            (n, n, t, t)
        } else {
            let bb = before_text.len();
            let ab = after_text.len();
            let bt = estimate_tokens_from_bytes(bb);
            let at = estimate_tokens_from_bytes(ab);
            if at >= bt {
                return;
            }
            (bb, ab, bt, at)
        };

    let pid = process::id();
    let agent = agent_id.unwrap_or_else(|| "cli".to_string());
    let mut record = StatsRecord::new(
        op,
        agent,
        before_bytes,
        before_tokens,
        after_bytes,
        after_tokens,
    )
    .with_before_text(before_text)
    .with_after_text(after_text);
    if let Some(sid) = session_id {
        record = record.with_session_id(sid);
    }
    if let Some(tuid) = tool_use_id {
        record = record.with_tool_use_id(tuid);
    }
    record = record.with_source_pid(pid as i64);

    if let Ok(recorder) = open_recorder() {
        let _ = recorder.record(&record);
    }
}

fn run() -> Result<(), (String, i32)> {
    let cli = Cli::parse();

    match cli.command {
        Commands::CompressSchema {
            file,
            batch,
            agent_id,
            session_id,
            tool_use_id,
        } => {
            let input = read_input(&file).map_err(|e| (e, 2))?;
            let value: serde_json::Value =
                serde_json::from_str(&input).map_err(|e| (format!("JSON parse error: {e}"), 2))?;

            let compressor = SchemaCompressor::new();

            let result_json = if batch {
                let arr = value
                    .as_array()
                    .ok_or_else(|| ("Expected a JSON array for --batch mode".to_string(), 1))?;
                let results: Vec<serde_json::Value> =
                    arr.iter().map(|item| compressor.compress(item)).collect();
                serde_json::to_string_pretty(&results)
                    .map_err(|e| (format!("Serialization error: {e}"), 2))?
            } else {
                let result = compressor.compress(&value);
                serde_json::to_string_pretty(&result)
                    .map_err(|e| (format!("Serialization error: {e}"), 2))?
            };

            let after_compact = serde_json::to_string(
                &serde_json::from_str::<serde_json::Value>(&result_json)
                    .unwrap_or(serde_json::Value::Null),
            )
            .unwrap_or_else(|_| result_json.clone());

            let before_tokens = estimate_tokens_from_bytes(input.len());
            let after_tokens = estimate_tokens_from_bytes(after_compact.len());
            let output_text = if after_tokens >= before_tokens {
                input.clone()
            } else {
                result_json
            };

            println!("{output_text}");

            record_compression_stats(
                OperationType::CompressSchema,
                agent_id,
                session_id,
                tool_use_id,
                input,
                output_text,
            );
        }
        Commands::CompressResponse {
            file,
            agent_id,
            session_id,
            tool_use_id,
        } => {
            let input = read_input(&file).map_err(|e| (e, 2))?;
            let value: serde_json::Value =
                serde_json::from_str(&input).map_err(|e| (format!("JSON parse error: {e}"), 2))?;

            let compressor = ResponseCompressor::new();
            let result_json = serde_json::to_string_pretty(&compressor.compress(&value))
                .map_err(|e| (format!("Serialization error: {e}"), 2))?;

            let after_compact = serde_json::to_string(
                &serde_json::from_str::<serde_json::Value>(&result_json)
                    .unwrap_or(serde_json::Value::Null),
            )
            .unwrap_or_else(|_| result_json.clone());

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
                agent_id,
                session_id,
                tool_use_id,
                input,
                output_text,
            );
        }
        Commands::CompressToon {
            file,
            agent_id,
            session_id,
            tool_use_id,
        } => {
            let input = read_input(&file).map_err(|e| (e, 2))?;
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
            println!("{display}");

            record_compression_stats(
                OperationType::CompressToon,
                agent_id,
                session_id,
                tool_use_id,
                input,
                display,
            );
        }
        Commands::DecompressToon { file } => {
            let input = read_input(&file).map_err(|e| (e, 2))?;
            let value: serde_json::Value = toon_format::decode_default(&input)
                .map_err(|e| (format!("toon decode failed: {e}"), 2))?;
            let output = serde_json::to_string_pretty(&value)
                .map_err(|e| (format!("Serialization error: {e}"), 2))?;
            let output = output.trim_end().to_string();
            if !output.is_empty() {
                println!("{output}");
            }
        }
        Commands::Rewrite {
            command,
            exclude,
            transparent_prefix,
            agent_id,
            session_id,
            tool_use_id,
        } => {
            let cmd = match command {
                Some(c) => c,
                None => read_input(&None).map_err(|e| (e, 2))?,
            };
            let cmd = cmd.trim().to_string();

            if !rtk_available() {
                eprintln!("[tokenless] RTK is not installed — using original command.");
                eprintln!("  Install: cargo install rtk  or  brew install TokenFleet-AI/rtk/rtk");
                println!("{cmd}");
                return Ok(());
            }

            match rewrite_command(&cmd, &exclude, &transparent_prefix) {
                Some(rewritten) => {
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
        }
        Commands::Hook(sub) => match sub {
            HookCommands::Rewrite(target) => match target {
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
                _ => {
                    eprintln!("[tokenless] hook rewrite {target:?} not yet implemented");
                }
            },
            HookCommands::Compress => {
                let input = read_input(&None).map_err(|e| (e, 2))?;
                let value: serde_json::Value = serde_json::from_str(&input)
                    .map_err(|e| (format!("JSON parse error: {e}"), 2))?;
                let compressor = ResponseCompressor::new();
                let result_json = serde_json::to_string_pretty(&compressor.compress(&value))
                    .map_err(|e| (format!("Serialization error: {e}"), 2))?;
                let after_compact = serde_json::to_string(
                    &serde_json::from_str::<serde_json::Value>(&result_json)
                        .unwrap_or(serde_json::Value::Null),
                )
                .unwrap_or_else(|_| result_json.clone());
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
            }
        },
        Commands::Init { global, agent } => {
            let agent = match agent.as_str() {
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
                _ => init::Agent::Claude,
            };
            let config = init::InitConfig { global };
            init::run(agent, &config).map_err(|e| (e, 1))?;
        }
        Commands::EnvCheck {
            tool,
            all,
            fix,
            checklist,
            json,
        } => {
            env_check::run(tool.as_deref(), all, fix, checklist, json)?;
        }
        Commands::Stats(stats_cmd) => {
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
        }
    }

    Ok(())
}

fn main() {
    if let Err((msg, code)) = run() {
        eprintln!("Error: {msg}");
        process::exit(code);
    }
}
