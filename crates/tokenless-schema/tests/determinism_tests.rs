//! Determinism and idempotency tests for tokenless-schema compressors.
//!
//! Verifies that every compressor / encoder produces byte-identical output
#![allow(clippy::unwrap_used, clippy::expect_used)]
//! across 100 invocations with the same input, and that `Value`-returning
//! compressors are idempotent (i.e. `compress(compress(x)) == compress(x)`).

use serde_json::Value;
use serde_json::json;
use tokenless_schema::{
    ResponseCompressor, SchemaCompressor, Strategy, compress_auto,
    encoding::{encode_cjson, encode_enhanced, encode_toon_hrv},
    select_strategy, strategy_name,
};

// ── Helper ──────────────────────────────────────────────────────────────

/// Build a string of exactly `n` characters (ASCII 'x').
fn long_str(n: usize) -> String {
    "x".repeat(n)
}

/// Serialize to a canonical JSON string for comparison.
fn to_string(value: &Value) -> String {
    serde_json::to_string(value).expect("serialization")
}

// ── SchemaCompressor ────────────────────────────────────────────────────

fn schema_test_value() -> Value {
    json!({
        "function": {
            "name": "test_func",
            "title": "Test Function",
            "description": "This is a long function description that will be truncated by the compressor because it exceeds the maximum allowed length threshold for testing purposes.",
            "parameters": {
                "type": "object",
                "title": "Parameters Title",
                "properties": {
                    "name": {
                        "type": "string",
                        "title": "Name Title",
                        "description": "User's full name field with a long description that exceeds the 160 char parameter limit so truncation is exercised.",
                        "examples": ["Alice", "Bob"]
                    },
                    "role": {
                        "type": "string",
                        "title": "Role Title",
                        "enum": ["admin", "user", "guest", "moderator", "viewer"],
                        "description": "Role assignment for access control."
                    },
                    "count": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": 100,
                        "title": "Count Title",
                        "examples": [42]
                    }
                },
                "required": ["name"]
            }
        }
    })
}

#[test]
fn test_schema_compressor_determinism_100x() {
    let compressor = SchemaCompressor::new().with_max_enum_items(10);
    let value = schema_test_value();
    let first = to_string(&compressor.compress(&value));
    for _ in 0..99 {
        assert_eq!(to_string(&compressor.compress(&value)), first);
    }
}

#[test]
fn test_schema_compressor_determinism_with_diff_configs() {
    // Different configurations should still be deterministic.
    let configs: Vec<SchemaCompressor> = vec![
        SchemaCompressor::new(),
        SchemaCompressor::new().with_drop_titles(false),
        SchemaCompressor::new().with_drop_examples(false),
        SchemaCompressor::new().with_drop_markdown(false),
        SchemaCompressor::new().with_max_enum_items(5),
        SchemaCompressor::new().with_max_enum_items(0),
        SchemaCompressor::new().with_func_desc_max_len(50),
        SchemaCompressor::new().with_param_desc_max_len(32),
        SchemaCompressor::new()
            .with_drop_titles(false)
            .with_drop_examples(false)
            .with_max_enum_items(3),
    ];
    let value = schema_test_value();
    for compressor in &configs {
        let first = to_string(&compressor.compress(&value));
        for _ in 0..99 {
            assert_eq!(
                to_string(&compressor.compress(&value)),
                first,
                "non-deterministic output for SchemaCompressor config"
            );
        }
    }
}

#[test]
fn test_schema_compressor_idempotency() {
    let compressor = SchemaCompressor::new().with_max_enum_items(10);
    let value = schema_test_value();
    let once = compressor.compress(&value);
    let twice = compressor.compress(&once);
    assert_eq!(
        to_string(&once),
        to_string(&twice),
        "SchemaCompressor::compress should be idempotent"
    );
}

// ── ResponseCompressor ──────────────────────────────────────────────────

