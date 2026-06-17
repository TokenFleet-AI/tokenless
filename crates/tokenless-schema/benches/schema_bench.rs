//! Criterion benchmarks for tokenless-schema compression operations.
//!
//! Covers schema compression, response compression for small and large inputs.
#![allow(missing_docs, reason = "bench functions don't need individual docs")]

use criterion::{Criterion, black_box, criterion_group, criterion_main};
use serde_json::json;
use tokenless_schema::{ResponseCompressor, SchemaCompressor};

fn bench_schema_compress_small(c: &mut Criterion) {
    let compressor = SchemaCompressor::new().with_compress_all(true);
    let input = json!({
        "type": "function",
        "function": {
            "name": "get_weather",
            "description": "Retrieves current weather conditions for a given location.",
            "parameters": {
                "type": "object",
                "properties": {
                    "location": {"type": "string", "description": "City name"},
                    "units": {"type": "string", "enum": ["celsius", "fahrenheit"]}
                }
            }
        }
    });
    c.bench_function("schema_compress_small", |b| {
        b.iter(|| {
            black_box(compressor.compress(black_box(&input)));
        });
    });
}

fn bench_schema_compress_large(c: &mut Criterion) {
    let compressor = SchemaCompressor::new().with_compress_all(true);
    let mut props = serde_json::Map::new();
    for i in 0..50 {
        props.insert(
            format!("field_{i}"),
            json!({
                "type": "string",
                "description": "A long description that should be truncated for efficiency.",
            }),
        );
    }
    let input = json!({
        "type": "function",
        "function": {
            "name": "large_fn",
            "parameters": {"type": "object", "properties": props}
        }
    });
    c.bench_function("schema_compress_large_50_props", |b| {
        b.iter(|| {
            black_box(compressor.compress(black_box(&input)));
        });
    });
}

fn bench_response_compress_small(c: &mut Criterion) {
    let compressor = ResponseCompressor::new();
    let input = json!({
        "items": [
            {"id": 1, "name": "a", "debug": "verbose"},
            {"id": 2, "name": "b", "debug": "verbose"}
        ],
        "meta": {"count": 2}
    });
    c.bench_function("response_compress_small", |b| {
        b.iter(|| {
            black_box(compressor.compress(black_box(&input)));
        });
    });
}

fn bench_response_compress_large(c: &mut Criterion) {
    let compressor = ResponseCompressor::new();
    let mut items = Vec::new();
    for i in 0..200 {
        items.push(json!({"id": i, "name": format!("item-{i}"), "debug": "internal data"}));
    }
    let input = json!({"items": items, "meta": {"total": 200, "debug": "hidden"}});
    c.bench_function("response_compress_large_200_items", |b| {
        b.iter(|| {
            black_box(compressor.compress(black_box(&input)));
        });
    });
}

criterion_group!(
    benches,
    bench_schema_compress_small,
    bench_schema_compress_large,
    bench_response_compress_small,
    bench_response_compress_large,
);
criterion_main!(benches);
