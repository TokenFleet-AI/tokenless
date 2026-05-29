# Performance Benchmarks

> Generated: 2026-05-29 — measured on macOS arm64 (Apple M-series)

## Compression Savings (fixture data)

| Strategy | Input | Output | Saved | % |
|----------|-------|--------|-------|---|
| Schema Compression | 962 B | 956 B | 6 B | 0.6% |
| Response Compression | 691 B | 402 B | 289 B | **41.8%** |
| Diff Response | 10 KB | ~200 B | ~9.8 KB | **~95%** |

**Note:** Savings vary significantly by input size and content type. Large schemas with long descriptions typically save 50-60%. The small fixture (962 B) has short descriptions, so minimal truncation occurs.

## Binary Size

| Profile | Size |
|---------|------|
| Release (opt-level=3, LTO, stripped) | ~4.1 MB |
| Debug | ~17.2 MB |

## Build Metrics

| Metric | Value |
|--------|-------|
| Rust edition | 2024 |
| Release profile | `opt-level=3`, `lto=true`, `codegen-units=1`, `panic=abort`, `strip=true` |
| Workspace crates | 4 (`schema`, `stats`, `cli`, `tui`) |
| Dependencies | ~42 crates (transitive) |
| Tests | 291 passed |

## Real-World Projections

| Strategy | Expected Savings | Best For |
|----------|-----------------|----------|
| Schema compression | ~57% | OpenAI tools with verbose descriptions |
| Response compression | 26-78% | JSON API responses with debug fields |
| Format router | 30-60% | Uniform arrays (TOON HRV) |
| TOON encoding | 15-40% | Structured data |
| Differential response | Up to 95% | Polling patterns (git status, etc.) |
| Command rewriting | 60-90% | CLI output via RTK (70+ commands) |
| Predictive cache | Near-zero latency | Repeated identical inputs |
