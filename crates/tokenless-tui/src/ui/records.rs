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
#[allow(clippy::too_many_arguments)]
pub fn render(
    f: &mut Frame,
    records: &[StatsRecord],
    selected: Option<usize>,
    filter_text: &str,
    time_range_label: &str,
    search_mode: bool,
    lang: &Lang,
    selected_project: Option<&str>,
) {
    let area = f.area();
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

    // Info header (always the third chunk, index 2)
    let has_filter = !filter_text.is_empty();
    let info_text = format!(
        "{} | {} | 📂 {}",
        time_range_label,
        lang.records_info(records.len(), records.len(), has_filter),
        lang.project_label(selected_project),
    );
    let info = Paragraph::new(Line::from(Span::raw(info_text))).block(
        Block::default()
            .borders(Borders::BOTTOM)
            .border_style(Style::default().fg(Color::Gray)),
    );
    f.render_widget(info, chunks[2]);

    // Record table (index 3) and status bar (index 4)
    let table_chunk = chunks[3];
    let status_chunk = chunks[4];
    let max_rows = (table_chunk.height as usize).saturating_sub(2);
    let rows: Vec<Row> = records
        .iter()
        .enumerate()
        .take(max_rows)
        .map(|(i, r)| {
            let ts = r.timestamp.format("%Y-%m-%d %H:%M:%S");
            let op = if r.experimental_mode {
                format!("{} ⚡", lang.op_label(&r.operation))
            } else {
                lang.op_label(&r.operation).to_string()
            };
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
            let proj = r.project.as_deref().unwrap_or("-");
            // Extract command from before_text JSON for rewrite-command records
            let cmd = extract_command(&r.before_text);
            Row::new(vec![
                r.id.to_string(),
                ts.to_string(),
                proj.to_string(),
                op.to_string(),
                cmd,
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
        "Project",
        lang.records_col_op(),
        "Command",
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
        Constraint::Length(20),
        Constraint::Length(10),
        Constraint::Length(7),
        Constraint::Length(7),
        Constraint::Length(7),
    ];

    let table = Table::new(rows, col_widths)
        .header(header)
        .block(Block::default().borders(Borders::ALL));

    f.render_widget(table, table_chunk);

    let status = format!(
        "📂 {} | {}",
        lang.project_label(selected_project),
        lang.records_status_bar()
    );
    render_status_bar(f, status_chunk, &status);
}

/// Extract a human-readable command from the stored before_text (hook payload JSON).
fn extract_command(before_text: &Option<String>) -> String {
    before_text
        .as_deref()
        .and_then(|text| {
            serde_json::from_str::<serde_json::Value>(text)
                .ok()
                .and_then(|v| {
                    v.pointer("/tool_input/command")
                        .and_then(|c| c.as_str())
                        .map(|s| {
                            // Truncate long commands at char boundary
                            if s.chars().count() > 28 {
                                let truncated: String = s.chars().take(27).collect();
                                format!("{truncated}…")
                            } else {
                                s.to_string()
                            }
                        })
                })
        })
        .unwrap_or_else(|| "-".to_string())
}
