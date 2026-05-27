//! TOON HRV (Header-Row-Value) encoder for uniform object arrays.
//!
//! Designed for API paginated lists where every item has the same shape.
//! Outputs a header line with field names followed by comma-separated value rows.
//!
//! # Examples
//!
//! Input:
//! ```json
//! [{"id":1,"name":"Alice","role":"admin"},{"id":2,"name":"Bob","role":"user"}]
//! ```
//!
//! Output:
//! ```text
//! items[2]{id,name,role}:
//!   1,Alice,admin
//!   2,Bob,user
//! ```

use serde_json::Value;

/// Encode a JSON value using TOON HRV format.
///
/// Only produces HRV output for uniform arrays with sufficient items.
/// Falls through to [`super::encode_enhanced`] for non-uniform or small arrays.
#[must_use]
pub fn encode(value: &Value) -> String {
    match value {
        Value::Array(arr) => {
            if arr.is_empty() {
                return "[]".to_string();
            }
            // Check uniformity: all items are objects with identical keys.
            let Some(first_keys) = extract_keys(&arr[0]) else {
                return fallback_encode(value);
            };

            let is_uniform = arr[1..arr.len().min(100)]
                .iter()
                .all(|item| extract_keys(item).is_some_and(|ks| ks == first_keys));

            if !is_uniform || arr.len() < 5 {
                return fallback_encode(value);
            }

            let count = arr.len();
            let fields: Vec<String> = first_keys.iter().map(|s| (*s).clone()).collect();
            let header = format!("items[{count}]{{{}}}:\n", fields.join(","));

            let rows: Vec<String> = arr
                .iter()
                .map(|item| format_row(item, &first_keys))
                .collect();

            format!("{header}{}", rows.join("\n"))
        }
        _ => fallback_encode(value),
    }
}

/// Extract sorted keys from a JSON object, if the value is one.
fn extract_keys(value: &Value) -> Option<Vec<&String>> {
    value.as_object().map(|o| {
        let mut ks: Vec<_> = o.keys().collect();
        ks.sort_unstable();
        ks
    })
}

/// Format a single row of values in field order.
fn format_row(item: &Value, fields: &[&String]) -> String {
    let Some(obj) = item.as_object() else {
        return "-".to_string();
    };

    let cells: Vec<String> = fields
        .iter()
        .map(|field| {
            obj.get(*field)
                .map_or_else(|| "-".to_string(), encode_cell_value)
        })
        .collect();

    format!("  {}", cells.join(","))
}

/// Encode a single cell value for HRV row output.
fn encode_cell_value(value: &Value) -> String {
    match value {
        Value::Null => "-".to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        Value::String(s) => {
            // Escape strings containing commas, spaces, backslashes, or newlines.
            if s.contains(',') || s.contains(' ') || s.contains('\\') || s.contains('\n') {
                format!(
                    "\\{}",
                    s.replace('\\', "\\\\")
                        .replace(',', "\\,")
                        .replace('\n', "\\n")
                )
            } else if s.is_empty() {
                "-".to_string()
            } else {
                s.clone()
            }
        }
        Value::Array(arr) => {
            let items: Vec<String> = arr.iter().map(encode_cell_value).collect();
            format!("[{}]", items.join(","))
        }
        Value::Object(obj) => {
            let pairs: Vec<String> = obj
                .iter()
                .map(|(k, v)| format!("{k}:{}", encode_cell_value(v)))
                .collect();
            format!("{{{}}}", pairs.join(","))
        }
    }
}

/// Fall back to Enhanced TOON encoding when HRV is not applicable.
fn fallback_encode(value: &Value) -> String {
    super::encode_enhanced(value, 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_encode_uniform_array_basic() {
        let value = json!([
            {"id": 1, "name": "Alice", "role": "admin"},
            {"id": 2, "name": "Bob", "role": "user"},
            {"id": 3, "name": "Carol", "role": "user"},
            {"id": 4, "name": "Dave", "role": "admin"},
            {"id": 5, "name": "Eve", "role": "user"}
        ]);
        let output = encode(&value);
        assert!(output.starts_with("items[5]{id,name,role}:"));
        assert!(output.contains("1,Alice,admin"));
        assert!(output.contains("2,Bob,user"));
    }

    #[test]
    fn test_encode_small_array_falls_through() {
        let value = json!([
            {"id": 1, "name": "Alice"},
            {"id": 2, "name": "Bob"},
            {"id": 3, "name": "Carol"}
        ]);
        let output = encode(&value);
        // Should fall through to enhanced TOON since < 5 items.
        assert!(!output.starts_with("items[3]"));
    }

    #[test]
    fn test_encode_null_value() {
        let value = json!([
            {"a": 1, "b": null, "c": "x"},
            {"a": 2, "b": null, "c": "y"},
            {"a": 3, "b": null, "c": "z"},
            {"a": 4, "b": null, "c": "w"},
            {"a": 5, "b": null, "c": "v"}
        ]);
        let output = encode(&value);
        assert!(output.contains("1,-,x"));
        assert!(output.contains("2,-,y"));
    }

    #[test]
    fn test_encode_escaped_string_with_comma() {
        let value = json!([
            {"id": 1, "desc": "hello, world"},
            {"id": 2, "desc": "plain"},
            {"id": 3, "desc": "simple"},
            {"id": 4, "desc": "test"},
            {"id": 5, "desc": "done"}
        ]);
        let output = encode(&value);
        // String with comma should be backslash-escaped.
        assert!(output.contains("\\hello\\, world"));
    }

    #[test]
    fn test_encode_escaped_string_with_space() {
        let value = json!([
            {"id": 1, "name": "Alice Smith"},
            {"id": 2, "name": "Bob"},
            {"id": 3, "name": "Carol"},
            {"id": 4, "name": "Dave"},
            {"id": 5, "name": "Eve"}
        ]);
        let output = encode(&value);
        assert!(output.contains("\\Alice Smith"));
    }

    #[test]
    fn test_encode_empty_array() {
        let value = json!([]);
        let output = encode(&value);
        assert_eq!(output, "[]");
    }

    #[test]
    fn test_encode_mixed_array_falls_through() {
        let value = json!([
            {"a": 1, "b": 2},
            {"a": 3, "b": 4},
            {"a": 5, "b": 6},
            {"a": 7, "b": 8},
            {"c": 9} // different key set
        ]);
        let output = encode(&value);
        // Should fall through since not all items have same keys.
        assert!(!output.starts_with("items["));
    }
}
