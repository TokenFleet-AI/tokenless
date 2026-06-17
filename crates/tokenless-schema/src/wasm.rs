//! WASM bindings for `tokenless-schema`.
//!
//! This module provides a small `wasm-bindgen` wrapper around the crate's core
//! compression entry points so browser and Node.js consumers can call them with
//! JSON strings.

use serde_json::{Value, json};
use wasm_bindgen::prelude::*;

use crate::{SchemaCompressor, compress_auto};

/// Compress a tool schema JSON string and return the compressed JSON string.
///
/// Returns a JavaScript exception when the input is not valid JSON.
///
/// # Errors
///
/// Returns an error when `input` is not valid JSON or the output cannot be
/// serialized back to a string.
#[wasm_bindgen]
pub fn compress_schema_json(input: &str) -> Result<String, JsValue> {
    let value: Value = serde_json::from_str(input)
        .map_err(|error| JsValue::from_str(&format!("invalid JSON schema input: {error}")))?;
    let compressed = SchemaCompressor::new().compress(&value);
    serde_json::to_string(&compressed)
        .map_err(|error| JsValue::from_str(&format!("failed to serialize schema output: {error}")))
}

/// Auto-compress a JSON payload and return the selected strategy together with
/// the compressed output.
///
/// The result is returned as a JSON object string so JavaScript callers can
/// parse it without needing custom bindings.
///
/// # Errors
///
/// Returns an error when `input` is not valid JSON or the result cannot be
/// serialized back to a string.
#[wasm_bindgen]
pub fn compress_json_auto(input: &str) -> Result<String, JsValue> {
    let value: Value = serde_json::from_str(input)
        .map_err(|error| JsValue::from_str(&format!("invalid JSON payload: {error}")))?;
    let (strategy, output) = compress_auto(&value, input);
    serde_json::to_string(&json!({
        "strategy": crate::strategy_name(&strategy),
        "output": output,
    }))
    .map_err(|error| {
        JsValue::from_str(&format!(
            "failed to serialize auto-compression output: {error}"
        ))
    })
}
