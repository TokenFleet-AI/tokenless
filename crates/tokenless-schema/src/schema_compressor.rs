use std::sync::LazyLock;

use regex::Regex;
use serde_json::Value;

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

#[must_use]
fn is_cjk(ch: char) -> bool {
    matches!(
        ch,
        '\u{4E00}'..='\u{9FFF}'
            | '\u{3400}'..='\u{4DBF}'
            | '\u{F900}'..='\u{FAFF}'
            | '\u{3040}'..='\u{309F}'
            | '\u{30A0}'..='\u{30FF}'
            | '\u{AC00}'..='\u{D7AF}'
    )
}

#[must_use]
fn estimate_tokens(text: &str) -> usize {
    let mut tokens = 0usize;
    let mut ascii_run = 0usize;
    for ch in text.chars() {
        if is_cjk(ch) {
            tokens += ascii_run.div_ceil(4);
            ascii_run = 0;
            tokens += 1;
        } else {
            ascii_run += 1;
        }
    }
    tokens += ascii_run.div_ceil(4);
    tokens
}

/// Compresses `OpenAI Function Calling` schemas by truncating descriptions,
/// removing titles/examples, and reducing token usage.
#[derive(Debug)]
pub struct SchemaCompressor {
    func_desc_max_len: usize,
    param_desc_max_len: usize,
    func_desc_max_tokens: usize,
    param_desc_max_tokens: usize,
    drop_examples: bool,
    drop_titles: bool,
    drop_markdown: bool,
    max_enum_items: usize,
}