fn response_test_value() -> Value {
    json!({
        "data": {
            "users": [
                {"id": 1, "name": "Alice", "role": "admin"},
                {"id": 2, "name": "Bob", "role": "user"}
            ],
            "total": 42,
            "page": 1
        },
        "debug": "should-be-removed",
        "trace": "should-also-be-removed",
        "long_text": long_str(600),
        "null_field": null,
        "empty_string": "",
        "empty_array": [],
        "empty_object": {},
        "items": [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20]
    })
}

#[test]
fn test_response_compressor_determinism_100x() {
    let compressor = ResponseCompressor::new()
        .with_truncate_strings_at(20)
        .with_truncate_arrays_at(5)
        .with_max_keys_per_object(10);
    let value = response_test_value();
    let first = to_string(&compressor.compress(&value));
    for _ in 0..99 {
        assert_eq!(to_string(&compressor.compress(&value)), first);
    }
}

#[test]
fn test_response_compressor_determinism_with_diff_configs() {
    let configs: Vec<ResponseCompressor> = vec![
        ResponseCompressor::new(),
        ResponseCompressor::new().with_truncate_strings_at(16),
        ResponseCompressor::new().with_truncate_arrays_at(3),
        ResponseCompressor::new().with_drop_nulls(false),
        ResponseCompressor::new().with_drop_empty_fields(false),
        ResponseCompressor::new().with_max_depth(3),
        ResponseCompressor::new().with_add_truncation_marker(false),
        ResponseCompressor::new().with_max_keys_per_object(3),
        ResponseCompressor::new().with_drop_field("data"),
        ResponseCompressor::new()
            .with_truncate_strings_at(10)
            .with_truncate_arrays_at(2)
            .with_max_keys_per_object(5)
            .with_drop_field("custom_debug"),
    ];
    let value = response_test_value();
    for compressor in &configs {
        let first = to_string(&compressor.compress(&value));
        for _ in 0..99 {
            assert_eq!(
                to_string(&compressor.compress(&value)),
                first,
                "non-deterministic output for ResponseCompressor config"
            );
        }
    }
}

#[test]
fn test_response_compressor_idempotency_without_markers() {
    // Without truncation markers, both string and array truncation are idempotent
    // because already-truncated content stays below the limits.
    let compressor = ResponseCompressor::new()
        .with_truncate_strings_at(32)
        .with_truncate_arrays_at(5)
        .with_add_truncation_marker(false);
    let value = response_test_value();
    let once = compressor.compress(&value);
    let twice = compressor.compress(&once);
    assert_eq!(
        to_string(&once),
        to_string(&twice),
        "without markers, ResponseCompressor should be idempotent"
    );
}

#[test]
fn test_response_compressor_idempotency_no_truncation_needed() {
    // When the test value stays within all limits, field-level operations
    // (drop nulls, drop empty fields, drop debug fields) are idempotent.
    let compressor = ResponseCompressor::new()
        .with_truncate_strings_at(10_000)
        .with_truncate_arrays_at(10_000)
        .with_max_keys_per_object(10_000);
    let value = response_test_value();
    let once = compressor.compress(&value);
    let twice = compressor.compress(&once);
    assert_eq!(
        to_string(&once),
        to_string(&twice),
        "without triggering truncation, ResponseCompressor should be idempotent"
    );
}

/// NOTE: When truncation markers are enabled (`add_truncation_marker(true)`, the default),
/// `ResponseCompressor::compress` is **not** strictly idempotent for arrays.  The truncation
/// sentinel string (e.g. `"<... up to N more items truncated>"`) itself counts as an array
/// item, which causes cascading truncation on the second pass.  String truncation markers
/// fortuitously avoid this because the marker is appended beyond the truncation cut-point,
/// but this behaviour is not guaranteed and should not be relied upon.
#[test]
fn test_response_compressor_idempotency_with_markers_cascading_known() {
    let compressor = ResponseCompressor::new()
        .with_truncate_strings_at(32)
        .with_truncate_arrays_at(5);
    let value = response_test_value();
    let once = compressor.compress(&value);
    let twice = compressor.compress(&once);

    // With markers enabled the output may differ (cascading truncation is expected),
    // but both results must be deterministic themselves.
    let once_again = compressor.compress(&value);
    let twice_again = compressor.compress(&once);
    assert_eq!(to_string(&once), to_string(&once_again));
    assert_eq!(to_string(&twice), to_string(&twice_again));
}

