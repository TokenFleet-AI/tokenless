# Tokenless Testing Strategy

## Test Architecture

```
crates/tokenless-schema/src/
├── schema_compressor.rs   ← #[cfg(test)] mod tests
└── response_compressor.rs ← #[cfg(test)] mod tests

crates/tokenless-stats/src/
├── record.rs              ← #[cfg(test)] mod tests
├── recorder.rs            ← #[cfg(test)] mod tests
└── tokenizer.rs           ← #[cfg(test)] mod tests

crates/tokenless-cli/src/
├── env_check.rs           ← #[cfg(test)] mod tests
└── init/mod.rs            ← #[cfg(test)] mod tests

tests/
└── fixtures/
    ├── tool-schema.json           # Real-world OpenAI Function schema
    ├── response.json              # Typical API response
    └── response-large.json        # Large nested response
```

## Test Principles

1. **Unit tests in the same file** under `#[cfg(test)] mod tests` — immediate proximity to implementation
2. **Test name convention**: `test_should_<expected_behavior>` or `test_<scenario>_<expected>`
3. **Error paths covered explicitly**: every `Err` variant, every edge case
4. **Zero external dependencies**: tests use in-memory SQLite, inline JSON fixtures, no network
5. **UTF-8 and CJK covered**: explicit multi-byte character tests for truncation safety

## Schema Compressor Tests (8 tests)

| Test | Category | What It Covers |
|------|---------|---------------|
| `test_compress_long_description` | Functional | Description truncation at function (256) and parameter (160) levels |
| `test_protected_fields_preserved` | Regression | `name`, `type`, `required`, `enum`, `default`, `const` survive compression |
| `test_title_and_examples_removed` | Functional | `title` and `examples` removed at all nesting levels |
| `test_empty_schema_no_panic` | Edge case | null, {}, {"function": {}} handled without panic |
| `test_nested_properties_recursive_compression` | Functional | Deeply nested schema properties recursively compressed |
| `test_truncate_at_sentence_boundary` | Functional | Sentence-boundary-aware truncation preserves readability |
| `test_markdown_removal` | Functional | Code blocks and inline code stripped from descriptions |
| `test_anyof_oneof_allof_compression` | Functional | JSON Schema combinators (`anyOf`/`oneOf`/`allOf`) recursively compressed |
| `truncate_description_cjk_no_panic` | UTF-8 safety | 100+ repeated CJK characters truncated safely |
| `test_no_change_returns_original` | Optimization | Zero-savings guard returns identical object reference |

## Response Compressor Tests (8 tests)

| Test | Category | What It Covers |
|------|---------|---------------|
| `test_string_truncation` | Functional | Custom truncation limit applied |
| `test_string_truncation_512_default` | Functional | Default 512-char limit verified |
| `test_array_compression` | Functional | Array truncated at limit + truncation marker |
| `test_drop_fields` | Functional | All 8 debug field names removed |
| `test_drop_nulls` / `test_drop_nulls_disabled` | Configurable | Null removal toggle |
| `test_drop_empty_fields` | Functional | Empty strings/arrays/objects removed |
| `test_max_depth_truncation` | Functional | Deep nesting replaced with type marker |
| `test_nested_object_recursive_compression` | Integration | Nested compression with string + null rules |
| `test_preserve_primitives` | Regression | bool/number/short-string survive unchanged |
| `test_utf8_safe_truncation` | UTF-8 safety | CJK string truncation is valid UTF-8 |
| `test_no_change_returns_original` | Optimization | Zero-savings guard |

## Stats Tests (14 tests)

### record.rs (5 tests)
- `test_operation_type_from_str` — all 4 variants + unknown error
- `test_savings_calculation` — chars/tokens saved math + zero-division safety
- `test_record_with_text` — optional text fields
- `test_format_summary_line` — output format contains all expected fields

### recorder.rs (8 tests)
- `test_record_and_retrieve` — write + read round-trip
- `test_count` — empty (0) and single-record (1) counts
- `test_all_records_empty` — empty database returns empty vec
- `test_all_records_with_limit` — LIMIT clause applied
- `test_clear` — deletion + auto-increment reset
- `test_record_by_id_not_found` — None for missing ID
- `test_stats_summary_from_empty` — zero-state summary
- `test_stats_summary_calculation` — multi-record aggregation

### tokenizer.rs (tests)
- Token estimation from byte length (4:1 ratio)
- Upper/lower bound verification

## env_check Tests (10 tests)

| Test | Category | What It Covers |
|------|---------|---------------|
| `normalize_dep_simple_string` | Parser | String format: `"jq"` |
| `normalize_dep_version_string` | Parser | Version string: `"rtk>=0.35"` |
| `normalize_dep_object` | Parser | Object format with binary/package/manager |
| `normalize_deps_mixed_array` | Parser | Mixed string + object array |
| `normalize_deps_empty` | Parser | Empty array |
| `extract_required_version_ge` | Version | `>=0.35` → `0.35` |
| `version_ge_equal/greater/less` | Version | Semver comparison logic |
| `version_ge_prefixed_v` | Version | `v22.1.0` ≥ `16.0.0` |
| `build_json_result_ready/not_ready` | Output | JSON status format |
| `format_status_all` | Output | All 4 status labels |
| `load_spec_skips_meta_keys` | Parser | `_comment` keys filtered out |

## init/mod.rs Tests (2 tests)

| Test | Category | What It Covers |
|------|---------|---------------|
| `test_merge_into_new_settings` | Integration | Fresh settings.json creation |
| `test_merge_preserves_existing` | Integration | Existing keys preserved during merge |

## Test Fixtures

```json
// tests/fixtures/tool-schema.json — Realistic OpenAI Function schema
{
  "function": {
    "name": "search_documentation",
    "description": "A very long description...",
    "parameters": {
      "type": "object",
      "properties": {
        "query": {
          "type": "string",
          "description": "...",
          "examples": ["rust async"]
        }
      }
    }
  }
}
```

## Test Coverage Gaps and Improvements Needed

| Area | Current State | Recommendation |
|------|--------------|---------------|
| CLI integration tests | Missing | Add `assert_cmd` tests for each subcommand |
| Schema compressor batch mode | Missing | Test batch array processing end-to-end |
| env_check auto-fix | Missing | Mock the fix script execution |
| Hook protocol round-trip | Missing | Test stdin→stdout for each agent protocol |
| TOON round-trip | Missing | Test encode→decode cycle preserves data |
| Large input (100MB+) | Missing | Add streaming/stress tests |
| Concurrent stats access | Missing | Multi-threaded record + query test |
| CJK edge cases | Partial | Add mixed CJK+ASCII, emoji, RTL text |
| Stats migration | Missing | Test schema upgrade from old DB version |
| Windows paths | Missing | Test BOM stripping, path separators |

## Running Tests

```bash
make test                           # All tests
cargo test --lib                    # Unit tests only
cargo test --test integration       # Integration tests
cargo test schema_compressor        # Specific test module
cargo test test_should_             # Test name filter
```

## CI Integration

```yaml
# Planned .github/workflows/test.yml
test:
  runs-on: ubuntu-latest
  steps:
    - uses: actions/checkout@v4
    - uses: dtolnay/rust-toolchain@stable
    - run: make test
    - run: cargo clippy --all-targets --all-features -- -D warnings -W clippy::pedantic
    - run: cargo fmt --check
    - run: cargo audit
    - run: cargo deny check
```
