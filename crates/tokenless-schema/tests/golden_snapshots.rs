//! Golden file / snapshot tests for core compressors.
//!
//! Uses the [`insta`] crate to capture and review compression output.
//! Each test reads a JSON fixture from `tests/fixtures/`, runs the
//! appropriate compressor, and asserts the result against a stored
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::missing_panics_doc,
    clippy::disallowed_methods
)]
//! snapshot file.  When fixtures or compressor logic change the
//! snapshots can be reviewed with `cargo insta review`.

use serde_json::Value;
use std::fs;
use std::path::PathBuf;

/// Resolve a fixture path relative to this test file's directory.
fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

/// Load and parse a JSON fixture file.
fn load_fixture(name: &str) -> Value {
    let path = fixture_path(name);
    let raw = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Failed to read fixture {}: {}", path.display(), e));
    serde_json::from_str(&raw)
        .unwrap_or_else(|e| panic!("Failed to parse fixture {}: {}", path.display(), e))
}

// ── SchemaCompressor snapshots ────────────────────────────────────────

#[test]
fn golden_schema_weather_function() {
    let input = load_fixture("schema_weather.json");
    let compressor = tokenless_schema::SchemaCompressor::new();
    let result = compressor.compress(&input);
    insta::assert_json_snapshot!(result);
}

#[test]
fn golden_schema_bare_with_defs() {
    let input = load_fixture("schema_bare.json");
    let compressor = tokenless_schema::SchemaCompressor::new();
    let result = compressor.compress(&input);
    insta::assert_json_snapshot!(result);
}

#[test]
fn golden_schema_with_enum_truncation() {
    let input = load_fixture("schema_bare.json");
    let compressor = tokenless_schema::SchemaCompressor::new().with_max_enum_items(3);
    let result = compressor.compress(&input);
    insta::assert_json_snapshot!(result);
}

#[test]
fn golden_schema_with_token_limits() {
    let input = load_fixture("schema_weather.json");
    let compressor = tokenless_schema::SchemaCompressor::new()
        .with_func_desc_max_tokens(20)
        .with_param_desc_max_tokens(10);
    let result = compressor.compress(&input);
    insta::assert_json_snapshot!(result);
}

// ── ResponseCompressor snapshots ──────────────────────────────────────

#[test]
fn golden_response_github_api() {
    let input = load_fixture("response_github_api.json");
    let compressor = tokenless_schema::ResponseCompressor::new();
    let result = compressor.compress(&input);
    insta::assert_json_snapshot!(result);
}

#[test]
fn golden_response_large_array() {
    let input = load_fixture("response_large_array.json");
    let compressor = tokenless_schema::ResponseCompressor::new();
    let result = compressor.compress(&input);
    insta::assert_json_snapshot!(result);
}

#[test]
fn golden_response_aggressive() {
    let input = load_fixture("response_large_array.json");
    let compressor = tokenless_schema::ResponseCompressor::new()
        .with_truncate_strings_at(40)
        .with_truncate_arrays_at(5)
        .with_max_keys_per_object(3);
    let result = compressor.compress(&input);
    insta::assert_json_snapshot!(result);
}

// ── format_router snapshots ───────────────────────────────────────────

/// Helper: build a structured snapshot value for `format_router` output.
fn router_snapshot(strategy: &tokenless_schema::Strategy, output: &str) -> Value {
    serde_json::json!({
        "strategy": tokenless_schema::strategy_name(strategy),
        "output": output,
    })
}

#[test]
fn golden_format_uniform_array_hrv() {
    let input = load_fixture("format_uniform_array.json");
    let input_str = serde_json::to_string(&input).unwrap();
    let (strategy, output) = tokenless_schema::compress_auto(&input, &input_str);
    insta::assert_json_snapshot!(router_snapshot(&strategy, &output));
}

#[test]
fn golden_format_schema_enum_enhanced() {
    let input = load_fixture("format_schema_enum.json");
    let input_str = serde_json::to_string(&input).unwrap();
    let (strategy, output) = tokenless_schema::compress_auto(&input, &input_str);
    insta::assert_json_snapshot!(router_snapshot(&strategy, &output));
}

#[test]
fn golden_format_deep_chain_enhanced() {
    let input = load_fixture("format_deep_chain.json");
    let input_str = serde_json::to_string(&input).unwrap();
    let (strategy, output) = tokenless_schema::compress_auto(&input, &input_str);
    insta::assert_json_snapshot!(router_snapshot(&strategy, &output));
}

#[test]
fn golden_format_irregular_cjson() {
    let input = load_fixture("format_irregular.json");
    let input_str = serde_json::to_string(&input).unwrap();
    let (strategy, output) = tokenless_schema::compress_auto(&input, &input_str);
    insta::assert_json_snapshot!(router_snapshot(&strategy, &output));
}

#[test]
fn golden_format_small_passthrough() {
    let input = load_fixture("format_small.json");
    let input_str = serde_json::to_string(&input).unwrap();
    let (strategy, output) = tokenless_schema::compress_auto(&input, &input_str);
    insta::assert_json_snapshot!(router_snapshot(&strategy, &output));
}