impl Default for SchemaCompressor {
    fn default() -> Self {
        Self {
            func_desc_max_len: 256,
            param_desc_max_len: 160,
            func_desc_max_tokens: usize::MAX,
            param_desc_max_tokens: usize::MAX,
            drop_examples: true,
            drop_titles: true,
            drop_markdown: true,
            max_enum_items: usize::MAX,
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

    /// Set maximum tokens for function-level descriptions (default unlimited).
    #[must_use]
    pub fn with_func_desc_max_tokens(mut self, max: usize) -> Self {
        self.func_desc_max_tokens = max;
        self
    }

    /// Set maximum tokens for parameter-level descriptions (default unlimited).
    #[must_use]
    pub fn with_param_desc_max_tokens(mut self, max: usize) -> Self {
        self.param_desc_max_tokens = max;
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

    /// Set maximum number of enum items to keep (default unlimited).
    ///
    /// When exceeded, the enum array is truncated to `max` items and a
    /// sentinel string is appended indicating how many values were omitted.
    #[must_use]
    pub fn with_max_enum_items(mut self, max: usize) -> Self {
        self.max_enum_items = max;
        self
    }

    /// Compress an `OpenAI Function Calling` schema.
    ///
    /// Returns the original value unchanged if compression yields no savings.
    /// NOTE: The function-wrapper and bare-schema branches share ~80% of their
    /// logic.  Deduplication is deferred — golden tests must be in place first.
    #[must_use]
    pub fn compress(&self, tool: &Value) -> Value {
        let original_text = serde_json::to_string(tool).unwrap_or_else(|e| {
            tracing::warn!("SchemaCompressor: serde serialization failed: {e}");
            String::new()
        });
        let mut result = tool.clone();

        // P3: Pre-compress $defs/definitions entries so $ref targets
        // are already compressed when referenced during traversal.
        for defs_path in &[
            "/function/parameters/$defs",
            "/function/parameters/definitions",
            "/$defs",
            "/definitions",
        ] {
            #[allow(clippy::collapsible_if)]
            if let Some(defs) = result.pointer_mut(defs_path) {
                if let Some(defs_obj) = defs.as_object_mut() {
                    for (_key, def_schema) in defs_obj.iter_mut() {
                        self.compress_json_schema(def_schema, 1);
                    }
                }
            }
        }

        if let Some(function) = result.get_mut("function") {
            if let Some(desc) = function.get("description").and_then(|d| d.as_str()) {
                function["description"] = Value::String(self.truncate_description(
                    desc,
                    self.func_desc_max_len,
                    self.func_desc_max_tokens,
                ));
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
                result["description"] = Value::String(self.truncate_description(
                    desc,
                    self.func_desc_max_len,
                    self.func_desc_max_tokens,
                ));
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
            // Bare-schema path: the manual truncate + title removal above may overlap
            // with compress_json_schema below.  The redundancy is intentional defense-in-depth
            // — edge cases where result["description"] or result["title"] differ from what
            // the recursive walker sees at depth 0 are handled correctly by both paths.
            if result.get("type").is_some() || result.get("properties").is_some() {
                self.compress_json_schema(&mut result, 0);
            }
        }

        let compressed_text = serde_json::to_string(&result).unwrap_or_else(|e| {
            tracing::warn!("SchemaCompressor: serde serialization failed: {e}");
            String::new()
        });
        if original_text == compressed_text {
            return tool.clone();
        }
        result
    }

    /// Recursively compress a JSON schema value in place.
    ///
    /// Handles: `title`, `examples`, `description`, `enum`, `properties`,
    /// `items`, `anyOf`, `oneOf`, `allOf`, `additionalProperties`,
    /// `patternProperties`, `$defs`, `definitions`.
    #[allow(clippy::too_many_lines)]
    pub(crate) fn compress_json_schema(&self, schema: &mut Value, depth: usize) {
        if depth > 64 {
            return;
        }
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
            let token_max = if depth == 0 {
                self.func_desc_max_tokens
            } else {
                self.param_desc_max_tokens
            };
            obj.insert(
                "description".into(),
                Value::String(self.truncate_description(&desc, max_len, token_max)),
            );
        }
        #[allow(clippy::collapsible_if)]
        if self.max_enum_items != usize::MAX {
            if let Some(enum_arr) = obj.get_mut("enum").and_then(|v| v.as_array_mut()) {
                let original_len = enum_arr.len();
                if original_len > self.max_enum_items {
                    let remaining = original_len - self.max_enum_items;
                    enum_arr.truncate(self.max_enum_items);
                    // Security: Do NOT inject fake sentinel strings into the enum
                    // array. LLMs and validators treat every array element as a
                    // legitimate enum value — a sentinel like "<... N more omitted>"
                    // would be interpreted as a selectable option, leading to
                    // parameter hallucination and downstream validation failures.
                    //
                    // Instead, signal truncation via an extension field at the
                    // object level. Extensions prefixed with "x-" are compatible
                    // with OpenAPI / JSON Schema extension conventions and are
                    // ignored by validators that don't recognize them.
                    #[allow(clippy::cast_possible_truncation)]
                    obj.insert(
                        "x-tokenless-enum-truncated".into(),
                        Value::Number(serde_json::Number::from(remaining as u64)),
                    );
                }
            }
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
        // P3: additional recursion targets
        #[allow(clippy::collapsible_if)]
        if let Some(additional) = obj.get_mut("additionalProperties") {
            if additional.is_object() {
                self.compress_json_schema(additional, depth + 1);
            }
        }
        #[allow(clippy::collapsible_if)]
        if let Some(pattern_props) = obj.get_mut("patternProperties") {
            if let Some(props_obj) = pattern_props.as_object_mut() {
                for (_pattern, prop_schema) in props_obj.iter_mut() {
                    self.compress_json_schema(prop_schema, depth + 1);
                }
            }
        }
        #[allow(clippy::collapsible_if)]
        if let Some(nested_defs) = obj.get_mut("$defs") {
            if let Some(defs_obj) = nested_defs.as_object_mut() {
                for (_key, def_schema) in defs_obj.iter_mut() {
                    self.compress_json_schema(def_schema, depth + 1);
                }
            }
        }
        #[allow(clippy::collapsible_if)]
        if let Some(nested_defs) = obj.get_mut("definitions") {
            if let Some(defs_obj) = nested_defs.as_object_mut() {
                for (_key, def_schema) in defs_obj.iter_mut() {
                    self.compress_json_schema(def_schema, depth + 1);
                }
            }
        }
    }

    /// Find the byte position where `estimate_tokens(&text[..pos])` first
    /// exceeds `max_tokens`, or `text.len()` if the entire text fits.
    fn token_limit_byte_pos(text: &str, max_tokens: usize) -> usize {
        if max_tokens == usize::MAX || estimate_tokens(text) <= max_tokens {
            return text.len();
        }
        // Walk backward from the end until the prefix fits.
        let mut pos = text.len();
        while pos > 0 {
            pos = text
                .char_indices()
                .rev()
                .find(|(p, _)| *p < pos)
                .map_or(0, |(p, _)| p);
            if estimate_tokens(&text[..pos]) <= max_tokens {
                return pos;
            }
        }
        0
    }

    /// Truncate a description at a sentence boundary within `max_len`.
    ///
    /// Strips markdown code blocks and inline code when `drop_markdown` is enabled.
    /// Attempts to break at sentence endings (`.`, `。`, `！`, `？`) in the
    /// approximate range `[max_len * 0.5, max_len]` before resorting to hard
    /// character truncation.
    #[must_use]
    pub fn truncate_description(&self, desc: &str, max_len: usize, max_tokens: usize) -> String {
        // Phase 1: Normalize whitespace and optionally strip markdown.
        let trimmed = desc.trim();
        let text = if self.drop_markdown {
            let text = CODE_BLOCK_RE.replace_all(trimmed, "").into_owned();
            let text = INLINE_CODE_RE.replace_all(&text, "");
            WHITESPACE_RE.replace_all(&text, " ").into_owned()
        } else {
            WHITESPACE_RE.replace_all(trimmed, " ").into_owned()
        };
        let text_str = text.trim();

        // Early return if under both limits.
        if text_str.chars().count() <= max_len {
            if max_tokens != usize::MAX {
                let pos = Self::token_limit_byte_pos(text_str, max_tokens);
                if pos < text_str.len() {
                    return text_str[..pos].trim().to_string();
                }
            }
            return text_str.to_string();
        }

        // Phase 2: Sentence-boundary truncation in range [max_len * 0.5, max_len].
        #[allow(
            clippy::cast_precision_loss,
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss
        )]
        let min_target = (max_len as f64 * 0.5) as usize;
        let min_pos = find_char_boundary(text_str, min_target);
        let max_pos = find_char_boundary(text_str, max_len.min(text_str.len()));
        let search_range = &text_str[min_pos..max_pos];

        let sentence_endings = ['.', '。', '！', '？'];
        #[allow(clippy::double_ended_iterator_last)]
        let best_pos = search_range
            .char_indices()
            .filter(|(_, c)| sentence_endings.contains(c))
            .last()
            .map(|(i, c)| min_pos + i + c.len_utf8());

        let text = if let Some(pos) = best_pos {
            text_str[..pos].trim().to_string()
        } else {
            let mut truncate_pos = max_len;
            while !text_str.is_char_boundary(truncate_pos) && truncate_pos > 0 {
                truncate_pos -= 1;
            }
            text_str[..truncate_pos].trim().to_string()
        };

        // Phase 3: Token soft limit.
        if max_tokens != usize::MAX {
            let pos = Self::token_limit_byte_pos(&text, max_tokens);
            if pos < text.len() {
                return text[..pos].trim().to_string();
            }
        }

        text
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

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
        let text = "Short intro text for testing. This sentence ends here. More text follows \
                    after that point.";
        let result = compressor.truncate_description(text, 60, usize::MAX);
        assert!(result.ends_with('.'));
        assert!(result.len() <= 60);
    }

    #[test]
    fn test_markdown_removal() {
        let compressor = SchemaCompressor::new();
        let text = "Some text with ```code block``` and `inline code` markers.";
        let result = compressor.truncate_description(text, 256, usize::MAX);
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
        let result = compressor.truncate_description(&cjk, 256, usize::MAX);
        assert!(result.chars().all(|c| c == '中'));
        assert!(result.chars().count() <= 256);

        let cjk_long = "中".repeat(300);
        assert!(
            compressor
                .truncate_description(&cjk_long, 256, usize::MAX)
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

    /// D2: Verify `compress()` does not panic (stack overflow) on deeply nested
    /// schemas. Before the depth-guard fix, this test causes SIGABRT due to
    /// unbounded recursion in `compress_json_schema`.
    #[test]
    fn test_compress_deeply_nested_schema_no_panic() {
        let compressor = SchemaCompressor::new();
        // Build a 128-level nested schema: {"properties": {"p": {"properties": {"p": ...}}}}
        // 128 > 64 (guard threshold), yet shallow enough to avoid recursive drop overflow.
        let mut schema = serde_json::json!({"type": "string"});
        for _ in 0..128 {
            let prev = std::mem::replace(&mut schema, serde_json::Value::Null);
            schema = serde_json::json!({
                "type": "object",
                "properties": { "p": prev }
            });
        }
        // Before fix: stack overflow / SIGABRT (unbounded recursion)
        // After fix: returns compressed result without panic
        let _result = compressor.compress(&serde_json::json!({
            "function": {
                "name": "deep",
                "parameters": schema
            }
        }));
        // Intentionally forget `_result` to avoid recursive serde_json drop for deep nesting.
        // The test goal is solely to verify no-panic — the depth guard is the fix.
    }

    #[test]
    fn test_truncate_description_edge_cases_preserved() {
        let compressor = SchemaCompressor::new();
        // Short string unchanged
        assert_eq!(
            compressor.truncate_description("hello", 100, usize::MAX),
            "hello"
        );
        assert_eq!(compressor.truncate_description("hello", 100, 100), "hello");
        // Empty and whitespace-only
        assert_eq!(compressor.truncate_description("", 100, usize::MAX), "");
        assert_eq!(compressor.truncate_description("   ", 100, usize::MAX), "");
        // Markdown removed
        let md =
            compressor.truncate_description("Some `code` and ```block``` here.", 256, usize::MAX);
        assert!(!md.contains('`'), "markdown removed");
        // Whitespace normalized
        let ws = compressor.truncate_description("  hello    world  ", 256, usize::MAX);
        assert_eq!(ws, "hello world", "whitespace normalized");
        // Sentence boundary
        let sentence = "First sentence. Second sentence here. Third.";
        let short = compressor.truncate_description(sentence, 30, usize::MAX);
        assert!(short.len() <= 30);
        assert!(!short.contains("Third"), "stopped before Third");
    }

    // ── P1: max_enum_items ───────────────────────────────────────────

    #[test]
    fn test_enum_truncation_with_max_items() {
        let compressor = SchemaCompressor::new().with_max_enum_items(20);
        let enum_vals: Vec<Value> = (0..50).map(|i| json!(format!("v{i}"))).collect();
        let schema = json!({
            "function": {
                "name": "test",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "color": {
                            "type": "string",
                            "enum": enum_vals
                        }
                    }
                }
            }
        });
        let result = compressor.compress(&schema);
        let color = &result["function"]["parameters"]["properties"]["color"];
        let arr = color["enum"].as_array().unwrap();
        // 20 items exactly — no sentinel in the array
        assert_eq!(arr.len(), 20, "20 values only, no sentinel injected");
        // Verify extension field signals truncation
        assert_eq!(
            color["x-tokenless-enum-truncated"].as_u64().unwrap(),
            30,
            "extension field should record 30 omitted values"
        );
        // Verify all items are real enum values
        for (i, item) in arr.iter().enumerate() {
            assert_eq!(*item, json!(format!("v{i}")));
        }
    }

    #[test]
    fn test_enum_under_limit_unchanged() {
        let compressor = SchemaCompressor::new().with_max_enum_items(20);
        let enum_vals: Vec<Value> = (0..10).map(|i| json!(format!("v{}", i))).collect();
        let schema = json!({
            "function": {
                "name": "test",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "color": {
                            "type": "string",
                            "enum": enum_vals.clone()
                        }
                    }
                }
            }
        });
        let result = compressor.compress(&schema);
        let arr = result["function"]["parameters"]["properties"]["color"]["enum"]
            .as_array()
            .unwrap();
        assert_eq!(arr.len(), 10, "10 items, no marker, not truncated");
    }

