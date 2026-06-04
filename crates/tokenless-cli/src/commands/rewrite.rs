//! Handler for `tokenless rewrite`.

use rtk_registry::{Classification, classify_command, rewrite_command};
use tokenless_stats::{OperationType, estimate_tokens_from_bytes};

use crate::shared::{read_input, record_compression_stats, rtk_available};

/// Handle `tokenless rewrite`.
pub(crate) fn rewrite(
    command: Option<String>,
    exclude: Vec<String>,
    transparent_prefix: Vec<String>,
    project: Option<String>,
    agent_id: Option<String>,
    session_id: Option<String>,
    tool_use_id: Option<String>,
) -> Result<(), (String, i32)> {
    let input = command.map_or_else(|| read_input(&None).map_err(|e| (e, 2)), Ok)?;
    let input = input.trim().to_string();
    if input.is_empty() {
        return Err(("Empty command".to_string(), 1));
    }

    let classification = classify_command(&input);
    match classification {
        Classification::Supported { .. } => {
            if let Some(rewritten) = rewrite_command(&input, &exclude, &transparent_prefix) {
                let before_tokens = estimate_tokens_from_bytes(input.len());
                let after_tokens = estimate_tokens_from_bytes(rewritten.len());
                let saved_pct = if before_tokens > 0 {
                    (before_tokens.saturating_sub(after_tokens) as f64 / before_tokens as f64)
                        * 100.0
                } else {
                    0.0
                };
                let rtk_status = if rtk_available() {
                    ""
                } else {
                    " (RTK not installed; dry-run)"
                };
                println!("   {input} → {rewritten}");
                println!(
                    "   tokens: ~{before_tokens} → ~{after_tokens}  saved: \
                     {saved_pct:.1}%{rtk_status}"
                );

                let user_name = Some(
                    tokenless_stats::TokenlessConfig::load()
                        .effective_user_name()
                        .to_string(),
                );
                record_compression_stats(
                    OperationType::RewriteCommand,
                    agent_id,
                    session_id,
                    tool_use_id,
                    project,
                    user_name,
                    input,
                    rewritten,
                    false,                      // RTK rewrite is always core
                    Some("RtkStandard".into()), // method
                );
            }
        }
        _ => {
            println!("No rewrite available for: {input}");
        }
    }
    Ok(())
}
