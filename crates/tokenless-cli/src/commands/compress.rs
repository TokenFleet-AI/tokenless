//! Handlers for `compress-schema`, `compress-response`, `compress-auto`.

use tokenless_schema::compress_auto as schema_compress_auto;
use tokenless_schema::strategy_name;
use tokenless_stats::estimate_tokens_from_bytes;

use crate::{
    cache,
    shared::{
        RESPONSE_COMPRESSOR, SCHEMA_COMPRESSOR, SEMANTIC_COMPRESSOR, eprint_report,
        is_experimental_enabled, read_input, record_compression_stats,
    },
};

/// Handle `tokenless compress-schema`.
pub(crate) fn compress_schema(
    file: Option<String>,
    batch: bool,
    report: bool,
    project: Option<String>,
    agent_id: Option<String>,
    session_id: Option<String>,
    tool_use_id: Option<String>,
) -> Result<(), (String, i32)> {
    let input = read_input(&file).map_err(|e| (e, 2))?;
    if let Some(cached) = cache::cache_get(&input) {
        println!("{cached}");
        return Ok(());
    }
    let value: serde_json::Value =
        serde_json::from_str(&input).map_err(|e| (format!("JSON parse error: {e}"), 2))?;
    let compressor = &*SCHEMA_COMPRESSOR;

    let (after_compact, result_json) = if batch {
        let arr = value
            .as_array()
            .ok_or_else(|| ("Expected a JSON array for --batch mode".to_string(), 1))?;
        let results: Vec<serde_json::Value> =
            arr.iter().map(|item| compressor.compress(item)).collect();
        let compact = serde_json::to_string(&results).unwrap_or_default();
        let pretty = serde_json::to_string_pretty(&results)
            .map_err(|e| (format!("Serialization error: {e}"), 2))?;
        (compact, pretty)
    } else {
        let result = compressor.compress(&value);
        let compact = serde_json::to_string(&result).unwrap_or_default();
        let pretty = serde_json::to_string_pretty(&result)
            .map_err(|e| (format!("Serialization error: {e}"), 2))?;
        (compact, pretty)
    };

    let before_tokens = estimate_tokens_from_bytes(input.len());
    let after_tokens = estimate_tokens_from_bytes(after_compact.len());
    let output_text = if after_tokens >= before_tokens {
        input.clone()
    } else {
        result_json
    };

    if report {
        eprint_report(
            input.len(),
            before_tokens,
            after_compact.len(),
            after_tokens,
        );
    }

    cache::cache_insert(&input, &output_text);
    println!("{output_text}");

    record_compression_stats(
        tokenless_stats::OperationType::CompressSchema,
        agent_id,
        session_id,
        tool_use_id,
        project,
        input,
        output_text,
        false,                         // core schema compression
        Some("CompressorOnly".into()), // method: basic schema compressor
    );
    Ok(())
}

