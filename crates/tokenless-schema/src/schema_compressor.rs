use regex::Regex;
use serde_json::Value;
use std::sync::LazyLock;

#[allow(clippy::expect_used)]
static CODE_BLOCK_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"```[\s\S]*?```").expect("valid regex"));
#[allow(clippy::expect_used)]
static INLINE_CODE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"`[^`]+`").expect("valid regex"));
#[allow(clippy::expect_used)]
static WHITESPACE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s+").expect("valid regex"));

fn find_char_boundary(s: &str, pos: usize) -> usize {
    let pos = pos.min(s.len());
    if s.is_char_boundary(pos) {
        pos
    } else {
        let mut i = pos;
        while i > 0 && !s.is_char_boundary(i) {
            i -= 1;
        }
        i
    }
}

/// Compresses `OpenAI Function Calling` schemas by truncating descriptions,
/// removing titles/examples, and reducing token usage.
#[derive(Debug)]
pub struct SchemaCompressor {
    func_desc_max_len: usize,
    param_desc_max_len: usize,
    drop_examples: bool,
    drop_titles: bool,
    drop_markdown: bool,
}

impl Default for SchemaCompressor {
    fn default() -> Self {
        Self {
            func_desc_max_len: 256,
            param_desc_max_len: 160,
            drop_examples: true,
            drop_titles: true,
            drop_markdown: true,
        }
    }
}

impl SchemaCompressor {
    /// Create a new `SchemaCompressor` with default settings.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set maximum length for function-level descriptions (default 256).
    #[must_use]
    pub fn with_func_desc_max_len(mut self, len: usize) -> Self {
        self.func_desc_max_len = len;
        self
    }

    /// Set maximum length for parameter-level descriptions (default 160).
    #[must_use]
    pub fn with_param_desc_max_len(mut self, len: usize) -> Self {
        self.param_desc_max_len = len;
        self
    }

    /// Set whether to drop `examples` from schema (default true).
    #[must_use]
    pub fn with_drop_examples(mut self, drop: bool) -> Self {
        self.drop_examples = drop;
        self
    }

    /// Set whether to drop `title` from schema (default true).
    #[must_use]
    pub fn with_drop_titles(mut self, drop: bool) -> Self {
        self.drop_titles = drop;
        self
    }

    /// Set whether to strip markdown formatting from descriptions (default true).
    #[must_use]
    pub fn with_drop_markdown(mut self, drop: bool) -> Self {
        self.drop_markdown = drop;
        self
    }

    /// Compress an `OpenAI Function Calling` schema.
    ///
    /// Returns the original value unchanged if compression yields no savings.
    #[must_use]
    pub fn compress(&self, tool: &Value) -> Value {
        let original_text = serde_json::to_string(tool).unwrap_or_default();
        let mut result = tool.clone();

        if let Some(function) = result.get_mut("function") {
            if let Some(desc) = function.get("description").and_then(|d| d.as_str()) {
                function["description"] =
                    Value::String(self.truncate_description(desc, self.func_desc_max_len));
            }
            #[allow(clippy::collapsible_if)]
            if self.drop_titles {
                if let Some(obj) = function.as_object_mut() {
                    obj.remove("title");
                }
            }
            if let Some(params) = function.get_mut("parameters") {
                self.compress_json_schema(params, 1);
            }
        } else {
            if let Some(desc) = result.get("description").and_then(|d| d.as_str()) {
                result["description"] =
                    Value::String(self.truncate_description(desc, self.func_desc_max_len));
            }
            #[allow(clippy::collapsible_if)]
            if self.drop_titles {
                if let Some(obj) = result.as_object_mut() {
                    obj.remove("title");
                }
            }
            if let Some(params) = result.get_mut("parameters") {
                self.compress_json_schema(params, 1);
            }
            if result.get("type").is_some() || result.get("properties").is_some() {
                self.compress_json_schema(&mut result, 0);
            }
        }

        let compressed_text = serde_json::to_string(&result).unwrap_or_default();
        if original_text == compressed_text {
            return tool.clone();
        }
        result
    }

