//! Tokenless Statistics Library
//!
//! Tracks compression metrics (characters, tokens, text content)
//! for schema compression, response compression, and command rewriting.

#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::collapsible_if,
    clippy::doc_markdown,
    clippy::expect_used,
    clippy::format_push_string,
    clippy::items_after_statements,
    clippy::map_unwrap_or,
    clippy::redundant_closure_for_method_calls,
    clippy::result_map_or_into_option,
    clippy::similar_names,
    clippy::single_match_else,
    clippy::unwrap_used
)]

pub mod config;
pub mod query;
pub mod record;
pub mod recorder;
pub mod tokenizer;

#[doc(inline)]
pub use config::TokenlessConfig;
#[doc(inline)]
pub use query::{format_list, format_show, format_summary};
#[doc(inline)]
pub use record::{OperationType, StatsRecord};
#[doc(inline)]
pub use recorder::{StatsError, StatsRecorder, StatsResult, StatsSummary};
#[doc(inline)]
pub use tokenizer::{estimate_tokens, estimate_tokens_from_bytes};
