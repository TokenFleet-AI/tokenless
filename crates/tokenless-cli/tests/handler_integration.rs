//! Integration tests for handler-level compression logic.
//!
//! tokenless-cli is a binary crate (no lib.rs), so handler functions defined
//! in `src/handlers.rs` are not directly importable from integration tests.
//! These tests use the `tokenless_schema` public API to verify the same
//! compression logic that handlers invoke.
//!
//! Once a `lib.rs` is extracted, these tests should be updated to import
//! handler functions directly.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::approx_constant)]

use serde_json::{Value, json};
use tokenless_schema::{ResponseCompressor, SchemaCompressor};

// ---------------------------------------------------------------------------
// compress_schema handler logic
// ---------------------------------------------------------------------------

#[test]
fn test_handler_compress_schema_basic() {
    // Simulates: tokenless compress-schema < schema.json
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

    // Same logic as handlers::compress_schema
    let compressor = SchemaCompressor::new();
    let result = compressor.compress(&input);
    let output = serde_json::to_string_pretty(&result).expect("must serialize to pretty JSON");

    // Verify valid JSON and function name preserved
    let parsed: Value = serde_json::from_str(&output).expect("output must be valid JSON");
    assert_eq!(parsed["function"]["name"], "get_weather");
}

#[test]
fn test_handler_compress_schema_batch() {
    // Simulates: tokenless compress-schema --batch < schemas.json
    let input = json!([
        {"function": {"name": "fn_a", "description": "Function A", "parameters": {"type": "object", "properties": {}}}},
        {"function": {"name": "fn_b", "description": "Function B", "parameters": {"type": "object", "properties": {}}}}
    ]);

    let compressor = SchemaCompressor::new();
    let arr = input.as_array().unwrap();
    let results: Vec<Value> = arr.iter().map(|item| compressor.compress(item)).collect();
    let output =
        serde_json::to_string_pretty(&results).expect("batch output must serialize to pretty JSON");

    let parsed: Value = serde_json::from_str(&output).expect("batch output must be valid JSON");
    assert!(parsed.is_array());
    assert_eq!(parsed.as_array().unwrap().len(), 2);
    assert_eq!(parsed[0]["function"]["name"], "fn_a");
    assert_eq!(parsed[1]["function"]["name"], "fn_b");
}

#[test]
fn test_handler_compress_schema_cache_skip_small() {
    // Inputs under 64 bytes are not cached by the CLI handler.
    // Verify that the handler's compress logic still works.
    let input = json!({"function": {"name": "tiny", "description": "tiny desc", "parameters": {"type": "object"}}});

    let compressor = SchemaCompressor::new();
    let result = compressor.compress(&input);
    let output = serde_json::to_string_pretty(&result).expect("must serialize");
    let parsed: Value = serde_json::from_str(&output).expect("valid JSON");
    assert!(parsed["function"]["name"].as_str().unwrap_or("") == "tiny");
}

// ---------------------------------------------------------------------------
// compress_response handler logic
// ---------------------------------------------------------------------------

#[test]
fn test_handler_compress_response_basic() {
    // Simulates: tokenless compress-response < response.json
    let input = json!({
        "results": [1, 2, 3],
        "debug": "should-be-dropped",
        "trace": "also-dropped",
        "data": "important",
        "null_field": null,
        "empty_array": []
    });

    let compressor = ResponseCompressor::new();
    let result = compressor.compress(&input);
    let output = serde_json::to_string_pretty(&result).expect("must serialize");

    // Verify debug/trace/null/empty are dropped
    let parsed: Value = serde_json::from_str(&output).expect("valid JSON");
    assert!(parsed.get("debug").is_none(), "debug must be dropped");
    assert!(parsed.get("trace").is_none(), "trace must be dropped");
    assert!(parsed.get("null_field").is_none(), "null must be dropped");
    assert!(
        parsed.get("empty_array").is_none(),
        "empty array must be dropped"
    );
    assert_eq!(parsed["data"], "important");
}