    /// Recursively compress a JSON schema value in place.
    pub fn compress_json_schema(&self, schema: &mut Value, depth: usize) {
        let Some(obj) = schema.as_object_mut() else {
            return;
        };

        if self.drop_titles {
            obj.remove("title");
        }
        if self.drop_examples {
            obj.remove("examples");
        }
        if let Some(desc) = obj
            .get("description")
            .and_then(|d| d.as_str())
            .map(String::from)
        {
            let max_len = if depth == 0 {
                self.func_desc_max_len
            } else {
                self.param_desc_max_len
            };
            obj.insert(
                "description".into(),
                Value::String(self.truncate_description(&desc, max_len)),
            );
        }
        #[allow(clippy::collapsible_if)]
        if let Some(properties) = obj.get_mut("properties") {
            if let Some(props_obj) = properties.as_object_mut() {
                for (_key, prop_schema) in props_obj.iter_mut() {
                    self.compress_json_schema(prop_schema, depth + 1);
                }
            }
        }
        if let Some(items) = obj.get_mut("items") {
            self.compress_json_schema(items, depth + 1);
        }
        #[allow(clippy::collapsible_if)]
        if let Some(any_of) = obj.get_mut("anyOf") {
            if let Some(arr) = any_of.as_array_mut() {
                for item in arr.iter_mut() {
                    self.compress_json_schema(item, depth + 1);
                }
            }
        }
        #[allow(clippy::collapsible_if)]
        if let Some(one_of) = obj.get_mut("oneOf") {
            if let Some(arr) = one_of.as_array_mut() {
                for item in arr.iter_mut() {
                    self.compress_json_schema(item, depth + 1);
                }
            }
        }
        #[allow(clippy::collapsible_if)]
        if let Some(all_of) = obj.get_mut("allOf") {
            if let Some(arr) = all_of.as_array_mut() {
                for item in arr.iter_mut() {
                    self.compress_json_schema(item, depth + 1);
                }
            }
        }
    }

