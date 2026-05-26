use std::collections::HashSet;

use serde_json::{Map, Value};

/// Compresses JSON API responses by truncating strings, limiting arrays,
/// removing nulls, and dropping debug fields.
#[derive(Debug)]
pub struct ResponseCompressor {
    drop_fields: HashSet<String>,
    truncate_strings_at: usize,
    truncate_arrays_at: usize,
    drop_nulls: bool,
    drop_empty_fields: bool,
    max_depth: usize,
    add_truncation_marker: bool,
}

impl Default for ResponseCompressor {
    fn default() -> Self {
        let mut drop_fields = HashSet::new();
        for f in &[
            "debug",
            "trace",
            "traces",
            "stack",
            "stacktrace",
            "logs",
            "logging",
        ] {
            drop_fields.insert((*f).to_string());
        }
        Self {
            drop_fields,
            truncate_strings_at: 512,
            truncate_arrays_at: 16,
            drop_nulls: true,
            drop_empty_fields: true,
            max_depth: 8,
            add_truncation_marker: true,
        }
    }
}

impl ResponseCompressor {
    /// Create a new `ResponseCompressor` with default settings.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set maximum string length before truncation (default 512).
    #[must_use]
    pub fn with_truncate_strings_at(mut self, len: usize) -> Self {
        self.truncate_strings_at = len;
        self
    }

    /// Set maximum array length before truncation (default 16).
    #[must_use]
    pub fn with_truncate_arrays_at(mut self, len: usize) -> Self {
        self.truncate_arrays_at = len;
        self
    }

    /// Set whether to drop `null` values (default true).
    #[must_use]
    pub fn with_drop_nulls(mut self, drop: bool) -> Self {
        self.drop_nulls = drop;
        self
    }

    /// Set whether to drop empty objects, arrays, and strings (default true).
    #[must_use]
    pub fn with_drop_empty_fields(mut self, drop: bool) -> Self {
        self.drop_empty_fields = drop;
        self
    }

    /// Set maximum nesting depth before truncation (default 8).
    #[must_use]
    pub fn with_max_depth(mut self, depth: usize) -> Self {
        self.max_depth = depth;
        self
    }

    /// Set whether to add truncation markers (default true).
    #[must_use]
    pub fn with_add_truncation_marker(mut self, add: bool) -> Self {
        self.add_truncation_marker = add;
        self
    }

    /// Add a field name to the drop-on-sight list.
    pub fn add_drop_field(&mut self, field: &str) {
        self.drop_fields.insert(field.to_string());
    }

    /// Compress a JSON response value.
    ///
    /// Returns the original value unchanged if compression yields no savings.
    #[must_use]
    pub fn compress(&self, response: &Value) -> Value {
        let original_text = serde_json::to_string(response).unwrap_or_default();
        let result = self.compress_value(response, 0);
        let compressed_text = serde_json::to_string(&result).unwrap_or_default();
        if original_text == compressed_text {
            return response.clone();
        }
        result
    }

    fn compress_value(&self, value: &Value, depth: usize) -> Value {
        if depth > self.max_depth {
            let type_name = match value {
                Value::Null => "null",
                Value::Bool(_) => "bool",
                Value::Number(_) => "number",
                Value::String(_) => "string",
                Value::Array(_) => "array",
                Value::Object(_) => "object",
            };
            return Value::String(format!("<{type_name} truncated at depth {depth}>"));
        }
        match value {
            Value::Null => Value::Null,
            Value::Bool(b) => Value::Bool(*b),
            Value::Number(n) => Value::Number(n.clone()),
            Value::String(s) => self.compress_string(s),
            Value::Array(arr) => self.compress_array(arr, depth),
            Value::Object(obj) => self.compress_object(obj, depth),
        }
    }

    fn compress_string(&self, s: &str) -> Value {
        let char_count = s.chars().count();
        if char_count <= self.truncate_strings_at {
            return Value::String(s.to_string());
        }
        let truncate_pos = s
            .char_indices()
            .nth(self.truncate_strings_at)
            .map_or(s.len(), |(i, _)| i);
        let truncated = &s[..truncate_pos];
        if self.add_truncation_marker {
            Value::String(format!("{truncated}… (truncated)"))
        } else {
            Value::String(truncated.to_string())
        }
    }

    fn compress_array(&self, arr: &[Value], depth: usize) -> Value {
        let mut result = Vec::new();
        let truncate = arr.len() > self.truncate_arrays_at;
        let limit = if truncate {
            self.truncate_arrays_at
        } else {
            arr.len()
        };
        for item in arr.iter().take(limit) {
            let compressed = self.compress_value(item, depth + 1);
            if self.drop_nulls && compressed.is_null() {
                continue;
            }
            if self.drop_empty_fields && Self::is_empty_value(&compressed) {
                continue;
            }
            result.push(compressed);
        }
        if truncate && self.add_truncation_marker {
            result.push(Value::String(format!(
                "<... {} more items truncated>",
                arr.len() - self.truncate_arrays_at
            )));
        }
        Value::Array(result)
    }

