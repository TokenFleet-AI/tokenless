//! Tokenless TUI — interactive terminal dashboard for compression statistics.
//!
//! Provides a real-time, keyboard-navigable dashboard that displays token
//! savings data from the `tokenless-stats` SQLite database. Designed to be
//! launched as `tokenless tui` from the CLI.

#![forbid(unsafe_code)]
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::module_name_repetitions
)]

mod app;
pub mod lang;
pub mod ui;

pub use app::App;
pub use lang::Lang;
pub use tokenless_stats::StatsRecorder;

/// Run the TUI application with the given stats recorder and refresh interval.
///
/// Enters crossterm raw mode, runs the event loop, and restores the terminal
/// on exit. Returns an error string if terminal setup fails.
pub fn run_tui(recorder: StatsRecorder, refresh_secs: u64, lang: Lang) -> Result<(), String> {
    let mut terminal = ratatui::init();
    let res = App::new(recorder, refresh_secs, lang).run(&mut terminal);
    ratatui::restore();
    res.map_err(|e| format!("TUI error: {e}"))
}
