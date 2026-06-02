# tokenless-tui

Interactive terminal dashboard for tokenless compression statistics.

Part of the [tokenless](https://github.com/TokenFleet-AI/tokenless) toolkit.

## Screens

| Tab | Key | Description |
|-----|-----|-------------|
| Dashboard | `1` | Overview: total savings, compression ratio, top agents |
| Records | `2` | Recent compression records with filtering and search |
| Trends | `3` | Daily/weekly savings charts |
| Agents | `4` | Per-agent breakdown |

## Usage

```rust
use tokenless_tui::App;
use tokenless_stats::StatsRecorder;

let recorder = StatsRecorder::new("~/.tokenless/stats.db")?;
let mut app = App::new(recorder);

// Run the TUI event loop
let mut terminal = ratatui::init();
app.run(&mut terminal)?;
ratatui::restore();
```

### Keyboard Shortcuts

| Key | Action |
|-----|--------|
| `h` / `Tab` | Next tab |
| `Shift+Tab` | Previous tab |
| `p` | Project filter picker |
| `e` | Toggle experimental mode (config overlay) |
| `c` | Config panel |
| `f` | Time range filter |
| `/` | Search |
| `j`/`k` | Navigate |
| `Enter` | Detail view |
| `d` | Back from detail |
| `Esc` | Dismiss overlay |
| `q` | Quit |

## Minimum Rust Version

Rust 2024 edition, MSRV 1.89.

License: Apache-2.0
