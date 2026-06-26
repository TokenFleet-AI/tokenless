# tokenless Agent Guide

This repository is a reusable Rust 2024 workspace template. These rules are mandatory when working in this template.

## Non-negotiables

- Never enter plan mode automatically.
- Preserve template placeholders such as `tokenless` unless the user explicitly asks to instantiate the template.
- Do not replace template variables with concrete project names during maintenance.
- Use `CLAUDE.md` as the single project-level agent instruction file.
- Do not commit, push, merge, release, deploy, install dependencies, or change ticket state without explicit user permission.
- Never run `cargo clean`; ask first if it is truly required.
- Never write `TODO`, `todo!()`, temporary stubs, or incomplete code. If blocked, stop, reassess, and implement the complete solution.
- Remove dead code instead of suppressing it. Do not add deprecation layers unless explicitly requested.
- Never expose secrets in commands, logs, URLs, comments, errors, or tool arguments.

## Working Process

- Start by understanding the relevant files, symbols, tests, and specs before editing.
- Keep changes minimal, cohesive, and aligned with SOLID, DRY, and KISS.
- Check for existing user changes before editing; never overwrite unrelated work.
- Prefer existing Makefile targets. For new automation, add a `Makefile` target instead of ad-hoc shell scripts.
- **Install path**: `make install` installs to `~/.local/bin/`. For **development mode**, use `./scripts/dev-install.sh` which installs to `~/.tokenfleet-ai/bin/`. Do not suggest `make install` when the user is developing locally — use `./scripts/dev-install.sh` instead.
- For dependency, Helm chart, or external-resource changes, check current upstream usage and security posture first. Put deep research under `docs/research/` after checking existing research.
- For specs, inspect `specs/`, place new files there, name them `{feature-name}-{type}.md`, and update `specs/index.md`.
- For docs, inspect `docs/`, place new files there, and update `docs/index.md`. If documentation was not explicitly requested but is useful, still place it under `docs/`.

## Required Validation

- Prefer the smallest validation that proves the change.
- Prefer existing Makefile targets over raw cargo commands: `make build`, `make test`, `make fmt`, `make clippy`, and `make lint`.
- Docs-only changes do not require the full Cargo suite; code, API, and dependency changes require relevant tests plus formatting and linting.
- When touching production Rust code, run `cargo clippy --all-targets --all-features -- -D warnings -W clippy::pedantic`.
- Run `cargo audit` and `cargo-deny` when dependency, license, or supply-chain risk changes are involved.
- Do not hide failing checks. Diagnose, fix, and rerun; ask the user only when blocked.

## Rust Baseline

- Use Rust 2024 and the pinned toolchain in `rust-toolchain.toml`.
- Keep workspace-wide lint policy in `Cargo.toml` via `[workspace.lints]` when possible.
- Forbid unsafe code at crate roots with `#![forbid(unsafe_code)]`.
- Enable core lint coverage such as `missing_docs` and `missing_debug_implementations`.
- All public items require documentation, including examples where useful and `# Errors`, `# Panics`, or `# Safety` sections when applicable.
- Derive or implement `Debug` for all types; redact sensitive fields manually.

## Error Handling

- Never use `unwrap()` or `expect()` in production code.
- Return `Result<T>` for fallible operations; do not use `Option<T>` to hide errors.
- Use `thiserror` for library or domain error enums and `anyhow` for application-level context.
- Add context with `.context()` or `.with_context()` when propagating errors.
- Panics are acceptable only for truly unrecoverable application bugs, never for library errors or external input.

## Type and API Design

- Make illegal states unrepresentable with newtypes, enums, `NonZero*`, and private fields.
- Prefer `From`, `TryFrom`, and `FromStr` for conversions; prefer `winnow` for custom grammars.
- Use `typed-builder` for structs with more than five fields; simple constructors are fine for small types.
- Mark library-facing structs `#[non_exhaustive]` when future fields are likely.
- Do not use `Option<T>` when `T` has a natural empty or default value such as `Vec`, `HashMap`, or `HashSet`.
- Prefer explicit public API types over `impl Trait`; use `impl Trait` for internal helpers.

## Async and Concurrency

