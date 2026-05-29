//! Trends tab panel — daily savings sparklines and data table.

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Sparkline},
};

use crate::lang::Lang;

use super::{render_status_bar, render_trends_tabs};

/// A single day's aggregated savings totals.
#[derive(Debug, Clone)]
pub struct DailyTotal {
    /// Date string in YYYY-MM-DD format.
    pub date: String,
    /// Total characters saved on this day.
    pub chars_saved: u64,
    /// Total tokens saved on this day.
    pub tokens_saved: u64,
}

/// Render the trends tab.
pub fn render(
    f: &mut Frame,
    daily_chars: &[u64],
    daily_tokens: &[u64],
    date_labels: &[String],
    lang: &Lang,
) {
    let area = f.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // tab bar
            Constraint::Length(7), // chars sparkline
            Constraint::Length(7), // tokens sparkline
            Constraint::Min(1),    // daily data table
            Constraint::Length(1), // status bar
        ])
        .split(area);

    render_trends_tabs(f, chunks[0], lang);

    // ── Char savings sparkline ──
    if daily_chars.is_empty() {
        render_empty_block(
            f,
            chunks[1],
            lang.trends_header_chars(),
            lang.trends_no_data(),
        );
    } else {
        let sparkline = Sparkline::default()
            .block(
                Block::default()
                    .title(lang.trends_header_chars())
                    .borders(Borders::ALL),
            )
            .data(daily_chars)
            .style(Style::default().fg(Color::Cyan));
        f.render_widget(sparkline, chunks[1]);
    }

    // ── Token savings sparkline ──
    if daily_tokens.is_empty() {
        render_empty_block(
            f,
            chunks[2],
            lang.trends_header_tokens(),
            lang.trends_no_data(),
        );
    } else {
        let sparkline = Sparkline::default()
            .block(
                Block::default()
                    .title(lang.trends_header_tokens())
                    .borders(Borders::ALL),
            )
            .data(daily_tokens)
            .style(Style::default().fg(Color::Green));
        f.render_widget(sparkline, chunks[2]);
    }

    // ── Daily summary table ──
    render_daily_table(f, chunks[3], date_labels, daily_chars, daily_tokens);

    render_status_bar(f, chunks[4], lang.trends_status_bar());
}

/// Render a bordered block with a centred "no data" message.
fn render_empty_block(f: &mut Frame, area: Rect, title: &str, no_data: &str) {
    let block = Block::default().title(title).borders(Borders::ALL);
    let inner = block.inner(area);
    f.render_widget(block, area);
    f.render_widget(
        Paragraph::new(Text::from(Line::from(Span::styled(
            no_data,
            Style::default().fg(Color::DarkGray),
        ))))
        .centered(),
        inner,
    );
}

/// Render the daily summary table showing the last ~N days.
fn render_daily_table(
    f: &mut Frame,
    area: Rect,
    date_labels: &[String],
    daily_chars: &[u64],
    daily_tokens: &[u64],
) {
    let block = Block::default()
        .title(" Daily Summary ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));
    let inner = block.inner(area);
    f.render_widget(block, area);

    if date_labels.is_empty() {
        return;
    }

    let max_rows = (inner.height as usize).saturating_sub(1); // reserve 1 for header
    let start = date_labels.len().saturating_sub(max_rows);

    let mut lines: Vec<Line> = Vec::with_capacity(max_rows + 1);

    // Header row
    lines.push(Line::from(Span::styled(
        format!(" {:<12} {:>10} {:>10}", "Date", "Chars", "Tokens"),
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )));

    // Data rows (most recent days)
    for (i, label) in date_labels.iter().enumerate().skip(start) {
        let chars = daily_chars.get(i).copied().unwrap_or(0);
        let tokens = daily_tokens.get(i).copied().unwrap_or(0);
        lines.push(Line::from(Span::raw(format!(
            " {:<12} {:>10} {:>10}",
            label, chars, tokens,
        ))));
    }

    f.render_widget(Paragraph::new(Text::from(lines)), inner);
}
