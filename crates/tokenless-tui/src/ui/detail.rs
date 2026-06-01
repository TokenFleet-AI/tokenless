//! Detail panel — full before/after text for a single record.

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Wrap},
};
use tokenless_stats::StatsRecord;

use super::render_status_bar;
use crate::lang::Lang;

/// Render the record detail view.
pub fn render(f: &mut Frame, record: &StatsRecord, lang: &Lang) {
    let area = f.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),   // header
            Constraint::Ratio(1, 2), // before text
            Constraint::Ratio(1, 2), // after text
            Constraint::Length(1),   // status bar
        ])
        .split(area);

    // Header
    let op = lang.op_label(&record.operation);
    let savings_pct = if record.before_tokens > 0 {
        ((record.before_tokens - record.after_tokens) as f64 / record.before_tokens as f64
            * 100.0
            * 10.0)
            .round()
            / 10.0
    } else {
        0.0
    };
    let agent_label = lang.agent_label(&record.agent_id);
    let header = Paragraph::new(Line::from(Span::raw(lang.detail_header(
        record.id,
        op,
        agent_label,
        savings_pct,
    ))))
    .style(Style::default().bg(Color::DarkGray).fg(Color::White));
    f.render_widget(header, chunks[0]);

    // Before text
    let before_block = Block::default()
        .title(lang.detail_before())
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));
    let before_text = record
        .before_text
        .as_deref()
        .unwrap_or(lang.detail_no_text());
    let before_inner = before_block.inner(chunks[1]);
    f.render_widget(before_block, chunks[1]);
    f.render_widget(
        Paragraph::new(Text::from(Line::from(Span::raw(before_text))))
            .block(Block::default())
            .wrap(Wrap { trim: false }),
        before_inner,
    );

    // After text
    let after_block = Block::default()
        .title(lang.detail_after())
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green));
    let after_text = record
        .after_text
        .as_deref()
        .unwrap_or(lang.detail_no_text());
    let after_inner = after_block.inner(chunks[2]);
    f.render_widget(after_block, chunks[2]);
    f.render_widget(
        Paragraph::new(Text::from(Line::from(Span::raw(after_text))))
            .block(Block::default())
            .wrap(Wrap { trim: false }),
        after_inner,
    );

    render_status_bar(f, chunks[3], lang.detail_status_bar());
}
