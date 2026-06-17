# tokenless-schema WASM build feasibility

## Summary

`tokenless-schema` is a strong candidate for WebAssembly because its core value is pure JSON transformation:

- schema compression via `SchemaCompressor`
- response compression via `ResponseCompressor`
- shape analysis and strategy selection via `shape_analyzer` and `format_router`
- string-based encoders in `encoding/`

These paths do not depend on filesystems, sockets, subprocesses, threads, or platform-native databases inside this crate, so the crate can expose a browser-safe API with a thin `wasm-bindgen` wrapper.

Target npm package name: `@tokenfleet/tokenless-wasm`

## What can compile directly to WASM

The following `tokenless-schema` capabilities are directly portable:

1. `SchemaCompressor`
   - Operates on `serde_json::Value`
   - Uses deterministic string and object/array transformations
   - No host I/O

2. `ResponseCompressor`
   - Pure in-memory JSON trimming and truncation
   - No OS dependencies

3. `shape_analyzer`
   - Structural inspection only
   - No unsupported runtime features

4. `format_router`
   - Strategy selection logic is pure Rust
   - Existing encoders produce strings, which map well to JS interop

5. `encoding`
   - Current encoders are CPU-only string formatting logic
   - Suitable for browser or Node.js execution

## Likely WASM-safe dependencies

Current crate dependencies are WASM-friendly for this use case:

- `serde_json`
- `regex`
- `tracing` (though browser logging integration is limited unless explicitly bridged)
- `wasm-bindgen` for exported bindings

## Gaps and replacement guidance across the wider workspace

This crate is feasible now, but a full workspace-to-WASM story needs substitutions for non-browser dependencies used elsewhere.

### Native database usage

Replacement guidance:

- `rusqlite` cannot be used directly in `wasm32-unknown-unknown`
- browser target replacement: IndexedDB via a dedicated abstraction layer
- Node.js target replacement: host-side SQLite service or filesystem-backed adapter outside the wasm module

Recommended direction:

- keep `tokenless-schema` pure and dependency-light
- introduce a persistence trait in higher-level crates if browser persistence is needed later

### Subprocess execution

Replacement guidance:

- `std::process::Command` patterns are not available in browser WASM
- browser target replacement: explicit no-op, capability error, or JS callback hook
- Node.js target replacement: perform subprocesses in the JS host, not inside wasm

Recommended direction:

- define host capability boundaries explicitly
- return structured `unsupported in wasm` errors instead of partial emulation

### Filesystem access

Replacement guidance:

- browser target replacement: in-memory buffers, File API, or host-provided content
- Node.js target replacement: host JS reads files and passes strings/bytes into wasm

### Networking

Replacement guidance:

- browser target replacement: JS `fetch` from the host environment
- wasm module should stay focused on transform/analysis, not transport

## Proposed exported API surface

Initial minimal exports should stay string-based for low-friction JS interop:

- `compress_schema_json(input: string): string`
- `compress_json_auto(input: string): string`

The second function currently returns a JSON string object containing:

- `strategy`
- `output`

This is intentionally simple for `wasm-pack` generated bindings and avoids exposing internal Rust enums directly.

## Build setup recommendation

Recommended packaging path:

1. keep `tokenless-schema` as the Rust source crate
2. enable `cdylib` output for wasm builds
3. gate wasm bindings behind a `wasm` Cargo feature
4. build with `wasm-pack`
5. publish generated package as `@tokenfleet/tokenless-wasm`

Suggested commands:

```bash
wasm-pack build crates/tokenless-schema \
  --target bundler \
  --features wasm \
  --out-dir ../../pkg/tokenless-wasm
```

Additional useful variants:

```bash
wasm-pack build crates/tokenless-schema --target web --features wasm
wasm-pack build crates/tokenless-schema --target nodejs --features wasm
```

## Integration notes

### Browser

Best fit:

- schema compression before sending tool definitions to browser-hosted agents
- client-side response trimming before relaying tool output to an LLM gateway
- playground/demo usage in docs or a future web UI

### Node.js

Best fit:

- CLI-adjacent JavaScript tools that want deterministic compression without rewriting logic in TypeScript
- MCP/web adapters that already operate on JSON strings

## Risks and caveats

1. `regex` increases wasm bundle size
   - acceptable for initial prototype
   - can be revisited later if bundle size becomes a product concern

2. `tracing` is not enough by itself for browser observability
   - host-side logging bridge may be needed later

3. `serde_json::Value` string round-trips are simple but not zero-copy
   - acceptable for initial compatibility-first packaging
   - later optimization could expose typed JS objects or byte-oriented APIs if needed

4. crate-level tests passing on host does not prove wasm-target correctness
   - CI should add a dedicated `wasm32-unknown-unknown` or `wasm-pack` build job later

## Next action plan

1. Add CI validation for `wasm-pack build --features wasm`
2. Create a small JS smoke test that imports the generated package and calls both exported functions
3. Decide initial package target priority:
   - `bundler` for app integration
   - `web` for direct browser demos
   - `nodejs` for server-side JS usage
4. Add npm package metadata and publish workflow for `@tokenfleet/tokenless-wasm`
5. Evaluate bundle size and startup latency after first build artifact is available
6. If broader workspace wasm support is pursued, introduce host abstraction layers for storage, subprocess, and file access
