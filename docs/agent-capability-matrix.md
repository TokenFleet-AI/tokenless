# Tokenless Agent Capability Matrix

> Last updated: 2026-06-17

## Capability Overview

| Agent | Rewrite | Compress | Zero Round-trip | Stats Attribution | Auto-fix env-check | Install Method |
|-------|:-------:|:--------:|:---------------:|:-----------------:|:------------------:|---------------|
| **Claude Code** | ✅ RTK | ✅ Schema+Response | ✅ | ✅ session+project | ✅ | `tokenless init` |
| **Cursor** | ✅ RTK | ✅ Response | ✅ | ✅ | ✅ | `tokenless init --agent cursor` |
| **Windsurf** | ✅ RTK | ✅ Response | ✅ | ✅ | ✅ | `tokenless init --agent windsurf` |
| **Cline** | ✅ RTK | — | — | ✅ | ✅ | `tokenless init --agent cline` |
| **Kilo Code** | ✅ RTK | — | — | ✅ | ✅ | `tokenless init --agent kilocode` |
| **Antigravity** | ✅ RTK | — | — | ✅ | ✅ | `tokenless init --agent antigravity` |
| **Augment** | ✅ RTK | — | — | ✅ | ✅ | `tokenless init --agent augment` |
| **Hermes CLI** | ✅ RTK | ✅ Response | ✅ | ✅ | ✅ | `tokenless init --agent hermes` |
| **Pi** | ✅ RTK | ✅ Response | ✅ | ✅ | ✅ | `tokenless init --agent pi` |
| **Gemini CLI** | ✅ RTK | ✅ Response | ✅ | ✅ | ✅ | `tokenless init --agent gemini` |
| **OpenCode** | ✅ RTK | — | — | ✅ | ✅ | `tokenless init --agent opencode --global` |
| **Copilot** | ✅ RTK | — | — | ✅ | ✅ | `tokenless init --agent copilot` |
| **Codex CLI** | ✅ RTK | — | — | — | — | `tokenless init --agent codex` |

## Capability Definitions

| Capability | Description |
|-----------|-------------|
| **Rewrite** | Shell command → RTK equivalent (60-90% token savings for common CLI tools) |
| **Compress** | Schema compression (BeforeModel) + Response compression (PostToolUse) |
| **Zero Round-trip** | No extra API calls for compression — hooks run locally before/after tool use |
| **Stats Attribution** | Per-session and per-project tracking via `tokenless stats` |
| **Auto-fix** | `tokenless env-check --fix` installs missing dependencies |
| **Install Method** | One-command hook installation |

## Agent-Specific Notes

### Claude Code (claude)
- Full feature support: rewrite + compress + diff + stats + semantic
- `settings.json` hooks: `PreToolUse(Bash)` + `PostToolUse(*)`
- Supports `--debug` for compress debug logs

### Cursor (cursor)
- Response compression via `hooks.json`
- Command rewriting via cursor integration
- Supports project-level and global install

### Codex CLI (codex)
- Rules-file based: `AGENTS.md` + `RTK.md`
- No hook protocol — RTK usage instructions embedded in rules files
- Global install only (`tokenless init --agent codex --global`)

### Windsurf / Cline / Kilo Code / Antigravity / Augment
- RTK rewrite via rules/config files
- Stats tracking via `tokenless stats` database

### Hermes CLI / Pi
- Full hook protocol with response compression
- Plugin-based integration

### Gemini CLI / OpenCode
- Settings + hook file integration
- Global install recommended

### Copilot
- RTK rewrite via `.github/hooks/rtk-rewrite.json`
- No response compression (Copilot has its own context management)

## Quick Install

```bash
# Claude Code (most common)
tokenless init

# Other agents
tokenless init --agent cursor
tokenless init --agent gemini
tokenless init --agent codex --global

# Verify
tokenless doctor
```
