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
    clippy::pedantic,
    reason = "pre-existing CLI code conventions; pedantic enforced at crate level"
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
        /// Project name for multi-project statistics.
        #[arg(long)]
        project: Option<String>,
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
        /// Project name for multi-project statistics.
        #[arg(long)]
        project: Option<String>,
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
        /// Project name for multi-project statistics.
        #[arg(long)]
        project: Option<String>,
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
        /// Project name for multi-project statistics.
        #[arg(long)]
        project: Option<String>,
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
        /// Project name for multi-project statistics.
        #[arg(long)]
        project: Option<String>,
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
        /// Enable debug logging for compress hook (~/.tokenfleet-ai/tokenless/compress-debug.log).
        #[arg(long)]
        debug: bool,
        /// Enable compress hook installation (default).
        #[arg(long, conflicts_with = "no_compress")]
        compress: bool,
        /// Disable compress hook installation.
        #[arg(long, conflicts_with = "compress")]
        no_compress: bool,
        /// Enable passthrough mode: hooks record logs but do not rewrite/compress.
        /// Use this to measure baseline token usage for cost comparison.
        #[arg(long)]
        passthrough: bool,
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
        /// Display language: "zh" or "en" (default: zh (Chinese)).
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
        /// Project name for multi-project statistics.
        #[arg(long)]
        project: Option<String>,
        /// User name override (default: auto-detected from config).
        #[arg(long)]
        user_name: Option<String>,
    },
    /// PostToolUse: compress tool response output.
    Compress {
        /// Enable semantic-aware field filtering (Level 2 ONNX or Level 1 rules).
        #[arg(long)]
        semantic: bool,
        /// Agent target (e.g., "claude").
        #[arg(short, long, default_value = "claude")]
        target: String,
        /// Project name for multi-project statistics.
        #[arg(long)]
        project: Option<String>,
        /// User name override (default: auto-detected from config).
        #[arg(long)]
        user_name: Option<String>,
        /// Write original/compressed text to debug log (~/.tokenfleet-ai/tokenless/compress-debug.log).
        #[arg(long)]
        debug: bool,
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
        #[arg(long)]
        project: Option<String>,
        #[arg(long)]
        namespace: Option<String>,
    },
    /// List recent records.
    List {
        #[arg(short, long, default_value = "20")]
        limit: usize,
        #[arg(long)]
        project: Option<String>,
        #[arg(long)]
        namespace: Option<String>,
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
        /// Project name for multi-project filtering.
        #[arg(long)]
        project: Option<String>,
    },
    /// Show stats recording status.
    Status,
    /// Enable stats recording.
    Enable,
    /// Disable stats recording.
    Disable,
    /// Enable experimental mode (format router, enhanced TOON, semantic, TUI, MCP).
    ExperimentalOn,
    /// Disable experimental mode (core compression only).
    ExperimentalOff,
    /// Show cumulative savings for a time period.
    Diff {
        #[arg(long)]
        since: Option<String>,
        #[arg(long)]
        project: Option<String>,
        #[arg(long)]
        namespace: Option<String>,
        #[arg(long)]
        until: Option<String>,
    },
}

// ── Dispatch ─────────────────────────────────────────────────────────────────

