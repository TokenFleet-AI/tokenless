//! JSON structure analyzer for format routing decisions.
//!
//! Performs a single-pass O(n) inspection of a JSON value to determine
//! its structural shape: uniformity, depth, enum/constraint presence.

use serde_json::Value;

use crate::encoding::is_schema_object;

/// Detected structural shape of a JSON value.
#[derive(Debug, Clone)]
pub struct JsonShape {
    /// Top-level type of the value.
    pub top_level: TopType,
    /// Total key count (for objects).
    pub key_count: usize,
    /// Item count (for arrays).
    pub item_count: usize,
    /// Maximum nesting depth.
    pub max_depth: usize,
    /// True if all array items are objects with identical key sets.
    pub is_uniform_array: bool,
    /// True if any object contains an `"enum"` key.
    pub has_enums: bool,
    /// True if any object contains `"minimum"`, `"maximum"`, or `"pattern"` constraints.
    pub has_constraints: bool,
    /// Longest single-child object chain depth.
    pub max_chain_depth: usize,
    /// Total characters of the input JSON string.
    pub char_count: usize,
}

/// Top-level type classification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TopType {
    /// Object (map).
    Object,
    /// Array.
    Array,
    /// Scalar (string, number, boolean, null).
    Scalar,
}

/// Analyze a JSON value and return its structural shape.
///
/// `input_str` should be the original JSON string (used only for `char_count`).
#[must_use]
pub fn analyze(value: &Value, input_str: &str) -> JsonShape {
    let top_level = match value {
        Value::Object(_) => TopType::Object,
        Value::Array(_) => TopType::Array,
        _ => TopType::Scalar,
    };

    let mut shape = JsonShape {
        top_level,
        key_count: 0,
        item_count: 0,
        max_depth: 0,
        is_uniform_array: false,
        has_enums: false,
        has_constraints: false,
        max_chain_depth: 0,
        char_count: input_str.len(),
    };

    match value {
        Value::Object(obj) => {
            shape.key_count = obj.len();
            analyze_object(obj, &mut shape, 1);
        }
        Value::Array(arr) => {
            shape.item_count = arr.len();
            shape.is_uniform_array = check_uniform_array(arr);
            for item in arr {
                analyze_value(item, &mut shape, 1);
            }
        }
        _ => {}
    }

    shape
}

fn analyze_value(value: &Value, shape: &mut JsonShape, depth: usize) {
    if depth > shape.max_depth {
        shape.max_depth = depth;
    }
    match value {
        Value::Object(obj) => analyze_object(obj, shape, depth),
        Value::Array(arr) => {
            for item in arr {
                analyze_value(item, shape, depth + 1);
            }
        }
        _ => {}
    }
}

fn analyze_object(obj: &serde_json::Map<String, Value>, shape: &mut JsonShape, depth: usize) {
    // Check for enum/constraints at this level, but only when the object
    // looks like a JSON Schema definition — not when a data object merely
    // shares key names with schema keywords.
    //
    // Without this guard, a data object such as:
    //   {"type": "premium", "pattern": "striped"}
    // would be flagged as has_enums/has_constraints and routed to Enhanced
    // TOON encoding, which would then flatten it incorrectly.
    if is_schema_object(obj) {
        if obj.contains_key("enum") {
            shape.has_enums = true;
        }
        if obj.contains_key("minimum") || obj.contains_key("maximum") || obj.contains_key("pattern")
        {
            shape.has_constraints = true;
        }
    }

    // Check chain depth: object with exactly 1 key whose value is also an object.
    if obj.len() == 1
        && let Some((_, Value::Object(inner))) = obj.iter().next()
    {
        let chain = count_chain_depth(inner) + 1;
        if chain > shape.max_chain_depth {
            shape.max_chain_depth = chain;
        }
    }

    for (_key, val) in obj {
        analyze_value(val, shape, depth + 1);
    }
}

fn count_chain_depth(obj: &serde_json::Map<String, Value>) -> usize {
    if obj.len() == 1
        && let Some((_, Value::Object(inner))) = obj.iter().next()
    {
        return count_chain_depth(inner) + 1;
    }
    0
}

/// Check if an array has uniform structure: all items are objects with identical keys.
///
/// Only checks the first 100 items for performance.
fn check_uniform_array(arr: &[Value]) -> bool {
    if arr.is_empty() {
        return false;
    }

    let first_keys = arr.first().and_then(|v| {
        v.as_object().map(|o| {
            let mut ks: Vec<_> = o.keys().collect();
            ks.sort_unstable();
            ks
        })
    });

    let Some(first_keys) = first_keys else {
        return false;
    };

    let limit = arr.len().min(100);
    arr.get(1..limit).unwrap_or(&[]).iter().all(|item| {
        item.as_object()
            .is_some_and(|o| key_set_matches(o, &first_keys))
    })
}

/// Check whether an object has exactly the same keys as `expected_keys` (in any order).
fn key_set_matches(obj: &serde_json::Map<String, Value>, expected_keys: &[&String]) -> bool {
    let mut ks: Vec<_> = obj.keys().collect();
    ks.sort_unstable();
    ks == expected_keys
}

