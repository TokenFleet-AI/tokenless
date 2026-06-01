//! Criterion benchmarks for `tokenless-core`.
#![allow(missing_docs)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use criterion::{Criterion, black_box};
use tokenless_core::Config;

/// Benchmark `Config::new` construction.
///
/// # Panics
///
/// Panics if `Config::new("my-app")` fails, which never happens with "my-app".
#[allow(clippy::unwrap_used)]
pub fn config_benchmark(c: &mut Criterion) {
    c.bench_function("Config::new", |b| {
        b.iter(|| Config::new(black_box("my-app")))
    });

    c.bench_function("Config::new + with_description", |b| {
        b.iter(|| {
            Config::new(black_box("my-app"))
                .unwrap()
                .with_description(black_box("A test application"))
        })
    });
}

criterion::criterion_group!(benches, config_benchmark);
criterion::criterion_main!(benches);
