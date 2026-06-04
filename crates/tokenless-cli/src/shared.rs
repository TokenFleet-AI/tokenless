//! Shared state and utility functions for the tokenless CLI.

use std::{
    fs,
    io::{self, Read},
    process,
    sync::{LazyLock, Mutex, OnceLock},
};

use rtk_registry::{Classification, RtkInstallStatus, is_rtk_installed};
use serde::Serialize;
use tokenless_schema::{CompressionProfile, ResponseCompressor, SchemaCompressor};
use tokenless_semantic::SemanticCompressor;
use tokenless_stats::{OperationType, StatsRecorder, TokenlessConfig, estimate_tokens_from_bytes};

/// Semantic-aware compressor (Level 1 rule matching; Level 2 ONNX via `--semantic`).
pub(crate) static SEMANTIC_COMPRESSOR: LazyLock<Mutex<SemanticCompressor>> =
    LazyLock::new(|| Mutex::new(SemanticCompressor::new()));
/// Reusable schema compressor instance (immutable after construction).
pub(crate) static SCHEMA_COMPRESSOR: LazyLock<SchemaCompressor> =
    LazyLock::new(SchemaCompressor::new);
/// Reusable response compressor instance (immutable after construction).
pub(crate) static RESPONSE_COMPRESSOR: LazyLock<ResponseCompressor> =
    LazyLock::new(ResponseCompressor::new);
/// Response compressor for shell commands / high-fidelity output (4096 chars, 128 items).
pub(crate) static RESPONSE_COMPRESSOR_HF: LazyLock<ResponseCompressor> =
    LazyLock::new(|| ResponseCompressor::new().with_profile(CompressionProfile::HighFidelity));

/// Select a response compressor based on the tool name from a hook payload.
pub(crate) fn compressor_for_tool(tool_name: &str) -> &ResponseCompressor {
    match tool_name {
        "Bash" | "bash" => &RESPONSE_COMPRESSOR_HF,
        _ => &RESPONSE_COMPRESSOR,
    }
}

/// Strip leading UTF-8 BOM bytes (Cursor on Windows may prepend one or two).
pub(crate) fn strip_leading_bom(input: &str) -> String {
    input
        .strip_prefix('\u{feff}')
        .or_else(|| input.strip_prefix("\u{feff}\u{feff}"))
        .unwrap_or(input)
        .to_string()
}

/// Cached result of `is_rtk_installed()`, checked at most once per process lifetime.
pub(crate) fn rtk_available() -> bool {
    static RTK_OK: OnceLock<bool> = OnceLock::new();
    *RTK_OK.get_or_init(|| matches!(is_rtk_installed(), RtkInstallStatus::Installed { .. }))
}

/// Check whether experimental mode is enabled.
///
/// Returns `false` when `TOKENLESS_EXPERIMENTAL=0`/`false` or
/// `experimental_mode: false` in `~/.tokenless/config.json`.
/// Defaults to `true` (all features enabled).
#[must_use]
pub(crate) fn is_experimental_enabled() -> bool {
    TokenlessConfig::load().is_experimental_enabled()
}

// ── File & DB utilities ──────────────────────────────────────────────────────

/// Read input from a file or stdin.
pub(crate) fn read_input(file: &Option<String>) -> Result<String, String> {
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
pub(crate) fn get_home_dir() -> String {
    dirs::home_dir()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| std::env::var("HOME").unwrap_or_else(|_| ".".to_string()))
}

/// Get the tokenless workspace directory (`~/.tokenfleet-ai/tokenless`).
///
/// Creates the directory if it does not exist.
#[must_use]
pub(crate) fn get_tokenless_dir() -> std::path::PathBuf {
    let dir = std::path::PathBuf::from(get_home_dir())
        .join(".tokenfleet-ai")
        .join("tokenless");
    #[allow(clippy::disallowed_methods)]
    if let Err(e) = fs::create_dir_all(&dir) {
        tracing::warn!(error = %e, path = %dir.display(), "failed to create tokenless dir");
    }
    dir
}

/// Get the stats database path from env or default.
pub(crate) fn get_db_path() -> String {
    std::env::var("TOKENLESS_STATS_DB")
        .unwrap_or_else(|_| get_tokenless_dir().join("stats.db").display().to_string())
}

