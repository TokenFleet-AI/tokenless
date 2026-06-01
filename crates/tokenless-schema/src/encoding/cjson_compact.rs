//! CJSON compact encoder for irregular/mixed JSON structures.
//!
//! Applies lossless structural compression:
//! - `true` → `T`, `false` → `F`, `null` → `~`
//! - Unquotes safe bare words matching `^[a-zA-Z_][a-zA-Z0-9_]*$`
//! - Removes non-semantic whitespace while preserving structural integrity.
//!
//! # Examples
//!
//! ```json
//! {"name":"Alice","active":true,"score":null}
//! ```
//! becomes:
//! ```text
//! {name:Alice,active:T,score:~}
//! ```

use serde_json::Value;

/// Check whether a string is a safe bare word (does not need quoting).
///
/// A safe bare word starts with a letter or underscore, followed by
/// alphanumeric characters or underscores.
#[allow(
    clippy::indexing_slicing,
    reason = "bounds: s.is_empty() check above ensures bytes[0] exists"
)]
fn is_safe_bare_word(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    let bytes = s.as_bytes();
    // First character: letter or underscore.
    if !bytes[0].is_ascii_alphabetic() && bytes[0] != b'_' {
        return false;
    }
    // Remaining characters: alphanumeric or underscore.
    bytes[1..]
        .iter()
        .all(|b| b.is_ascii_alphanumeric() || *b == b'_')
}

/// Encode a JSON value using CJSON compact format.
#[must_use]
pub fn encode(value: &Value) -> String {
    encode_value(value)
}

fn encode_value(value: &Value) -> String {
    match value {
        Value::Null => "~".to_string(),
        Value::Bool(true) => "T".to_string(),
        Value::Bool(false) => "F".to_string(),
        Value::Number(n) => n.to_string(),
        Value::String(s) => encode_string(s),
        Value::Array(arr) => {
            let items: Vec<String> = arr.iter().map(encode_value).collect();
            format!("[{}]", items.join(","))
        }
        Value::Object(obj) => {
            let items: Vec<String> = obj
                .iter()
                .map(|(k, v)| format!("{}:{}", encode_string(k), encode_value(v)))
                .collect();
            format!("{{{}}}", items.join(","))
        }
    }
}

fn encode_string(s: &str) -> String {
    if is_safe_bare_word(s) {
        s.to_string()
    } else {
        // Use serde_json to properly escape and quote the string.
        serde_json::to_string(s).unwrap_or_else(|_| format!("\"{s}\""))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_encode_basic_object() {
        let value = json!({"name": "Alice", "active": true, "score": null});
        let output = encode(&value);
        // Should use compact notation.
        assert!(output.contains('T'));
        assert!(output.contains('~'));
        assert!(!output.contains("true"));
        assert!(!output.contains("null"));
    }

    #[test]
    fn test_safe_bare_word_unquoted() {
        let value = json!({"status": "active"});
        let output = encode(&value);
        assert!(output.contains("status:active"));
        assert!(!output.contains("\"status\""));
    }

    #[test]
    fn test_special_chars_stay_quoted() {
        let value = json!({"message": "hello world"});
        let output = encode(&value);
        // Space is not allowed in bare words, so string should stay quoted.
        assert!(output.contains("\"hello world\""));
    }

    #[test]
    fn test_safe_bare_word_rejects_empty() {
        assert!(!is_safe_bare_word(""));
    }

    #[test]
    fn test_safe_bare_word_rejects_numeric_start() {
        assert!(!is_safe_bare_word("123abc"));
    }

    #[test]
    fn test_safe_bare_word_accepts_underscore_start() {
        assert!(is_safe_bare_word("_private"));
    }

    #[test]
    fn test_safe_bare_word_rejects_hyphen() {
        assert!(!is_safe_bare_word("hello-world"));
    }

    #[test]
    fn test_encode_nested_structure() {
        let value = json!({
            "user": {
                "name": "Bob",
                "tags": ["admin", "verified"]
            },
            "count": 42
        });
        let output = encode(&value);
        assert!(output.starts_with("{count:"));
        assert!(output.contains("tags:[admin,verified]"));
        assert!(output.contains("count:42"));
        assert!(!output.contains(' ')); // No non-semantic whitespace.
    }

    #[test]
    fn test_encode_array_of_scalars() {
        let value = json!([true, false, null, 42, "hello"]);
        let output = encode(&value);
        assert_eq!(output, "[T,F,~,42,hello]");
    }

    #[test]
    fn test_encode_empty_object() {
        let value = json!({});
        let output = encode(&value);
        assert_eq!(output, "{}");
    }
}
