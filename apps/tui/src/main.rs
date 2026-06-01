//! Standalone TUI dashboard binary for tokenless.
//!
//! This binary launches the interactive terminal dashboard directly,
//! without going through the CLI subcommand.
#![allow(
    clippy::disallowed_methods,
    reason = "standalone binary, no tokio::fs needed"
)]

use std::process::ExitCode;

use tokenless_stats::StatsRecorder;
use tokenless_tui::Lang;

fn main() -> ExitCode {
    // Initialize file-based tracing
    let state_dir = dirs::state_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("tokenless");
    _ = std::fs::create_dir_all(&state_dir);

    let file_appender = tracing_appender::rolling::daily(&state_dir, "tui.log");
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "tokenless_tui=info".into()),
        )
        .with_writer(file_appender)
        .init();

    // Detect locale from environment
    let lang = match std::env::var("LANG").unwrap_or_default() {
        s if s.starts_with("zh") => Lang::Zh,
        _ => Lang::En,
    };

    // Initialize stats recorder with default database path
    let db_path = state_dir.join("stats.db");
    let recorder = match StatsRecorder::new(&db_path) {
        Ok(r) => r,
        Err(e) => {
            eprintln!(
                "Failed to open stats database at {}: {e}",
                db_path.display()
            );
            return ExitCode::from(1);
        }
    };

    // Run TUI
    let result = run_standalone(recorder, lang);

    if let Err(e) = result {
        eprintln!("Error: {e}");
        ExitCode::from(1)
    } else {
        ExitCode::from(0)
    }
}

fn run_standalone(recorder: StatsRecorder, lang: Lang) -> anyhow::Result<()> {
    // Install panic hook to restore terminal before panic output
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = crossterm::terminal::disable_raw_mode();
        let _ = crossterm::execute!(std::io::stderr(), crossterm::terminal::LeaveAlternateScreen);
        original_hook(info);
    }));

    // Setup terminal
    crossterm::terminal::enable_raw_mode()?;
    let mut stderr = std::io::stderr();
    crossterm::execute!(
        stderr,
        crossterm::terminal::EnterAlternateScreen,
        crossterm::event::EnableMouseCapture,
    )?;

    let backend = ratatui::backend::CrosstermBackend::new(stderr);
    let mut terminal = ratatui::Terminal::new(backend)?;

    // Use the tokenless_tui library to run the dashboard
    let result = tokenless_tui::run_tui(recorder, 1, lang);

    // Always restore terminal state
    let _ = crossterm::execute!(
        terminal.backend_mut(),
        crossterm::terminal::LeaveAlternateScreen,
        crossterm::event::DisableMouseCapture,
    );
    let _ = crossterm::terminal::disable_raw_mode();
    let _ = terminal.show_cursor();

    result.map_err(|e| anyhow::anyhow!("TUI error: {e}"))?;
    Ok(())
}