// ── CJSON Compact ───────────────────────────────────────────────────────

fn cjson_test_value() -> Value {
    json!({
        "user": {
            "name": "Alice",
            "active": true,
            "score": null,
            "tags": ["admin", "verified"]
        },
        "count": 42
    })
}

#[test]
fn test_cjson_compact_determinism_100x() {
    let value = cjson_test_value();
    let first = encode_cjson(&value);
    for _ in 0..99 {
        assert_eq!(encode_cjson(&value), first);
    }
}

#[test]
fn test_cjson_compact_determinism_edge_cases() {
    let values: Vec<Value> = vec![
        json!(null),
        json!(true),
        json!(false),
        json!(42),
        json!("hello_world"),
        json!("hello world"),
        json!([]),
        json!({}),
        json!([1, "two", true, null, {"key": "val"}]),
    ];
    for value in &values {
        let first = encode_cjson(value);
        for _ in 0..99 {
            assert_eq!(
                encode_cjson(value),
                first,
                "cjson non-deterministic for {value}"
            );
        }
    }
}

// ── Enhanced TOON ───────────────────────────────────────────────────────

fn enhanced_toon_test_value() -> Value {
    json!({
        "name": {"type": "string", "description": "User's full name for display"},
        "role": {"type": "string", "enum": ["admin", "user", "guest"]},
        "age": {"type": "integer", "minimum": 1, "maximum": 150},
        "code": {"type": "string", "pattern": "^[a-z]{3,10}$"}
    })
}

#[test]
fn test_enhanced_toon_determinism_100x() {
    let value = enhanced_toon_test_value();
    let first = encode_enhanced(&value, 0);
    for _ in 0..99 {
        assert_eq!(encode_enhanced(&value, 0), first);
    }
}

#[test]
fn test_enhanced_toon_determinism_with_nested_and_chain() {
    let values: Vec<(Value, &str)> = vec![
        (
            json!({"user": {"name": "Alice", "address": {"city": "NYC"}}}),
            "nested object",
        ),
        (json!({"a": {"b": {"c": {"d": {"e": 1}}}}}), "deep chain"),
        (json!({"items": [{"id": 1}, {"id": 2}]}), "array of objects"),
        (json!({"list": []}), "empty array"),
        (json!({}), "empty object"),
        (json!(true), "boolean"),
        (json!(null), "null"),
        (json!(std::f64::consts::PI), "float"),
    ];
    for (value, label) in &values {
        let first = encode_enhanced(value, 0);
        for _ in 0..99 {
            assert_eq!(
                encode_enhanced(value, 0),
                first,
                "enhanced_toon non-deterministic for {label}"
            );
        }
    }
}

// ── TOON HRV ────────────────────────────────────────────────────────────

fn toon_hrv_test_value() -> Value {
    json!([
        {"id": 1, "name": "Alice", "role": "admin"},
        {"id": 2, "name": "Bob", "role": "user"},
        {"id": 3, "name": "Carol", "role": "user"},
        {"id": 4, "name": "Dave", "role": "admin"},
        {"id": 5, "name": "Eve", "role": "user"},
        {"id": 6, "name": "Frank", "role": "user"},
        {"id": 7, "name": "Grace", "role": "admin"},
        {"id": 8, "name": "Hank", "role": "user"},
        {"id": 9, "name": "Ivy", "role": "admin"},
        {"id": 10, "name": "Jack", "role": "user"}
    ])
}

#[test]
fn test_toon_hrv_determinism_100x() {
    let value = toon_hrv_test_value();
    let first = encode_toon_hrv(&value);
    for _ in 0..99 {
        assert_eq!(encode_toon_hrv(&value), first);
    }
}

