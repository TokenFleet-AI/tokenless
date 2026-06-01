#![allow(clippy::expect_used, clippy::unwrap_used, clippy::approx_constant)]
//! Stress tests for `ResponseCompressor` with large inputs.
//!
//! Verifies that the compressor handles large JSON inputs without
//! crashing, OOM, or hanging.

use serde_json::{Value, json};
use tokenless_schema::ResponseCompressor;

/// Build a large JSON object with `num_keys` keys.
fn build_large_json(num_keys: usize) -> Value {
    let mut map = serde_json::Map::with_capacity(num_keys);
    for i in 0..num_keys {
        let key = format!("key_{i:05}");
        let value = json!({
            "id": i,
            "name": format!("item_number_{}", i),
            "description": "This is a description for an item in the stress test. \
                             It contains enough text to make the JSON reasonably large \
                             for each key. The quick brown fox jumps over the lazy dog. \
                             Lorem ipsum dolor sit amet consectetur adipiscing elit.",
            "metadata": {
                "created_at": "2025-01-15T10:30:00Z",
                "updated_at": "2025-06-01T14:22:00Z",
                "tags": ["stress", "test", "large", "json"],
                "scores": [0.95, 0.87, 0.73, 0.91, 0.62]
            },
            "nested": {
                "level2": {
                    "level3": {
                        "deep_value": format!("deep_{}", i)
                    }
                }
            }
        });
        map.insert(key, value);
    }
    Value::Object(map)
}

#[test]
fn test_large_json_response_compressor_no_oom() {
    // Generate a JSON with 1,000 keys (smaller than 10K to keep test runtime reasonable).
    // The compressor should handle this without OOM or panic.
    let large: Value = build_large_json(1000);

    // Serialize to check size
    let serialized = serde_json::to_string(&large).expect("serialization must succeed");
    assert!(
        serialized.len() > 100_000,
        "input should be > 100KB, got {} bytes",
        serialized.len()
    );

    let compressor = ResponseCompressor::new();
    let result = compressor.compress(&large);

    // Output must still be valid JSON
    let result_str = serde_json::to_string(&result).expect("compressed output must be valid JSON");
    let _parsed: Value =
        serde_json::from_str(&result_str).expect("compressed output must parse as JSON");

    // Compression should not make things larger than original
    assert!(
        result_str.len() <= serialized.len() || result_str.len() == serialized.len(),
        "compressed output should not be larger than original"
    );
}

#[test]
fn test_large_json_with_max_keys_per_object() {
    // With max_keys_per_object=50, a 1,000-key object should be truncated.
    let large: Value = build_large_json(1000);

    let compressor = ResponseCompressor::new().with_max_keys_per_object(50);
    let result = compressor.compress(&large);

    let obj = result.as_object().expect("result should be an object");
    // Should have at most 51 keys (50 data + 1 marker)
    assert!(
        obj.len() <= 51,
        "truncated object should have <= 51 keys, got {}",
        obj.len()
    );
    assert!(
        obj.contains_key("<...keys_truncated>"),
        "truncated object should have a marker key"
    );
}

#[test]
fn test_large_json_deep_nesting() {
    // A moderately deep structure (depth 7, within default max_depth=8)
    let mut deep: Value = json!("leaf");
    for _ in 0..6 {
        deep = json!({"nested": deep, "extra": "padding text to increase size"});
    }

    let compressor = ResponseCompressor::new();
    let result = compressor.compress(&deep);
    assert!(
        result.is_object(),
        "result should be an object, not a truncation marker"
    );
}

#[test]
fn test_large_string_array_truncation() {
    // Large array of long strings
    let arr: Vec<Value> = (0..100)
        .map(|i| {
            json!(format!(
                "item_{:03}_with_a_very_long_description_that_should_be_truncated_because_it_exceeds_the_default_512_character_limit_xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx", i
            ))
        })
        .collect();

    let input = json!({"items": arr});
    let compressor = ResponseCompressor::new().with_truncate_arrays_at(16);
    let result = compressor.compress(&input);

    let items = result["items"]
        .as_array()
        .expect("items should be an array");
    assert!(
        items.len() <= 17, // 16 items + 1 marker
        "truncated array should have <= 17 elements"
    );

    // Each string should be within the truncation limit
    for item in items.iter().take(16) {
        let s = item.as_str().expect("array item should be string");
        assert!(
            s.len() <= 530, // 512 + "… (truncated)" overhead
            "truncated string length {} exceeds limit",
            s.len()
        );
    }
}