#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "tests use unwrap/expect for clarity and panic-on-failure semantics"
)]
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_analyze_uniform_array() {
        let value = json!([{"a": 1, "b": 2}, {"a": 3, "b": 4}, {"a": 5, "b": 6}]);
        let shape = analyze(&value, "");
        assert!(shape.is_uniform_array);
        assert_eq!(shape.item_count, 3);
        assert_eq!(shape.top_level, TopType::Array);
    }

    #[test]
    fn test_analyze_mixed_array() {
        let value = json!([{"a": 1}, {"b": 2}, {"a": 3, "b": 4}]);
        let shape = analyze(&value, "");
        assert!(!shape.is_uniform_array);
    }

    #[test]
    fn test_analyze_has_enums() {
        let value = json!({"type": "string", "enum": ["a", "b", "c"]});
        let shape = analyze(&value, "");
        assert!(shape.has_enums);
    }

    #[test]
    fn test_analyze_has_constraints() {
        // Must include a valid schema type to be recognized as schema-like.
        let value = json!({"type": "integer", "minimum": 1, "maximum": 100});
        let shape = analyze(&value, "");
        assert!(shape.has_constraints);
    }

    #[test]
    fn test_analyze_deep_chain() {
        let value = json!({"a": {"b": {"c": {"d": {"e": 1}}}}});
        let shape = analyze(&value, "");
        assert!(shape.max_chain_depth >= 3);
        assert!(!shape.has_enums);
    }

    // ── Security: shape_analyzer false-positive guards ──────────────────

    #[test]
    fn test_analyze_data_object_no_false_enum() {
        // A data object with `"type": "premium"` is NOT a schema.
        let value = json!({"product": {"type": "premium", "color": "blue"}});
        let shape = analyze(&value, "");
        assert!(!shape.has_enums, "data object should not trigger has_enums");
        assert!(
            !shape.has_constraints,
            "data object should not trigger has_constraints"
        );
    }

    #[test]
    fn test_analyze_data_object_with_enum_key_no_false_positive() {
        // A data object where "enum" is a key but it's not a schema.
        let value = json!({"config": {"enum": ["a", "b"], "color": "red"}});
        let shape = analyze(&value, "");
        assert!(
            !shape.has_enums,
            "data object with 'enum' key should not trigger has_enums"
        );
    }

    #[test]
    fn test_analyze_data_object_with_pattern_key_no_false_positive() {
        // A data object with "pattern" and "minimum" keys but not a schema.
        let value = json!({"filter": {"pattern": "*.js", "minimum": 1, "unit": "files"}});
        let shape = analyze(&value, "");
        assert!(
            !shape.has_constraints,
            "data object with 'pattern'/'minimum' keys should not trigger has_constraints"
        );
    }

    #[test]
    fn test_analyze_actual_schema_still_detected() {
        // A real schema should still be detected.
        let value = json!({
            "name": {"type": "string", "description": "User's name"},
            "role": {"type": "string", "enum": ["admin", "user"]}
        });
        let shape = analyze(&value, "");
        assert!(
            shape.has_enums,
            "actual schema with enum should be detected"
        );
    }

    #[test]
    fn test_analyze_is_schema_object_rejects_non_schema() {
        let obj: serde_json::Map<String, Value> = [
            ("type".to_string(), json!("premium")),
            ("color".to_string(), json!("blue")),
        ]
        .into_iter()
        .collect();
        assert!(!is_schema_object(&obj));
    }

    #[test]
    fn test_analyze_is_schema_object_accepts_schema() {
        let obj: serde_json::Map<String, Value> = [
            ("type".to_string(), json!("string")),
            ("enum".to_string(), json!(["a", "b"])),
        ]
        .into_iter()
        .collect();
        assert!(is_schema_object(&obj));
    }

    #[test]
    fn test_analyze_scalar() {
        let value = json!("hello");
        let shape = analyze(&value, "");
        assert_eq!(shape.top_level, TopType::Scalar);
        assert_eq!(shape.max_depth, 0);
    }

    #[test]
    fn test_analyze_empty_object() {
        let value = json!({});
        let shape = analyze(&value, "");
        assert_eq!(shape.key_count, 0);
        assert_eq!(shape.max_depth, 0);
    }

    #[test]
    fn test_analyze_empty_array() {
        let value = json!([]);
        let shape = analyze(&value, "");
        assert_eq!(shape.item_count, 0);
        assert!(!shape.is_uniform_array);
    }

    #[test]
    fn test_analyze_char_count() {
        let raw = r#"{"key": "value"}"#;
        let value: Value = serde_json::from_str(raw).unwrap();
        let shape = analyze(&value, raw);
        assert_eq!(shape.char_count, raw.len());
    }

    #[test]
    fn test_analyze_nested_array_not_uniform() {
        let value = json!([[1, 2], [3]]);
        let shape = analyze(&value, "");
        assert!(!shape.is_uniform_array);
    }
}
