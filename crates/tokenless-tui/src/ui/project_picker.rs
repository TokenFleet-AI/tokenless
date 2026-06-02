//! Project picker overlay — centered selection list with keyboard navigation.

use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
};

use crate::lang::Lang;

/// Truncate a string to at most `max_width` characters, appending "…" when clipped.
fn truncate_str(s: &str, max_width: usize) -> String {
    if s.chars().count() <= max_width {
        s.to_string()
    } else {
        let mut result: String = s.chars().take(max_width.saturating_sub(1)).collect();
        result.push('\u{2026}'); // ellipsis
        result
    }
}

/// Render the project picker overlay centered on screen.
pub fn render(f: &mut Frame, all_projects: &[String], cursor: usize, lang: &Lang) {
    let area = f.area();

    let width = 40.min(area.width.saturating_sub(4));
    // Title (1 border) + 1 all-projects + N projects + footer (1 border) + 1 border
    let item_count = 1 + all_projects.len();
    let height = (item_count + 3).min(area.height.saturating_sub(4) as usize) as u16;
    let x = (area.width - width) / 2;
    let y = (area.height - height) / 2;
    let overlay = Rect::new(x, y, width, height);

    // Clear the overlay area
    f.render_widget(Clear, overlay);

    // Outer bordered block
    let block = Block::default()
        .title(lang.project_picker_title())
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let inner = block.inner(overlay);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(inner);

    // Build list items
    let mut items: Vec<ListItem> = Vec::new();

    // First item: "All Projects"
    let all_style = if cursor == 0 {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };
    items.push(ListItem::new(Line::from(Span::styled(
        format!("  {}", lang.project_all()),
        all_style,
    ))));

    // Project items — truncate names to fit the overlay width
    // 4 chars indent + 2 chars border/buffer ≈ 6 chars overhead
    let max_name_width = (width as usize).saturating_sub(6);
    for (i, proj) in all_projects.iter().enumerate() {
        let item_cursor = i + 1;
        let style = if cursor == item_cursor {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        let display = truncate_str(proj, max_name_width);
        items.push(ListItem::new(Line::from(Span::styled(
            format!("    {display}"),
            style,
        ))));
    }

    let list = List::new(items);
    f.render_widget(list, chunks[0]);

    // Footer with dismiss hint
    let footer = Paragraph::new(Line::from(Span::styled(
        lang.project_dismiss_hint(),
        Style::default().fg(Color::Gray),
    )))
    .alignment(Alignment::Center);
    f.render_widget(footer, chunks[1]);

    // Render the block border on top of inner content
    f.render_widget(block, overlay);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate_str_short() {
        assert_eq!(truncate_str("hello", 10), "hello");
    }

    #[test]
    fn test_truncate_str_exact_fit() {
        assert_eq!(truncate_str("hello", 5), "hello");
    }

    #[test]
    fn test_truncate_str_long() {
        // "hello world" = 11 chars, max 8 → 7 chars + ellipsis = 8 chars
        let result = truncate_str("hello world", 8);
        assert!(
            result.chars().count() <= 8,
            "result char count should not exceed max_width"
        );
        assert!(result.ends_with('\u{2026}'), "should end with ellipsis");
    }

    #[test]
    fn test_truncate_str_empty() {
        assert_eq!(truncate_str("", 5), "");
    }
}