fn run() -> Result<(), (String, i32)> {
    // Init tracing: stderr + log file
    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::util::SubscriberInitExt;

    let log_path = crate::shared::get_tokenless_dir().join("tokenless.log");
    let log_file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .ok(); // None if file can't be opened

    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn"));

    let stderr_layer = tracing_subscriber::fmt::layer()
        .with_writer(std::io::stderr)
        .with_target(false);

    if let Some(file) = log_file {
        let file_layer = tracing_subscriber::fmt::layer()
            .with_writer(std::sync::Mutex::new(file))
            .with_ansi(false)
            .with_target(false);
        tracing_subscriber::registry()
            .with(env_filter)
            .with(stderr_layer)
            .with(file_layer)
            .init();
    } else {
        tracing_subscriber::registry()
            .with(env_filter)
            .with(stderr_layer)
            .init();
    }

    // Sync experimental mode from config/env to the SDK so library
    // callers (compress_auto, etc.) also respect the setting.
    let config = tokenless_stats::TokenlessConfig::load();
    tokenless_schema::set_experimental_mode(config.is_experimental_enabled());

    let cli = Cli::parse();

    match cli.command {
        Commands::CompressSchema {
            file,
            batch,
            report,
            project,
            agent_id,
            session_id,
            tool_use_id,
        } => commands::compress::compress_schema(
            file,
            batch,
            report,
            project,
            agent_id,
            session_id,
            tool_use_id,
        ),
        Commands::CompressResponse {
            file,
            report,
            context,
            semantic,
            project,
            agent_id,
            session_id,
            tool_use_id,
        } => commands::compress::compress_response(
            file,
            report,
            semantic,
            context,
            project,
            agent_id,
            session_id,
            tool_use_id,
        ),
        Commands::CompressAuto {
            file,
            report,
            project,
            agent_id,
            session_id,
            tool_use_id,
        } => commands::compress::compress_auto(
            file,
            report,
            project,
            agent_id,
            session_id,
            tool_use_id,
        ),
        Commands::CompressToon {
            file,
            project,
            agent_id,
            session_id,
            tool_use_id,
        } => commands::toon::compress_toon(file, project, agent_id, session_id, tool_use_id),
        Commands::DecompressToon { file } => commands::toon::decompress_toon(file),
        Commands::Rewrite {
            command,
            exclude,
            transparent_prefix,
            project,
            agent_id,
            session_id,
            tool_use_id,
        } => commands::rewrite::rewrite(
            command,
            exclude,
            transparent_prefix,
            project,
            agent_id,
            session_id,
            tool_use_id,
        ),
        Commands::Hook(hook_cmd) => match hook_cmd {
            HookCommands::Rewrite {
                target,
                project,
                user_name,
            } => commands::hook::hook_rewrite(&target, project, user_name),
            HookCommands::Compress {
                semantic,
                target,
                project,
                user_name,
                debug,
            } => commands::hook::hook_compress(semantic, &target, project, user_name, debug),
            HookCommands::Diff => commands::hook::hook_diff(),
        },
        Commands::Init {
            global,
            agent,
            debug,
            compress,
            no_compress,
            passthrough,
        } => commands::init_cmd::handle(global, agent, debug, compress, no_compress, passthrough),
        Commands::EnvCheck {
            tool,
            all,
            fix,
            checklist,
            json,
        } => commands::env_check_cmd::handle(tool, all, fix, checklist, json),
        Commands::Mcp(McpAction::Start) => {
            if !shared::is_experimental_enabled() {
                return Err((
                    "MCP server is an experimental feature. Enable with: tokenless stats experimental-on"
                        .to_string(),
                    1,
                ));
            }
            commands::mcp_cmd::handle();
            Ok(())
        }
        Commands::Demo => {
            println!("{}", commands::demo::generate());
            Ok(())
        }
        Commands::Tui { refresh, lang } => {
            if !shared::is_experimental_enabled() {
                return Err((
                    "TUI dashboard is an experimental feature. Enable with: tokenless stats experimental-on"
                        .to_string(),
                    1,
                ));
            }
            commands::tui::handle(refresh, lang)
        }
        Commands::Stats(stats_cmd) => match stats_cmd {
            StatsCommands::Summary {
                limit,
                project,
                namespace,
            } => commands::stats::stats_summary(limit, project, namespace),
            StatsCommands::List {
                limit,
                project,
                namespace,
            } => commands::stats::stats_list(limit, project, namespace),
            StatsCommands::Show { id } => commands::stats::stats_show(id),
            StatsCommands::Clear { yes } => commands::stats::stats_clear(yes),
            StatsCommands::Rewrites {
                limit,
                offset,
                project,
            } => commands::stats::stats_rewrites(limit, offset, project),
            StatsCommands::Status => commands::stats::stats_status(),
            StatsCommands::Enable => commands::stats::stats_enable(),
            StatsCommands::Disable => commands::stats::stats_disable(),
            StatsCommands::ExperimentalOn => commands::stats::stats_experimental_on(),
            StatsCommands::ExperimentalOff => commands::stats::stats_experimental_off(),
            StatsCommands::Diff {
                since,
                until,
                project,
                namespace: _,
            } => commands::stats::stats_diff(since, until, project),
        },
    }
}

fn main() {
    if let Err((msg, code)) = run() {
        eprintln!("Error: {msg}");
        std::process::exit(code);
    }
}