- Use Tokio with explicit features when an async runtime is needed.
- Prefer actors and message passing over shared mutable state.
- Use `tokio::sync::mpsc` for MPSC and `flume` when a faster channel is justified.
- For non-`Send` or non-`Sync` resources, isolate ownership in a dedicated actor or thread instead of wrapping them in locks.
- Prefer `DashMap` over `Mutex<HashMap>` or `RwLock<HashMap>` for concurrent maps.
- Use `ArcSwap` for infrequently updated shared configuration.
- Handle all spawned task results and panics; prefer `JoinSet` for groups of tasks.
- Avoid blocking inside async code; use `tokio::task::spawn_blocking` when required.
- Use native async traits unless object safety requires `async-trait`; document that reason at module level.

## Input, Security, and Resource Boundaries

- Treat every value crossing HTTP, IPC, file, env, CLI, deserialization, or queue boundaries as hostile until validated.
- Validate immediately at deserialization and parse boundaries; reject invalid data instead of sanitizing it.
- Bound all externally supplied strings by byte length, all collections by element count, and all numbers by explicit ranges.
- Use charset allowlists for identifiers and slugs; avoid blocklists.
- Prevent path traversal by rejecting `..`, absolute paths, NUL bytes, and separators before canonicalization.
- Prevent SSRF by parsing URLs, allowlisting schemes, rejecting private, loopback, and link-local targets, and pinning resolved IPs.
- Use parameterized database APIs; never format user input into SQL.
- Use argv-form process execution; never concatenate user input into shell commands.
- Use the `regex` crate for untrusted text matching; cap untrusted regex pattern size before compilation.
- Add body limits, timeouts, concurrency caps, recursion limits, decompression limits, and rate limits at trust boundaries.
- Use checked, saturating, or explicitly wrapping arithmetic for external numeric input.

## Cryptography and Secrets

- Use `rustls` with the `aws-lc-rs` backend for TLS in new code.
- Use constant-time comparison for tokens, MACs, signatures, password hashes, and similar secrets.
- Use Argon2id for password hashing with parameters tuned for at least 250 ms on target hardware.
- Use OS randomness such as `OsRng` or `getrandom` for security-sensitive keys, tokens, nonces, and IDs.
- Wrap secrets with `secrecy` types and assert redacted `Debug` output in tests for custom secret-bearing types.
- Load secrets from environment variables or secret managers only; never hard-code or commit `.env*` files.
- Design key and token systems for rotation with multiple active keys.

## Serialization and Configuration

- Use strongly typed `serde` models. Use `serde_json::Value` only for truly dynamic schemas.
- Use `#[serde(rename_all = "camelCase")]` for JSON-facing types.
- Use `#[serde(alias = "...")]` for backward compatibility and `#[serde(default)]` for defaultable fields.
- Use `#[serde(skip_serializing_if = "Option::is_none")]` to omit null JSON fields.
- Validate deserialized data immediately, with custom deserializers or validated newtypes when needed.
- Prefer the `config` crate and YAML files for runtime-tunable configuration; keep compile-time constants in code.

## Testing

- Add or update tests with every behavior change.
- Put unit tests in the same file under `#[cfg(test)] mod tests`; use `tests/` for integration tests.
- Name tests with `test_should_...` and cover error paths explicitly with `matches!` where appropriate.
- Use `rstest` for parameterized cases and `proptest` for invariants.
- Use `mockall` or `wiremock` only when isolation is valuable; prefer fast real implementations.
- Use doc tests for public examples. Mark slow tests `#[ignore]` and run them in CI when relevant.

## Logging and Observability

- Use `tracing`; never use `println!` or `dbg!` in production code.
- Prefer structured fields over string concatenation, especially for user-controlled values.
- Use `error!`, `warn!`, `info!`, `debug!`, and `trace!` intentionally.
- Add `#[instrument]` to meaningful async boundaries and skip large or sensitive parameters.
- Use JSON logging for production and human-readable output for local development.

## Performance

- Profile before optimizing.
- Avoid unnecessary allocation and cloning; prefer borrowing, `Arc`, `Cow<str>`, and `Bytes` where appropriate.
- Preallocate with `Vec::with_capacity()` when final size is known.
- Prefer iterators and small focused functions.
- Consider `SmallVec` or `smallbox` only when profiling or data shape justifies it.
- Add Criterion benchmarks only after behavior stabilizes.

