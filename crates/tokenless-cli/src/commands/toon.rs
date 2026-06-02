//! Handlers for `compress-toon` and `decompress-toon`.

use tokenless_stats::OperationType;

use crate::{
    cache,
    shared::{read_input, record_compression_stats},
};

/// Handle `tokenless compress-toon`.
pub(crate) fn compress_toon(
    file: Option<String>,
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
    let encoded =
        toon_format::encode_default(&value).map_err(|e| (format!("TOON encode error: {e}"), 2))?;
    cache::cache_insert(&input, &encoded);
    println!("{encoded}");

    record_compression_stats(
        OperationType::CompressToon,
        agent_id,
        session_id,
        tool_use_id,
        project,
        input,
        encoded,
        false, // basic TOON is always core
    );
    Ok(())
}

/// Handle `tokenless decompress-toon`.
pub(crate) fn decompress_toon(file: Option<String>) -> Result<(), (String, i32)> {
    let input = read_input(&file).map_err(|e| (e, 2))?;
    let decoded: serde_json::Value =
        toon_format::decode_default(&input).map_err(|e| (format!("toon decode failed: {e}"), 2))?;
    let output = serde_json::to_string_pretty(&decoded)
        .map_err(|e| (format!("Serialization error: {e}"), 2))?;
    println!("{output}");
    Ok(())
}
