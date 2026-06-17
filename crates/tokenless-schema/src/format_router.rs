//! Intelligent format router: analyzes JSON structure and selects the
//! optimal encoding strategy for maximum token savings.
//!
//! # Routing Logic
//!
//! 1. Input < 200 chars → skip encoding (fall back to [`Strategy::CompressorOnly`]).
//! 2. Uniform object arrays with >= 5 items → [`Strategy::ToonHrv`].
//! 3. Schema-like with enums or constraints → [`Strategy::EnhancedToon`].
//! 4. Deep single-child chains (> 3 levels) → [`Strategy::EnhancedToon`].
//! 5. Everything else → [`Strategy::CjsonCompact`].

use serde_json::Value;

use crate::{
    encoding,
    shape_analyzer::{self, JsonShape},
};

/// Selected encoding strategy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Strategy {
    /// TOON Header-Row-Value for uniform object arrays.
    ToonHrv,
    /// Enhanced TOON for schemas with enums, ranges, and deep chains.
    EnhancedToon,
    /// CJSON compact encoding for irregular structures.
    CjsonCompact,
    /// Skip encoding — input is too small to benefit from encoding.
    CompressorOnly,
}

/// Auto-select the best strategy based on JSON shape.
#[must_use]
pub fn select_strategy(shape: &JsonShape) -> Strategy {
    // Too small? Don't bother encoding.
    if shape.char_count < 200 {
        return Strategy::CompressorOnly;
    }

    // Uniform array with >= 5 items? TOON HRV is optimal.
    if shape.is_uniform_array && shape.item_count >= 5 {
        return Strategy::ToonHrv;
    }

    // Schema-like with enums or constraints? Enhanced TOON.
    if shape.has_enums || shape.has_constraints {
        return Strategy::EnhancedToon;
    }

    // Deep chains (> 3 levels)? Enhanced TOON with dot-path.
    if shape.max_chain_depth > 3 {
        return Strategy::EnhancedToon;
    }

    // Default: CJSON compact as safe fallback.
    Strategy::CjsonCompact
}

/// Analyze, select strategy, and encode in one call.
///
/// Returns the selected strategy and the compressed output string.
/// When experimental mode is disabled via [`crate::set_experimental_mode`],
/// falls back to core response compression instead of using enhanced encoders.
///
/// Prefer [`compress_auto_with`] for explicit, testable control — this
/// function reads the global [`crate::is_experimental_mode`] flag.
#[must_use]
pub fn compress_auto(value: &Value, input_str: &str) -> (Strategy, String) {
    let options =
        crate::CompressionOptions::default().with_experimental_mode(crate::is_experimental_mode());
    compress_auto_with(value, input_str, &options)
}

/// Analyze, select strategy, and encode with explicit [`crate::CompressionOptions`].
///
/// This is the core entry point. Unlike [`compress_auto`], it accepts an
/// explicit options struct so callers can control experimental features
/// without touching global state — ideal for testing.
///
/// # Examples
///
/// ```
/// use serde_json::json;
/// use tokenless_schema::{CompressionOptions, compress_auto_with};
///
/// let value = json!({"name": "test"});
/// let input_str = serde_json::to_string(&value).unwrap();
///
/// // Use default options (experimental mode on):
/// let (strategy, output) = compress_auto_with(&value, &input_str, &CompressionOptions::default());
///
/// // Explicitly disable experimental features:
/// let opts = CompressionOptions::new().with_experimental_mode(false);
/// let (strategy, output) = compress_auto_with(&value, &input_str, &opts);
/// ```
#[must_use]
pub fn compress_auto_with(
    value: &Value,
    input_str: &str,
    options: &crate::CompressionOptions,
) -> (Strategy, String) {
    // When experimental mode is off, skip the format router entirely
    // and fall back to core response compression.
    if !options.experimental_mode {
        let compressor = crate::ResponseCompressor::new();
        let compressed = compressor.compress(value);
        let output = serde_json::to_string(&compressed).unwrap_or_default();
        return (Strategy::CompressorOnly, output);
    }

    let shape = shape_analyzer::analyze(value, input_str);
    let strategy = select_strategy(&shape);
    let output = match strategy {
        Strategy::ToonHrv => encoding::encode_toon_hrv(value),
        Strategy::EnhancedToon => encoding::encode_enhanced(value, 0),
        Strategy::CjsonCompact => encoding::encode_cjson(value),
        Strategy::CompressorOnly => serde_json::to_string(value).unwrap_or_default(),
    };
    (strategy, output)
}

