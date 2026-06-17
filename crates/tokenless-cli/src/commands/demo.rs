//! Handler and rendering for `tokenless demo`.

use std::fmt::Write as FmtWrite;

use tokenless_stats::estimate_tokens_from_bytes;

use crate::shared::{RESPONSE_COMPRESSOR, SCHEMA_COMPRESSOR, rtk_available};

/// Generate and return the demo output string.
#[allow(clippy::too_many_lines, clippy::cast_precision_loss)]
pub(crate) fn generate() -> String {
    let mut out = String::new();
    out.push_str("╔══════════════════════════════════════════╗\n");
    out.push_str("║     Tokenless Compression Demo           ║\n");
    out.push_str("╚══════════════════════════════════════════╝\n\n");

    // Schema Compression — use json! macro to avoid runtime parsing of static data
    let schema_val = serde_json::json!({
        "function": {
            "name": "get_weather",
            "description": "Get the current weather conditions for a specified city including temperature, humidity, wind speed, and precipitation forecast for the next 24 hours.",
            "parameters": {
                "type": "object",
                "properties": {
                    "city": {
                        "type": "string",
                        "description": "The city name to get weather for",
                        "examples": ["Beijing", "Tokyo", "London"]
                    },
                    "units": {
                        "type": "string",
                        "description": "Temperature unit: celsius or fahrenheit",
                        "examples": ["celsius"]
                    }
                }
            }
        }
    });
    let schema_input = serde_json::to_string(&schema_val).unwrap_or_default();
    out.push_str("1. Schema Compression\n─────────────────────\n");
    let result = SCHEMA_COMPRESSOR.compress(&schema_val);
    let after = serde_json::to_string(&result).unwrap_or_default();
    let bt = estimate_tokens_from_bytes(schema_input.len());
    let at = estimate_tokens_from_bytes(after.len());
    let _ = write!(
        out,
        "   chars: {} → {}  tokens: ~{bt} → ~{at}  saved: {pct:.1}%\n\n",
        schema_input.len(),
        after.len(),
        pct = if bt > 0 {
            (bt.saturating_sub(at) as f64 / bt as f64) * 100.0
        } else {
            0.0
        },
    );

    // Response Compression — use json! macro for static data
    let response_val = serde_json::json!({
        "status": "ok",
        "data": {
            "id": 12345,
            "name": "Alice",
            "email": "alice@example.com"
        },
        "debug": {
            "query_time_ms": 42,
            "cache_hit": false
        },
        "trace": "request-id-abc-123",
        "logs": ["step1", "step2", "step3"],
        "null_field": null,
        "empty_array": []
    });
    let response_input = serde_json::to_string(&response_val).unwrap_or_default();
    out.push_str("2. Response Compression\n───────────────────────\n");
    let result = RESPONSE_COMPRESSOR.compress(&response_val);
    let after = serde_json::to_string(&result).unwrap_or_default();
    let bt = estimate_tokens_from_bytes(response_input.len());
    let at = estimate_tokens_from_bytes(after.len());
    let _ = write!(
        out,
        "   drops: debug, trace, logs, null, empty[]\n   chars: {} → {}  tokens: ~{bt} → ~{at}  \
         saved: {pct:.1}%\n\n",
        response_input.len(),
        after.len(),
        pct = if bt > 0 {
            (bt.saturating_sub(at) as f64 / bt as f64) * 100.0
        } else {
            0.0
        },
    );

    // TOON — use json! macro for static data
    let toon_val = serde_json::json!({
        "name": "Alice",
        "age": 30,
        "hobbies": ["reading", "coding", "hiking"],
        "address": {
            "city": "Beijing",
            "zip": "100000"
        }
    });
    let toon_input = serde_json::to_string(&toon_val).unwrap_or_default();
    out.push_str("3. TOON Encoding\n─────────────────\n");
    let before = toon_input.len();
    let bt = estimate_tokens_from_bytes(before);
    if let Ok(encoded) = toon_format::encode_default(&toon_val) {
        let encoded = encoded.trim_end();
        let at = estimate_tokens_from_bytes(encoded.len());
        #[allow(clippy::cast_precision_loss)]
        let _ = write!(
            out,
            "   JSON → TOON\n   chars: {before} → {}  tokens: ~{bt} → ~{at}  saved: {pct:.1}%\n\n",
            encoded.len(),
            pct = if bt > 0 {
                (bt.saturating_sub(at) as f64 / bt as f64) * 100.0
            } else {
                0.0
            },
        );
    }

    // Command Rewriting
    out.push_str("4. Command Rewriting\n─────────────────────\n");
    let examples = ["git status", "kubectl get pods", "cargo test", "docker ps"];
    if rtk_available() {
        for cmd in &examples {
            if let Some(rewritten) = rtk_registry::rewrite_command(cmd, &[], &[]) {
                let _ = writeln!(out, "   {cmd} → {rewritten}");
            } else {
                let _ = writeln!(out, "   {cmd} → (no rewrite)");
            }
        }
    } else {
        for cmd in &examples {
            let _ = writeln!(out, "   {cmd} → rtk {cmd}");
        }
        let _ = writeln!(out, "\n   (RTK not installed; showing expected output)");
    }
    out.push('\n');
    out.push_str("──────────────────────────\n");
    out.push_str("Demo complete. To enable automatic optimization:\n");
    out.push_str("  tokenless init\n");
    out
}
