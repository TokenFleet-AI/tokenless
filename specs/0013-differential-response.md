# Spec 0013: Differential Response Compression

## Summary

When the same tool command runs repeatedly (e.g., `git status` polling every 30s),
instead of sending the full output each time, send only a unified diff from the
previous output. This saves 90-95% tokens for polling-style tool calls.

## Architecture Decision

A new `Diff` variant on `HookCommands` accepts a JSON payload of the form
`{"command": "<shell command>", "output": "<tool response text>"}` via stdin.
The hook stores the last output per command key in an in-process `HashMap` and,
on subsequent calls, computes a line-level unified diff. If the diff is shorter
than 70% of the full output, the diff is emitted; otherwise the full output is
sent as-is.

### Key Design Points

- **In-process baseline**: `LAST_RESPONSES` is a `Mutex<HashMap<String, (String, Instant)>>`.
  No persistence; baselines reset on process restart.
- **Threshold gating**: `TOKENLESS_DIFF_THRESHOLD` env var (default 0.7).
  Diff is only used when `diff_len < threshold * full_output_len`.
- **Unified diff**: Common-prefix/suffix detection with up to 3 lines of context
  on each side. Removed lines prefixed `-`, added lines prefixed `+`.
- **Unchanged outputs** emit the literal `(unchanged)` marker.

### Example

```
Input #1:  {"command":"git status","output":"M src/main.rs\n?? file.txt\n"}
Output #1: M src/main.rs\n?? file.txt\n

Input #2:  {"command":"git status","output":"M src/main.rs\nM cache.rs\n?? file.txt\n"}
Output #2: [diff from previous call — 2→3 lines, showing changes]
             M src/main.rs
           + M cache.rs
             ?? file.txt
```

## Threshold Design

The 70% threshold (`DEFAULT_DIFF_THRESHOLD = 0.7`) is chosen so that unless the
diff saves at least 30% of the token cost, the full output is preferred. This
avoids the overhead of interpreting diff syntax for large changes where the diff
representation would be comparable in size to the raw output. The threshold is
tunable via `TOKENLESS_DIFF_THRESHOLD` for environments with different cost
models.