/// Get the reports directory for sending before-stats to agent-proxy.
pub(crate) fn get_reports_dir() -> std::path::PathBuf {
    let dir = get_tokenless_dir().join("reports");
    #[allow(clippy::disallowed_methods)]
    if let Err(e) = fs::create_dir_all(&dir) {
        tracing::warn!(error = %e, path = %dir.display(), "failed to create reports dir");
    }
    dir
}

/// Ensure the stats database directory exists.
pub(crate) fn ensure_db_dir() -> Result<(), (String, i32)> {
    let db_path = get_db_path();
    if let Some(parent) = std::path::Path::new(&db_path).parent() {
        #[allow(clippy::disallowed_methods)]
        fs::create_dir_all(parent)
            .map_err(|e| (format!("Failed to create database directory: {e}"), 1))?;
    }
    Ok(())
}

/// Open the stats recorder, creating the DB directory if needed.
pub(crate) fn open_recorder() -> Result<StatsRecorder, (String, i32)> {
    ensure_db_dir()?;
    StatsRecorder::new(get_db_path()).map_err(|e| (format!("Failed to open database: {e}"), 1))
}

// ── Stats helpers ────────────────────────────────────────────────────────────

/// Record compression stats — fail-silent so compression output is never blocked.
///
/// `experimental` should be `true` only when the operation used experimental
/// features (format router, semantic compression, diff, etc.).
/// `method` identifies the specific compression strategy used (e.g. `"ToonHrv"`,
/// `"HighFidelity"`, `"RtkStandard"`).
#[allow(clippy::too_many_arguments)]
pub(crate) fn record_compression_stats(
    op: OperationType,
    agent_id: Option<String>,
    session_id: Option<String>,
    tool_use_id: Option<String>,
    project: Option<String>,
    user_name: Option<String>,
    before_text: String,
    after_text: String,
    experimental: bool,
    method: Option<String>,
) {
    if !TokenlessConfig::load().is_stats_enabled() {
        return;
    }

    let (before_bytes, after_bytes, before_tokens, after_tokens) = if op
        == OperationType::RewriteCommand
    {
        let n = before_text.len();
        let bt = estimate_tokens_from_bytes(n);
        // Use RTK's own classification to estimate savings rate and
        // average output tokens per command category.
        let (rate, avg_out) = estimate_rtk_savings(&before_text);
        // Use RTK's per-category avg output tokens if available (more
        // accurate than flat percentage), otherwise fall back to rate.
        let at = if avg_out > 0 && avg_out < bt {
            avg_out
        } else {
            (bt as f64 * (1.0 - rate)).ceil() as usize
        };
        let ab = (n as f64 * (1.0 - rate)).ceil() as usize;
        (n, ab, bt, at)
    } else {
        let bb = before_text.len();
        let ab = after_text.len();
        let bt = estimate_tokens_from_bytes(bb);
        let at = estimate_tokens_from_bytes(ab);
        if at >= bt {
            eprintln!("[tokenless] record_compression_stats SKIP: at={at} >= bt={bt} op={op:?}");
            return;
        }
        (bb, ab, bt, at)
    };

    let pid = process::id();
    let agent = agent_id.unwrap_or_else(|| "cli".to_string());
    let saved_tokens = before_tokens.saturating_sub(after_tokens);
    let saved_bytes = before_bytes.saturating_sub(after_bytes);

    let mut record = tokenless_stats::StatsRecord::new(
        op.clone(),
        agent.clone(),
        before_bytes,
        before_tokens,
        after_bytes,
        after_tokens,
    )
    .with_before_text(before_text)
    .with_after_text(after_text)
    .with_experimental_mode(experimental);
    if let Some(sid) = session_id.clone() {
        record = record.with_session_id(sid);
    }
    if let Some(tuid) = tool_use_id.clone() {
        record = record.with_tool_use_id(tuid);
    }
    if let Some(p) = project.clone() {
        record = record.with_project(p);
    }
    record = record.with_source_pid(pid as i64);

    if let Ok(recorder) = open_recorder() {
        if let Err(e) = recorder.record(&record) {
            tracing::warn!(error = %e, "stats record insert failed");
        }
    } else {
        tracing::warn!("failed to open stats recorder");
    }

    // ── Fire-and-forget: append report for agent-proxy ─────────────
    if let Some(sid) = session_id {
        let _ = append_report_to_file(ProxyReport {
            session_id: sid,
            agent_id: agent,
            project_path: project,
            user_name,
            op_type: op.clone(),
            method,
            before_tokens: before_tokens as u64,
            after_tokens: after_tokens as u64,
            saved_tokens: saved_tokens as u64,
            before_bytes: before_bytes as u64,
            after_bytes: after_bytes as u64,
            saved_bytes: saved_bytes as u64,
            timestamp: chrono::Utc::now().to_rfc3339(),
        });
    }
}

