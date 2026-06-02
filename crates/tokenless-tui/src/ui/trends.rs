//! Trends tab panel — daily savings sparklines and data table.

use std::collections::HashMap;

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Sparkline},
};
use tokenless_stats::StatsRecord;

use super::{format_bytes, render_status_bar, render_trends_tabs};
use crate::lang::Lang;

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
    records: &[StatsRecord],
    lang: &Lang,
    selected_project: Option<&str>,
) {
    let area = f.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // tab bar
            Constraint::Length(7), // chars sparkline
            Constraint::Length(7), // tokens sparkline
            Constraint::Min(1),    // daily data table
            Constraint::Length(4), // project breakdown
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
        render_sparkline_with_summary(
            f,
            chunks[1],
            lang.trends_header_chars(),
            daily_chars,
            Color::Cyan,
        );
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
        render_sparkline_with_summary(
            f,
            chunks[2],
            lang.trends_header_tokens(),
            daily_tokens,
            Color::Green,
        );
    }

    // ── Daily summary table ──
    render_daily_table(f, chunks[3], date_labels, daily_chars, daily_tokens);

    // ── Per-project breakdown ──
    render_project_breakdown(f, chunks[4], records);

    let status = format!(
        "📂 {} | {}",
        lang.project_label(selected_project),
        lang.trends_status_bar()
    );
    render_status_bar(f, chunks[5], &status);
}

/// Render a sparkline with a summary line showing total / max / days.
fn render_sparkline_with_summary(
    f: &mut Frame,
    area: Rect,
    title: &str,
    data: &[u64],
    color: Color,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(area);

    let sparkline = Sparkline::default()
        .block(Block::default().title(title).borders(Borders::ALL))
        .data(data)
        .style(Style::default().fg(color));
    f.render_widget(sparkline, chunks[0]);

    let total: u64 = data.iter().sum();
    let max = data.iter().max().copied().unwrap_or(0);
    let days = data.len();
    let summary = format!(
        " Total: {} | Max: {} | Days: {} ",
        format_bytes(total as usize),
        format_bytes(max as usize),
        days,
    );
    let p = Paragraph::new(Line::from(Span::styled(
        summary,
        Style::default().fg(Color::Gray),
    )));
    f.render_widget(p, chunks[1]);
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

/// Render per-project breakdown from filtered records.
fn render_project_breakdown(f: &mut Frame, area: Rect, records: &[StatsRecord]) {
    let block = Block::default()
        .title(" By Project ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));
    let inner = block.inner(area);
    f.render_widget(block, area);

    if records.is_empty() {
        return;
    }

    // Aggregate per project
    let mut projects: HashMap<String, (usize, u64, u64)> = HashMap::new();
    for r in records {
        let proj = r.project.as_deref().unwrap_or("(unassigned)").to_string();
        let entry = projects.entry(proj).or_default();
        entry.0 += 1;
        entry.1 += r.before_chars.saturating_sub(r.after_chars) as u64;
        entry.2 += r.before_tokens.saturating_sub(r.after_tokens) as u64;
    }

    let mut sorted: Vec<_> = projects.into_iter().collect();
    sorted.sort_by(|a, b| b.1.1.cmp(&a.1.1)); // sort by chars saved desc

    let mut lines: Vec<Line> = Vec::new();
    // Header
    lines.push(Line::from(Span::styled(
        format!(
            " {:<20} {:>6} {:>10} {:>10}",
            "Project", "Recs", "Chars", "Tokens"
        ),
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )));

    for (proj, (count, chars, tokens)) in sorted.iter().take(3) {
        // Truncate long project names at char boundary
        let name = if proj.chars().count() > 18 {
            let truncated: String = proj.chars().take(17).collect();
            format!("{truncated}…")
        } else {
            proj.clone()
        };
        lines.push(Line::from(Span::raw(format!(
            " {:<20} {:>6} {:>10} {:>10}",
            name,
            count,
            format_bytes(*chars as usize),
            format_bytes(*tokens as usize),
        ))));
    }

    f.render_widget(Paragraph::new(Text::from(lines)), inner);
}
