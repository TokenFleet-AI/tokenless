//! Tokenless CLI — LLM token optimization via schema and response compression.
//!
//! Command handlers live under [`commands`]; shared state and utilities live
//! under [`shared`].  The `main()` entry point parses CLI args and dispatches
//! to the appropriate handler.
#![allow(
    clippy::disallowed_methods,
    clippy::disallowed_types,
    clippy::collapsible_if,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::unreadable_literal,
    clippy::unnecessary_map_or,
    clippy::useless_format,
    reason = "pre-existing CLI code conventions"
)]

mod cache;
mod commands;
mod env_check;
mod init;
mod mcp;
mod shared;

use clap::{Parser, Subcommand};

// ── CLI definition ───────────────────────────────────────────────────────────

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
        /// Print token savings report to stderr.
        #[arg(long)]
        report: bool,
        /// Agent ID for statistics attribution.
        #[arg(long)]
        agent_id: Option<String>,
        /// Session ID for statistics grouping.
        #[arg(long)]
        session_id: Option<String>,
        /// Tool use ID for statistics.
        #[arg(long)]
        tool_use_id: Option<String>,
    },
    /// Compress a JSON API response (drop debug fields, truncate strings).
    CompressResponse {
        #[arg(short, long)]
        file: Option<String>,
        #[arg(long)]
        report: bool,
        /// User task context for semantic-aware field compression
        /// (e.g. "今天天气怎么样" or "deploy to kubernetes").
        #[arg(long)]
        context: Option<String>,
        /// Enable ONNX Level 2 embedding model for semantic compression.
        /// Falls back to Level 1 (keyword rules) if model is unavailable.
        #[arg(long)]
        semantic: bool,
        #[arg(long)]
        agent_id: Option<String>,
        #[arg(long)]
        session_id: Option<String>,
        #[arg(long)]
        tool_use_id: Option<String>,
    },
    /// Auto-detect the best compression strategy and apply it.
    CompressAuto {
        #[arg(short, long)]
        file: Option<String>,
        #[arg(long)]
        report: bool,
        #[arg(long)]
        agent_id: Option<String>,
        #[arg(long)]
        session_id: Option<String>,
        #[arg(long)]
        tool_use_id: Option<String>,
    },
    /// Encode JSON as TOON format.
    CompressToon {
        #[arg(short, long)]
        file: Option<String>,
        #[arg(long)]
        agent_id: Option<String>,
        #[arg(long)]
        session_id: Option<String>,
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
        #[arg(long)]
        agent_id: Option<String>,
        #[arg(long)]
        session_id: Option<String>,
        #[arg(long)]
        tool_use_id: Option<String>,
    },
    /// Agent hook protocol handlers (stdin JSON → stdout JSON).
    #[command(subcommand)]
    Hook(HookCommands),
    /// Install tokenless hooks for supported AI coding agents.
    Init {
        /// Install hooks globally (for all projects).
        #[arg(long)]
        global: bool,
        /// Agent name (claude, cursor, windsurf, cline, copilot, gemini, etc.).
        #[arg(short, long, default_value = "claude")]
        agent: String,
    },
    /// Check tool environment readiness.
    EnvCheck {
        /// Check a specific tool only.
        #[arg(long)]
        tool: Option<String>,
        /// Check all known tools.
        #[arg(long)]
        all: bool,
        /// Attempt automatic fixes.
        #[arg(long)]
        fix: bool,
        /// Output an installation checklist.
        #[arg(long)]
        checklist: bool,
        /// Output results as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Start MCP JSON-RPC 2.0 server on stdin/stdout.
    #[command(subcommand)]
    Mcp(McpAction),
    /// Show a demo of all compression strategies.
    Demo,
    /// Launch the interactive TUI dashboard.
    Tui {
        /// Refresh interval in seconds (default: 1).
        #[arg(long, default_value = "1")]
        refresh: u64,
        /// Display language: "zh" or "en" (default: detect from LANG env).
        #[arg(long, default_value = "zh")]
        lang: String,
    },
}

#[derive(Subcommand)]
enum McpAction {
    /// Start the MCP server.
    Start,
}