## Dependencies

- Minimize dependency count and prefer pure Rust crates over FFI bindings.
- Use workspace dependencies for shared crates.
- Pin intentionally: `~` for patch-only updates when needed, default caret requirements for normal minor updates.
- Audit maintenance status, security history, and code quality before adding a dependency.
- Use package managers for dependency changes; do not manually edit lockfiles or manifests for installs or upgrades.

## Documentation

- Write doc comments for all public items.
- Use `//!` for module-level documentation.
- Include examples where helpful, and document `# Errors`, `# Panics`, and `# Safety` sections when applicable.
- Generate docs with `cargo doc --no-deps` when documentation rendering needs verification.

## Code Style

- Import order: standard library, external dependencies, local modules.
- Use specific imports and refer to imported names directly; avoid fully qualified paths in implementations except for macros.
- Follow Rust naming conventions: `snake_case`, `PascalCase`, and `SCREAMING_SNAKE_CASE`.
- Keep functions small and focused; split complex logic into named helpers.
- Order items consistently: imports, constants, types, functions, tests.
- Use trailing commas in multi-line calls and literals.
- Run `rustfmt` rather than hand-formatting.

## Clippy Pedantic Alignment

All code should pass `cargo clippy --all-targets --all-features -- -D warnings -W clippy::pedantic`. Prefer these idioms:

- `x.map_or(a, f)` instead of `x.map(f).unwrap_or(a)`.
- `v.and_then(Value::as_u64)` instead of redundant closures.
- `"value: {x}"` instead of positional format arguments.
- Backtick identifiers in docs, including environment variables and fields.
- Combine same-body match arms with `|`; keep wildcard arms last.
- Restructure instead of using needless `continue`.
- Collapse nested `if` expressions when conditions can be combined.
- Use `&str` instead of `String` for non-consuming parameters.
- Add `#[must_use]` to pure value-returning functions.
- Avoid wildcard imports and similar-looking variable names.
- Use `.try_into()` for lossy conversions and `.into()` or `as` only for provably lossless ones.
- Return the inner type when a function always returns `Some` or `Ok`.
- Prefer one-pass `.filter_map(f)` over `.filter().map()` or `.map().flatten()`.

## Agent Role → Model Mapping (Auto-Apply)

When spawning agents via `Agent()` or `Task()`, ALWAYS apply this mapping.
The proxy translates Claude model names to domestic models automatically.

### Model Tiers

| Tier | Claude Model | Domestic Model | Use For |
|------|-------------|----------------|---------|
| **Reasoning** | `opus` | deepseek-v4-flash | architect, security-architect, system-architect |
| **Primary** | `sonnet` | deepseek-v4-pro | coder, tester, reviewer |
| **Fast** | `haiku` | gpt-5.4 | researcher, scanner, formatter, documenter |

### Role Assignment

```
architect          → model: "opus"    (深度推理,系统设计)
security-architect → model: "opus"    (威胁建模,安全审计)
system-architect   → model: "opus"    (架构设计)

coder              → model: "sonnet"  (代码生成主力)
tester             → model: "sonnet"  (测试用例生成)
reviewer           → model: "sonnet"  (代码审查)

researcher         → model: "haiku"   (快速搜索/扫描)
formatter          → model: "haiku"   (格式化/统计)
documenter         → model: "haiku"   (文档生成)
```

### Spawn Template

```javascript
// Opus tier — deepseek-v4-flash
Agent({ model: "opus",  subagent_type: "system-architect", name: "architect", run_in_background: true })
Agent({ model: "opus",  subagent_type: "security-auditor", name: "security",   run_in_background: true })

// Sonnet tier — deepseek-v4-pro
Agent({ model: "sonnet", subagent_type: "coder",           name: "coder",     run_in_background: true })
Agent({ model: "sonnet", subagent_type: "tester",          name: "tester",    run_in_background: true })
Agent({ model: "sonnet", subagent_type: "reviewer",        name: "reviewer",  run_in_background: true })

// Haiku tier — gpt-5.4
Agent({ model: "haiku",  subagent_type: "researcher",      name: "researcher", run_in_background: true })
Agent({ model: "haiku",  subagent_type: "Explore",         name: "scanner",   run_in_background: true })
```
