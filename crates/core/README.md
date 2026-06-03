# tokenless-core

[![Crates.io](https://img.shields.io/crates/v/tokenless-core.svg)](https://crates.io/crates/tokenless-core)
[![Docs](https://docs.rs/tokenless-core/badge.svg)](https://docs.rs/tokenless-core)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://github.com/TokenFleet-AI/tokenless/blob/master/LICENSE)

![tokenless](https://raw.githubusercontent.com/TokenFleet-AI/tokenless/master/assets/tokenless.svg)

Shared core types and utilities for the tokenless workspace.

Part of the [tokenless](https://github.com/TokenFleet-AI/tokenless) toolkit.

## Why tokenless-core?

Every tokenless crate needs validated configuration, path traversal protection, and a unified error type. Rather than duplicating these across 6 crates, `tokenless-core` provides them once — with compile-time safety guarantees:

- **`Config`** validates at construction, not at use-site
- **`SafePath`** rejects `..`, `/`, and NUL bytes — you can't accidentally construct a dangerous path
- **`CoreError`** gives the whole workspace a single `Result<T>` alias

## Quick Start

```toml
[dependencies]
tokenless-core = "0.4"
```

```rust
use tokenless_core::{Config, SafePath, CoreError};

// Validated configuration
let config = Config::new("my-app")?
    .with_description("CLI tool");

// Path traversal-safe relative paths (rejects "..", "/", and NUL at construction)
let safe = SafePath::new("data/cache")?;
let display_path = safe.as_path().display();

// Unified error handling
fn do_work() -> tokenless_core::Result<()> {
    Ok(())
}
```

## Types

### `Config`

Validated runtime configuration.

| Method | Description |
|--------|-------------|
| `Config::new(name)` | Create with app name (fails on empty) |
| `with_description(desc)` | Optional description |
| `name()` | Return app name |
| `description()` | Return optional description |

### `SafePath`

Path traversal-resistant relative path. Rejects `..`, absolute paths, and NUL bytes at construction time — no runtime checks needed.

| Method | Description |
|--------|-------------|
| `SafePath::new(path)` | Create; returns `Err` for traversal attempts |
| `as_path()` | Borrow as `&Path` |
| `impl Display` | Human-readable display |
| `impl AsRef<Path>` | Interop with `std::path` APIs |

### `CoreError`

Unified error type (`#[non_exhaustive]`). Variants: `Io`, `Serialization`, `App`, `Path`.

### `Result<T>`

Type alias for `std::result::Result<T, CoreError>`.

## Minimum Rust Version

Rust 2024 edition, MSRV 1.89.

License: Apache-2.0
