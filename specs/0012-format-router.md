# 0012 — Intelligent Format Router

## 1. Motivation

Tokenless currently compresses JSON through a fixed pipeline: `ResponseCompressor` → (optional) `TOON encode`. This works well but leaves savings on the table — different JSON shapes benefit from different encoding strategies:

| JSON Shape | Best Format | Expected Savings |
|-----------|-------------|-----------------|
| Uniform object arrays (API paginated lists) | TOON HRV (Header-Row-Value) | 50-60% vs JSON |
| Deep single-child chains (k8s configs, DB configs) | TOON dot-path | 40-57% vs JSON |
| Schemas with enums/ranges/constraints | Enhanced TOON (type abbrev + inline constraints) | 40-55% vs JSON |
| Irregular/mixed structures | CJSON-style compact | 30-40% vs JSON |
| Already small (<200 chars) | Skip encoding, ResponseCompressor only | Avoid overhead |

The router analyzes input structure and selects the optimal encoding automatically.

## 2. Architecture

```
                            ┌─────────────────┐
JSON Input ───────────────▶│ StructureAnalyzer │
                            └────────┬────────┘
                                     │
                         ┌───────────┴───────────┐
                         ▼                       ▼
                   Shape detected          Score each strategy
                         │                       │
                         └───────────┬───────────┘
                                     ▼
                            ┌─────────────────┐
                            │  FormatRouter   │
                            │  select(shape)   │
                            └────────┬────────┘
                                     │
              ┌──────────────────────┼──────────────────────┐
              ▼                      ▼                      ▼
       ┌────────────┐        ┌────────────┐        ┌────────────┐
       │ TOON HRV   │        │ Enhanced   │        │ CJSON      │
       │ Encoder    │        │ TOON       │        │ Compact    │
       └─────┬──────┘        └─────┬──────┘        └─────┬──────┘
              │                    │                      │
              └────────────────────┼──────────────────────┘
                                   ▼
                            Compressed Output
```

## 3. Structure Analyzer

Runs O(n) single-pass over JSON. Produces `JsonShape`:

```rust
struct JsonShape {
    /// Top-level type
    top_level: TopType,           // Object | Array | Scalar
    /// For objects: total key count
    key_count: usize,
    /// For arrays: item count
    item_count: usize,
    /// Maximum nesting depth
    max_depth: usize,
    /// True if all array items are objects with identical key sets
    is_uniform_array: bool,
    /// True if any object contains "enum" key
    has_enums: bool,
    /// True if any object contains "minimum"/"maximum"/"pattern" constraints
    has_constraints: bool,
    /// Longest single-child object chain (for dot-path optimization)
    max_chain_depth: usize,
    /// Total characters of input JSON
    char_count: usize,
}
```

### Detection rules:

1. **Uniform array**: If top-level is Array, iterate items. If all items are Objects with identical `.keys().collect::<Vec<_>>()` → uniform. Check up to first 100 items only (performance guard).

2. **Chain depth**: Walk the tree. If an object has exactly one key whose value is an object, depth++. Track max across all paths.

3. **Has enums/constraints**: Single-pass DFS flag check for keys `"enum"`, `"minimum"`, `"maximum"`, `"pattern"`.

## 4. Encoding Strategies

### Strategy A: TOON HRV (Header-Row-Value)

Best for uniform object arrays. Example:

```json
// Input:
[{"id":1,"name":"Alice","role":"admin"},{"id":2,"name":"Bob","role":"user"}]

// TOON HRV:
items[2]{id,name,role}:
  1,Alice,admin
  2,Bob,user
```

