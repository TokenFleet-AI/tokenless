//! UI panels for the tokenless TUI dashboard.

pub mod agent_detail;
pub mod agents;
pub mod config;
pub mod dashboard;
pub mod detail;
pub mod help;
pub mod records;
pub mod trends;

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use crate::lang::Lang;

/// Shared tab header rendered at the top of every panel.
pub fn render_tabs(f: &mut Frame, area: Rect, tab_displays: &[&str], active_idx: usize) {
    let titles: Vec<Line> = tab_displays
        .iter()
        .enumerate()
        .map(|(i, display)| {
            let prefix = if i == active_idx { "▸ " } else { "  " };
            let style = if i == active_idx {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Gray)
            };
            Line::from(Span::styled(format!("{prefix}{display}"), style))
        })
        .collect();

    let block = Block::default()
        .borders(Borders::BOTTOM)
        .border_style(Style::default().fg(Color::Gray));
    let inner = block.inner(area);
    let constraints: Vec<Constraint> = (0..tab_displays.len())
        .map(|_| Constraint::Length(20))
        .collect();
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(constraints)
        .horizontal_margin(2)
        .split(inner);

    for (i, title) in titles.iter().enumerate() {
        f.render_widget(Paragraph::new(title.clone()), chunks[i]);
    }
    f.render_widget(block, area);
}

/// Shared tab header — dashboard tab set (index 0 active).
pub fn render_dashboard_tabs(f: &mut Frame, area: Rect, lang: &Lang) {
    render_tabs(
        f,
        area,
        &[
            lang.tab_dashboard(),
            lang.tab_records_inactive(),
            lang.tab_agents_inactive(),
            lang.tab_trends_inactive(),
        ],
        0,
    );
}

/// Shared tab header — records tab set (index 1 active).
pub fn render_records_tabs(f: &mut Frame, area: Rect, lang: &Lang) {
    render_tabs(
        f,
        area,
        &[
            lang.tab_dashboard_inactive(),
            lang.tab_records(),
            lang.tab_agents_inactive(),
            lang.tab_trends_inactive(),
        ],
        1,
    );
}

/// Shared tab header — agents tab set (index 2 active).
pub fn render_agents_tabs(f: &mut Frame, area: Rect, lang: &Lang) {
    render_tabs(
        f,
        area,
        &[
            lang.tab_dashboard_inactive(),
            lang.tab_records_inactive(),
            lang.tab_agents(),
            lang.tab_trends_inactive(),
        ],
        2,
    );
}

/// Shared tab header — trends tab set (index 3 active).
pub fn render_trends_tabs(f: &mut Frame, area: Rect, lang: &Lang) {
    render_tabs(
        f,
        area,
        &[
            lang.tab_dashboard_inactive(),
            lang.tab_records_inactive(),
            lang.tab_agents_inactive(),
            lang.tab_trends(),
        ],
        3,
    );
}

/// Render the status bar at the bottom.
pub fn render_status_bar(f: &mut Frame, area: Rect, text: &str) {
    let bar = Paragraph::new(Line::from(Span::styled(
        text,
        Style::default().fg(Color::Gray),
    )));
    f.render_widget(bar, area);
}

/// Format bytes with human-readable suffix.
#[must_use]
pub fn format_bytes(n: usize) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;

    /// Returns the concatenated text of every non-empty, non-border cell.
    fn collect_text(buf: &ratatui::buffer::Buffer) -> String {
        let mut s = String::new();
        for cell in buf.content.iter() {
            let ch = cell.symbol();
            let trimmed = ch.trim();
            if !trimmed.is_empty()
                && trimmed != "\u{2500}"
                && trimmed != "\u{2502}"
                && trimmed != "\u{251c}"
                && trimmed != "\u{2524}"
                && trimmed != "\u{252c}"
                && trimmed != "\u{2534}"
                && trimmed != "\u{250c}"
                && trimmed != "\u{2510}"
                && trimmed != "\u{2514}"
                && trimmed != "\u{2518}"
            {
                s.push_str(trimmed);
            }
        }
        s
    }

    #[test]
    fn test_render_tabs_four_items() {
        let backend = TestBackend::new(100, 3);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                let area = Rect::new(0, 0, 100, 3);
                render_tabs(f, area, &["Dashboard", "Records", "Agents", "Trends"], 1);
            })
            .unwrap();
        let content = collect_text(terminal.backend().buffer());
        assert!(
            content.contains("Dashboard"),
            "Dashboard tab should be present; got: {content:?}"
        );
        assert!(
            content.contains("Records"),
            "Records tab should be present; got: {content:?}"
        );
        assert!(
            content.contains("Agents"),
            "Agents tab should be present; got: {content:?}"
        );
        assert!(
            content.contains("Trends"),
            "Trends tab should be present; got: {content:?}"
        );
    }
}
