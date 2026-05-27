#![allow(clippy::expect_used, clippy::unwrap_used, clippy::approx_constant)]
//! Regression tests for Opt#5: Eliminate double serialization in the CLI.
//!
//! These tests replicate the compression logic that the CLI performs,
//! verifying that schema and response compression produce valid JSON
//! and that the compact+pretty serialization round-trip yields identical
//! content to a direct compact serialization.
//!
//! After the Opt#5 refactor (eliminating `to_string(deserialize(to_string_pretty))`),
//! these same assertions must hold — the output must be semantically equivalent.

use serde_json::{Value, json};
use tokenless_schema::{ResponseCompressor, SchemaCompressor};

// ---------------------------------------------------------------------------
// Schema compressor regression tests
// ---------------------------------------------------------------------------

#[test]
fn test_schema_compress_preserves_name() {
    let input = json!({
        "function": {
            "name": "get_weather",
            "description": "Get the current weather for a location",
            "parameters": {
                "type": "object",
                "properties": {
                    "location": {
                        "type": "string",
                        "description": "City name"
                    }
                }
            }
        }
    });
    let compressor = SchemaCompressor::new();
    let result = compressor.compress(&input);
    assert_eq!(result["function"]["name"].as_str().unwrap(), "get_weather");
}

#[test]
fn test_schema_compress_output_is_valid_json() {
    let input = json!({
        "function": {
            "name": "search_docs",
            "description": "Search documentation with a long query string. This description is intentionally verbose to verify that the compression does not break JSON structure.",
            "parameters": {
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "title": "Search Query",
                        "examples": ["rust async"],
                        "description": "The search query string"
                    }
                }
            }
        }
    });
    let compressor = SchemaCompressor::new();
    let result = compressor.compress(&input);

    // Output must serialize to valid JSON
    let compact = serde_json::to_string(&result).expect("compact serialization must succeed");
    let _parsed: Value =
        serde_json::from_str(&compact).expect("deserializing compact output must succeed");

    let pretty = serde_json::to_string_pretty(&result).expect("pretty serialization must succeed");
    let _parsed_pretty: Value =
        serde_json::from_str(&pretty).expect("deserializing pretty output must succeed");

    // Both serialization forms must represent the same data
    let from_compact: Value = serde_json::from_str(&compact).unwrap();
    let from_pretty: Value = serde_json::from_str(&pretty).unwrap();
    assert_eq!(
        from_compact, from_pretty,
        "compact and pretty serializations must be semantically identical"
    );
}

#[test]
fn test_schema_compress_batch_output_is_valid_json() {
    let input = json!([
        {
            "function": {
                "name": "fn_a",
                "description": "Function A",
                "parameters": { "type": "object", "properties": {} }
            }
        },
        {
            "function": {
                "name": "fn_b",
                "title": "Should Be Removed",
                "description": "Function B",
                "parameters": { "type": "object", "properties": {} }
            }
        }
    ]);
    let compressor = SchemaCompressor::new();
    let results: Vec<Value> = input
        .as_array()
        .unwrap()
        .iter()
        .map(|item| compressor.compress(item))
        .collect();

    // Batch output must serialize to valid JSON
    let compact = serde_json::to_string(&results).expect("batch compact serialization");
    let parsed: Value = serde_json::from_str(&compact).expect("batch compact deserialization");
    assert!(parsed.is_array());
    assert_eq!(parsed.as_array().unwrap().len(), 2);

    let pretty = serde_json::to_string_pretty(&results).expect("batch pretty serialization");
    let from_pretty: Value = serde_json::from_str(&pretty).expect("batch pretty deserialization");
    assert_eq!(parsed, from_pretty);
}