    /// Truncate a description at a sentence boundary within `max_len`.
    ///
    /// Strips markdown code blocks and inline code when `drop_markdown` is enabled.
    /// Attempts to break at sentence endings (`.`, `。`, `！`, `？`) in the
    /// approximate range `[max_len * 0.5, max_len]` before resorting to hard
    /// character truncation.
    #[must_use]
    pub fn truncate_description(&self, desc: &str, max_len: usize) -> String {
        let mut text = desc.trim().to_string();
        if self.drop_markdown {
            text = CODE_BLOCK_RE.replace_all(&text, "").to_string();
            text = INLINE_CODE_RE.replace_all(&text, "").to_string();
        }
        text = WHITESPACE_RE.replace_all(&text, " ").to_string();
        text = text.trim().to_string();

        if text.chars().count() <= max_len {
            return text;
        }

        // Use f64 for fractional mid-point calculation; loss on 64-bit is acceptable here.
        #[allow(
            clippy::cast_precision_loss,
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss
        )]
        let min_target = (max_len as f64 * 0.5) as usize;
        let min_pos = find_char_boundary(&text, min_target);
        let max_pos = find_char_boundary(&text, max_len.min(text.len()));
        let search_range = &text[min_pos..max_pos];

        let sentence_endings = ['.', '。', '！', '？'];
        #[allow(clippy::double_ended_iterator_last)]
        let best_pos = search_range
            .char_indices()
            .filter(|(_, c)| sentence_endings.contains(c))
            .last()
            .map(|(i, c)| min_pos + i + c.len_utf8());

        if let Some(pos) = best_pos {
            return text[..pos].trim().to_string();
        }

        let mut truncate_pos = max_len;
        while !text.is_char_boundary(truncate_pos) && truncate_pos > 0 {
            truncate_pos -= 1;
        }
        text[..truncate_pos].trim().to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_compress_long_description() {
        let compressor = SchemaCompressor::new();
        let schema = json!({
            "function": {
                "name": "test_func",
                "description": "This is a very long description that should be truncated. It contains a lot of text that goes on and on. The quick brown fox jumps over the lazy dog. Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "param1": {
                            "type": "string",
                            "description": "Another long description for a parameter that should be truncated to a shorter length. This text is intentionally verbose to test the truncation logic properly."
                        }
                    }
                }
            }
        });
        let result = compressor.compress(&schema);
        assert!(result["function"]["description"].as_str().unwrap().len() <= 256);
        assert!(
            result["function"]["parameters"]["properties"]["param1"]["description"]
                .as_str()
                .unwrap()
                .len()
                <= 160
        );
    }

    #[test]
    fn test_protected_fields_preserved() {
        let compressor = SchemaCompressor::new();
        let schema = json!({
            "function": {
                "name": "my_function",
                "parameters": {
                    "type": "object",
                    "required": ["field1"],
                    "properties": {
                        "field1": {
                            "type": "string",
                            "enum": ["a", "b", "c"],
                            "default": "a",
                            "const": "fixed_value"
                        }
                    }
                }
            }
        });
        let result = compressor.compress(&schema);
        assert_eq!(result["function"]["name"], "my_function");
        assert_eq!(result["function"]["parameters"]["type"], "object");
        assert!(result["function"]["parameters"]["required"].is_array());
        assert_eq!(
            result["function"]["parameters"]["properties"]["field1"]["default"],
            "a"
        );
        assert_eq!(
            result["function"]["parameters"]["properties"]["field1"]["const"],
            "fixed_value"
        );
    }

    #[test]
    fn test_title_and_examples_removed() {
        let compressor = SchemaCompressor::new();
        let schema = json!({
            "function": {
                "name": "test",
                "title": "Test Function Title",
                "parameters": {
                    "type": "object",
                    "title": "Parameters Title",
                    "properties": {
                        "field1": {
                            "type": "string",
                            "title": "Field Title",
                            "examples": ["example1", "example2"]
                        }
                    }
                }
            }
        });
        let result = compressor.compress(&schema);
        assert!(result["function"].get("title").is_none());
        assert!(result["function"]["parameters"].get("title").is_none());
        assert!(
            result["function"]["parameters"]["properties"]["field1"]
                .get("title")
                .is_none()
        );
        assert!(
            result["function"]["parameters"]["properties"]["field1"]
                .get("examples")
                .is_none()
        );
    }

    #[test]
    fn test_empty_schema_no_panic() {
        let compressor = SchemaCompressor::new();
        assert!(compressor.compress(&json!({})).is_object());
        assert!(compressor.compress(&Value::Null).is_null());
        assert!(compressor.compress(&json!({"function": {}}))["function"].is_object());
    }

    #[test]
    fn test_nested_properties_recursive_compression() {
        let compressor = SchemaCompressor::new();
        let schema = json!({
            "function": {
                "name": "nested_test",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "level1": {
                            "type": "object",
                            "title": "Level 1 Title",
                            "description": "Level 1 description that is quite long and should be truncated according to the parameter max length setting.",
                            "properties": {
                                "level2": {
                                    "type": "object",
                                    "title": "Level 2 Title",
                                    "examples": ["ex1"],
                                    "properties": {
                                        "level3": { "type": "string", "title": "Level 3 Title" }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        });
        let result = compressor.compress(&schema);
        assert!(
            result["function"]["parameters"]["properties"]["level1"]
                .get("title")
                .is_none()
        );
        assert!(
            result["function"]["parameters"]["properties"]["level1"]["properties"]["level2"]
                .get("title")
                .is_none()
        );
        assert!(
            result["function"]["parameters"]["properties"]["level1"]["properties"]["level2"]
                ["properties"]["level3"]
                .get("title")
                .is_none()
        );
        assert!(
            result["function"]["parameters"]["properties"]["level1"]["properties"]["level2"]
                .get("examples")
                .is_none()
        );
    }

    #[test]
    fn test_truncate_at_sentence_boundary() {
        let compressor = SchemaCompressor::new();
        let text = "Short intro text for testing. This sentence ends here. More text follows after that point.";
        let result = compressor.truncate_description(text, 60);
        assert!(result.ends_with('.'));
        assert!(result.len() <= 60);
    }

    #[test]
    fn test_markdown_removal() {
        let compressor = SchemaCompressor::new();
        let text = "Some text with ```code block``` and `inline code` markers.";
        let result = compressor.truncate_description(text, 256);
        assert!(!result.contains("```"));
        assert!(!result.contains('`'));
    }

    #[test]
    fn test_anyof_oneof_allof_compression() {
        let compressor = SchemaCompressor::new();
        let schema = json!({
            "function": {
                "name": "combo_test",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "field1": { "anyOf": [{"type": "string", "title": "String", "examples": ["ex"]}, {"type": "number", "title": "Number"}] },
                        "field2": { "oneOf": [{"type": "boolean", "title": "Bool"}] },
                        "field3": { "allOf": [{"type": "object", "title": "Obj"}] }
                    }
                }
            }
        });
        let result = compressor.compress(&schema);
        assert!(
            result["function"]["parameters"]["properties"]["field1"]["anyOf"][0]
                .get("title")
                .is_none()
        );
        assert!(
            result["function"]["parameters"]["properties"]["field1"]["anyOf"][0]
                .get("examples")
                .is_none()
        );
        assert!(
            result["function"]["parameters"]["properties"]["field2"]["oneOf"][0]
                .get("title")
                .is_none()
        );
        assert!(
            result["function"]["parameters"]["properties"]["field3"]["allOf"][0]
                .get("title")
                .is_none()
        );
    }

    #[test]
    fn truncate_description_cjk_no_panic() {
        let compressor = SchemaCompressor::new();
        let cjk = "中".repeat(100);
        let result = compressor.truncate_description(&cjk, 256);
        assert!(result.chars().all(|c| c == '中'));
        assert!(result.chars().count() <= 256);

        let cjk_long = "中".repeat(300);
        assert!(
            compressor
                .truncate_description(&cjk_long, 256)
                .chars()
                .count()
                <= 256
        );
    }

    #[test]
    fn test_no_change_returns_original() {
        let compressor = SchemaCompressor::new();
        let schema = json!({"function": {"name": "short", "description": "short", "parameters": {"type": "object"}}});
        assert_eq!(compressor.compress(&schema), schema);
    }
}
