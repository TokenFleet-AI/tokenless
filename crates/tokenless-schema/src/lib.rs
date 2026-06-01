//! Tokenless schema and response compression library.
//!
//! Provides [`SchemaCompressor`] for compressing `OpenAI Function Calling` tool
//! definitions and [`ResponseCompressor`] for compressing JSON API responses.
//! The [`format_router`] module intelligently selects the optimal encoding
//! strategy based on JSON structural shape.

#![forbid(unsafe_code)]

/// JSON response compression — removes debug fields, truncates strings / arrays.
pub mod response_compressor;

/// `OpenAI Function Calling` schema compression — truncates descriptions, removes titles/examples.
pub mod schema_compressor;

/// JSON structure analyzer — detects shape, uniformity, and constraints.
pub mod shape_analyzer;

/// Encoding strategies for the format router (TOON HRV, Enhanced TOON, CJSON Compact).
pub mod encoding;

/// Intelligent format router — selects optimal encoding strategy based on JSON shape.
pub mod format_router;

pub use format_router::{Strategy, compress_auto, select_strategy, strategy_name};
pub use response_compressor::{CompressionProfile, ResponseCompressor};
pub use schema_compressor::SchemaCompressor;
pub use shape_analyzer::{JsonShape, TopType, analyze};
