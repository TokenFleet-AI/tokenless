//! Help overlay — centered keybinding reference table.

use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Clear, Paragraph, Row, Table},
};

use crate::lang::Lang;

/// Render the help overlay centered on screen.
pub fn render(f: &mut Frame, lang: &Lang) {
    let area = f.area();

    // Centered overlay dimensions (about 44 cols wide, fits all rows)
    let width = 44.min(area.width.saturating_sub(4));
    let height = 15.min(area.height.saturating_sub(4));
    let x = (area.width - width) / 2;
    let y = (area.height - height) / 2;
    let overlay = Rect::new(x, y, width, height);

    // Clear the overlay area so stale content does not show behind
    f.render_widget(Clear, overlay);

    // Outer bordered block
    let block = Block::default()
        .title(lang.help_title())
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    // Content area inside the block border
    let inner = block.inner(overlay);

    // Split inner into table area and dismiss footer
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(inner);

    // ── Keybinding table ──
    let key_style = Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD);

    let rows = [
        Row::new(vec![
            Cell::from(Span::styled("Tab / \u{2190}\u{2192}", key_style)),
            Cell::from(lang.help_action("switch_tabs")),
        ]),
        Row::new(vec![
            Cell::from(Span::styled("\u{2191}\u{2193}", key_style)),
            Cell::from(lang.help_action("navigate")),
        ]),
        Row::new(vec![
            Cell::from(Span::styled("Enter", key_style)),
            Cell::from(lang.help_action("detail")),
        ]),
        Row::new(vec![
            Cell::from(Span::styled("/", key_style)),
            Cell::from(lang.help_action("search")),
        ]),
        Row::new(vec![
            Cell::from(Span::styled("t", key_style)),
            Cell::from(lang.help_action("time_range")),
        ]),
        Row::new(vec![
            Cell::from(Span::styled("e", key_style)),
            Cell::from(lang.help_action("export")),
        ]),
        Row::new(vec![
            Cell::from(Span::styled("c", key_style)),
            Cell::from(lang.help_action("config")),
        ]),
        Row::new(vec![
            Cell::from(Span::styled("?", key_style)),
            Cell::from(lang.help_action("help")),
        ]),
        Row::new(vec![
            Cell::from(Span::styled("q", key_style)),
            Cell::from(lang.help_action("quit")),
        ]),
    ];

    let table = Table::new(rows, [Constraint::Length(14), Constraint::Min(10)]).column_spacing(2);

    f.render_widget(table, chunks[0]);

    // ── Dismiss message ──
    let dismiss = Paragraph::new(Line::from(Span::styled(
        lang.help_dismiss(),
        Style::default().fg(Color::Gray),
    )))
    .alignment(Alignment::Center);

    f.render_widget(dismiss, chunks[1]);

    // Render the block border on top of the inner content
    f.render_widget(block, overlay);
}
