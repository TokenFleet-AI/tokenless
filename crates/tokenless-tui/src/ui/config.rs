//! Config panel — displays the current tokenless configuration.

use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};
use tokenless_stats::TokenlessConfig;

use crate::lang::Lang;

/// Render the config overlay centered on screen.
pub fn render(f: &mut Frame, config: &TokenlessConfig, lang: &Lang) {
    let area = f.area();

    // Centered overlay dimensions
    let width = 46.min(area.width.saturating_sub(4));
    let height = 10.min(area.height.saturating_sub(4));
    let x = (area.width - width) / 2;
    let y = (area.height - height) / 2;
    let overlay = Rect::new(x, y, width, height);

    // Clear the overlay area
    f.render_widget(Clear, overlay);

    // Outer bordered block
    let block = Block::default()
        .title(lang.config_title())
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    // Content area inside the block border
    let inner = block.inner(overlay);

    // Build config content lines
    let stats_value = if config.is_stats_enabled() {
        lang.config_enabled()
    } else {
        lang.config_disabled()
    };

    let bold = Style::default().add_modifier(Modifier::BOLD);
    let green = Style::default().fg(Color::Green);

    let cache_size = std::env::var("TOKENLESS_CACHE_SIZE").unwrap_or_else(|_| "512".to_string());
    let diff_threshold =
        std::env::var("TOKENLESS_DIFF_THRESHOLD").unwrap_or_else(|_| "0.7".to_string());

    let lines = vec![
        Line::from(vec![
            Span::styled(format!("{}:  ", lang.config_stats()), bold),
            Span::styled(stats_value, green),
        ]),
        Line::from(vec![
            Span::styled(format!("{}:  ", lang.config_cache()), bold),
            Span::raw(cache_size),
        ]),
        Line::from(vec![
            Span::styled(format!("{}:  ", lang.config_threshold()), bold),
            Span::raw(diff_threshold),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            lang.config_dismiss(),
            Style::default().fg(Color::Gray),
        )),
    ];

    f.render_widget(Paragraph::new(lines), inner);
    f.render_widget(block, overlay);
}
