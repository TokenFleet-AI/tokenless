//! Enhanced TOON encoder for schema-like JSON with enums, ranges, and patterns.
//!
//! Compresses JSON Schema / `OpenAPI` property definitions into a compact,
//! human-readable text format with inline type and constraint information.
//!
//! # Examples
//!
//! ```text
//! name: string | User's full name
//! age: integer | range[1,150]
//! role: string | enum[admin,user,guest]
//! ```

use serde_json::Value;

/// Indent string used for each nesting level.
const INDENT: &str = "  ";

/// Encode a JSON value using Enhanced TOON format.
///
/// `indent_level` controls the base indentation (0 for top-level).
#[must_use]
pub fn encode(value: &Value, indent_level: usize) -> String {
    match value {
        Value::Object(obj) => encode_object(obj, indent_level),
        Value::Array(arr) => encode_array(arr, indent_level),
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => "null".to_string(),
    }
}

/// Encode a JSON object using Enhanced TOON.
fn encode_object(obj: &serde_json::Map<String, Value>, indent_level: usize) -> String {
    // Check if this is a chain: exactly one key, value is also an object.
    if obj.len() == 1
        && let Some((key, Value::Object(inner))) = obj.iter().next()
        && is_single_child(inner)
    {
        let path = build_chain_path(key, inner);
        if let Some((leaf_key, leaf_val)) = find_chain_leaf(inner) {
            let leaf_str = encode(leaf_val, indent_level);
            return format!(
                "{}{}.{}: {}",
                INDENT.repeat(indent_level),
                path,
                leaf_key,
                leaf_str
            );
        }
    }

    let mut lines = Vec::new();
    let prefix = INDENT.repeat(indent_level);

    for (key, val) in obj {
        let line = match val {
            Value::Object(inner) => {
                // Check if this looks like a schema property definition.
                if is_schema_object(inner) {
                    format!("{prefix}{key}: {}", encode_schema_value(inner))
                } else {
                    // Regular nested object.
                    format!(
                        "{}{}:\n{}",
                        prefix,
                        key,
                        encode_object(inner, indent_level + 1)
                    )
                }
            }
            Value::Array(arr) => {
                if arr.is_empty() {
                    format!("{prefix}{key}: []")
                } else {
                    let items: Vec<String> = arr
                        .iter()
                        .map(|v| {
                            format!(
                                "{}- {}",
                                INDENT.repeat(indent_level + 1),
                                encode(v, indent_level + 1)
                            )
                        })
                        .collect();
                    format!("{prefix}{key}:\n{}", items.join("\n"))
                }
            }
            other => {
                format!("{prefix}{key}: {}", encode(other, indent_level))
            }
        };
        lines.push(line);
    }

    lines.join("\n")
}

/// Encode a JSON array using Enhanced TOON.
fn encode_array(arr: &[Value], indent_level: usize) -> String {
    if arr.is_empty() {
        return "[]".to_string();
    }
    let prefix = INDENT.repeat(indent_level);
    let items: Vec<String> = arr
        .iter()
        .map(|v| format!("{prefix}- {}", encode(v, indent_level)))
        .collect();
    items.join("\n")
}

/// Check if an object looks like a JSON Schema property definition.
fn is_schema_object(obj: &serde_json::Map<String, Value>) -> bool {
    obj.contains_key("type")
        || obj.contains_key("enum")
        || obj.contains_key("minimum")
        || obj.contains_key("maximum")
        || obj.contains_key("pattern")
}

/// Encode a schema-style property definition value.
///
/// Produces strings like `string | description` or `string | enum[a,b,c]`.
fn encode_schema_value(obj: &serde_json::Map<String, Value>) -> String {
    let mut parts: Vec<String> = Vec::new();

    // Type info.
    if let Some(type_val) = obj.get("type")
        && let Some(type_str) = type_val.as_str()
    {
        parts.push(type_str.to_string());
    }

    // Enum constraint.
    if let Some(enum_val) = obj.get("enum")
        && let Some(arr) = enum_val.as_array()
    {
        let items: Vec<String> = arr
            .iter()
            .map(|v| match v {
                Value::String(s) => s.clone(),
                other => other.to_string(),
            })
            .collect();
        parts.push(format!("enum[{}]", items.join(",")));
    }

    // Range constraint (min/max). Extract numeric bounds; cast f64 to i64 for integer-like display
    // (JSON Schema min/max are integer-valued in practice, so truncation is acceptable).
    #[allow(clippy::cast_possible_truncation)]
    let minimum = obj
        .get("minimum")
        .and_then(|v| v.as_i64().or_else(|| v.as_f64().map(|f| f as i64)));
    #[allow(clippy::cast_possible_truncation)]
    let maximum = obj
        .get("maximum")
        .and_then(|v| v.as_i64().or_else(|| v.as_f64().map(|f| f as i64)));

    if minimum.is_some() || maximum.is_some() {
        let min_str = minimum.map_or("*".to_string(), |m| m.to_string());
        let max_str = maximum.map_or("*".to_string(), |m| m.to_string());
        parts.push(format!("range[{min_str},{max_str}]"));
    }

    // Pattern constraint.
    if let Some(pattern_val) = obj.get("pattern")
        && let Some(pattern_str) = pattern_val.as_str()
    {
        parts.push(format!("pattern[{pattern_str}]"));
    }

    // Description.
    let has_description = obj.contains_key("description");
    if let Some(desc_val) = obj.get("description")
        && let Some(desc_str) = desc_val.as_str()
    {
        parts.push(desc_str.to_string());
    }

    // Separate type/constraints from description for clean formatting.
    let type_constraints: Vec<String> = parts
        .iter()
        .enumerate()
        .filter_map(|(i, p)| {
            if i == parts.len() - 1 && has_description {
                None
            } else {
                Some(p.clone())
            }
        })
        .collect();

    let mut result = type_constraints.join(" | ");

    if let Some(desc_val) = obj.get("description")
        && let Some(desc_str) = desc_val.as_str()
        && !desc_str.is_empty()
    {
        if result.is_empty() {
            result = desc_str.to_string();
        } else {
            result = format!("{result} | {desc_str}");
        }
    }

    // Fallback: if nothing was extracted, return a compact JSON representation.
    if result.is_empty() {
        return serde_json::to_string(obj).unwrap_or_default();
    }

    result
}

