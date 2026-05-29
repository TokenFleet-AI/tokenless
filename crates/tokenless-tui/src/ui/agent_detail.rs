//! Agent detail panel — per-operation breakdown for a single agent.
//!
//! Shows each operation the agent has performed with a visual bar
//! indicating the relative char savings compared to the most
//! impactful operation.

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph},
};
use tokenless_stats::OperationType;

use crate::lang::Lang;

use super::{format_bytes, render_status_bar};

/// Render the agent detail view.
pub fn render(
    f: &mut Frame,
    agent_id: &str,
    records: usize,
    ops: &[(OperationType, usize, usize)],
    lang: &Lang,
) {
    let area = f.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // header
            Constraint::Min(1),    // per-op breakdown
            Constraint::Length(1), // status bar
        ])
        .split(area);

    // Header
    let header = Paragraph::new(Line::from(Span::raw(
        lang.agent_detail_header(agent_id, records),
    )))
    .style(Style::default().bg(Color::DarkGray));
    f.render_widget(header, chunks[0]);

    // Per-operation breakdown block
    let block = Block::default()
        .title(lang.section_breakdown())
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));
    let inner = block.inner(chunks[1]);
    f.render_widget(block, chunks[1]);

    if ops.is_empty() {
        return;
    }

    // Scale bars relative to the operation with the highest chars_saved
    let max_saved = ops
        .iter()
        .map(|(_, _, saved)| *saved)
        .max()
        .unwrap_or(1)
        .max(1);
    let bar_len = 30;

    let mut lines: Vec<Line> = ops
        .iter()
        .map(|(op, count, chars_saved)| {
            let label = lang.op_label(op);
            let pct = *chars_saved as f64 / max_saved as f64 * 100.0;
            let filled = ((pct / 100.0) * bar_len as f64) as usize;
            let bar: String = format!(
                "{}{}",
                "█".repeat(filled.min(bar_len)),
                "░".repeat(bar_len.saturating_sub(filled))
            );

            Line::from(Span::raw(format!(
                " {label:14} {bar} {count:4}次  {}",
                format_bytes(*chars_saved),
            )))
        })
        .collect();

    // Trailing blank line
    lines.push(Line::from(Span::raw("")));

    f.render_widget(
        Paragraph::new(Text::from(lines)).block(Block::default()),
        inner,
    );

    render_status_bar(f, chunks[2], lang.agent_detail_status_bar());
}