/// Return a human-readable strategy name (for CLI output).
#[must_use]
pub fn strategy_name(strategy: &Strategy) -> &'static str {
    match strategy {
        Strategy::ToonHrv => "toon-hrv",
        Strategy::EnhancedToon => "enhanced-toon",
        Strategy::CjsonCompact => "cjson-compact",
        Strategy::CompressorOnly => "passthrough",
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::shape_analyzer::TopType;

    fn make_shape(char_count: usize) -> JsonShape {
        JsonShape {
            top_level: TopType::Object,
            key_count: 0,
            item_count: 0,
            max_depth: 1,
            is_uniform_array: false,
            has_enums: false,
            has_constraints: false,
            max_chain_depth: 0,
            char_count,
        }
    }

    #[test]
    fn test_router_small_input_passthrough() {
        let shape = make_shape(50);
        let strategy = select_strategy(&shape);
        assert_eq!(strategy, Strategy::CompressorOnly);
    }

    #[test]
    fn test_router_selects_hrv() {
        let shape = JsonShape {
            top_level: TopType::Array,
            key_count: 0,
            item_count: 10,
            max_depth: 2,
            is_uniform_array: true,
            has_enums: false,
            has_constraints: false,
            max_chain_depth: 0,
            char_count: 500,
        };
        let strategy = select_strategy(&shape);
        assert_eq!(strategy, Strategy::ToonHrv);
    }

    #[test]
    fn test_router_selects_enhanced_for_enums() {
        let shape = JsonShape {
            top_level: TopType::Object,
            key_count: 5,
            item_count: 0,
            max_depth: 3,
            is_uniform_array: false,
            has_enums: true,
            has_constraints: false,
            max_chain_depth: 0,
            char_count: 500,
        };
        let strategy = select_strategy(&shape);
        assert_eq!(strategy, Strategy::EnhancedToon);
    }

    #[test]
    fn test_router_selects_enhanced_for_constraints() {
        let shape = JsonShape {
            top_level: TopType::Object,
            key_count: 3,
            item_count: 0,
            max_depth: 3,
            is_uniform_array: false,
            has_enums: false,
            has_constraints: true,
            max_chain_depth: 0,
            char_count: 500,
        };
        let strategy = select_strategy(&shape);
        assert_eq!(strategy, Strategy::EnhancedToon);
    }

    #[test]
    fn test_router_selects_enhanced_for_deep_chain() {
        let shape = JsonShape {
            top_level: TopType::Object,
            key_count: 1,
            item_count: 0,
            max_depth: 6,
            is_uniform_array: false,
            has_enums: false,
            has_constraints: false,
            max_chain_depth: 5,
            char_count: 500,
        };
        let strategy = select_strategy(&shape);
        assert_eq!(strategy, Strategy::EnhancedToon);
    }

    #[test]
    fn test_router_selects_compact_as_default() {
        let shape = JsonShape {
            top_level: TopType::Object,
            key_count: 3,
            item_count: 0,
            max_depth: 2,
            is_uniform_array: false,
            has_enums: false,
            has_constraints: false,
            max_chain_depth: 1,
            char_count: 500,
        };
        let strategy = select_strategy(&shape);
        assert_eq!(strategy, Strategy::CjsonCompact);
    }

    #[test]
    #[allow(
        clippy::unwrap_used,
        reason = "test: unwrap on known-valid serialization"
    )]
    fn test_compress_auto_integration() {
        let value = json!({
            "name": {"type": "string", "enum": ["admin", "user"]}
        });
        let input_str = serde_json::to_string(&value).unwrap();
        let (strategy, output) = compress_auto(&value, &input_str);
        // Has enums but also less than 200 chars.
        // With tiny input, it'll be CompressorOnly.
        assert!(matches!(
            strategy,
            Strategy::CompressorOnly | Strategy::EnhancedToon
        ));
        assert!(!output.is_empty());
    }

    #[test]
    fn test_strategy_name_all_variants() {
        assert_eq!(strategy_name(&Strategy::ToonHrv), "toon-hrv");
        assert_eq!(strategy_name(&Strategy::EnhancedToon), "enhanced-toon");
        assert_eq!(strategy_name(&Strategy::CjsonCompact), "cjson-compact");
        assert_eq!(strategy_name(&Strategy::CompressorOnly), "passthrough");
    }

    // ── compress_auto_with ────────────────────────────────────────────

    #[test]
    #[allow(
        clippy::unwrap_used,
        reason = "test: unwrap on known-valid serialization"
    )]
    fn test_compress_auto_with_experimental_disabled_falls_back() {
        let value = json!({
            "items": [
                {"name": "admin", "role": "superuser"},
                {"name": "user", "role": "member"},
                {"name": "guest", "role": "viewer"},
                {"name": "editor", "role": "writer"},
                {"name": "moderator", "role": "manager"}
            ]
        });
        let input_str = serde_json::to_string(&value).unwrap();
        let options = crate::CompressionOptions::new().with_experimental_mode(false);
        let (strategy, output) = compress_auto_with(&value, &input_str, &options);
        assert_eq!(
            strategy,
            Strategy::CompressorOnly,
            "experimental=false should always use CompressorOnly"
        );
        assert!(!output.is_empty());
    }

    #[test]
    #[allow(
        clippy::unwrap_used,
        reason = "test: unwrap on known-valid serialization"
    )]
    fn test_compress_auto_with_experimental_enabled_uses_router() {
        // Top-level uniform array with >= 5 items and > 200 chars routes to ToonHrv.
        let value = json!([
            {"name": "admin", "role": "superuser", "permissions": "full_access"},
            {"name": "user", "role": "member", "permissions": "restricted_access"},
            {"name": "guest", "role": "viewer", "permissions": "read_only_access"},
            {"name": "editor", "role": "writer", "permissions": "content_management"},
            {"name": "moderator", "role": "manager", "permissions": "moderation_access"}
        ]);
        let input_str = serde_json::to_string(&value).unwrap();
        assert!(
            input_str.len() >= 200,
            "test fixture must exceed 200 chars to trigger routing; got {}",
            input_str.len()
        );
        let options = crate::CompressionOptions::default();
        let (strategy, output) = compress_auto_with(&value, &input_str, &options);
        // With experimental mode on, top-level uniform array of 5 items → ToonHrv
        assert_eq!(
            strategy,
            Strategy::ToonHrv,
            "experimental=true should route uniform arrays to ToonHrv"
        );
        assert!(!output.is_empty());
    }

    #[test]
    #[allow(
        clippy::unwrap_used,
        reason = "test: unwrap on known-valid serialization"
    )]
    fn test_compress_auto_and_compress_auto_with_are_consistent() {
        // When the global flag matches the explicit option, both functions
        // should produce identical output.
        crate::set_experimental_mode(true);
        let value = json!({
            "items": [
                {"a": 1, "b": 2},
                {"a": 3, "b": 4},
                {"a": 5, "b": 6},
                {"a": 7, "b": 8},
                {"a": 9, "b": 10}
            ]
        });
        let input_str = serde_json::to_string(&value).unwrap();
        let (s1, o1) = compress_auto(&value, &input_str);
        let opts = crate::CompressionOptions::default();
        let (s2, o2) = compress_auto_with(&value, &input_str, &opts);
        assert_eq!(s1, s2);
        assert_eq!(o1, o2);
    }

    #[test]
    #[allow(
        clippy::unwrap_used,
        reason = "test: unwrap on known-valid serialization"
    )]
    fn test_compress_auto_with_respects_disabled_despite_global() {
        // Even if the global flag is true, compress_auto_with should
        // respect the explicit CompressionOptions.
        crate::set_experimental_mode(true);
        let value = json!({
            "items": [
                {"name": "a"}, {"name": "b"}, {"name": "c"},
                {"name": "d"}, {"name": "e"}
            ]
        });
        let input_str = serde_json::to_string(&value).unwrap();
        let options = crate::CompressionOptions::new().with_experimental_mode(false);
        let (strategy, _output) = compress_auto_with(&value, &input_str, &options);
        assert_eq!(
            strategy,
            Strategy::CompressorOnly,
            "explicit options should take precedence over global state"
        );
    }
}
