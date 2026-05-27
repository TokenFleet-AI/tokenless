# Tokenless Hook Protocol Specification

## Architecture

Tokenless integrates with AI coding agents through their hook/plugin systems. Each agent has a unique hook protocol; tokenless normalizes these into a unified pipeline.

```
┌──────────────────────────────────────────────────────────┐
│                   Tokenless CLI Binary                    │
│                                                          │
│  ┌─────────────┐  ┌──────────────┐  ┌────────────────┐  │
│  │ hook rewrite│  │ hook compress│  │ env-check      │  │
│  │ <agent>     │  │ (stdin)      │  │ --tool --json  │  │
│  └──────┬──────┘  └──────┬───────┘  └───────┬────────┘  │
│         │                │                   │           │
└─────────┼────────────────┼───────────────────┼───────────┘
          │                │                   │
    ┌─────┴─────┐    ┌─────┴─────┐      ┌──────┴──────┐
    │ PreToolUse│    │PostToolUse│      │  PreToolUse  │
    │ (rewrite) │    │ (compress)│      │ (env-check)  │
    └───────────┘    └───────────┘      └─────────────┘
```

## Protocol: Claude Code (Recommended)

### PreToolUse Hook (Command Rewriting)

**Agent invokes**: `tokenless hook rewrite claude`

**Input format** (stdin JSON):
```json
{
  "tool_name": "Bash",
  "tool_input": {
    "command": "git status"
  }
}
```

**Output format** (stdout JSON) — on rewrite:
```json
{
  "hookSpecificOutput": {
    "hookEventName": "PreToolUse",
    "permissionDecision": "allow",
    "permissionDecisionReason": "tokenless auto-rewrite",
    "updatedInput": {
      "command": "rtk git status"
    }
  }
}
```

**No-rewrite case**: Produce no output (empty stdout) — Claude Code passes through unchanged.

**Key property**: Zero additional round-trips. The hook modifies `updatedInput.command` and Claude Code executes the rewritten command directly.

### PostToolUse Hook (Response Compression)

**Agent invokes**: `tokenless hook compress`

**Input format** (stdin): Raw JSON response from tool execution.

**Output format** (stdout): Compressed JSON (or original if no savings).

**Skip conditions**: None explicit — PostToolUse matcher is `*` (all tools).

### Installation

```bash
tokenless init                    # Project-local (.claude/settings.json)
tokenless init --global           # Global (~/.claude/settings.json)
```

**Generated config** (.claude/settings.json):
```json
{
  "hooks": {
    "PreToolUse": [{
      "matcher": "Bash",
      "hooks": [{"type": "command", "command": "tokenless hook rewrite claude"}]
    }],
    "PostToolUse": [{
      "matcher": "*",
      "hooks": [{"type": "command", "command": "tokenless hook compress"}]
    }]
  }
}
```

## Protocol: Cursor

### PreToolUse Hook

**Agent invokes**: `tokenless hook rewrite cursor`

**Input format** (stdin JSON, UTF-8 BOM stripped):
```json
{
  "tool_name": "Shell",
  "tool_input": {
    "command": "git status"
  }
}
```

**Output format** (stdout JSON) — on rewrite:
```json
{
  "continue": true,
  "permission": "allow",
  "updated_input": {
    "command": "rtk git status"
  }
}
```

**No-rewrite case**: `{}` (empty JSON object).

**Installation**:
```bash
tokenless init --global --agent cursor  # → ~/.cursor/hooks.json
```

## Protocol: Gemini CLI

### BeforeTool Hook

**Agent invokes**: `tokenless hook rewrite gemini` (via shell script wrapper)

**Input format** (stdin JSON):
```json
{
  "tool_name": "run_shell_command",
  "tool_input": {
    "command": "git status"
  }
}
```

**Output format** (stdout JSON) — on rewrite:
```json
{
  "decision": "allow",
  "hookSpecificOutput": {
    "tool_input": {
      "command": "rtk git status"
    }
  }
}
```

**No-rewrite case**: `{"decision": "allow"}` (pass-through).

**Installation**:
```bash
tokenless init --agent gemini  # → .gemini/settings.json + .gemini/hooks/tokenless-hook-gemini.sh
```

The shell wrapper script (`tokenless-hook-gemini.sh`) is made executable (0o755) and referenced in settings.json since Gemini CLI requires an absolute path to the hook command.

## Protocol: GitHub Copilot

### Dual Format Detection

Copilot has two formats depending on the environment:

**CLI mode** (detected by presence of `toolName`):
```json
{
  "toolName": "shell",
  "toolArgs": "{\"command\":\"git status\"}"
}
```

**VS Code mode** (detected by `tool_name` key):
```json
{
  "tool_name": "Bash",
  "tool_input": {
    "command": "git status"
  }
}
```

### CLI Mode Output

```json
{
  "permissionDecision": "deny",
  "permissionDecisionReason": "Token savings: use `rtk git status` instead (rtk saves 60-90% tokens)"
}
```

Note: CLI mode uses `deny` as the action — the agent must re-execute with the suggested command (one extra round-trip).

### VS Code Mode Output

Same as Claude Code protocol (zero round-trip).

**Installation**:
```bash
tokenless init --agent copilot  # → .github/hooks/rtk-rewrite.json
```