    #[test]
    fn test_enum_default_unlimited() {
        let compressor = SchemaCompressor::new(); // default: no limit
        let enum_vals: Vec<Value> = (0..100).map(|i| json!(format!("v{}", i))).collect();
        let schema = json!({
            "function": {
                "name": "test",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "color": {
                            "type": "string",
                            "enum": enum_vals
                        }
                    }
                }
            }
        });
        let result = compressor.compress(&schema);
        let arr = result["function"]["parameters"]["properties"]["color"]["enum"]
            .as_array()
            .unwrap();
        assert_eq!(arr.len(), 100, "default unlimited: all 100 items preserved");
    }

    #[test]
    fn test_enum_truncation_zero_max() {
        let compressor = SchemaCompressor::new().with_max_enum_items(0);
        let enum_vals: Vec<Value> = (0..5).map(|i| json!(format!("v{}", i))).collect();
        let schema = json!({
            "function": {
                "name": "test",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "color": {
                            "type": "string",
                            "enum": enum_vals
                        }
                    }
                }
            }
        });
        let result = compressor.compress(&schema);
        let color = &result["function"]["parameters"]["properties"]["color"];
        let arr = color["enum"].as_array().unwrap();
        assert_eq!(arr.len(), 0, "all values truncated, no sentinel in array");
        let omitted = color["x-tokenless-enum-truncated"].as_u64().unwrap();
        assert_eq!(omitted, 5, "extension field records 5 omitted values");
    }

    #[test]
    fn test_no_enum_key_no_effect() {
        let compressor = SchemaCompressor::new().with_max_enum_items(10);
        let schema = json!({
            "function": {
                "name": "test",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "name": {
                            "type": "string",
                            "description": "Just a name, no enum here"
                        }
                    }
                }
            }
        });
        let result = compressor.compress(&schema);
        // Should not panic, should not add enum key
        let props = &result["function"]["parameters"]["properties"]["name"];
        assert!(props.get("enum").is_none(), "no enum key should be added");
        assert_eq!(props["type"], "string");
    }

    #[test]
    fn test_enum_in_nested_property_truncated() {
        let compressor = SchemaCompressor::new().with_max_enum_items(5);
        let enum_vals: Vec<Value> = (0..30).map(|i| json!(format!("v{}", i))).collect();
        let schema = json!({
            "function": {
                "name": "test",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "level1": {
                            "type": "object",
                            "properties": {
                                "level2": {
                                    "type": "object",
                                    "properties": {
                                        "color": {
                                            "type": "string",
                                            "enum": enum_vals
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        });
        let result = compressor.compress(&schema);
        let color = &result["function"]["parameters"]["properties"]["level1"]
            ["properties"]["level2"]["properties"]["color"];
        let arr = color["enum"].as_array().unwrap();
        assert_eq!(arr.len(), 5, "5 values only, no sentinel in array");
        // Verify extension field
        assert_eq!(
            color["x-tokenless-enum-truncated"].as_u64().unwrap(),
            25,
            "extension field records 25 omitted values"
        );
        // Verify first 5 values are the original first 5
        for (i, item) in arr.iter().enumerate() {
            assert_eq!(*item, json!(format!("v{i}")));
        }
    }

    // ── P2: Token-aware Description Truncation ──────────────────────────

    #[test]
    fn test_estimate_tokens_english() {
        // "hello world " repeated 50 times = 600 chars all ASCII = ~150 tokens
        let text = "hello world ".repeat(50);
        let tokens = super::estimate_tokens(&text);
        assert!(
            (140..=160).contains(&tokens),
            "English text: ~150 tokens expected, got {tokens}"
        );
    }

    #[test]
    fn test_estimate_tokens_cjk() {
        let text = "你好世界".repeat(50); // 200 CJK chars = ~200 tokens
        let tokens = super::estimate_tokens(&text);
        assert_eq!(tokens, 200, "CJK: 200 chars = 200 tokens, got {tokens}");
    }

    #[test]
    fn test_estimate_tokens_mixed() {
        // "hello" (5 ASCII -> 2 tokens) + "你好" (2 CJK -> 2 tokens) + "world" (5 ASCII -> 2
        // tokens)
        let text = "hello你好world";
        let tokens = super::estimate_tokens(text);
        assert_eq!(tokens, 6, "5+2+5: 2+2+2 = 6 tokens, got {tokens}");
    }

    #[test]
    fn test_estimate_tokens_empty() {
        assert_eq!(super::estimate_tokens(""), 0);
    }

    #[test]
    fn test_token_aware_truncation_cjk_stricter() {
        let compressor = SchemaCompressor::new().with_param_desc_max_tokens(50);
        // 200 CJK chars = 200 tokens. Token limit 50 should bind tighter than char limit 160.
        let cjk_text = "你好世界".repeat(50); // 200 chars, 200 tokens
        let result = compressor.truncate_description(&cjk_text, 160, 50);
        assert!(
            result.chars().count() <= 60,
            "token limit should reduce below char limit, got {} chars",
            result.chars().count()
        );
        assert!(
            super::estimate_tokens(&result) <= 50,
            "result should be <= 50 tokens, got {}",
            super::estimate_tokens(&result)
        );
    }

    #[test]
    fn test_token_aware_default_unlimited() {
        // Default: no token limit. Char limit of 50 should be the only constraint.
        let compressor = SchemaCompressor::new();
        let cjk_text = "你好世界".repeat(50); // 200 chars
        let result = compressor.truncate_description(&cjk_text, 50, usize::MAX);
        assert!(
            result.chars().count() <= 50,
            "only char limit applies: should be <= 50 chars"
        );
    }

    #[test]
    fn test_token_aware_english_char_limit_binds_first() {
        let compressor = SchemaCompressor::new().with_func_desc_max_tokens(100);
        // English text: 600 chars = ~150 tokens. Char limit 200 binds first (200 < 600).
        let text = "hello world ".repeat(50); // 600 chars
        let result = compressor.truncate_description(&text, 200, 100);
        // char limit applied first: <= 200 chars. Token check after: ~50 tokens, which is < 100.
        assert!(result.chars().count() <= 200);
    }

    #[test]
    fn test_token_aware_both_under_limits() {
        let compressor = SchemaCompressor::new().with_func_desc_max_tokens(100);
        // Short text: 20 chars, well under both limits
        let text = "Short description here.";
        let result = compressor.truncate_description(text, 256, 100);
        assert_eq!(result, text, "unchanged when under both limits");
    }

    #[test]
    fn test_is_cjk_detection() {
        assert!(super::is_cjk('中'), "CJK ideograph");
        assert!(super::is_cjk('あ'), "Hiragana");
        assert!(super::is_cjk('カ'), "Katakana");
        assert!(super::is_cjk('한'), "Hangul");
        assert!(!super::is_cjk('a'), "ASCII");
        assert!(!super::is_cjk('1'), "digit");
        assert!(!super::is_cjk(' '), "space");
        assert!(!super::is_cjk('😀'), "emoji");
    }

    // ── P3: $ref / $defs Recursive Compression ─────────────────────────

    #[test]
    fn test_defs_descriptions_compressed() {
        let compressor = SchemaCompressor::new();
        let schema = json!({
            "function": {
                "name": "test",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "pet": { "$ref": "#/$defs/Pet" }
                    },
                    "$defs": {
                        "Pet": {
                            "type": "object",
                            "title": "Pet Title",
                            "description": "A pet description that is intentionally very long so that it exceeds the default parameter description maximum length limit of 160 characters for testing purposes.",
                            "properties": {
                                "name": { "type": "string", "title": "Name Title" }
                            }
                        }
                    }
                }
            }
        });
        let result = compressor.compress(&schema);
        let pet_def = &result["function"]["parameters"]["$defs"]["Pet"];
        assert!(pet_def.get("title").is_none(), "title should be removed");
        assert!(
            pet_def["description"].as_str().unwrap().len() <= 160,
            "description should be truncated"
        );
        assert!(
            pet_def["properties"]["name"].get("title").is_none(),
            "nested title should be removed"
        );
        assert_eq!(
            result["function"]["parameters"]["properties"]["pet"]["$ref"],
            "#/$defs/Pet"
        );
    }

    #[test]
    fn test_definitions_key_compat() {
        let compressor = SchemaCompressor::new();
        let schema = json!({
            "function": {
                "name": "test",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "pet": { "$ref": "#/definitions/Pet" }
                    },
                    "definitions": {
                        "Pet": {
                            "type": "object",
                            "title": "Pet Title",
                            "description": "A pet description that is intentionally very long so that it exceeds the default parameter description maximum length limit of 160 characters for testing purposes.",
                            "properties": {
                                "name": { "type": "string", "title": "Name Title" }
                            }
                        }
                    }
                }
            }
        });
        let result = compressor.compress(&schema);
        let pet_def = &result["function"]["parameters"]["definitions"]["Pet"];
        assert!(
            pet_def.get("title").is_none(),
            "title should be removed from definitions key"
        );
        assert!(
            pet_def["description"].as_str().unwrap().len() <= 160,
            "description should be truncated in definitions key"
        );
        assert!(
            pet_def["properties"]["name"].get("title").is_none(),
            "nested title should be removed in definitions key"
        );
        assert_eq!(
            result["function"]["parameters"]["properties"]["pet"]["$ref"],
            "#/definitions/Pet"
        );
    }

    #[test]
    fn test_additional_properties_compressed() {
        let compressor = SchemaCompressor::new();
        let schema = json!({
            "function": {
                "name": "test",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "meta": { "type": "object" }
                    },
                    "additionalProperties": {
                        "type": "string",
                        "title": "Extra Title",
                        "description": "Additional properties description that is intentionally verbose and exceeds the default parameter description maximum length limit of 160 characters for testing compression behavior."
                    }
                }
            }
        });
        let result = compressor.compress(&schema);
        let ap = &result["function"]["parameters"]["additionalProperties"];
        assert!(
            ap.get("title").is_none(),
            "title should be removed from additionalProperties"
        );
        assert!(
            ap["description"].as_str().unwrap().len() <= 160,
            "description should be truncated in additionalProperties"
        );
    }

    #[test]
    fn test_additional_properties_bool_unchanged() {
        let compressor = SchemaCompressor::new();
        let schema = json!({
            "function": {
                "name": "test",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "name": { "type": "string" }
                    },
                    "additionalProperties": false
                }
            }
        });
        let result = compressor.compress(&schema);
        let ap = &result["function"]["parameters"]["additionalProperties"];
        assert!(
            ap.as_bool() == Some(false),
            "additionalProperties: false must remain false"
        );
    }

    #[test]
    fn test_no_defs_no_crash() {
        let compressor = SchemaCompressor::new();
        let schema = json!({
            "function": {
                "name": "test",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "name": { "type": "string", "title": "Name", "description": "A simple name field." }
                    }
                }
            }
        });
        let result = compressor.compress(&schema);
        assert_eq!(result["function"]["name"], "test");
        assert!(
            result["function"]["parameters"]["properties"]["name"]
                .get("title")
                .is_none()
        );
    }

    #[test]
    fn test_empty_defs_no_crash() {
        let compressor = SchemaCompressor::new();
        let schema = json!({
            "function": {
                "name": "test",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "name": { "type": "string" }
                    },
                    "$defs": {}
                }
            }
        });
        let result = compressor.compress(&schema);
        assert_eq!(result["function"]["name"], "test");
        assert_eq!(result["function"]["parameters"]["$defs"], json!({}));
    }

    #[test]
    fn test_bare_schema_with_defs() {
        let compressor = SchemaCompressor::new();
        let schema = json!({
            "title": "Root Title",
            "type": "object",
            "description": "Root description that is long enough to exceed the function description maximum length limit of 256 characters so we can verify truncation works correctly at the root level of a bare schema without function wrapper.",
            "$defs": {
                "Pet": {
                    "type": "object",
                    "title": "Pet Title",
                    "description": "A pet description that is intentionally very long so that it exceeds the default parameter description maximum length limit of 160 characters for testing purposes.",
                    "properties": {
                        "name": { "type": "string", "title": "Name Title" }
                    }
                }
            }
        });
        let result = compressor.compress(&schema);
        assert!(
            result.get("title").is_none(),
            "root title should be removed"
        );
        let pet_def = &result["$defs"]["Pet"];
        assert!(
            pet_def.get("title").is_none(),
            "def title should be removed"
        );
        assert!(
            pet_def["description"].as_str().unwrap().len() <= 160,
            "def description should be truncated"
        );
        assert!(
            pet_def["properties"]["name"].get("title").is_none(),
            "nested def title should be removed"
        );
    }

    #[test]
    fn test_defs_with_recursive_refs_compressed() {
        let compressor = SchemaCompressor::new();
        let schema = json!({
            "function": {
                "name": "test",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "tree": { "$ref": "#/$defs/Node" }
                    },
                    "$defs": {
                        "Node": {
                            "type": "object",
                            "title": "Node Title",
                            "description": "A tree node description that should be truncated because it is intentionally made very long to exceed the 160 character parameter description limit for compression testing.",
                            "properties": {
                                "value": { "type": "integer", "title": "Value Title" },
                                "children": {
                                    "type": "array",
                                    "title": "Children Title",
                                    "items": { "$ref": "#/$defs/Node" }
                                }
                            }
                        }
                    }
                }
            }
        });
        let result = compressor.compress(&schema);
        let node_def = &result["function"]["parameters"]["$defs"]["Node"];
        assert!(
            node_def.get("title").is_none(),
            "title should be removed from Node def"
        );
        assert!(
            node_def["description"].as_str().unwrap().len() <= 160,
            "description should be truncated in Node def"
        );
        assert!(
            node_def["properties"]["value"].get("title").is_none(),
            "nested value title should be removed"
        );
        assert!(
            node_def["properties"]["children"].get("title").is_none(),
            "children title should be removed"
        );
        assert_eq!(
            node_def["properties"]["children"]["items"]["$ref"],
            "#/$defs/Node"
        );
    }

    #[test]
    fn test_anyof_with_defs_compressed() {
        let compressor = SchemaCompressor::new();
        let schema = json!({
            "function": {
                "name": "test",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "animal": {
                            "anyOf": [
                                { "$ref": "#/$defs/Dog" },
                                { "$ref": "#/$defs/Cat" }
                            ]
                        }
                    },
                    "$defs": {
                        "Dog": {
                            "type": "object",
                            "title": "Dog Title",
                            "description": "A dog description that is intentionally verbose and exceeds the default parameter description length limit of 160 characters to ensure compression works correctly across anyOf references.",
                            "properties": {
                                "breed": { "type": "string", "title": "Breed Title" }
                            }
                        },
                        "Cat": {
                            "type": "object",
                            "title": "Cat Title",
                            "description": "A cat description that is also intentionally verbose and exceeds the default parameter description length limit of 160 characters to ensure compression works correctly across anyOf references.",
                            "properties": {
                                "color": { "type": "string", "title": "Color Title" }
                            }
                        }
                    }
                }
            }
        });
        let result = compressor.compress(&schema);
        let dog_def = &result["function"]["parameters"]["$defs"]["Dog"];
        assert!(
            dog_def.get("title").is_none(),
            "dog title should be removed"
        );
        assert!(
            dog_def["description"].as_str().unwrap().len() <= 160,
            "dog description should be truncated"
        );
        assert!(
            dog_def["properties"]["breed"].get("title").is_none(),
            "dog breed title should be removed"
        );

        let cat_def = &result["function"]["parameters"]["$defs"]["Cat"];
        assert!(
            cat_def.get("title").is_none(),
            "cat title should be removed"
        );
        assert!(
            cat_def["description"].as_str().unwrap().len() <= 160,
            "cat description should be truncated"
        );
        assert!(
            cat_def["properties"]["color"].get("title").is_none(),
            "cat color title should be removed"
        );
    }
}
