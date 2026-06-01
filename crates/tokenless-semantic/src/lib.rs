#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![warn(missing_debug_implementations)]
#![allow(
    clippy::pedantic,
    clippy::missing_errors_doc,
    reason = "error types documented at enum level"
)]

//! Semantic-aware JSON field compression.
//!
//! # Levels
//!
//! **Level 1** (default, zero deps): keyword-based context detection with
//! compiled-in TOML profiles.  No model download required.
//!
//! **Level 2** (`onnx` feature): ONNX embedding model (`all-MiniLM-L6-v2`,
//! ~15 MB) that computes cosine similarity between field names and the
//! user's task context.  Model files are auto-downloaded on first use from
//! GitHub Releases and cached in `~/.tokenless/models/`.  Falls back to
//! Level 1 automatically when the model is unavailable.

mod rules;

#[cfg(feature = "onnx")]
mod embedder;

#[cfg(feature = "onnx")]
use std::cell::RefCell;
use std::fmt;

use rules::{FieldAction, classify_field, detect_category};
use serde_json::Value;

/// Errors that can occur during semantic compression.
#[derive(Debug, thiserror::Error)]
pub enum EmbedderError {
    /// I/O error (filesystem or network).
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Model file not found at the expected path.
    #[error("Model not found at {0}")]
    ModelNotFound(std::path::PathBuf),

    /// Tokenizer file not found at the expected path.
    #[error("Tokenizer not found at {0}")]
    TokenizerNotFound(std::path::PathBuf),

    /// Failed to load the tokenizer.
    #[error("Tokenizer load error: {0}")]
    TokenizerLoad(String),

    /// Failed to tokenize input text.
    #[error("Tokenization error: {0}")]
    Tokenize(String),

    /// ONNX Runtime error.
    #[cfg(feature = "onnx")]
    #[error("ONNX error: {0}")]
    Ort(String),

    /// Network download error.
    #[error("Download error: {0}")]
    Download(String),
}

/// Semantic-aware JSON response compressor.
///
/// Accepts a user task context string (e.g. `"今天天气怎么样"`) and
/// compresses the JSON response by dropping or truncating fields based
/// on their relevance.
pub struct SemanticCompressor {
    /// Similarity threshold for Level 2 (cosine similarity in [0, 1]).
    /// Fields below this threshold are dropped.  Default: 0.3.
    threshold: f32,
    /// Level 2 embedder. `None` until [`load_onnx`] succeeds.
    /// Wrapped in `RefCell` because ONNX `session.run()` requires `&mut self`.
    #[cfg(feature = "onnx")]
    embedder: Option<RefCell<embedder::Embedder>>,
}

impl fmt::Debug for SemanticCompressor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SemanticCompressor")
            .field("threshold", &self.threshold)
            .finish_non_exhaustive()
    }
}

impl Default for SemanticCompressor {
    fn default() -> Self {
        Self {
            threshold: 0.3,
            #[cfg(feature = "onnx")]
            embedder: None,
        }
    }
}

impl SemanticCompressor {
    /// Create a new compressor with default settings.
    ///
    /// Level 2 (ONNX) is NOT loaded — call [`load_onnx`] to enable it.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Attempt to load the ONNX embedding model.
    ///
    /// Downloads model files on first use if they are not cached in
    /// `~/.tokenless/models/`.  Returns `Ok(true)` if Level 2 is ready,
    /// `Ok(false)` if the model is unavailable (Level 1 fallback).
    ///
    /// When the `onnx` feature is disabled at compile time, always returns
    /// `Ok(false)` (Level 1 only).
    pub fn load_onnx(&mut self) -> Result<bool, EmbedderError> {
        #[cfg(feature = "onnx")]
        {
            let model_dir = model_dir();
            embedder::ensure_models(&model_dir)?;

            match embedder::Embedder::load(&model_dir) {
                Ok(e) => {
                    self.embedder = Some(RefCell::new(e));
                    tracing::info!("ONNX embedder loaded (Level 2 enabled)");
                    Ok(true)
                }
                Err(e) => {
                    tracing::warn!("Failed to load ONNX model, falling back to Level 1: {e}");
                    Ok(false)
                }
            }
        }
        #[cfg(not(feature = "onnx"))]
        {
            let _ = self;
            Ok(false)
        }
    }

    /// Compress a JSON value using semantic rules.
    ///
    /// With the `onnx` feature and a loaded embedder: uses cosine similarity
    /// between field names and the task context.  Fields below `threshold`
    /// are dropped.
    ///
    /// Without ONNX: uses keyword-based TOML rules (Level 1).
    #[must_use]
    pub fn compress(&self, value: &Value, context: &str) -> Value {
        #[cfg(feature = "onnx")]
        if let Some(ref embedder) = self.embedder
            && let Ok(ctx_embedding) = embedder.borrow_mut().embed(context)
        {
            return self.compress_with_embedding(value, &ctx_embedding, embedder);
        }

        // Level 1 fallback: rule-based classification.
        let category = detect_category(context);
        self.compress_with_rules(value, category, context)
    }

    /// Check whether a field name should be kept regardless of truncation
    /// limits, based on the user's task context.
    #[must_use]
    pub fn is_field_kept(&self, field_name: &str, context: &str) -> bool {
        let category = detect_category(context);
        matches!(classify_field(field_name, category), FieldAction::Keep)
    }