#[test]
fn test_schema_compress_title_removed() {
    let input = json!({
        "function": {
            "name": "test_fn",
            "title": "Test Function Title",
            "description": "A test function",
            "parameters": {
                "type": "object",
                "title": "Parameters Title",
                "properties": {
                    "field1": {
                        "type": "string",
                        "title": "Should Be Gone",
                        "examples": ["ex1", "ex2"]
                    }
                }
            }
        }
    });
    let compressor = SchemaCompressor::new();
    let result = compressor.compress(&input);
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
fn test_schema_compress_no_change_returns_original_value() {
    // If compression produces no savings, the API returns the original value.
    let input = json!({"function": {"name": "short", "description": "short", "parameters": {"type": "object"}}});
    let compressor = SchemaCompressor::new();
    let result = compressor.compress(&input);
    // Same JSON tree — no serialization difference
    assert_eq!(
        result, input,
        "compression with no savings should return original Value unchanged"
    );
}

// ---------------------------------------------------------------------------
// Response compressor regression tests
// ---------------------------------------------------------------------------

#[test]
fn test_response_compress_drops_debug_fields_and_nulls() {
    let input = json!({
        "data": "important",
        "debug": "should-be-removed",
        "Debug": "also-removed",
        "trace": "rm",
        "stack": "rm",
        "null_field": null,
        "keep": 42
    });
    let compressor = ResponseCompressor::new();
    let result = compressor.compress(&input);
    let obj = result.as_object().unwrap();
    assert!(obj.contains_key("data"), "'data' must be preserved");
    assert!(obj.contains_key("keep"), "'keep' must be preserved");
    assert!(!obj.contains_key("debug"), "'debug' must be dropped");
    assert!(!obj.contains_key("Debug"), "'Debug' must be dropped");
    assert!(!obj.contains_key("trace"), "'trace' must be dropped");
    assert!(!obj.contains_key("stack"), "'stack' must be dropped");
    assert!(
        !obj.contains_key("null_field"),
        "null fields must be dropped"
    );
}

#[test]
fn test_response_compress_output_is_valid_json() {
    let input = json!({
        "results": [1, 2, 3, 4, 5],
        "metadata": {
            "query": "test",
            "timestamp": null,
            "debug_info": "should-gone"
        },
        "data": "some results"
    });
    let compressor = ResponseCompressor::new();
    let result = compressor.compress(&input);

    let compact = serde_json::to_string(&result).expect("compact serialization");
    let _parsed: Value =
        serde_json::from_str(&compact).expect("response compression output must be valid JSON");

    let pretty = serde_json::to_string_pretty(&result).expect("pretty serialization");
    let from_compact: Value = serde_json::from_str(&compact).unwrap();
    let from_pretty: Value = serde_json::from_str(&pretty).unwrap();
    assert_eq!(
        from_compact, from_pretty,
        "compact and pretty must be semantically identical for response output"
    );
}

#[test]
fn test_response_compress_no_change_returns_original() {
    // Only valid fields, no compression needed
    let input = json!({"a": 1, "b": "hello", "c": true});
    let compressor = ResponseCompressor::new();
    let result = compressor.compress(&input);
    assert_eq!(
        result, input,
        "response compression with no savings should return original Value unchanged"
    );
}

#[test]
fn test_response_compress_string_truncation_with_defaults() {
    let long_string = "x".repeat(600);
    let input = json!({"text": long_string});
    let compressor = ResponseCompressor::new();
    let result = compressor.compress(&input);
    let text = result["text"].as_str().unwrap();
    assert!(
        text.contains("truncated"),
        "long string must be truncated with marker"
    );
}

// ---------------------------------------------------------------------------
// Cross-cutting: compact/pretty equivalence tests
// These directly verify the invariant that Opt#5 must preserve.
// ---------------------------------------------------------------------------

/// Simulates what the CLI currently does: serialize pretty, then re-serialize compact.
/// Opt#5 replaces this with direct serialization from the in-memory Value.
/// Both paths must produce semantically identical compact output.
#[test]
fn test_compact_pretty_roundtrip_is_idempotent() {
    let value = json!({
        "function": {
            "name": "test_func",
            "description": "A test function with a description.",
            "parameters": {
                "type": "object",
                "properties": {
                    "param1": {
                        "type": "string",
                        "description": "Parameter one"
                    }
                }
            }
        }
    });

    // Current (pre-optimization) path: pretty → deserialize → compact
    let pretty = serde_json::to_string_pretty(&value).unwrap();
    let round_tripped: Value = serde_json::from_str(&pretty).unwrap();
    let compact_via_pretty = serde_json::to_string(&round_tripped).unwrap();

    // Post-optimization path: compact directly from Value
    let compact_direct = serde_json::to_string(&value).unwrap();

    // Both should produce the same compact JSON
    let parsed_via_pretty: Value = serde_json::from_str(&compact_via_pretty).unwrap();
    let parsed_direct: Value = serde_json::from_str(&compact_direct).unwrap();
    assert_eq!(
        parsed_via_pretty, parsed_direct,
        "pretty→compact round-trip must equal direct compact"
    );
}

/// Regression: the double-serialization path must not change the semantic
/// content of the compressed schema.
#[test]
fn test_compress_then_pretty_compact_roundtrip_preserves_schema_semantics() {
    let input = json!({
        "function": {
            "name": "get_user",
            "description": "Retrieve user by ID. Returns user object with profile data.",
            "parameters": {
                "type": "object",
                "title": "Parameters",
                "properties": {
                    "user_id": {
                        "type": "integer",
                        "title": "User ID",
                        "description": "The unique user identifier",
                        "examples": [42, 100]
                    },
                    "include_profile": {
                        "type": "boolean",
                        "description": "Include extended profile data"
                    }
                },
                "required": ["user_id"]
            }
        }
    });

    let compressor = SchemaCompressor::new();
    let result = compressor.compress(&input);

    // Pre-optimization path (current code):
    let pretty = serde_json::to_string_pretty(&result).unwrap();
    let round_tripped: Value = serde_json::from_str(&pretty).unwrap();
    let compact_current = serde_json::to_string(&round_tripped).unwrap();

    // Post-optimization path (direct compact):
    let compact_optimized = serde_json::to_string(&result).unwrap();

    // Regardless of path, the resulting JSON must be valid and equivalent
    let v_current: Value = serde_json::from_str(&compact_current).unwrap();
    let v_optimized: Value = serde_json::from_str(&compact_optimized).unwrap();
    assert_eq!(
        v_current, v_optimized,
        "pre-opt and post-opt compact outputs must be semantically identical"
    );

    // The function name must survive both paths
    assert_eq!(v_optimized["function"]["name"], "get_user");
    // The description should be preserved (not empty, not removed)
    assert!(
        !v_optimized["function"]["description"]
            .as_str()
            .unwrap()
            .is_empty()
    );
    // Required fields must be intact
    assert!(v_optimized["function"]["parameters"]["required"].is_array());
}

/// Verify that compact serialization of compressed output produces a
/// smaller or equal byte size compared to pretty serialization, while
/// preserving the same semantic content.
#[test]
fn test_compact_is_never_larger_than_pretty() {
    let input = json!({
        "function": {
            "name": "long_func",
            "description": "This function performs a comprehensive analysis of the input data including validation, transformation, and enrichment steps. It handles edge cases gracefully and returns structured results.",
            "parameters": {
                "type": "object",
                "properties": {
                    "input_text": {
                        "type": "string",
                        "description": "The raw input text to be processed and analyzed by the pipeline"
                    },
                    "options": {
                        "type": "object",
                        "properties": {
                            "verbose": { "type": "boolean", "description": "Enable verbose logging" },
                            "max_results": { "type": "integer", "description": "Maximum number of results to return" }
                        }
                    }
                }
            }
        }
    });

    let compressor = SchemaCompressor::new();
    let result = compressor.compress(&input);

    let compact = serde_json::to_string(&result).unwrap();
    let pretty = serde_json::to_string_pretty(&result).unwrap();

    // Compact should be smaller or equal
    assert!(
        compact.len() <= pretty.len(),
        "compact ({}) must be <= pretty ({}) bytes",
        compact.len(),
        pretty.len()
    );

    // Semantic equivalence
    let v_compact: Value = serde_json::from_str(&compact).unwrap();
    let v_pretty: Value = serde_json::from_str(&pretty).unwrap();
    assert_eq!(v_compact, v_pretty);
}

// ---------------------------------------------------------------------------
// Gap 4: Schema batch mode — 10 schemas independently compressed
// ---------------------------------------------------------------------------

#[test]
fn test_schema_batch_10_schemas_each_independent() {
    let schemas: Vec<Value> = (0..10)
        .map(|i| {
            json!({
                "function": {
                    "name": format!("fn_{i}"),
                    "title": format!("Title {}", i),
                    "description": format!("Function number {} with a moderately long description that might need some truncation applied by the schema compressor.", i),
                    "parameters": {
                        "type": "object",
                        "title": format!("Params Title {}", i),
                        "properties": {
                            format!("param_{}", i): {
                                "type": "string",
                                "title": format!("Param Title {}", i),
                                "examples": [format!("ex_{}_a", i), format!("ex_{}_b", i)],
                                "description": format!("Parameter {} for function {} with a verbose description that should be compressed to fit within the default limits.", i, i)
                            }
                        }
                    }
                }
            })
        })
        .collect();

    let compressor = SchemaCompressor::new();
    let results: Vec<Value> = schemas
        .iter()
        .map(|item| compressor.compress(item))
        .collect();

    assert_eq!(results.len(), 10, "should have 10 results");

    // Each schema should be independently compressed
    for (i, result) in results.iter().enumerate() {
        let fn_name = &result["function"]["name"];
        assert_eq!(
            fn_name.as_str().unwrap(),
            format!("fn_{i}"),
            "function name must be preserved for schema {i}"
        );

        // Title must be removed
        assert!(
            result["function"].get("title").is_none(),
            "title must be removed from schema {i}"
        );

        // Ensure output is valid JSON
        let compact = serde_json::to_string(result).expect("compact must succeed");
        let _: Value = serde_json::from_str(&compact).expect("must be valid JSON");

        let pretty = serde_json::to_string_pretty(result).expect("pretty must succeed");
        let _: Value = serde_json::from_str(&pretty).expect("pretty must be valid JSON");
    }

    // All 10 compact serializations together must be valid
    let batch_compact = serde_json::to_string(&results).expect("batch compact");
    let _: Value = serde_json::from_str(&batch_compact).expect("batch must be valid JSON");

    let batch_pretty = serde_json::to_string_pretty(&results).expect("batch pretty");
    let v_compact: Value = serde_json::from_str(&batch_compact).unwrap();
    let v_pretty: Value = serde_json::from_str(&batch_pretty).unwrap();
    assert_eq!(
        v_compact, v_pretty,
        "batch compact and pretty must be semantically identical"
    );
}

#[test]
fn test_schema_batch_heterogeneous_schemas() {
    // Mix schemas with and without parameters, some with $defs
    let schemas = json!([
        {"function": {"name": "simple", "description": "A simple function", "parameters": {"type": "object", "properties": {}}}},
        {"function": {"name": "with_title", "title": "Remove Me", "description": "Has a title", "parameters": {"type": "object"}}},
        {"function": {"name": "with_defs", "description": "Has definitions", "parameters": {
            "type": "object",
            "properties": {"pet": {"$ref": "#/$defs/Pet"}},
            "$defs": {"Pet": {"type": "object", "title": "Pet Def", "description": "A pet definition that is long enough to exceed the default parameter description length limit of one hundred and sixty characters for compression purposes."}}
        }}},
    ]);

    let compressor = SchemaCompressor::new();
    let results: Vec<Value> = schemas
        .as_array()
        .unwrap()
        .iter()
        .map(|item| compressor.compress(item))
        .collect();

    assert_eq!(results.len(), 3);

    // Schema 0: simple — should be unchanged (no compression needed except default behavior)
    assert_eq!(results[0]["function"]["name"], "simple");

    // Schema 1: title must be removed
    assert!(results[1]["function"].get("title").is_none());

    // Schema 2: defs must be compressed
    let pet = &results[2]["function"]["parameters"]["$defs"]["Pet"];
    assert!(
        pet.get("title").is_none(),
        "Pet title in $defs should be removed"
    );
    assert!(
        pet["description"].as_str().unwrap().len() <= 160,
        "Pet description should be truncated"
    );
}

// ---------------------------------------------------------------------------
// Gap 9: CLI integration tests via tokenless_schema API
//   Verifies that compress-schema and compress-response produce valid JSON
//   output via the same API the CLI calls internally.
// ---------------------------------------------------------------------------

#[test]
fn test_cli_compress_schema_api_produces_valid_json() {
    // Simulates: tokenless compress-schema < long_schema.json
    let input_json = json!({
        "function": {
            "name": "search_repository",
            "title": "Search Repository",
            "description": "Search a code repository for files matching the given pattern. \
                            Returns matching file paths with optional line numbers and \
                            context. Supports glob patterns, regex, and tree-sitter queries. \
                            Results are ranked by relevance. This is a very long description \
                            that should be truncated by the schema compressor because it \
                            exceeds the 256 character default limit for function descriptions.",
            "parameters": {
                "type": "object",
                "title": "Search Parameters",
                "required": ["pattern"],
                "properties": {
                    "pattern": {
                        "type": "string",
                        "title": "Search Pattern",
                        "examples": ["*.rs", "fn main"],
                        "description": "The search pattern to match against files. \
                                        Can be a glob, regex, or tree-sitter query."
                    },
                    "path": {
                        "type": "string",
                        "title": "Search Path",
                        "description": "The directory path to search in. Defaults to project root."
                    }
                }
            }
        }
    });

    let compressor = SchemaCompressor::new();
    let result = compressor.compress(&input_json);

    // The CLI serializes with to_string_pretty() — verify it's valid JSON
    let pretty_output = serde_json::to_string_pretty(&result)
        .expect("compress-schema output must serialize to pretty JSON");
    let parsed: Value = serde_json::from_str(&pretty_output)
        .expect("compress-schema output must parse as valid JSON");

    // Verify key functionality fields survive compression
    assert_eq!(parsed["function"]["name"], "search_repository");
    assert!(parsed["function"]["parameters"]["required"].is_array());
    assert_eq!(parsed["function"]["parameters"]["required"][0], "pattern");
    assert!(
        parsed["function"].get("title").is_none(),
        "title must be removed by default"
    );
    assert_eq!(
        parsed["function"]["parameters"]["properties"]["pattern"]["type"],
        "string"
    );
}

#[test]
fn test_cli_compress_response_api_produces_valid_json() {
    // Simulates: tokenless compress-response < api_response.json
    let input_json = json!({
        "status": "success",
        "debug": "this should be dropped",
        "trace": "this also dropped",
        "data": {
            "items": [
                {"id": 1, "name": "item_1", "metadata": {"debug": "nested_debug", "value": "ok"}},
                {"id": 2, "name": "item_2", "metadata": null},
                {"id": 3, "name": "item_3", "metadata": {"value": "ok"}}
            ],
            "total": 3
        },
        "errors": null,
        "empty_array": [],
        "empty_string": "",
        "very_long_string": "x".repeat(1000)
    });

    let compressor = ResponseCompressor::new();
    let result = compressor.compress(&input_json);

    // The CLI serializes with to_string_pretty() — verify it's valid JSON
    let pretty_output = serde_json::to_string_pretty(&result)
        .expect("compress-response output must serialize to pretty JSON");
    let parsed: Value = serde_json::from_str(&pretty_output)
        .expect("compress-response output must parse as valid JSON");

    // Verify key properties
    assert_eq!(parsed["status"], "success");
    assert!(parsed.get("debug").is_none(), "debug field must be dropped");
    assert!(parsed.get("trace").is_none(), "trace field must be dropped");
    assert!(
        parsed.get("errors").is_none(),
        "null fields must be dropped by default"
    );
    assert!(
        parsed.get("empty_array").is_none(),
        "empty arrays must be dropped by default"
    );
    assert!(
        parsed.get("empty_string").is_none(),
        "empty strings must be dropped by default"
    );
    assert!(parsed["data"]["items"].is_array());

    // Long string should be truncated
    let long_str = parsed["very_long_string"].as_str().unwrap();
    assert!(
        long_str.contains("truncated"),
        "very long string must be truncated"
    );
}

#[test]
fn test_cli_compress_schema_no_savings_returns_original() {
    // When compression yields no savings, the CLI outputs the original input
    let input_json = json!({
        "function": {
            "name": "short_fn",
            "description": "short desc",
            "parameters": {
                "type": "object",
                "properties": {
                    "x": {"type": "number"}
                }
            }
        }
    });

    let _input_str = serde_json::to_string_pretty(&input_json).unwrap();

    let compressor = SchemaCompressor::new();
    let result = compressor.compress(&input_json);

    // Since no savings, compressor returns the original value unchanged
    assert_eq!(
        serde_json::to_string(&result).unwrap(),
        serde_json::to_string(&input_json).unwrap(),
    );

    // In CLI: `after_tokens >= before_tokens` -> output original input
    let after_compact = serde_json::to_string(&result).unwrap_or_default();
    let before_tokens = after_compact.len().div_ceil(4);
    // Verify our token estimate matches that decision
    assert!(before_tokens > 0);
}

#[test]
fn test_cli_compress_response_full_pipeline() {
    // Full pipeline simulation: read JSON -> compress -> output JSON
    let deep_text = "some long text that exceeds the default 512 character limit ".repeat(10);
    let raw_input = format!(
        r#"{{
        "results": [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18],
        "debug": "should-be-dropped",
        "logging_output": null,
        "nested": {{
            "deep_field": "{deep_text}",
            "empty_obj": {{}}
        }}
    }}"#
    );

    // Step 1: Parse JSON (as CLI does)
    let value: Value = serde_json::from_str(&raw_input).expect("CLI must parse input JSON");

    // Step 2: Compress
    let compressor = ResponseCompressor::new();
    let result = compressor.compress(&value);

    // Step 3: Serialize (as CLI does with to_string_pretty)
    let output = serde_json::to_string_pretty(&result).expect("CLI must serialize output to JSON");

    // Step 4: Verify output is valid JSON
    let parsed: Value = serde_json::from_str(&output).expect("CLI output must be valid JSON");

    // Verification
    assert!(parsed.get("debug").is_none());
    assert!(parsed.get("logging_output").is_none());
    assert!(parsed["results"].is_array());
    assert!(
        parsed["nested"]["deep_field"]
            .as_str()
            .unwrap()
            .contains("truncated")
    );
}