Rules:
- First line: `{key}[{count}]{{{fields}}}`
- Each subsequent line: comma-separated values in field order
- Strings with spaces/commas get `\` escaping
- Empty/missing values become `-`

### Strategy B: Enhanced TOON

Best for schemas/configs with enums, ranges, patterns. Uses Level 3 rules:

| JSON | Enhanced TOON |
|------|--------------|
| `"type":"string"` | `string` (inline after key) |
| `"type":"boolean"` | `boolean` |
| `"type":"integer"` | `integer` |
| `"enum":["a","b","c"]` | `enum[a,b,c]` |
| `"minimum":1,"maximum":7` | `range[1,7]` |
| `"pattern":"^[a-z]+$"` | `pattern[^[a-z]+$]` |
| `"description":"long text"` | Appended after `\|` |

### Strategy C: CJSON Compact

Best for irregular mixed structures. Minimal transformation:
- Remove all non-semantic whitespace
- Keep JSON structural integrity (braces/brackets)
- Compact `true`/`false`/`null` to `T`/`F`/`~`
- Unquote safe string values (bare words matching `^[a-zA-Z_][a-zA-Z0-9_]*$`)

### Strategy D: ResponseCompressor Only

Best for already-small inputs (<200 chars). Skip encoding entirely, just apply structural compression (drop debug/null/empty, truncate strings/arrays).

## 5. Router Algorithm

```rust
fn select_strategy(shape: &JsonShape) -> EncodingStrategy {
    // Too small? Don't bother encoding.
    if shape.char_count < 200 {
        return ResponseCompressorOnly;
    }

    // Uniform array with ≥5 items? TOON HRV is king.
    if shape.is_uniform_array && shape.item_count >= 5 {
        return ToonHrv;
    }

    // Schema-like with enums or constraints? Enhanced TOON.
    if shape.has_enums || shape.has_constraints {
        return EnhancedToon;
    }

    // Deep chains (>3 levels)? TOON dot-path.
    if shape.max_chain_depth > 3 {
        return EnhancedToon; // shares dot-path logic
    }

    // Default: CJSON compact as safe fallback.
    CjsonCompact
}
```

## 6. CLI Integration

New subcommand:

```bash
tokenless compress-auto -f input.json          # auto-select strategy
tokenless compress-auto -f input.json --json    # output with strategy info
```

Output in `--json` mode:
```json
{
  "strategy": "toon-hrv",
  "compressed": "...",
  "savings": {"chars_before": 1000, "chars_after": 350, "pct": 65.0}
}
```

## 7. Implementation Plan

### New files:
- `crates/tokenless-schema/src/shape_analyzer.rs` (~100 lines, JsonShape + analyze function)
- `crates/tokenless-schema/src/encoding/toon_hrv.rs` (~80 lines, TOON HRV encoder)
- `crates/tokenless-schema/src/encoding/enhanced_toon.rs` (~120 lines, Enhanced TOON encoder)
- `crates/tokenless-schema/src/encoding/cjson_compact.rs` (~60 lines, CJSON compact encoder)
- `crates/tokenless-schema/src/encoding/mod.rs` (~10 lines, module re-exports)
- `crates/tokenless-schema/src/format_router.rs` (~50 lines, select_strategy + orchestrate)

### Changed files:
- `crates/tokenless-schema/src/lib.rs` — add `pub mod shape_analyzer` and `pub mod format_router`
- `crates/tokenless-cli/src/main.rs` — add `CompressAuto` subcommand

### No new dependencies — all encoders use existing `serde_json::Value`.

## 8. Test Strategy

| Test | What it verifies |
|------|-----------------|
| `test_analyze_uniform_array` | Shape detection: 10 identical objects → is_uniform_array=true |
| `test_analyze_mixed_array` | Shape detection: mixed types → is_uniform_array=false |
| `test_analyze_deep_chain` | Shape detection: 5-level chain → max_chain_depth=5 |
| `test_router_selects_hrv` | Router: uniform array → ToonHrv strategy |
| `test_router_selects_enhanced` | Router: schema with enums → EnhancedToon strategy |
| `test_router_selects_compact` | Router: irregular JSON → CjsonCompact strategy |
| `test_toon_hrv_roundtrip` | Encode uniform array → valid TOON → decode → same data |
| `test_enhanced_toon_enum` | Enum array → `enum[a,b,c]` format |
| `test_enhanced_toon_range` | min+max → `range[1,7]` format |
| `test_cjson_compact_safe` | Safe strings unquoted, special chars stay quoted |
| `test_compress_auto_integration` | End-to-end: JSON in → auto-compressed out |
