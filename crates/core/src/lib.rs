#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![warn(missing_debug_implementations)]

//! Shared core types for tokenless.
//!
//! This crate provides foundational types used across the workspace:
//! - [`CoreError`] — unified error type
//! - [`Config`] — validated configuration
//! - [`SafePath`] — path traversal-resistant relative path

mod config;
mod error;
mod safe_path;

pub use config::Config;
pub use error::{CoreError, Result};
pub use safe_path::SafePath;