    fn compress_object(&self, obj: &Map<String, Value>, depth: usize) -> Value {
        let mut result = Map::new();
        for (key, value) in obj {
            if self.drop_fields.contains(key) {
                continue;
            }
            let compressed = self.compress_value(value, depth + 1);
            if self.drop_nulls && compressed.is_null() {
                continue;
            }
            if self.drop_empty_fields && Self::is_empty_value(&compressed) {
                continue;
            }
            result.insert(key.clone(), compressed);
        }
        Value::Object(result)
    }

    fn is_empty_value(value: &Value) -> bool {
        match value {
            Value::String(s) => s.is_empty(),
            Value::Array(arr) => arr.is_empty(),
            Value::Object(obj) => obj.is_empty(),
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn test_string_truncation() {
        let compressor = ResponseCompressor::new().with_truncate_strings_at(20);
        let long_string = "This is a very long string that should be truncated";
        let result = compressor.compress(&json!(long_string));
        assert!(result.as_str().unwrap().contains("… (truncated)"));
    }

    #[test]
    fn test_string_truncation_512_default() {
        let compressor = ResponseCompressor::new();
        let long_string = "x".repeat(600);
        let result = compressor.compress(&json!(long_string));
        assert!(result.as_str().unwrap().contains("… (truncated)"));
    }

    #[test]
    fn test_array_compression() {
        let compressor = ResponseCompressor::new().with_truncate_arrays_at(3);
        let arr: Vec<i32> = (1..=10).collect();
        let result = compressor.compress(&json!(arr));
        let arr_result = result.as_array().unwrap();
        assert_eq!(arr_result.len(), 4);
        assert!(arr_result[3].as_str().unwrap().contains("truncated"));
    }

    #[test]
    fn test_drop_fields() {
        let compressor = ResponseCompressor::new();
        let obj = json!({"data": "important", "debug": "rm", "trace": "rm", "traces": "rm", "stack": "rm", "stacktrace": "rm", "logs": "rm", "logging": "rm"});
        let result = compressor.compress(&obj);
        let obj_result = result.as_object().unwrap();
        assert!(obj_result.contains_key("data"));
        for f in &[
            "debug",
            "trace",
            "traces",
            "stack",
            "stacktrace",
            "logs",
            "logging",
        ] {
            assert!(!obj_result.contains_key(*f));
        }
    }

    #[test]
    fn test_drop_nulls() {
        let compressor = ResponseCompressor::new();
        let obj = json!({"name": "test", "value": null, "count": 5});
        let result = compressor.compress(&obj);
        assert!(!result.as_object().unwrap().contains_key("value"));
    }

    #[test]
    fn test_drop_nulls_disabled() {
        let compressor = ResponseCompressor::new().with_drop_nulls(false);
        let obj = json!({"name": "test", "value": null});
        let result = compressor.compress(&obj);
        assert!(result.as_object().unwrap().contains_key("value"));
    }

    #[test]
    fn test_drop_empty_fields() {
        let compressor = ResponseCompressor::new();
        let obj = json!({"name": "test", "empty_string": "", "empty_array": [], "empty_object": {}, "valid": "data"});
        let result = compressor.compress(&obj);
        assert!(!result.as_object().unwrap().contains_key("empty_string"));
        assert!(!result.as_object().unwrap().contains_key("empty_array"));
        assert!(!result.as_object().unwrap().contains_key("empty_object"));
    }

    #[test]
    fn test_max_depth_truncation() {
        let compressor = ResponseCompressor::new().with_max_depth(2);
        let deep = json!({"level1": {"level2": {"level3": {"level4": "deep value"}}}});
        let result = compressor.compress(&deep);
        assert!(
            result["level1"]["level2"]["level3"]
                .as_str()
                .unwrap()
                .contains("truncated at depth")
        );
    }

    #[test]
    fn test_nested_object_recursive_compression() {
        let compressor = ResponseCompressor::new()
            .with_truncate_strings_at(10)
            .with_drop_nulls(true);
        let nested = json!({"outer": {"inner": {"long_text": "This is a very long text that should be truncated", "null_field": null, "number": 42}}});
        let result = compressor.compress(&nested);
        assert!(
            result["outer"]["inner"]["long_text"]
                .as_str()
                .unwrap()
                .contains("truncated")
        );
        assert!(result["outer"]["inner"].get("null_field").is_none());
        assert_eq!(result["outer"]["inner"]["number"], 42);
    }

    #[test]
    fn test_preserve_primitives() {
        let compressor = ResponseCompressor::new();
        assert_eq!(compressor.compress(&json!(true)), json!(true));
        assert_eq!(compressor.compress(&json!(42)), json!(42));
        assert_eq!(compressor.compress(&json!("short")), json!("short"));
    }

    #[test]
    fn test_utf8_safe_truncation() {
        let compressor = ResponseCompressor::new().with_truncate_strings_at(10);
        assert!(
            compressor
                .compress(&json!("你好世界，这是测试"))
                .as_str()
                .is_some()
        );
    }

    #[test]
    fn test_no_change_returns_original() {
        let compressor = ResponseCompressor::new();
        let v = json!({"a": 1, "b": "hello"});
        assert_eq!(compressor.compress(&v), v);
    }
}
