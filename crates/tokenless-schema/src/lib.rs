//! Tokenless schema and response compression library.
//!
//! Provides [`SchemaCompressor`] for compressing `OpenAI Function Calling` tool
//! definitions and [`ResponseCompressor`] for compressing JSON API responses.
//! The [`format_router`] module intelligently selects the optimal encoding
//! strategy based on JSON structural shape.
//!
//! # Experimental Mode
//!
//! Call [`set_experimental_mode(false)`] to disable enhanced features
//! (format router, enhanced TOON encoders) and fall back to core
//! compression only. Default is `true` (all features enabled).

#![forbid(unsafe_code)]

use std::sync::atomic::{AtomicBool, Ordering};

/// JSON response compression — removes debug fields, truncates strings / arrays.
pub mod response_compressor;

/// `OpenAI Function Calling` schema compression — truncates descriptions, removes titles/examples.
pub mod schema_compressor;

/// JSON structure analyzer — detects shape, uniformity, and constraints.
pub mod shape_analyzer;

/// Encoding strategies for the format router (TOON HRV, Enhanced TOON, CJSON Compact).
pub mod encoding;

/// Intelligent format router — selects optimal encoding strategy based on JSON shape.
pub mod format_router;

pub use format_router::{Strategy, compress_auto, select_strategy, strategy_name};
pub use response_compressor::{CompressionProfile, ResponseCompressor};
pub use schema_compressor::SchemaCompressor;
pub use shape_analyzer::{JsonShape, TopType, analyze};

// ── Experimental mode global toggle ──────────────────────────────────────────

static EXPERIMENTAL_MODE: AtomicBool = AtomicBool::new(true);

/// Enable or disable experimental mode.
///
/// When disabled, [`compress_auto`] falls back to core response compression
/// instead of using the format router with enhanced TOON encoders.
/// Default is `true` (all features enabled).
///
/// # Examples
///
/// ```
/// use tokenless_schema;
///
/// // Disable experimental features
/// tokenless_schema::set_experimental_mode(false);
/// assert!(!tokenless_schema::is_experimental_mode());
///
/// // Re-enable
/// tokenless_schema::set_experimental_mode(true);
/// assert!(tokenless_schema::is_experimental_mode());
/// ```
pub fn set_experimental_mode(enabled: bool) {
    EXPERIMENTAL_MODE.store(enabled, Ordering::Release);
}

/// Check whether experimental mode is enabled.
///
/// Returns `true` by default. See [`set_experimental_mode`].
#[must_use]
pub fn is_experimental_mode() -> bool {
    EXPERIMENTAL_MODE.load(Ordering::Acquire)
}
