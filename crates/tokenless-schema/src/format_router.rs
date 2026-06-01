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
#[must_use]
pub fn compress_auto(value: &Value, input_str: &str) -> (Strategy, String) {
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
}