/// Estimate RTK token savings using `rtk-registry`'s built-in classification.
///
/// Returns `(savings_rate, avg_output_tokens)` where `savings_rate` is 0.0–1.0
/// and `avg_output_tokens` is RTK's estimate of the raw command output size.
fn estimate_rtk_savings(before_text: &str) -> (f64, usize) {
    // Parse the original command from the PreToolUse payload.
    let cmd = serde_json::from_str::<serde_json::Value>(before_text)
        .ok()
        .and_then(|v| {
            v.pointer("/tool_input/command")
                .and_then(|c| c.as_str())
                .map(|s| s.to_string())
        })
        .unwrap_or_default();

    if cmd.is_empty() {
        return (0.60, 0);
    }

    let classification = rtk_registry::classify_command(&cmd);
    match classification {
        Classification::Supported {
            estimated_savings_pct,
            category,
            ..
        } => {
            let rate = (estimated_savings_pct / 100.0).clamp(0.0, 1.0);
            // Extract subcommand for finer token estimation
            let subcmd = cmd.split_whitespace().nth(1).unwrap_or("");
            let avg_tokens = rtk_registry::category_avg_tokens(category, subcmd);
            (rate, avg_tokens)
        }
        _ => (0.60, 0),
    }
}

/// Print a token savings report to stderr.
pub(crate) fn eprint_report(
    before_chars: usize,
    before_tokens: usize,
    after_chars: usize,
    after_tokens: usize,
) {
    let saved_pct = if before_tokens > 0 {
        (before_tokens.saturating_sub(after_tokens) as f64 / before_tokens as f64) * 100.0
    } else {
        0.0
    };
    tracing::info!(
        before_chars,
        before_tokens,
        after_chars,
        after_tokens,
        saved_pct,
        "compression report"
    );
}

// ── Proxy report (agent-proxy integration) ─────────────────────────────────

/// A single compression event reported to agent-proxy via the shared reports
/// directory. Appended as one JSON line to `{reports_dir}/{session_id}.jsonl`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProxyReport {
    session_id: String,
    agent_id: String,
    project_path: Option<String>,
    user_name: Option<String>,
    op_type: OperationType,
    method: Option<String>,
    before_tokens: u64,
    after_tokens: u64,
    saved_tokens: u64,
    before_bytes: u64,
    after_bytes: u64,
    saved_bytes: u64,
    timestamp: String,
}

/// Append a single compression report line to the session report file.
///
/// The file lives at `~/.tokenfleet-ai/tokenless/reports/{session_id}.jsonl`.
/// agent-proxy reads and consumes these files via rename-then-read.
///
/// This is fire-and-forget: failures are traced and written to an error
/// log file but never block compression output.
fn append_report_to_file(report: ProxyReport) -> Result<(), ()> {
    use std::io::Write;

    let session_id = &report.session_id;
    let safe_sid: String = session_id
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .take(128)
        .collect();

    let file_path = get_reports_dir().join(format!("{safe_sid}.jsonl"));

    let line = serde_json::to_string(&report).map_err(|e| {
        eprintln!("[tokenless] report serialize failed: {e}");
        tracing::warn!(error = %e, session_id = %safe_sid, "report serialize failed");
    })?;

    let mut f = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&file_path)
        .map_err(|e| {
            eprintln!(
                "[tokenless] report file open failed: {e} path={}",
                file_path.display()
            );
            tracing::warn!(error = %e, path = %file_path.display(), "report file open failed");
        })?;

    eprintln!(
        "[tokenless] report written: op={:?} saved={} session={}",
        report.op_type, report.saved_tokens, safe_sid
    );
    writeln!(f, "{line}").map_err(|e| {
        eprintln!("[tokenless] report write failed: {e}");
        tracing::warn!(error = %e, session_id = %safe_sid, "report write failed");
    })
}