## Protocol: Cline / Roo Code, Windsurf, Kilo Code, Antigravity, Augment

These agents do not support executable hooks — instead tokenless installs a **rules file** containing RTK usage instructions:

```
# RTK - Rust Token Killer ({agent_name})

Always prefix shell commands with `rtk` to minimize token consumption.

Examples:
  rtk git status
  rtk cargo test
  rtk ls src/
```

The agent reads these rules and self-enforces RTK usage. No hook protocol — behavior-based optimization.

**Installation**:

| Agent | Command | Output Path |
|-------|---------|-------------|
| Windsurf | `tokenless init --agent windsurf` | `.windsurfrules` |
| Cline | `tokenless init --agent cline` | `.clinerules` |
| Kilo Code | `tokenless init --agent kilocode` | `.kilocode/rules/rtk-rules.md` |
| Antigravity | `tokenless init --agent antigravity` | `.agents/rules/antigravity-rtk-rules.md` |
| Augment | `tokenless init --agent augment` | `.augment/rules/rtk.md` |

## Protocol: OpenCode

### Plugin Format

OpenCode uses a JSON plugin manifest with exec-based hooks:

```json
{
  "name": "tokenless-rewrite",
  "version": "0.1.0",
  "hooks": {
    "before_tool_call": {
      "exec": "tokenless rewrite {{command}}"
    },
    "tool_result_persist": {
      "exec": "tokenless compress-response"
    }
  }
}
```

**Global-only** — OpenCode plugins are installed to `~/.opencode/plugins/tokenless/`.

```bash
tokenless init --global --agent opencode
```

## Protocol: Hermes Agent

### Plugin Architecture (Python)

Hermes uses a Python plugin with three hooks:

```python
# __init__.py
def on_session_start(ctx):     # Record session ID
def pre_tool_call(ctx):        # Env-check + RTK rewrite
def transform_tool_result(ctx): # Response compress + TOON encode
```

### pre_tool_call Limitation

Hermes hooks cannot modify command parameters directly. The workflow is:

```
Agent: execute("kubectl get pods")
  → pre_tool_call: rtk rewrite → "rtk kubectl get pods"
  → return {action: "block", message: "建议使用 rtk kubectl get pods"}
Agent: execute("rtk kubectl get pods")  // re-execute with suggestion
  → pre_tool_call: already has rtk prefix, pass-through
```

One extra round-trip per command, but final token savings still achieved.

### transform_tool_result

- Skips content-retrieval tools, skill files, non-JSON, <200 char responses
- Step 1: ResponseCompressor via `tokenless compress-response`
- Step 2: TOON encoding via `tokenless compress-toon` (if enabled)
- Returns `None` when no compression achieved (agent uses original)

**Installation**:
```bash
make hermes-install  # → .hermes/plugins/tokenless/
```

## Protocol: OpenClaw

### Plugin Architecture (TypeScript)

```typescript
// index.ts
session_start        → record sessionId mapping
before_tool_call (p5) → Tool Ready env pre-check
before_tool_call (p10) → RTK command rewrite
tool_result_persist  → Response compress → TOON encode
```

### Priority Ordering

Two `before_tool_call` handlers run in priority order:
- **Priority 5**: Tool Ready — env-check first (may block execution)
- **Priority 10**: RTK Rewrite — command transformation

### Graceful Degradation

| Condition | Behavior |
|-----------|----------|
| `tokenless` binary missing | Skip compression, TOON, tool-ready |
| `rtk` binary missing | Skip rewrite only |
| RTK version < 0.35 | Skip rewrite, log warning |
| env-check returns UNKNOWN | Skip (tool not in spec) |
| env-check returns NOT_READY | Auto-fix attempt, block on failure |

### Configuration (openclaw.plugin.json)

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `rtk_enabled` | bool | true | Enable RTK command rewriting |
| `response_compression_enabled` | bool | true | Enable response compression |
| `tool_ready_enabled` | bool | true | Enable env pre-checks |
| `toon_compression_enabled` | bool | false | Enable TOON encoding (opt-in) |
| `skip_tools` | string[] | [Read, read_file, Glob, ...] | Tools excluded from compression |
| `verbose` | bool | false | Detailed logging |

## Protocol: Pi Agent

### Extension Format (TypeScript)

Pi uses a TypeScript extension file:

```typescript
// tokenless.ts
export function preToolUse(command: string): string {
  const result = await exec("tokenless rewrite " + command);
  return result || command;
}
```

**Installation**:
```bash
tokenless init --agent pi  # → .pi/agent/extensions/tokenless.ts
```

## Exit Codes

All hook subcommands follow consistent exit codes:

| Code | Meaning |
|------|---------|
| 0 | Success (rewrite applied or pass-through) |
| 1 | Usage/config error |
| 2 | Parse/serialization error |

Hook commands **never exit non-zero** for compression failures — output original content instead.

## Stats Tracking Across Protocols

All hook protocols support optional stats metadata via environment variables:

```bash
TOKENLESS_AGENT_ID=claude-code    # Agent identifier
TOKENLESS_SESSION_ID=abc123       # Session grouping
TOKENLESS_TOOL_USE_ID=call_xyz    # Tool call correlation
```

These are passed through by hooks that support environment configuration (Claude Code, OpenClaw, Hermes).
