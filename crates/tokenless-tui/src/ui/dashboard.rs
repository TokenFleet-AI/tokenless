//! Dashboard panel — summary overview with stat cards and recent activity.

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph},
};
use tokenless_stats::{OperationType, StatsRecord, StatsSummary};

use super::{format_bytes, render_dashboard_tabs, render_status_bar};
use crate::lang::Lang;

/// Render the dashboard tab.
pub fn render(
    f: &mut Frame,
    summary: &StatsSummary,
    records: &[StatsRecord],
    lang: &Lang,
    selected_project: Option<&str>,
) {
    let area = f.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // tab bar
            Constraint::Length(10), // stat cards (was 10, +1 for project indicator)
            Constraint::Length(8),  // per-op breakdown
            Constraint::Min(1),     // recent activity
            Constraint::Length(1),  // status bar
        ])
        .split(area);

    render_dashboard_tabs(f, chunks[0], lang);
    render_stat_cards(f, chunks[1], summary, lang, selected_project);
    render_breakdown(f, chunks[2], records, lang);
    render_recent(f, chunks[3], records, lang);
    let status = format!(
        "Project: {} | {}",
        lang.project_label(selected_project),
        lang.dashboard_status_bar()
    );
    render_status_bar(f, chunks[4], &status);
}

fn render_stat_cards(
    f: &mut Frame,
    area: Rect,
    summary: &StatsSummary,
    lang: &Lang,
    selected_project: Option<&str>,
) {
    // Split: project indicator + 3 stat cards
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(1)])
        .split(area);

    // Project indicator line
    let proj_text = format!(" 📂 {}", lang.project_label(selected_project));
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            proj_text,
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ))),
        rows[0],
    );

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Ratio(1, 3),
            Constraint::Ratio(1, 3),
            Constraint::Ratio(1, 3),
        ])
        .split(rows[1]);

    let total_saved = summary.chars_saved();
    let schema_saved = summary
        .total_before_chars
        .saturating_sub(summary.total_after_chars);
    let savings_pct = if summary.total_before_chars > 0 {
        (schema_saved as f64 / summary.total_before_chars as f64 * 100.0 * 10.0).round() / 10.0
    } else {
        0.0
    };
    let record_count = summary.total_records;

    let cards = [
        (
            lang.stat_total_saved(),
            format_bytes(total_saved),
            Color::Cyan,
        ),
        (
            lang.stat_total_records(),
            record_count.to_string(),
            Color::Green,
        ),
        (
            lang.stat_avg_savings(),
            format!("{savings_pct:.1}%"),
            Color::Yellow,
        ),
    ];

    for (i, (title, value, color)) in cards.iter().enumerate() {
        let block = Block::default()
            .borders(Borders::ALL)
            .title(*title)
            .border_style(Style::default().fg(*color));
        let inner = block.inner(chunks[i]);
        f.render_widget(block, chunks[i]);
        f.render_widget(
            Paragraph::new(Text::from(Line::from(Span::styled(
                value.clone(),
                Style::default()
                    .fg(*color)
                    .add_modifier(ratatui::style::Modifier::BOLD),
            ))))
            .centered()
            .block(Block::default()),
            inner,
        );
    }
}

fn render_breakdown(f: &mut Frame, area: Rect, records: &[StatsRecord], lang: &Lang) {
    let block = Block::default()
        .title(lang.section_breakdown())
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Gray));
    let inner = block.inner(area);
    f.render_widget(block, area);

    if records.is_empty() {
        return;
    }

    // Compute per-operation stats from actual records
    let ops = [
        OperationType::CompressResponse,
        OperationType::CompressSchema,
        OperationType::CompressToon,
        OperationType::RewriteCommand,
    ];

    let mut lines = Vec::new();
    for op in &ops {
        let op_records: Vec<_> = records.iter().filter(|r| &r.operation == op).collect();
        if op_records.is_empty() {
            continue;
        }
        let before: usize = op_records.iter().map(|r| r.before_chars).sum();
        let after: usize = op_records.iter().map(|r| r.after_chars).sum();
        let saved = before.saturating_sub(after);
        let op_pct = if before > 0 {
            (saved as f64 / before as f64 * 100.0 * 10.0).round() / 10.0
        } else {
            0.0
        };

        let label = lang.op_label(op);
        let bar_len: usize = 30;
        let filled = ((op_pct / 100.0) * bar_len as f64).min(bar_len as f64) as usize;
        let bar: String = format!(
            "{}{}",
            "█".repeat(filled),
            "░".repeat(bar_len.saturating_sub(filled))
        );

        lines.push(Line::from(Span::raw(format!(
            " {label:14} {bar} {op_pct:5.1}%  ({})",
            format_bytes(saved),
        ))));
    }

    f.render_widget(
        Paragraph::new(Text::from(lines)).block(Block::default()),
        inner,
    );
}

fn render_recent(f: &mut Frame, area: Rect, records: &[StatsRecord], lang: &Lang) {
    let block = Block::default()
        .title(lang.section_recent())
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Gray));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let max_rows = (inner.height as usize).saturating_sub(1);
    let lines: Vec<Line> = records
        .iter()
        .take(max_rows)
        .map(|r| {
            let ts = r.timestamp.format("%m-%d %H:%M:%S");
            let op_label = lang.op_label(&r.operation);
            let op = if r.experimental_mode {
                format!("{:14}", format!("{op_label} ⚡"))
            } else {
                format!("{:14}", op_label)
            };
            let agent = lang.agent_label(&r.agent_id);
            let savings_pct = if r.before_tokens > 0 {
                ((r.before_tokens - r.after_tokens) as f64 / r.before_tokens as f64 * 100.0 * 10.0)
                    .round()
                    / 10.0
            } else {
                0.0
            };
            let proj = r.project.as_deref().unwrap_or("-");
            Line::from(Span::raw(format!(
                " {ts}  {op}  {proj:12}  {agent:12}  {}▸{}  -{savings_pct:.1}%",
                format_bytes(r.before_chars),
                format_bytes(r.after_chars),
            )))
        })
        .collect();

    f.render_widget(Paragraph::new(Text::from(lines)), inner);
}
