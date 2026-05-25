//! Tokenless schema and response compression library.
//!
//! Provides [`SchemaCompressor`] for compressing `OpenAI Function Calling` tool
//! definitions and [`ResponseCompressor`] for compressing JSON API responses.

#![allow(clippy::unwrap_used, clippy::expect_used)]

/// JSON response compression — removes debug fields, truncates strings / arrays.
pub mod response_compressor;

/// `OpenAI Function Calling` schema compression — truncates descriptions, removes titles/examples.
pub mod schema_compressor;

pub use response_compressor::ResponseCompressor;
pub use schema_compressor::SchemaCompressor;