#[test]
fn test_toon_hrv_determinism_edge_cases() {
    // Small array (< 5 items) → falls through to enhanced TOON.
    let small = json!([{"a": 1}, {"a": 2}]);
    let first = encode_toon_hrv(&small);
    for _ in 0..99 {
        assert_eq!(encode_toon_hrv(&small), first);
    }

    // Empty array → "[]".
    let empty = json!([]);
    let first = encode_toon_hrv(&empty);
    for _ in 0..99 {
        assert_eq!(encode_toon_hrv(&empty), first);
    }

    // Mixed keys → falls through.
    let mixed = json!([
        {"a": 1, "b": 2},
        {"a": 3, "b": 4},
        {"a": 5, "b": 6},
        {"a": 7, "b": 8},
        {"c": 9}
    ]);
    let first = encode_toon_hrv(&mixed);
    for _ in 0..99 {
        assert_eq!(encode_toon_hrv(&mixed), first);
    }
}

// ── format_router: compress_auto ────────────────────────────────────────

fn toon_hrv_input_for_router() -> (Value, String) {
    let value = json!([
        {"id": 1, "name": "Alice", "role": "admin", "dept": "Engineering"},
        {"id": 2, "name": "Bob", "role": "user", "dept": "Marketing"},
        {"id": 3, "name": "Carol", "role": "user", "dept": "Sales"},
        {"id": 4, "name": "Dave", "role": "admin", "dept": "Engineering"},
        {"id": 5, "name": "Eve", "role": "user", "dept": "Marketing"},
        {"id": 6, "name": "Frank", "role": "user", "dept": "Sales"},
        {"id": 7, "name": "Grace", "role": "admin", "dept": "Engineering"}
    ]);
    let s = to_string(&value);
    (value, s)
}

fn enhanced_toon_input_for_router() -> (Value, String) {
    let value = json!({
        "properties": {
            "name": {
                "type": "string",
                "description": "The full name of the user — this extra text ensures the total character count exceeds 200 for the format router to actually select a strategy.",
                "enum": ["admin", "user", "guest", "moderator", "superadmin", "viewer", "editor"]
            }
        }
    });
    let s = to_string(&value);
    (value, s)
}

fn cjson_compact_input_for_router() -> (Value, String) {
    let value = json!({
        "user": {
            "name": "Alice",
            "address": {
                "city": "New York",
                "zip": "10001",
                "country": "USA",
                "details": "Extra padding text to increase the total character count beyond 200 so that the format router selects a non-passthrough strategy and exercises the encoding code path."
            }
        },
        "count": 42,
        "active": true,
        "tags": ["tag1", "tag2", "tag3", "tag4", "tag5", "tag6", "tag7"]
    });
    let s = to_string(&value);
    (value, s)
}

fn compressor_only_input() -> (Value, String) {
    let value = json!({"key": "value"});
    let s = to_string(&value);
    (value, s)
}

#[test]
fn test_compress_auto_determinism_all_strategies() {
    let inputs: Vec<(&str, Value, String)> = vec![
        (
            "ToonHrv",
            toon_hrv_input_for_router().0,
            toon_hrv_input_for_router().1,
        ),
        (
            "EnhancedToon",
            enhanced_toon_input_for_router().0,
            enhanced_toon_input_for_router().1,
        ),
        (
            "CjsonCompact",
            cjson_compact_input_for_router().0,
            cjson_compact_input_for_router().1,
        ),
        (
            "CompressorOnly",
            compressor_only_input().0,
            compressor_only_input().1,
        ),
    ];

    for (label, value, input_str) in &inputs {
        let (first_strategy, first_output) = compress_auto(value, input_str);
        for _ in 0..99 {
            let (strategy, output) = compress_auto(value, input_str);
            assert_eq!(
                strategy, first_strategy,
                "compress_auto strategy changed across invocations for {label}"
            );
            assert_eq!(
                output, first_output,
                "compress_auto output changed across invocations for {label}"
            );
        }
    }
}

