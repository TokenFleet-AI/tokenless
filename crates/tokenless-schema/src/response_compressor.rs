use std::collections::HashSet;

use serde_json::{Map, Value};

/// Compression profile for different tool output types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompressionProfile {
    /// Shell commands, error/diff output — minimal filtering, large limits.
    HighFidelity,
    /// API JSON responses — balanced defaults.
    Standard,
    /// Large logs — aggressive truncation and key capping.
    Aggressive,
}

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
    max_keys_per_object: usize,
    /// Fields whose values are never truncated — critical information
    /// (error, stderr, output, etc.) should survive compression intact.
    preserve_fields: HashSet<String>,
}

impl Default for ResponseCompressor {
    fn default() -> Self {
        let mut drop_fields = HashSet::new();
        for f in &[
            "debug",
            "Debug",
            "trace",
            "Trace",
            "traces",
            "Traces",
            "stack",
            "Stack",
            "stacktrace",
            "Stacktrace",
            "logs",
            "Logs",
            "logging",
            "Logging",
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
            max_keys_per_object: usize::MAX,
            preserve_fields: {
                let mut pf = HashSet::new();
                for f in &["error", "stderr", "output", "message", "data", "result"] {
                    pf.insert((*f).to_string());
                }
                pf
            },
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

    /// Set the maximum number of keys per object before truncation (default unlimited).
    #[must_use]
    pub fn with_max_keys_per_object(mut self, max: usize) -> Self {
        self.max_keys_per_object = max;
        self
    }

    /// Add a field name to the drop-on-sight list.
    #[must_use]
    pub fn with_drop_field(mut self, field: impl Into<String>) -> Self {
        self.drop_fields.insert(field.into());
        self
    }

    /// Add a field name to the preserve list — values under these keys
    /// are never truncated regardless of the compression profile.
    #[must_use]
    pub fn with_preserve_field(mut self, field: impl Into<String>) -> Self {
        self.preserve_fields.insert(field.into());
        self
    }

    /// Set the entire preserve-fields set, replacing the defaults.
    #[must_use]
    pub fn with_preserve_fields(mut self, fields: HashSet<String>) -> Self {
        self.preserve_fields = fields;
        self
    }

    /// Apply a compression profile preset, overriding current settings.
    #[must_use]
    pub fn with_profile(mut self, profile: CompressionProfile) -> Self {
        match profile {
            CompressionProfile::HighFidelity => {
                self.truncate_strings_at = 4096;
                self.truncate_arrays_at = 128;
                self.drop_nulls = false;
                self.drop_empty_fields = false;
            }
            CompressionProfile::Standard => {
                self.truncate_strings_at = 512;
                self.truncate_arrays_at = 16;
                self.drop_nulls = true;
                self.drop_empty_fields = true;
            }
            CompressionProfile::Aggressive => {
                self.truncate_strings_at = 256;
                self.truncate_arrays_at = 8;
                self.drop_nulls = true;
                self.drop_empty_fields = true;
                self.max_keys_per_object = 20;
            }
        }
        self
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

    /// Truncate a string at the configured code-point limit.
    ///
    /// Truncation operates at the Unicode code-point level, not at the
    /// grapheme-cluster level. This is acceptable for LLM tokenizer
    /// consumers because tokenizers also operate on code-point sequences
    /// rather than rendered grapheme clusters.
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
            if result.is_empty() {
                result.push(Value::String(format!(
                    "<... array truncated: prefix empty, up to {} more items>",
                    arr.len() - limit
                )));
            } else {
                result.push(Value::String(format!(
                    "<... up to {} more items truncated>",
                    arr.len() - limit
                )));
            }
        }
        Value::Array(result)
    }

    fn compress_object(&self, obj: &Map<String, Value>, depth: usize) -> Value {
        let mut result = Map::new();
        let mut count: usize = 0;
        let mut truncated: usize = 0;
        for (key, value) in obj {
            if self.drop_fields.contains(key) {
                continue;
            }
            // Preserved fields bypass all truncation — critical information
            // like error messages, stderr output, and data payloads survive intact.
            if self.preserve_fields.contains(key) {
                if self.drop_nulls && value.is_null() {
                    continue;
                }
                if self.drop_empty_fields && Self::is_empty_value(value) {
                    continue;
                }
                result.insert(key.clone(), value.clone());
                count += 1;
                continue;
            }
            if count >= self.max_keys_per_object {
                truncated += 1;
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
            count += 1;
        }
        if truncated > 0 {
            result.insert(
                "<...keys_truncated>".to_string(),
                Value::String(format!("<... {truncated} more keys truncated>")),
            );
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

    // D4: Array truncation count wording
    #[test]
    fn test_array_truncation_marker_says_up_to() {
        let compressor = ResponseCompressor::new().with_truncate_arrays_at(5);
        let arr: Vec<i32> = (1..=10).collect();
        let result = compressor.compress(&json!(arr));
        let last = result.as_array().unwrap().last().unwrap();
        assert!(last.as_str().unwrap().contains("up to"));
    }

    // D5: PascalCase drop fields
    #[test]
    fn test_drop_fields_pascal_case() {
        let compressor = ResponseCompressor::new();
        let obj = json!({"Debug": "rm", "Trace": "rm", "Stack": "rm", "keep": "yes"});
        let result = compressor.compress(&obj);
        let obj_result = result.as_object().unwrap();
        assert!(
            !obj_result.contains_key("Debug"),
            "PascalCase 'Debug' should be dropped"
        );
        assert!(
            !obj_result.contains_key("Trace"),
            "PascalCase 'Trace' should be dropped"
        );
        assert!(
            !obj_result.contains_key("Stack"),
            "PascalCase 'Stack' should be dropped"
        );
        assert!(
            obj_result.contains_key("keep"),
            "'keep' should be preserved"
        );
    }

    // D6: Combining character truncation
    #[test]
    fn test_combining_character_truncation_known_limitation() {
        let compressor = ResponseCompressor::new().with_truncate_strings_at(4);
        let result = compressor.compress(&json!("cafe\u{0301} extra"));
        assert!(result.as_str().unwrap().starts_with("caf"));
    }

    // D7: Builder chaining with with_drop_field
    #[test]
    fn test_with_drop_field_builder_chaining() {
        let compressor = ResponseCompressor::new()
            .with_drop_field("custom_debug")
            .with_truncate_strings_at(100);
        let obj = json!({"custom_debug": "rm", "keep": "yes"});
        let result = compressor.compress(&obj);
        assert!(!result.as_object().unwrap().contains_key("custom_debug"));
        assert!(result.as_object().unwrap().contains_key("keep"));
    }

    // D8: max_keys_per_object
    #[test]
    fn test_max_keys_per_object() {
        let compressor = ResponseCompressor::new().with_max_keys_per_object(3);
        let mut obj_map = serde_json::Map::new();
        for i in 0..10 {
            obj_map.insert(format!("key_{i}"), json!(i));
        }
        let result = compressor.compress(&Value::Object(obj_map));
        let obj_result = result.as_object().unwrap();
        assert!(obj_result.len() <= 4, "at most 3 data keys + 1 marker");
        assert!(obj_result.contains_key("<...keys_truncated>"));
    }

    #[test]
    fn test_max_keys_default_unlimited() {
        let compressor = ResponseCompressor::new();
        let mut obj_map = serde_json::Map::new();
        for i in 0..1000 {
            obj_map.insert(format!("key_{i}"), json!(i));
        }
        let result = compressor.compress(&Value::Object(obj_map));
        assert_eq!(result.as_object().unwrap().len(), 1000);
    }

    #[test]
    fn test_max_keys_respects_drop_fields() {
        let compressor = ResponseCompressor::new().with_max_keys_per_object(2);
        let obj = json!({"debug": "rm", "keep1": 1, "keep2": 2, "keep3": 3});
        let result = compressor.compress(&obj);
        let obj_result = result.as_object().unwrap();
        assert!(obj_result.contains_key("keep1"));
        assert!(obj_result.contains_key("keep2"));
        assert!(!obj_result.contains_key("keep3"));
        assert!(!obj_result.contains_key("debug"));
    }

    // D9: Empty array prefix marker
    #[test]
    fn test_array_truncation_all_null_prefix() {
        let compressor = ResponseCompressor::new().with_truncate_arrays_at(3);
        let arr = json!([null, null, null, 1, 2]);
        let result = compressor.compress(&arr);
        let arr_result = result.as_array().unwrap();
        assert_eq!(arr_result.len(), 1, "all null prefix → only marker remains");
        let marker = arr_result[0].as_str().unwrap();
        assert!(
            marker.contains("prefix empty"),
            "marker should indicate prefix was empty: {marker}"
        );
    }

    // ── Gap 6: CJK boundary, RTL, ZWJ tests ──────────────────────────

    #[test]
    fn test_cjk_truncation_no_panic() {
        // CJK text: 100 repeated CJK chars, truncate at 20 char limit
        let compressor = ResponseCompressor::new().with_truncate_strings_at(20);
        let cjk_text = "你好世界这是测试文本".repeat(10); // 10 * 8 = 80 CJK chars
        let result = compressor.compress(&json!(cjk_text));
        let output = result.as_str().unwrap();
        // Should be truncated
        assert!(
            output.contains("truncated"),
            "CJK text should be truncated at char limit, got len={}",
            output.chars().count()
        );
    }

    #[test]
    fn test_mixed_cjk_ascii_emoji_truncation() {
        // Mixed CJK + ASCII + Emoji — truncation should not split mid-char
        let compressor = ResponseCompressor::new().with_truncate_strings_at(30);
        let mixed = "Hello 你好 World 🌍 世界 🚀 cont".repeat(5);
        let result = compressor.compress(&json!(mixed));
        let output = result.as_str().unwrap();
        // Result should be valid UTF-8, no panics
        assert!(output.contains("truncated"));
        // Verify it starts with the correct prefix
        assert!(output.starts_with("Hello 你好 World "));
    }

    #[test]
    fn test_rtl_danish_text_preserved_in_truncation() {
        // Arabic RTL text — truncation should be at char boundary
        let compressor = ResponseCompressor::new().with_truncate_strings_at(100);
        let arabic = "مرحبا بالعالم هذا نص عربي للاختبار".repeat(5);
        let result = compressor.compress(&json!(arabic));
        let output = result.as_str().unwrap();
        // Arabic text is long (~200 chars), should be truncated at ~100
        let chars = output.chars().count();
        // "… (truncated)" adds 15 chars to the limit
        assert!(
            chars <= 115,
            "RTL text truncation should respect char boundary, got {chars} chars"
        );
        assert!(output.contains("truncated"));
    }

    #[test]
    fn test_rtl_hebrew_text_no_corruption() {
        let compressor = ResponseCompressor::new().with_truncate_strings_at(50);
        let hebrew = "שלום עולם".repeat(20); // Hebrew RTL
        let result = compressor.compress(&json!(hebrew));
        let output = result.as_str().unwrap();
        assert!(output.contains("truncated"));
        // Verify output is valid UTF-8 by checking we can iterate chars
        let char_count = output.chars().count();
        assert!(char_count > 0);
    }

    #[test]
    fn test_zero_width_joiner_preserved() {
        // Zero-width joiner (U+200D) should not be split
        let compressor = ResponseCompressor::new().with_truncate_strings_at(200);
        let zwj_text = "family\u{200d}man\u{200d}woman\u{200d}girl\u{200d}boy ".repeat(10);
        let result = compressor.compress(&json!(zwj_text));
        let output = result.as_str().unwrap();
        assert!(
            output.contains("\u{200d}"),
            "ZWJ should be preserved in output"
        );
    }

    #[test]
    fn test_combining_char_at_boundary() {
        // Combining character right at the truncation boundary should be handled
        let compressor = ResponseCompressor::new().with_truncate_strings_at(10);
        // "cafe" (4) + combining acute accent (1 char) + " extra..."
        // Total chars: "cafe\u{0301} extra" = 12 chars
        let result = compressor.compress(&json!("cafe\u{0301} extra text"));
        let output = result.as_str().unwrap();
        assert!(output.contains("truncated"));
        // Key: should be valid UTF-8, no panics
        let _: Vec<char> = output.chars().collect(); // must not panic
    }

    #[test]
    fn test_cjk_with_array_truncation() {
        // Array of CJK strings truncated
        let compressor = ResponseCompressor::new()
            .with_truncate_strings_at(15)
            .with_truncate_arrays_at(3);
        let arr = json!([
            "这是第一段中文文本用于测试",
            "这是第二段较长中文文本用于测试截断功能",
            "第三段文本",
            "第四段文本应该被截断",
            "第五段文本"
        ]);
        let result = compressor.compress(&arr);
        let arr_result = result.as_array().unwrap();
        assert_eq!(arr_result.len(), 4, "3 items + 1 marker = 4");
        // Verify CJK strings are within char limit
        for item in arr_result.iter().take(3) {
            let s = item.as_str().unwrap();
            assert!(
                s.chars().count() <= 30, // 15 + marker overhead
                "CJK string should be truncated: {s}"
            );
        }
    }

    // ── Preserve fields ───────────────────────────────────────────────

    #[test]
    fn test_preserve_fields_not_truncated() {
        let compressor = ResponseCompressor::new()
            .with_truncate_strings_at(10)
            .with_truncate_arrays_at(2);
        let obj = json!({
            "error": "This is a very long error message that would normally be truncated",
            "data": [1, 2, 3, 4, 5, 6],
            "unimportant": "short"
        });
        let result = compressor.compress(&obj);
        let r = result.as_object().unwrap();
        // error should be preserved in full
        assert_eq!(
            r["error"].as_str().unwrap(),
            "This is a very long error message that would normally be truncated"
        );
        // data array should be preserved in full
        assert_eq!(r["data"].as_array().unwrap().len(), 6);
    }

    #[test]
    fn test_preserve_fields_drops_null_when_configured() {
        let compressor = ResponseCompressor::new().with_drop_nulls(true);
        let obj = json!({"error": null, "keep": "yes"});
        let result = compressor.compress(&obj);
        assert!(!result.as_object().unwrap().contains_key("error"));
    }

    #[test]
    fn test_preserve_fields_keeps_null_when_disabled() {
        let compressor = ResponseCompressor::new().with_drop_nulls(false);
        let obj = json!({"error": null});
        let result = compressor.compress(&obj);
        assert!(result.as_object().unwrap().contains_key("error"));
    }

    #[test]
    fn test_preserve_fields_stderr_output() {
        let compressor = ResponseCompressor::new()
            .with_truncate_strings_at(20)
            .with_truncate_arrays_at(3);
        let obj = json!({
            "stderr": "error: cannot find module '.internal/models/user'\n  --> src/main.rs:42",
            "output": ["file1.txt", "file2.txt", "file3.txt", "file4.txt", "file5.txt"],
            "status": 1
        });
        let result = compressor.compress(&obj);
        let r = result.as_object().unwrap();
        // stderr is preserved full
        assert!(r["stderr"].as_str().unwrap().contains(".internal/models/user"));
        // output array preserved full
        assert_eq!(r["output"].as_array().unwrap().len(), 5);
    }

    // ── Compression profiles ──────────────────────────────────────────

    #[test]
    fn test_high_fidelity_profile() {
        let compressor = ResponseCompressor::new().with_profile(CompressionProfile::HighFidelity);
        assert_eq!(compressor.truncate_strings_at, 4096);
        assert_eq!(compressor.truncate_arrays_at, 128);
        assert!(!compressor.drop_nulls);
        assert!(!compressor.drop_empty_fields);
    }

    #[test]
    fn test_aggressive_profile() {
        let compressor = ResponseCompressor::new().with_profile(CompressionProfile::Aggressive);
        assert_eq!(compressor.truncate_strings_at, 256);
        assert_eq!(compressor.truncate_arrays_at, 8);
        assert_eq!(compressor.max_keys_per_object, 20);
        assert!(compressor.drop_nulls);
        assert!(compressor.drop_empty_fields);
    }

    #[test]
    fn test_standard_profile_is_default() {
        let standard = ResponseCompressor::new().with_profile(CompressionProfile::Standard);
        let default = ResponseCompressor::new();
        assert_eq!(standard.truncate_strings_at, default.truncate_strings_at);
        assert_eq!(standard.truncate_arrays_at, default.truncate_arrays_at);
    }

    #[test]
    fn test_preserve_field_builder() {
        let compressor = ResponseCompressor::new().with_preserve_field("custom_key");
        assert!(compressor.preserve_fields.contains("custom_key"));
        // Default preserved fields still present
        assert!(compressor.preserve_fields.contains("error"));
    }
}