#[derive(Subcommand)]
enum HookCommands {
    /// PreToolUse: rewrite shell commands via RTK.
    Rewrite {
        /// Agent target (e.g., "claude").
        #[arg(short, long, default_value = "claude")]
        target: String,
    },
    /// PostToolUse: compress tool response output.
    Compress {
        /// Enable semantic-aware field filtering (Level 2 ONNX or Level 1 rules).
        #[arg(long)]
        semantic: bool,
    },
    /// PostToolUse: differential response (unified diff).
    Diff,
}

#[derive(Debug, Subcommand)]
enum RewriteTarget {
    /// Claude Code PreToolUse hook.
    Claude,
    Cursor,
    Gemini,
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
    Show { id: i64 },
    /// Clear all statistics.
    Clear {
        #[arg(long)]
        yes: bool,
    },
    /// Show rewrite-command breakdown by original command.
    Rewrites {
        #[arg(short, long, default_value = "20")]
        limit: usize,
        #[arg(long, default_value = "0")]
        offset: usize,
    },
    /// Show stats recording status.
    Status,
    /// Enable stats recording.
    Enable,
    /// Disable stats recording.
    Disable,
    /// Show cumulative savings for a time period.
    Diff {
        #[arg(long)]
        since: Option<String>,
        #[arg(long)]
        until: Option<String>,
    },
}

// ── Dispatch ─────────────────────────────────────────────────────────────────

fn run() -> Result<(), (String, i32)> {
    let cli = Cli::parse();

    match cli.command {
        Commands::CompressSchema {
            file,
            batch,
            report,
            agent_id,
            session_id,
            tool_use_id,
        } => commands::compress::compress_schema(
            file,
            batch,
            report,
            agent_id,
            session_id,
            tool_use_id,
        ),
        Commands::CompressResponse {
            file,
            report,
            context,
            semantic,
            agent_id,
            session_id,
            tool_use_id,
        } => commands::compress::compress_response(
            file,
            report,
            semantic,
            context,
            agent_id,
            session_id,
            tool_use_id,
        ),
        Commands::CompressAuto {
            file,
            report,
            agent_id,
            session_id,
            tool_use_id,
        } => commands::compress::compress_auto(file, report, agent_id, session_id, tool_use_id),
        Commands::CompressToon {
            file,
            agent_id,
            session_id,
            tool_use_id,
        } => commands::toon::compress_toon(file, agent_id, session_id, tool_use_id),
        Commands::DecompressToon { file } => commands::toon::decompress_toon(file),
        Commands::Rewrite {
            command,
            exclude,
            transparent_prefix,
            agent_id,
            session_id,
            tool_use_id,
        } => commands::rewrite::rewrite(
            command,
            exclude,
            transparent_prefix,
            agent_id,
            session_id,
            tool_use_id,
        ),
        Commands::Hook(hook_cmd) => match hook_cmd {
            HookCommands::Rewrite { target } => commands::hook::hook_rewrite(&target),
            HookCommands::Compress { semantic } => commands::hook::hook_compress(semantic),
            HookCommands::Diff => commands::hook::hook_diff(),
        },
        Commands::Init { global, agent } => commands::init_cmd::handle(global, agent),
        Commands::EnvCheck {
            tool,
            all,
            fix,
            checklist,
            json,
        } => commands::env_check_cmd::handle(tool, all, fix, checklist, json),
        Commands::Mcp(McpAction::Start) => {
            commands::mcp_cmd::handle();
            Ok(())
        }
        Commands::Demo => {
            println!("{}", commands::demo::generate());
            Ok(())
        }
        Commands::Tui { refresh, lang } => commands::tui::handle(refresh, lang),
        Commands::Stats(stats_cmd) => match stats_cmd {
            StatsCommands::Summary { limit } => commands::stats::stats_summary(limit),
            StatsCommands::List { limit } => commands::stats::stats_list(limit),
            StatsCommands::Show { id } => commands::stats::stats_show(id),
            StatsCommands::Clear { yes } => commands::stats::stats_clear(yes),
            StatsCommands::Rewrites { limit, offset } => {
                commands::stats::stats_rewrites(limit, offset)
            }
            StatsCommands::Status => commands::stats::stats_status(),
            StatsCommands::Enable => commands::stats::stats_enable(),
            StatsCommands::Disable => commands::stats::stats_disable(),
            StatsCommands::Diff { since, until } => commands::stats::stats_diff(since, until),
        },
    }
}

fn main() {
    if let Err((msg, code)) = run() {
        eprintln!("Error: {msg}");
        std::process::exit(code);
    }
}