#[test]
fn test_compress_auto_selects_expected_strategies() {
    // Verify that the strategy selection is as expected for each input shape.
    let (v_hrv, s_hrv) = toon_hrv_input_for_router();
    let (strategy, _) = compress_auto(&v_hrv, &s_hrv);
    assert_eq!(
        strategy,
        Strategy::ToonHrv,
        "uniform array should route to ToonHrv"
    );

    let (v_enh, s_enh) = enhanced_toon_input_for_router();
    let (strategy, _) = compress_auto(&v_enh, &s_enh);
    // EnhancedToon should be selected because has_enums is true and char_count >= 200.
    assert_eq!(
        strategy,
        Strategy::EnhancedToon,
        "schema with enums should route to EnhancedToon"
    );

    let (v_cjson, s_cjson) = cjson_compact_input_for_router();
    let (strategy, _) = compress_auto(&v_cjson, &s_cjson);
    assert_eq!(
        strategy,
        Strategy::CjsonCompact,
        "irregular object should route to CjsonCompact"
    );

    let (v_pass, s_pass) = compressor_only_input();
    let (strategy, _) = compress_auto(&v_pass, &s_pass);
    assert_eq!(
        strategy,
        Strategy::CompressorOnly,
        "small input should route to CompressorOnly"
    );
}

// ── select_strategy ─────────────────────────────────────────────────────

#[test]
fn test_select_strategy_determinism() {
    use tokenless_schema::shape_analyzer::{JsonShape, TopType};

    let shapes: Vec<JsonShape> = vec![
        // CompressorOnly
        JsonShape {
            top_level: TopType::Object,
            key_count: 2,
            item_count: 0,
            max_depth: 1,
            is_uniform_array: false,
            has_enums: false,
            has_constraints: false,
            max_chain_depth: 0,
            char_count: 50,
        },
        // ToonHrv
        JsonShape {
            top_level: TopType::Array,
            key_count: 0,
            item_count: 10,
            max_depth: 2,
            is_uniform_array: true,
            has_enums: false,
            has_constraints: false,
            max_chain_depth: 0,
            char_count: 500,
        },
        // EnhancedToon (enums)
        JsonShape {
            top_level: TopType::Object,
            key_count: 3,
            item_count: 0,
            max_depth: 3,
            is_uniform_array: false,
            has_enums: true,
            has_constraints: false,
            max_chain_depth: 1,
            char_count: 500,
        },
        // EnhancedToon (constraints)
        JsonShape {
            top_level: TopType::Object,
            key_count: 3,
            item_count: 0,
            max_depth: 3,
            is_uniform_array: false,
            has_enums: false,
            has_constraints: true,
            max_chain_depth: 1,
            char_count: 500,
        },
        // EnhancedToon (deep chain)
        JsonShape {
            top_level: TopType::Object,
            key_count: 1,
            item_count: 0,
            max_depth: 6,
            is_uniform_array: false,
            has_enums: false,
            has_constraints: false,
            max_chain_depth: 5,
            char_count: 500,
        },
        // CjsonCompact (default)
        JsonShape {
            top_level: TopType::Object,
            key_count: 5,
            item_count: 0,
            max_depth: 2,
            is_uniform_array: false,
            has_enums: false,
            has_constraints: false,
            max_chain_depth: 1,
            char_count: 500,
        },
    ];

    for shape in &shapes {
        let first = select_strategy(shape);
        let name = strategy_name(&first);
        for _ in 0..99 {
            assert_eq!(
                select_strategy(shape),
                first,
                "select_strategy non-deterministic for shape producing {name}"
            );
        }
    }
}

// ── strategy_name ───────────────────────────────────────────────────────

#[test]
fn test_strategy_name_determinism() {
    let strategies = [
        Strategy::ToonHrv,
        Strategy::EnhancedToon,
        Strategy::CjsonCompact,
        Strategy::CompressorOnly,
    ];
    for strategy in &strategies {
        let first = strategy_name(strategy);
        for _ in 0..99 {
            assert_eq!(strategy_name(strategy), first);
        }
    }
}
