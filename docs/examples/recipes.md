# Tokenless Examples & Recipes

## Quick Start Recipes

### 1. Claude Code — Full Setup (3 minutes)
```bash
# Install
brew install tokenfleet/tap/tokenless
# or: cargo install tokenless

# Verify environment
tokenless doctor

# Install hooks
tokenless init

# Restart Claude Code, then verify:
tokenless status
```

### 2. See Compression in Action
```bash
tokenless demo
```

### 3. Check Weekly Savings
```bash
tokenless stats share
tokenless stats share --format markdown  # For sharing in PRs/issues
```

## Scenario Recipes

### Git Repository Polling (save 90-95%)
Claude Code polls `git status` every 30s. Differential response compression:
```bash
# Enabled by default when experimental mode is on
tokenless stats experimental-on
# Threshold: TOKENLESS_DIFF_THRESHOLD=0.7 (diff must be <70% of full output)
```

### Kubernetes Commands (save 70-85%)
```bash
# RTK automatically rewrites: kubectl, helm, docker, git, npm, cargo, etc.
# No manual config needed — just run commands normally via Claude Code
```

### Multi-Project Setup
```bash
# Project A (per-project stats isolation)
cd project-a && tokenless init

# Project B
cd project-b && tokenless init

# View per-project stats
tokenless stats summary --project tokenless
```

### Passthrough Mode (Baseline Measurement)
```bash
# Measure baseline token usage without compression
tokenless init --passthrough
# After collecting data, compare savings
```

## Maintenance

### Database Cleanup
```bash
tokenless stats delete --before 2026-01-01  # Remove old records
tokenless stats vacuum                       # Reclaim disk space
tokenless stats export -o backup.json        # Backup before cleanup
```

### Environment Troubleshooting
```bash
tokenless doctor          # Full diagnostic
tokenless env-check --all # Check tool dependencies
```
