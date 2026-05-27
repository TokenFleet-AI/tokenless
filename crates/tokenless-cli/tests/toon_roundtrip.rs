#![allow(clippy::expect_used, clippy::unwrap_used, clippy::approx_constant)]
//! TOON format round-trip tests.
//!
//! Verify that JSON -> TOON -> JSON produces identical data for
//! various edge cases: nested objects, arrays, CJK text, special chars.

use serde_json::{Value, json};
use toon_format::{decode_default, encode_default};

#[test]
fn test_toon_roundtrip_simple_object() {
    let original = json!({"name": "Alice", "age": 30, "active": true});
    let encoded = encode_default(&original).expect("TOON encode failed");
    let decoded: Value = decode_default(&encoded).expect("TOON decode failed");
    assert_eq!(
        original, decoded,
        "simple object round-trip must be identical"
    );
}

#[test]
fn test_toon_roundtrip_nested_object() {
    let original = json!({
        "user": {
            "name": "Bob",
            "profile": {
                "email": "bob@example.com",
                "preferences": {
                    "theme": "dark",
                    "notifications": true
                }
            }
        }
    });
    let encoded = encode_default(&original).expect("TOON encode failed");
    let decoded: Value = decode_default(&encoded).expect("TOON decode failed");
    assert_eq!(
        original, decoded,
        "nested object round-trip must be identical"
    );
}

#[test]
fn test_toon_roundtrip_array() {
    let original = json!({
        "items": [1, 2, 3, 4, 5],
        "names": ["alice", "bob", "charlie"],
        "mixed": [1, "two", 4.5, true, null]
    });
    let encoded = encode_default(&original).expect("TOON encode failed");
    let decoded: Value = decode_default(&encoded).expect("TOON decode failed");
    assert_eq!(original, decoded, "array round-trip must be identical");
}

#[test]
fn test_toon_roundtrip_cjk_text() {
    let original = json!({
        "message": "你好世界",
        "description": "这是中文测试文本，包含标点符号！",
        "tags": ["中文", "日本語", "한국어"]
    });
    let encoded = encode_default(&original).expect("TOON encode failed");
    let decoded: Value = decode_default(&encoded).expect("TOON decode failed");
    assert_eq!(original, decoded, "CJK text round-trip must be identical");
}

#[test]
fn test_toon_roundtrip_special_characters() {
    let original = json!({
        "special": "Line1\nLine2\tTabbed",
        "quotes": "He said \"hello\"",
        "backslash": "C:\\Users\\test",
        "unicode_escape": "\u{00e9}\u{00f1}",
        "symbols": "!@#$%^&*()_+-=[]{}|;:',.<>?/~`"
    });
    let encoded = encode_default(&original).expect("TOON encode failed");
    let decoded: Value = decode_default(&encoded).expect("TOON decode failed");
    assert_eq!(
        original, decoded,
        "special characters round-trip must be identical"
    );
}

#[test]
fn test_toon_roundtrip_deeply_nested_array_of_objects() {
    let original = json!({
        "users": [
            {"id": 1, "name": "Alice", "tags": ["admin", "moderator"]},
            {"id": 2, "name": "Bob", "tags": ["user"]},
            {"id": 3, "name": "Charlie", "tags": []}
        ],
        "metadata": {
            "total": 3,
            "page": 1
        }
    });
    let encoded = encode_default(&original).expect("TOON encode failed");
    let decoded: Value = decode_default(&encoded).expect("TOON decode failed");
    assert_eq!(
        original, decoded,
        "nested array of objects round-trip must be identical"
    );
}

#[test]
fn test_toon_roundtrip_numbers() {
    let original = json!({
        "int_max": 2_147_483_647,
        "int_neg": -42,
        "float": 3.14159,
        "zero": 0,
        "small_int": 42
    });
    let encoded = encode_default(&original).expect("TOON encode failed");
    let decoded: Value = decode_default(&encoded).expect("TOON decode failed");
    // Compare as Values — floating point may differ in text representation
    assert_eq!(decoded["int_max"], original["int_max"]);
    assert_eq!(decoded["int_neg"], original["int_neg"]);
    assert_eq!(decoded["zero"], original["zero"]);
    assert_eq!(decoded["small_int"], original["small_int"]);
    // Float comparison with epsilon
    let orig_float = decoded["float"].as_f64().unwrap();
    assert!(
        (orig_float - 3.14159).abs() < 0.001,
        "float round-trip drift: {orig_float}"
    );
}

#[test]
fn test_toon_roundtrip_emoji() {
    let original = json!({
        "greeting": "Hello 👋 World 🌍",
        "emoji_list": ["😀", "🚀", "💻", "🦀"],
        "flags": "🇺🇸 🇨🇳 🇯🇵"
    });
    let encoded = encode_default(&original).expect("TOON encode failed");
    let decoded: Value = decode_default(&encoded).expect("TOON decode failed");
    assert_eq!(original, decoded, "emoji round-trip must be identical");
}

#[test]
fn test_toon_roundtrip_empty_values() {
    let original = json!({
        "empty_string": "",
        "empty_array": [],
        "empty_object": {},
        "null_field": null
    });
    let encoded = encode_default(&original).expect("TOON encode failed");
    let decoded: Value = decode_default(&encoded).expect("TOON decode failed");
    assert_eq!(
        original, decoded,
        "empty values round-trip must be identical"
    );
}

#[test]
fn test_toon_roundtrip_mixed_cjk_ascii_emoji() {
    let original = json!({
        "mixed": "Hello 你好 World 🌍 世界",
        "rtl_placeholder": "مرحبا بالعالم",
        "zero_width": "test\u{200d}ing"
    });
    let encoded = encode_default(&original).expect("TOON encode failed");
    let decoded: Value = decode_default(&encoded).expect("TOON decode failed");
    assert_eq!(
        original, decoded,
        "mixed CJK+ASCII+emoji+zwj round-trip must be identical"
    );
}
