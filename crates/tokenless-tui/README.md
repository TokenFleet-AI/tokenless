# tokenless-tui

[![Crates.io](https://img.shields.io/crates/v/tokenless-tui.svg)](https://crates.io/crates/tokenless-tui)
[![Docs](https://docs.rs/tokenless-tui/badge.svg)](https://docs.rs/tokenless-tui)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://github.com/TokenFleet-AI/tokenless/blob/master/LICENSE)

![tokenless](https://raw.githubusercontent.com/TokenFleet-AI/tokenless/master/assets/tokenless.svg)

Interactive terminal dashboard for tokenless compression statistics.

Part of the [tokenless](https://github.com/TokenFleet-AI/tokenless) toolkit.

## Why tokenless-tui?

`tokenless stats summary` gives you numbers. The TUI gives you insight — browse records, filter by project, compare agent performance, and track savings trends over time. It reads directly from the same SQLite database that the CLI writes to, so there's zero setup beyond enabling recording.

## Quick Start

```toml
[dependencies]
tokenless-tui = "0.4"
```

```rust
use tokenless_tui::{run_tui, App};
use tokenless_tui::lang::Lang;
use tokenless_stats::StatsRecorder;

let recorder = StatsRecorder::new("~/.tokenless/stats.db")?;

// One-liner: auto-creates terminal, runs event loop, restores on exit
run_tui(recorder, 1, Lang::Zh)?;

// Or manual control:
let mut app = App::new(recorder, 1, Lang::from_env());
let mut terminal = ratatui::init();
app.run(&mut terminal)?;
ratatui::restore();
```

Set language via env: `TOKENLESS_LANG=zh` or `TOKENLESS_LANG=en`.

## Screens

| Tab | Key | Description |
|-----|-----|-------------|
| Dashboard | `1` | Overview: total savings, compression ratio, top agents |
| Records | `2` | Recent compression records with filtering, search, and project picker |
| Trends | `3` | Daily/weekly savings charts |
| Agents | `4` | Per-agent breakdown |

### Overlays

| Overlay | Key | Description |
|---------|-----|-------------|
| Help | `?` | Keyboard shortcut reference |
| Config | `c` | Stats toggle, cache size, threshold, experimental mode |
| Project Picker | `p` | Filter records by project name |
| Agent Detail | `Enter` | Detail view for a record |

## Keyboard Shortcuts

| Key | Action |
|-----|--------|
| `h` / `Tab` | Next tab |
| `Shift+Tab` | Previous tab |
| `↑` `↓` / `j` `k` | Navigate lists |
| `Enter` | Detail view |
| `d` | Back from detail |
| `?` | Help overlay |
| `p` | Project filter picker |
| `t` | Time range filter (cycles presets) |
| `/` | Search records |
| `e` | Export records to JSON |
| `c` | Config panel |
| `e` (in config) | Toggle experimental mode |
| `Esc` | Dismiss overlay |
| `q` | Quit |

## Minimum Rust Version

Rust 2024 edition, MSRV 1.89.

License: Apache-2.0
