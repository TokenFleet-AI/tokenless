//! Agents panel — list of proxy agents with per-agent statistics.

use ratatui::{
    Frame,
    layout::{Constraint, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Row, Table},
};

use super::{format_bytes, render_agents_tabs, render_status_bar};
use crate::lang::Lang;

/// Summary statistics for a single proxy agent.
#[derive(Debug, Clone)]
pub struct AgentSummary {
    /// Unique agent identifier (e.g., "copilot-shell", "cli").
    pub agent_id: String,
    /// Number of compression records attributed to this agent.
    pub record_count: usize,
    /// Cumulative character count before compression.
    pub total_before_chars: usize,
    /// Cumulative character count after compression.
    pub total_after_chars: usize,
    /// Cumulative token count before compression.
    pub total_before_tokens: usize,
    /// Cumulative token count after compression.
    pub total_after_tokens: usize,
}

/// Render the agents tab.
pub fn render(
    f: &mut Frame,
    summaries: &[AgentSummary],
    selected: Option<usize>,
    total_records: usize,
    lang: &Lang,
) {
    let area = f.area();
    let chunks = Layout::default()
        .direction(ratatui::layout::Direction::Vertical)
        .constraints([
            Constraint::Length(3), // tab bar
            Constraint::Length(4), // info header
            Constraint::Min(1),    // agent table
            Constraint::Length(1), // status bar
        ])
        .split(area);

    render_agents_tabs(f, chunks[0], lang);

    // Info header — shows total agent count
    let info = Paragraph::new(Line::from(Span::raw(lang.agents_header(total_records)))).block(
        Block::default()
            .borders(Borders::BOTTOM)
            .border_style(Style::default().fg(Color::DarkGray)),
    );
    f.render_widget(info, chunks[1]);

    // Agent table — one row per agent
    let max_rows = (chunks[2].height as usize).saturating_sub(2);
    let rows: Vec<Row> = summaries
        .iter()
        .enumerate()
        .take(max_rows)
        .map(|(i, a)| {
            let agent = lang.agent_label(&a.agent_id);
            let chars_saved = a.total_before_chars.saturating_sub(a.total_after_chars);
            let tokens_saved = a.total_before_tokens.saturating_sub(a.total_after_tokens);
            let avg_pct = if a.total_before_chars > 0 {
                ((chars_saved as f64 / a.total_before_chars as f64) * 100.0 * 10.0).round() / 10.0
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
                agent.to_string(),
                a.record_count.to_string(),
                format_bytes(chars_saved),
                format!("{}", tokens_saved),
                format!("{avg_pct:.1}%"),
            ])
            .style(style)
        })
        .collect();

    let header_style = Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD);
    let header = Row::new(vec![
        lang.agents_col_agent(),
        lang.agents_col_records(),
        lang.agents_col_chars_saved(),
        lang.agents_col_tokens_saved(),
        "Avg %",
    ])
    .style(header_style);

    let col_widths = [
        Constraint::Length(16),
        Constraint::Length(10),
        Constraint::Length(13),
        Constraint::Length(13),
        Constraint::Length(8),
    ];

    let table = Table::new(rows, col_widths)
        .header(header)
        .block(Block::default().borders(Borders::ALL));

    f.render_widget(table, chunks[2]);

    render_status_bar(f, chunks[3], lang.agents_status_bar());
}