/// Handle `tokenless compress-response`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn compress_response(
    file: Option<String>,
    report: bool,
    semantic: bool,
    context: Option<String>,
    project: Option<String>,
    agent_id: Option<String>,
    session_id: Option<String>,
    tool_use_id: Option<String>,
) -> Result<(), (String, i32)> {
    let input = read_input(&file).map_err(|e| (e, 2))?;
    if let Some(cached) = cache::cache_get(&input) {
        println!("{cached}");
        return Ok(());
    }
    let value: serde_json::Value =
        serde_json::from_str(&input).map_err(|e| (format!("JSON parse error: {e}"), 2))?;

    // Apply semantic-aware field filtering when context is provided.
    #[allow(unused_variables)]
    let (value, semantic_dropped, used_experimental) = if let Some(ref ctx) = context {
        if semantic && !is_experimental_enabled() {
            eprintln!(
                "Warning: --semantic requires experimental mode. \
                 Enable with: tokenless stats experimental-on"
            );
        }
        let use_semantic = semantic && is_experimental_enabled();

        let mut sc = SEMANTIC_COMPRESSOR
            .lock()
            .map_err(|e| (format!("Semantic compressor lock error: {e}"), 1))?;

        // Enable ONNX Level 2 on first `--semantic` invocation.
        if use_semantic {
            sc.load_onnx()
                .map_err(|e| (format!("Failed to load ONNX model: {e}"), 1))?;
        }

        let before_count = count_fields(&value);
        let compressed = sc.compress(&value, ctx);
        let after_count = count_fields(&compressed);
        let dropped = before_count.saturating_sub(after_count);

        if report && dropped > 0 {
            eprintln!(
                "Semantic: dropped {dropped} field(s) (category: {})",
                sc.detect_category(ctx)
            );
        }

        (compressed, dropped, use_semantic)
    } else {
        (value, 0, false)
    };
    // Track semantic-dropped fields in the report (printed above).

    let compressor = &*RESPONSE_COMPRESSOR;
    let result = compressor.compress(&value);
    let after_compact = serde_json::to_string(&result).unwrap_or_default();
    let result_json = serde_json::to_string_pretty(&result)
        .map_err(|e| (format!("Serialization error: {e}"), 2))?;

    let before_tokens = estimate_tokens_from_bytes(input.len());
    let after_tokens = estimate_tokens_from_bytes(after_compact.len());
    let output_text = if after_tokens >= before_tokens {
        input.clone()
    } else {
        result_json
    };

    if report {
        eprint_report(
            input.len(),
            before_tokens,
            after_compact.len(),
            after_tokens,
        );
    }

    cache::cache_insert(&input, &output_text);
    println!("{output_text}");

    record_compression_stats(
        tokenless_stats::OperationType::CompressResponse,
        agent_id,
        session_id,
        tool_use_id,
        project,
        input,
        output_text,
        used_experimental, // only true when semantic compression was applied
        Some(
            if used_experimental {
                "Semantic"
            } else {
                "Standard"
            }
            .into(),
        ), // method
    );
    Ok(())
}

/// Handle `tokenless compress-auto`.
///
/// Delegates to [`tokenless_schema::compress_auto`], which internally
/// respects the experimental mode flag set at startup.
pub(crate) fn compress_auto(
    file: Option<String>,
    report: bool,
    project: Option<String>,
    agent_id: Option<String>,
    session_id: Option<String>,
    tool_use_id: Option<String>,
) -> Result<(), (String, i32)> {
    let input = read_input(&file).map_err(|e| (e, 2))?;
    if let Some(cached) = cache::cache_get(&input) {
        println!("{cached}");
        return Ok(());
    }
    let value: serde_json::Value =
        serde_json::from_str(&input).map_err(|e| (format!("JSON parse error: {e}"), 2))?;
    let (strategy, result) = schema_compress_auto(&value, &input);
    let after_compact = serde_json::to_string(&result).unwrap_or_default();
    let result_json = serde_json::to_string_pretty(&result)
        .map_err(|e| (format!("Serialization error: {e}"), 2))?;

    let before_tokens = estimate_tokens_from_bytes(input.len());
    let after_tokens = estimate_tokens_from_bytes(after_compact.len());
    let output_text = if after_tokens >= before_tokens {
        input.clone()
    } else {
        result_json
    };

    if report {
        eprintln!("Strategy: {}", tokenless_schema::strategy_name(&strategy));
        eprint_report(
            input.len(),
            before_tokens,
            after_compact.len(),
            after_tokens,
        );
    }

    cache::cache_insert(&input, &output_text);
    println!("{output_text}");

    record_compression_stats(
        tokenless_stats::OperationType::CompressSchema,
        agent_id,
        session_id,
        tool_use_id,
        project,
        input,
        output_text,
        is_experimental_enabled(), // format router is experimental
        Some(strategy_name(&strategy).into()), // method
    );
    Ok(())
}

/// Count total keys in a JSON object tree (recursive).
fn count_fields(value: &serde_json::Value) -> usize {
    match value {
        serde_json::Value::Object(obj) => {
            let mut count = obj.len();
            for v in obj.values() {
                count += count_fields(v);
            }
            count
        }
        serde_json::Value::Array(arr) => arr.iter().map(count_fields).sum(),
        _ => 0,
    }
}
