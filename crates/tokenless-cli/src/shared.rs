//! Shared state and utility functions for the tokenless CLI.

use std::{
    fs,
    io::{self, Read},
    process,
    sync::{LazyLock, Mutex, OnceLock},
};

use rtk_registry::{RtkInstallStatus, is_rtk_installed};
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

/// Get the stats database path from env or default.
pub(crate) fn get_db_path() -> String {
    std::env::var("TOKENLESS_STATS_DB")
        .unwrap_or_else(|_| format!("{}/.tokenless/stats.db", get_home_dir()))
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
pub(crate) fn record_compression_stats(
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
    let mut record = tokenless_stats::StatsRecord::new(
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
    eprintln!(
        "chars: {before_chars} → {after_chars}  tokens: ~{before_tokens} → ~{after_tokens}  \
         saved: {saved_pct:.1}%",
    );
}