    /// Return the context category detected from the given text.
    #[must_use]
    pub fn detect_category(&self, context: &str) -> &'static str {
        detect_category(context)
    }

    // ── Level 1 internals ────────────────────────────────────────────────

    #[allow(
        clippy::only_used_in_recursion,
        reason = "parameters needed for recursive calls"
    )]
    fn compress_with_rules(&self, value: &Value, category: &str, context: &str) -> Value {
        match value {
            Value::Object(obj) => {
                let mut result = serde_json::Map::new();
                for (key, val) in obj {
                    match classify_field(key, category) {
                        FieldAction::Drop => {}
                        FieldAction::Keep | FieldAction::Truncate => {
                            let compressed_val = self.compress_with_rules(val, category, context);
                            result.insert(key.clone(), compressed_val);
                        }
                    }
                }
                Value::Object(result)
            }
            Value::Array(arr) => {
                let compressed: Vec<Value> = arr
                    .iter()
                    .map(|v| self.compress_with_rules(v, category, context))
                    .collect();
                Value::Array(compressed)
            }
            other => other.clone(),
        }
    }

    // ── Level 2 internals ────────────────────────────────────────────────

    #[cfg(feature = "onnx")]
    fn compress_with_embedding(
        &self,
        value: &Value,
        ctx_embedding: &[f32],
        embedder: &RefCell<embedder::Embedder>,
    ) -> Value {
        match value {
            Value::Object(obj) => {
                let mut result = serde_json::Map::new();
                for (key, val) in obj {
                    if let Ok(field_emb) = embedder.borrow_mut().embed(key) {
                        let sim = embedder::Embedder::cosine_similarity(ctx_embedding, &field_emb);
                        if sim < self.threshold {
                            continue; // irrelevant field → drop
                        }
                    }
                    let compressed_val = self.compress_with_embedding(val, ctx_embedding, embedder);
                    result.insert(key.clone(), compressed_val);
                }
                Value::Object(result)
            }
            Value::Array(arr) => {
                let compressed: Vec<Value> = arr
                    .iter()
                    .map(|v| self.compress_with_embedding(v, ctx_embedding, embedder))
                    .collect();
                Value::Array(compressed)
            }
            other => other.clone(),
        }
    }
}

/// Path to the model cache directory.
#[cfg(feature = "onnx")]
fn model_dir() -> std::path::PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(".tokenless")
        .join("models")
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use serde_json::json;

    use super::*;

    #[test]
    fn test_compress_weather_drops_station_id() {
        let compressor = SemanticCompressor::new();
        let value = json!({
            "temperature": 22.5,
            "wind_speed": 12.0,
            "station_id": "WX-001",
            "sensor_version": "3.1.0",
        });
        let result = compressor.compress(&value, "今天天气怎么样");
        assert!(result.get("temperature").is_some());
        assert!(result.get("wind_speed").is_some());
        assert!(result.get("station_id").is_none());
        assert!(result.get("sensor_version").is_none());
    }

    #[test]
    fn test_compress_devops_drops_uid() {
        let compressor = SemanticCompressor::new();
        let value = json!({
            "pod_status": "Running",
            "cpu_usage": 0.45,
            "uid": "abc-123-def",
            "self_link": "/api/v1/...",
        });
        let result = compressor.compress(&value, "deploy to kubernetes");
        assert!(result.get("pod_status").is_some());
        assert!(result.get("cpu_usage").is_some());
        assert!(result.get("uid").is_none());
        assert!(result.get("self_link").is_none());
    }

    #[test]
    fn test_compress_default_drops_debug() {
        let compressor = SemanticCompressor::new();
        let value = json!({
            "name": "Alice",
            "age": 30,
            "debug": "some debug info",
            "trace": "trace data",
        });
        let result = compressor.compress(&value, "hello");
        assert!(result.get("name").is_some());
        assert!(result.get("age").is_some());
        assert!(result.get("debug").is_none());
        assert!(result.get("trace").is_none());
    }

    #[test]
    fn test_compress_nested_object() {
        let compressor = SemanticCompressor::new();
        let value = json!({
            "data": {
                "temperature": 22.5,
                "station_id": "WX-001",
                "nested": {
                    "wind_speed": 12.0,
                    "calibration_date": "2025-01-01",
                }
            }
        });
        let result = compressor.compress(&value, "天气");
        let data = &result["data"];
        assert!(data["temperature"].is_f64());
        assert!(data.get("station_id").is_none());
        let nested = &data["nested"];
        assert!(nested["wind_speed"].is_f64());
        assert!(nested.get("calibration_date").is_none());
    }

    #[test]
    fn test_compress_array_of_objects() {
        let compressor = SemanticCompressor::new();
        let value = json!([
            {"temperature": 22.5, "station_id": "A"},
            {"temperature": 18.0, "station_id": "B"},
        ]);
        let result = compressor.compress(&value, "天气");
        let arr = result.as_array().unwrap();
        assert_eq!(arr.len(), 2);
        assert!(arr[0].get("station_id").is_none());
        assert!(arr[1].get("station_id").is_none());
    }

    #[test]
    fn test_is_field_kept() {
        let compressor = SemanticCompressor::new();
        assert!(compressor.is_field_kept("temperature", "天气怎么样"));
        assert!(!compressor.is_field_kept("station_id", "天气怎么样"));
    }

    #[test]
    fn test_detect_category_public() {
        let compressor = SemanticCompressor::new();
        assert_eq!(compressor.detect_category("天气"), "weather");
        assert_eq!(compressor.detect_category("unknown"), "default");
    }
}
