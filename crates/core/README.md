# tokenless-core

Shared core types and utilities for the tokenless workspace.

Part of the [tokenless](https://github.com/TokenFleet-AI/tokenless) toolkit.

## Types

```rust
use tokenless_core::{Config, SafePath, CoreError};

// Validated configuration
let config = Config::load()?;

// Path traversal-safe relative paths
let safe = SafePath::new("data/cache")?;
assert!(!safe.contains_traversal());

// Unified error handling
fn do_work() -> tokenless_core::Result<()> {
    Ok(())
}
```

- **`Config`** — validated runtime configuration (stats enabled, experimental mode, thresholds)
- **`SafePath`** — path traversal-resistant relative path wrapper
- **`CoreError`** — unified error type for the workspace
- **`Result<T>`** — type alias for `std::result::Result<T, CoreError>`

## Minimum Rust Version

Rust 2024 edition, MSRV 1.89.

License: Apache-2.0