#[test]
fn test_handler_compress_response_long_string_truncation() {
    let long_str = "x".repeat(600);
    let input = json!({"text": long_str});

    let compressor = ResponseCompressor::new();
    let result = compressor.compress(&input);
    let output = serde_json::to_string_pretty(&result).expect("must serialize");

    let parsed: Value = serde_json::from_str(&output).expect("valid JSON");
    let text = parsed["text"].as_str().unwrap();
    assert!(
        text.contains("truncated"),
        "long string must be truncated: {text}"
    );
    assert!(
        text.len() < long_str.len(),
        "truncated string must be shorter than original"
    );
}

// ---------------------------------------------------------------------------
// compress_auto handler logic (strategy selection)
// ---------------------------------------------------------------------------

#[test]
fn test_handler_compress_auto_with_schema() {
    // compress_auto selects the best strategy
    let input = json!({
        "function": {
            "name": "search_docs",
            "description": "Search documentation. ",
            "parameters": {
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "title": "Query",
                        "description": "The search query string"
                    }
                }
            }
        }
    });

    let input_str = serde_json::to_string_pretty(&input).unwrap();
    let (strategy, _compressed) = tokenless_schema::compress_auto(&input, &input_str);

    // A strategy should be selected
    assert!(
        !tokenless_schema::strategy_name(&strategy).is_empty(),
        "strategy should be non-empty"
    );
}

// ---------------------------------------------------------------------------
// compress_toon / decompress_toon roundtrip
// ---------------------------------------------------------------------------

#[test]
fn test_handler_toon_roundtrip() {
    let input = json!({
        "items": [
            {"id": 1, "name": "item_one"},
            {"id": 2, "name": "item_two"}
        ],
        "total": 2
    });

    // Encode with toon
    let encoded =
        toon_format::encode_default(&input).expect("toon encode must succeed for simple JSON");

    // Decode back
    let decoded: serde_json::Value =
        toon_format::decode_default(&encoded).expect("toon decode must succeed for valid TOON");
    let decoded_json =
        serde_json::to_string_pretty(&decoded).expect("must serialize to pretty JSON");

    let parsed: Value = serde_json::from_str(&decoded_json).expect("must be valid JSON");
    assert_eq!(parsed["items"][0]["name"], "item_one");
    assert_eq!(parsed["total"], 2);
}

#[test]
fn test_handler_toon_encode_simple_string() {
    let input = json!("hello world");
    let encoded = toon_format::encode_default(&input).expect("toon encode string");
    let decoded: serde_json::Value =
        toon_format::decode_default(&encoded).expect("toon decode string");
    assert_eq!(decoded, input);
}

// ---------------------------------------------------------------------------
// Edge cases that handler code must handle
// ---------------------------------------------------------------------------

#[test]
fn test_handler_compress_schema_no_savings_returns_original() {
    // When compressed output is larger, handler returns original via
    // token comparison in estimate_tokens_from_bytes.
    let input = json!({
        "function": {
            "name": "tiny",
            "description": "tiny",
            "parameters": {"type": "object", "properties": {}}
        }
    });

    let compressor = SchemaCompressor::new();
    let result = compressor.compress(&input);

    // The loss check uses estimate_tokens_from_bytes on input vs output lengths.
    // If compressed output >= input, handler falls back to original.
    // Verify the compressor itself doesn't inflate.
    let input_bytes = serde_json::to_string(&input).unwrap().len();
    let output_bytes = serde_json::to_string(&result).unwrap().len();

    // For a schema this short, compression may not reduce size
    // but shouldn't significantly inflate either
    assert!(
        output_bytes <= input_bytes + 10,
        "compressor should not significantly inflate output: input={input_bytes}, output={output_bytes}"
    );
}

#[test]
fn test_handler_decompress_toon_from_valid_json() {
    // Simulates: tokenless decompress-toon < toon_output.json
    let input = json!({"a": 1, "b": [2, 3, 4]});
    let encoded = toon_format::encode_default(&input).expect("toon encode");

    let decoded: serde_json::Value = toon_format::decode_default(&encoded).expect("toon decode");
    assert_eq!(decoded, input, "roundtrip should recover original value");
}
