//! Tokenless schema and response compression library.
//!
//! Provides [`SchemaCompressor`] for compressing `OpenAI Function Calling` tool
//! definitions and [`ResponseCompressor`] for compressing JSON API responses.
//! The [`format_router`] module intelligently selects the optimal encoding
//! strategy based on JSON structural shape.
//!
//! # Experimental Mode
//!
//! Call [`set_experimental_mode`] to disable enhanced features
//! (format router, enhanced TOON encoders) and fall back to core
//! compression only. Default is `true` (all features enabled).
//!
//! # Explicit Configuration
//!
//! Prefer [`compress_auto_with`] over [`compress_auto`] for testable,
//! side-effect-free control. The [`CompressionOptions`] struct lets callers
//! explicitly pass the experimental mode flag without touching global state.

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

/// Browser/Node.js WASM bindings exposed via `wasm-bindgen`.
#[cfg(feature = "wasm")]
pub mod wasm;

pub use format_router::{
    Strategy, compress_auto, compress_auto_with, select_strategy, strategy_name,
};
pub use response_compressor::{CompressionProfile, ResponseCompressor};
pub use schema_compressor::SchemaCompressor;
pub use shape_analyzer::{JsonShape, TopType, analyze};

// ── Compression options ────────────────────────────────────────────────────────

/// Configuration for compression behavior.
///
/// Pass this to [`compress_auto_with`] for explicit, testable control over
/// experimental features instead of relying on the global [`set_experimental_mode`].
///
/// # Examples
///
/// ```
/// use tokenless_schema::CompressionOptions;
///
/// let options = CompressionOptions::default();
/// assert!(options.experimental_mode);
///
/// let disabled = CompressionOptions::new().with_experimental_mode(false);
/// assert!(!disabled.experimental_mode);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompressionOptions {
    /// Whether experimental features (format router, enhanced TOON) are enabled.
    pub experimental_mode: bool,
}

impl Default for CompressionOptions {
    fn default() -> Self {
        Self {
            experimental_mode: true,
        }
    }
}

impl CompressionOptions {
    /// Create options with all defaults.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set whether experimental mode is enabled.
    ///
    /// When disabled, [`compress_auto_with`] falls back to core response
    /// compression instead of using the format router.
    #[must_use]
    pub fn with_experimental_mode(mut self, enabled: bool) -> Self {
        self.experimental_mode = enabled;
        self
    }
}

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
