# tokenless

LLM token optimization CLI — schema/response compression, command rewriting, TOON encoding, environment checks.

```bash
cargo install tokenless
```

## Commands

```bash
# Compress OpenAI Function Calling schemas
tokenless compress-schema -f tool.json

# Compress API responses
tokenless compress-response -f response.json

# TOON encoding
echo '{"name":"Alice"}' | tokenless compress-toon

# Command rewriting
tokenless rewrite "git status"
# → rtk git status

# Environment checks
tokenless env-check --all

# Compression statistics
tokenless stats summary

# Install hooks for AI coding agents
tokenless init
```

## Agent Integration

```bash
tokenless init                    # Claude Code (project)
tokenless init --global           # Claude Code (global)
tokenless init --agent cursor     # Cursor
tokenless init --agent windsurf   # Windsurf
```

Supports 11 agents: Claude, Cursor, Windsurf, Cline, Kilo Code, Antigravity, Augment, Hermes, Pi, Gemini, OpenCode.

License: Apache-2.0
