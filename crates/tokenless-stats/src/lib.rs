//! Tokenless Statistics Library
//!
//! Tracks compression metrics (characters, tokens, text content)
//! for schema compression, response compression, and command rewriting.

#![forbid(unsafe_code)]
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::collapsible_if,
    clippy::disallowed_methods,
    clippy::disallowed_types,
    clippy::doc_markdown,
    clippy::format_push_string,
    clippy::items_after_statements,
    clippy::map_unwrap_or,
    clippy::redundant_closure_for_method_calls,
    clippy::result_map_or_into_option,
    clippy::similar_names,
    clippy::single_match_else
)]

pub mod compress_log;
pub mod config;
pub mod query;
pub mod record;
pub mod recorder;
pub mod tokenizer;

#[doc(inline)]
pub use config::TokenlessConfig;
#[doc(inline)]
pub use query::parse_time_range;
#[doc(inline)]
pub use record::{OperationType, StatsRecord};
#[doc(inline)]
pub use recorder::{
    DbInfo, RedactedText, RedactionOutcome, StatsError, StatsRecorder, StatsResult, StatsSummary,
    sanitize_stats_text,
};
#[doc(inline)]
pub use tokenizer::{estimate_tokens, estimate_tokens_cjk_aware, estimate_tokens_from_bytes};
