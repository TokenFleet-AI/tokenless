# tokenless-stats

SQLite-based compression metrics tracking for tokenless.

Part of the [tokenless](https://github.com/TokenFleet-AI/tokenless) toolkit.

## Usage

```rust
use tokenless_stats::{StatsRecorder, StatsRecord, OperationType};

let recorder = StatsRecorder::new(":memory:")?;
let record = StatsRecord::new(
    OperationType::CompressResponse,
    "my-agent".into(),
    1000,   // before_chars
    250,    // before_tokens
    500,    // after_chars
    125,    // after_tokens
);
recorder.record(&record)?;
```

Tracks compression metrics for schema compression, response compression, command rewriting, and TOON encoding.

License: Apache-2.0
