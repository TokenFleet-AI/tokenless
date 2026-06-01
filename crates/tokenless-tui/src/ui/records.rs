//! Records panel — scrollable list of compression records.

use ratatui::{
    Frame,
    layout::{Constraint, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Row, Table},
};
use tokenless_stats::StatsRecord;

use super::{format_bytes, render_records_tabs, render_status_bar};
use crate::lang::Lang;

/// Render the records tab.
pub fn render(
    f: &mut Frame,
    records: &[StatsRecord],
    selected: Option<usize>,
    filter_text: &str,
    time_range_label: &str,
    search_mode: bool,
    lang: &Lang,
) {
    let area = f.area();
    let search_chunk = if search_mode { 1 } else { 0 };
    let chunks = Layout::default()
        .direction(ratatui::layout::Direction::Vertical)
        .constraints(
            [
                Constraint::Length(3), // tab bar
                if search_mode {
                    Constraint::Length(1)
                } else {
                    Constraint::Length(0)
                },
                Constraint::Length(3), // header
                Constraint::Min(1),    // record table
                Constraint::Length(1), // status bar
            ]
            .to_vec(),
        )
        .split(area);

    render_records_tabs(f, chunks[0], lang);

    // Search bar
    if search_mode {
        let search_bar = Paragraph::new(Line::from(Span::styled(
            format!("{} {}", lang.search_prompt(), filter_text),
            Style::default().fg(Color::Cyan),
        )))
        .block(
            Block::default()
                .borders(Borders::BOTTOM)
                .border_style(Style::default().fg(Color::Cyan)),
        );
        f.render_widget(search_bar, chunks[1]);
    }

    // Info header
    let has_filter = !filter_text.is_empty();
    let info_text = format!(
        "{} | {}",
        time_range_label,
        lang.records_info(records.len(), records.len(), has_filter),
    );
    let info = Paragraph::new(Line::from(Span::raw(info_text))).block(
        Block::default()
            .borders(Borders::BOTTOM)
            .border_style(Style::default().fg(Color::Gray)),
    );
    f.render_widget(info, chunks[1 + search_chunk]);

    // Record table
    let table_chunk = chunks[1 + search_chunk + 1];
    let status_chunk = chunks[1 + search_chunk + 2];
    let max_rows = (table_chunk.height as usize).saturating_sub(2);
    let rows: Vec<Row> = records
        .iter()
        .enumerate()
        .take(max_rows)
        .map(|(i, r)| {
            let ts = r.timestamp.format("%Y-%m-%d %H:%M:%S");
            let op = lang.op_label(&r.operation);
            let agent = lang.agent_label(&r.agent_id);
            let savings_pct = if r.before_tokens > 0 {
                (r.before_tokens - r.after_tokens) as f64 / r.before_tokens as f64 * 100.0
            } else {
                0.0
            };
            let style = if Some(i) == selected {
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            Row::new(vec![
                r.id.to_string(),
                ts.to_string(),
                op.to_string(),
                agent.to_string(),
                format_bytes(r.before_chars),
                format_bytes(r.after_chars),
                format!("{savings_pct:.1}%"),
            ])
            .style(style)
        })
        .collect();

    let header_style = Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD);
    let header = Row::new(vec![
        lang.records_col_id(),
        lang.records_col_time(),
        lang.records_col_op(),
        lang.records_col_agent(),
        lang.records_col_before(),
        lang.records_col_after(),
        lang.records_col_savings(),
    ])
    .style(header_style);

    let col_widths = [
        Constraint::Length(6),
        Constraint::Length(19),
        Constraint::Length(14),
        Constraint::Length(12),
        Constraint::Length(8),
        Constraint::Length(8),
        Constraint::Length(9),
    ];

    let table = Table::new(rows, col_widths)
        .header(header)
        .block(Block::default().borders(Borders::ALL));

    f.render_widget(table, table_chunk);

    render_status_bar(f, status_chunk, lang.records_status_bar());
}