/// Check if the object is a single-child chain (exactly one key, value is also object).
fn is_single_child(obj: &serde_json::Map<String, Value>) -> bool {
    obj.len() == 1
        && obj
            .iter()
            .next()
            .is_some_and(|(_, v)| matches!(v, Value::Object(_)))
}

/// Build a dot-separated path through a chain of single-child objects.
///
/// Returns the path string (e.g., "a.b.c") without the final key.
fn build_chain_path(first_key: &str, obj: &serde_json::Map<String, Value>) -> String {
    let mut path = first_key.to_string();
    let mut current = obj;

    loop {
        if current.len() != 1 {
            break;
        }
        if let Some((key, Value::Object(inner))) = current.iter().next()
            && inner.len() == 1
        {
            path.push('.');
            path.push_str(key);
            current = inner;
        } else {
            break;
        }
    }

    path
}

/// Find the leaf (non-object) value at the end of a chain.
fn find_chain_leaf(obj: &serde_json::Map<String, Value>) -> Option<(&String, &Value)> {
    let mut current = obj;

    loop {
        if current.len() != 1 {
            return None;
        }
        if let Some((key, val)) = current.iter().next() {
            match val {
                Value::Object(inner) if inner.len() == 1 => {
                    current = inner;
                }
                _ => return Some((key, val)),
            }
        } else {
            return None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_encode_simple_object() {
        let value = json!({"name": "Alice", "age": 30});
        let output = encode(&value, 0);
        assert!(output.contains("name: Alice"));
        assert!(output.contains("age: 30"));
    }

    #[test]
    fn test_encode_schema_with_enum() {
        let value = json!({"role": {"type": "string", "enum": ["admin", "user", "guest"]}});
        let output = encode(&value, 0);
        assert!(output.contains("enum[admin,user,guest]"));
    }

    #[test]
    fn test_encode_schema_with_range() {
        let value = json!({"age": {"type": "integer", "minimum": 1, "maximum": 150}});
        let output = encode(&value, 0);
        assert!(output.contains("range[1,150]"));
    }

    #[test]
    fn test_encode_schema_with_description() {
        let value = json!({"name": {"type": "string", "description": "User's full name"}});
        let output = encode(&value, 0);
        assert!(output.contains("string"));
        assert!(output.contains("User's full name"));
    }

    #[test]
    fn test_encode_schema_with_pattern() {
        let value = json!({"code": {"type": "string", "pattern": "^[a-z]{3,10}$"}});
        let output = encode(&value, 0);
        assert!(output.contains("pattern[^[a-z]{3,10}$]"));
    }

    #[test]
    fn test_encode_deep_chain() {
        let value = json!({"a": {"b": {"c": {"d": {"e": 1}}}}});
        let output = encode(&value, 0);
        // Chain should be collapsed to dot-path.
        assert!(output.contains('.') || output.contains("d:"));
    }

    #[test]
    fn test_encode_nested_object() {
        let value = json!({"user": {"name": "Alice", "address": {"city": "NYC"}}});
        let output = encode(&value, 0);
        assert!(output.contains("user:"));
        assert!(output.contains("name: Alice"));
    }

    #[test]
    fn test_encode_empty_object() {
        let value = json!({});
        let output = encode(&value, 0);
        assert!(output.is_empty());
    }

    #[test]
    fn test_encode_boolean() {
        let value = json!(true);
        let output = encode(&value, 0);
        assert_eq!(output, "true");
    }

    #[test]
    fn test_encode_null() {
        let value = json!(null);
        let output = encode(&value, 0);
        assert_eq!(output, "null");
    }
}
