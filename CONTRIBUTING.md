# Contributing to Tokenless

Thank you for your interest in contributing to Tokenless! This document provides guidelines and workflows for contributors.

## Code of Conduct

This project follows a standard open-source code of conduct. Be respectful, constructive, and inclusive.

## Getting Started

### Prerequisites

- Rust toolchain >= 1.85 (see `rust-toolchain.toml`)
- `cargo-audit` and `cargo-deny` for security checks

### Setup

```bash
git clone https://github.com/TokenFleet-AI/tokenless
cd tokenless
make setup
```

## Development Workflow

### 1. Pick an Issue

Check [GitHub Issues](https://github.com/TokenFleet-AI/tokenless/issues) for open tasks. Comment to claim one.

### 2. Branch

```bash
git checkout -b feat/your-feature-name
# or: fix/your-bug-fix
# or: docs/your-doc-improvement
```

Branch naming convention: `{type}/{short-description}` where type is `feat`, `fix`, `docs`, `refactor`, `perf`, `test`, or `chore`.

### 3. Make Changes

Follow the project conventions:

- **Rust 2024 edition** with all clippy pedantic lints enabled
- `#![forbid(unsafe_code)]` at crate roots (exceptions documented per `unsafe` block)
- Builder pattern for configurable types
- `thiserror` for library errors, `anyhow` for application errors
- No `unwrap()` / `expect()` in production code
- All public items require doc comments
- Use `tracing` for logging, never `println!`/`dbg!`

### 4. Run Validation

```bash
make build     # Release build
make test      # All tests (257+)
make lint      # fmt + clippy + cargo-audit
```

Always run `make lint` before committing. Clippy pedantic warnings are errors.

### 5. Commit

Keep commits focused and atomic. Write descriptive commit messages:

```text
feat: add semantic-aware compression engine

Implements a 3-tier architecture with rule-based, ONNX, and
remote API fallback. Closes #42.
```

### 6. Pull Request

- Title matches commit style: `{type}: {description}`
- Description explains what and why, with test plan
- Link related issues

## Project Architecture

```
crates/tokenless-schema/   # Core compression library
  src/
    schema_compressor.rs   # OpenAI Function Calling schema compression
    response_compressor.rs # JSON API response compression
    shape_analyzer.rs      # JSON structure analyzer
    format_router.rs       # Intelligent encoding strategy selector
    encoding/              # TOON HRV, Enhanced TOON, CJSON compact

crates/tokenless-stats/    # SQLite compression metrics tracking

crates/tokenless-cli/      # CLI binary + MCP server + env checker

adapters/tokenless/        # Agent plugin adapters (OpenClaw, Hermes)
```

## Testing Guidelines

- Unit tests: same file under `#[cfg(test)] mod tests`
- Integration tests: `crates/*/tests/`
- Name tests `test_should_...` for clarity
- Cover error paths alongside happy paths
- Use `rstest` for parameterized cases, `proptest` for invariants

## Documentation

- Keep README.md (English) and README.zh.md (Chinese) in sync
- Spec documents go in `specs/` with sequential numbering
- User-facing guides go in `docs/`
- Update `docs/index.md` and `specs/index.md` when adding documents
- Public API items need doc comments with `# Errors`, `# Panics`, or `# Safety` sections

## Release Process

Releases follow semantic versioning via release-please. The maintainer will:

1. Run `release-please` to generate the release PR
2. Verify CHANGELOG.md updates
3. Publish to crates.io and GitHub Releases

## Getting Help

- Open a [GitHub Discussion](https://github.com/TokenFleet-AI/tokenless/discussions)
- Check `specs/` for design rationale
- Check `docs/` for usage guides
