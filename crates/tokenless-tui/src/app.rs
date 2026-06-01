//! Main TUI application state and event loop.

use std::time::Duration;

use ratatui::{
    Frame,
    crossterm::event::{self, Event, KeyCode, KeyEventKind},
};
use tokenless_stats::{StatsRecorder, StatsSummary};

use crate::{
    lang::Lang,
    ui,
    ui::{agents::AgentSummary, trends::DailyTotal},
};

/// Active tab in the TUI.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Tab {
    Dashboard,
    Records,
    Agents,
    Trends,
}

impl Tab {
    fn next(self) -> Self {
        match self {
            Tab::Dashboard => Tab::Records,
            Tab::Records => Tab::Agents,
            Tab::Agents => Tab::Trends,
            Tab::Trends => Tab::Dashboard,
        }
    }

    fn prev(self) -> Self {
        match self {
            Tab::Dashboard => Tab::Trends,
            Tab::Records => Tab::Dashboard,
            Tab::Agents => Tab::Records,
            Tab::Trends => Tab::Agents,
        }
    }
}

/// Time range filter for records.
#[derive(Debug, Clone, Copy, PartialEq)]
enum TimeRange {
    Today,
    ThisWeek,
    AllTime,
}

impl TimeRange {
    fn next(self) -> Self {
        match self {
            TimeRange::Today => TimeRange::ThisWeek,
            TimeRange::ThisWeek => TimeRange::AllTime,
            TimeRange::AllTime => TimeRange::Today,
        }
    }
}

/// Main application state.
pub struct App {
    recorder: StatsRecorder,
    refresh_secs: u64,
    lang: Lang,
    active_tab: Tab,
    summary: StatsSummary,
    records: Vec<tokenless_stats::StatsRecord>,
    filtered_records: Vec<tokenless_stats::StatsRecord>,
    selected_record: Option<usize>,
    selected_agent: Option<usize>,
    detail_record: Option<tokenless_stats::StatsRecord>,
    detail_agent: Option<String>,

    // Search / filter
    filter_text: String,
    time_range: TimeRange,
    search_mode: bool,
    export_msg: Option<String>,

    // Overlays
    show_help: bool,
    show_config: bool,
}

impl App {
    /// Create a new TUI application.
    #[must_use]
    pub fn new(recorder: StatsRecorder, refresh_secs: u64, lang: Lang) -> Self {
        Self {
            recorder,
            refresh_secs,
            lang,
            active_tab: Tab::Dashboard,
            summary: StatsSummary::default(),
            records: Vec::new(),
            filtered_records: Vec::new(),
            selected_record: None,
            selected_agent: None,
            detail_record: None,
            detail_agent: None,
            filter_text: String::new(),
            time_range: TimeRange::AllTime,
            search_mode: false,
            export_msg: None,
            show_help: false,
            show_config: false,
        }
    }

    /// Refresh data from the database.
    fn refresh(&mut self) {
        if let Ok(records) = self.recorder.all_records(None) {
            self.summary = StatsSummary::from_records(&records);
            self.records = records;
            self.apply_filters();
        }
    }

    /// Apply time range and text filter to produce filtered_records.
    fn apply_filters(&mut self) {
        let now = chrono::Local::now();

        self.filtered_records = self
            .records
            .iter()
            .filter(|r| {
                // Time range filter
                match self.time_range {
                    TimeRange::Today => r.timestamp.date_naive() == now.date_naive(),
                    TimeRange::ThisWeek => {
                        let days = (now.date_naive() - r.timestamp.date_naive())
                            .num_days()
                            .unsigned_abs();
                        days < 7
                    }
                    TimeRange::AllTime => true,
                }
            })
            .filter(|r| {
                // Text filter
                if self.filter_text.is_empty() {
                    return true;
                }
                let pattern = self.filter_text.to_lowercase();
                r.agent_id.to_lowercase().contains(&pattern)
                    || r.operation.as_str().contains(&pattern)
            })
            .cloned()
            .collect();
    }

    /// Build agent summaries from filtered records.
    fn compute_agent_summaries(&self) -> Vec<AgentSummary> {
        use std::collections::HashMap;
        let mut map: HashMap<String, AgentSummary> = HashMap::new();

        for r in &self.filtered_records {
            let entry = map
                .entry(r.agent_id.clone())
                .or_insert_with(|| AgentSummary {
                    agent_id: r.agent_id.clone(),
                    record_count: 0,
                    total_before_chars: 0,
                    total_after_chars: 0,
                    total_before_tokens: 0,
                    total_after_tokens: 0,
                });
            entry.record_count += 1;
            entry.total_before_chars += r.before_chars;
            entry.total_after_chars += r.after_chars;
            entry.total_before_tokens += r.before_tokens;
            entry.total_after_tokens += r.after_tokens;
        }

        let mut result: Vec<AgentSummary> = map.into_values().collect();
        result.sort_by_key(|a| std::cmp::Reverse(a.record_count));
        result
    }

    /// Build daily totals from filtered records.
    fn compute_daily_totals(&self) -> Vec<DailyTotal> {
        use std::collections::HashMap;
        let mut daily: HashMap<String, (u64, u64)> = HashMap::new();
        for r in &self.filtered_records {
            let date = r.timestamp.format("%Y-%m-%d").to_string();
            let entry = daily.entry(date).or_insert((0, 0));
            entry.0 += r.before_chars.saturating_sub(r.after_chars) as u64;
            entry.1 += r.before_tokens.saturating_sub(r.after_tokens) as u64;
        }
        let mut totals: Vec<DailyTotal> = daily
            .into_iter()
            .map(|(date, (chars, tokens))| DailyTotal {
                date,
                chars_saved: chars,
                tokens_saved: tokens,
            })
            .collect();
        totals.sort_by(|a, b| a.date.cmp(&b.date));
        totals
    }

    /// Export filtered records to a JSON file.
    fn export_json(&mut self) {
        let ts = chrono::Local::now().format("%Y%m%d_%H%M%S");
        let path = format!("tokenless-export_{ts}.json");
        #[allow(clippy::disallowed_methods)]
        match serde_json::to_string_pretty(&self.filtered_records) {
            Ok(json) => match std::fs::write(&path, &json) {
                Ok(()) => self.export_msg = Some(self.lang.export_success(&path)),
                Err(e) => self.export_msg = Some(self.lang.export_error(&e.to_string())),
            },
            Err(e) => self.export_msg = Some(self.lang.export_error(&e.to_string())),
        }
    }

    /// Run the TUI event loop.
    pub fn run(&mut self, terminal: &mut ratatui::DefaultTerminal) -> Result<(), String> {
        self.refresh();
        let tick_rate = Duration::from_secs(self.refresh_secs);

        loop {
            terminal
                .draw(|f| self.render(f))
                .map_err(|e| format!("Render error: {e}"))?;

            if event::poll(tick_rate).map_err(|e| format!("Event poll error: {e}"))?
                && let Event::Key(key) = event::read().map_err(|e| format!("Event read: {e}"))?
                && key.kind == KeyEventKind::Press
            {
                match key.code {
                    // ── Search mode ──
                    KeyCode::Char('/') if !self.search_mode => {
                        self.search_mode = true;
                        self.filter_text.clear();
                    }
                    KeyCode::Esc if self.search_mode => {
                        self.search_mode = false;
                    }
                    KeyCode::Enter | KeyCode::Tab if self.search_mode => {
                        self.search_mode = false;
                        self.apply_filters();
                    }
                    KeyCode::Backspace if self.search_mode => {
                        self.filter_text.pop();
                        self.apply_filters();
                    }
                    KeyCode::Char(c) if self.search_mode => {
                        self.filter_text.push(c);
                        self.apply_filters();
                    }

                    // ── Toggle help overlay (always respond to ?) ──
                    KeyCode::Char('?') | KeyCode::Char('/') => {
                        self.show_help = !self.show_help;
                    }
                    // ── Toggle config overlay (always respond to c) ──
                    KeyCode::Char('c') => {
                        self.show_config = !self.show_config;
                    }

                    // ── Dismiss overlays on Esc ──
                    KeyCode::Esc if self.show_help || self.show_config => {
                        self.show_help = false;
                        self.show_config = false;
                    }

                    // ── Ignore navigation keys while overlay is active ──
                    _ if self.show_help || self.show_config => {}

                    // ── Normal mode ──
                    KeyCode::Char('q') | KeyCode::Esc => break,
                    KeyCode::Char('h') | KeyCode::Tab => {
                        self.active_tab = self.active_tab.next();
                        self.detail_record = None;
                        self.detail_agent = None;
                    }
                    KeyCode::BackTab => {
                        self.active_tab = self.active_tab.prev();
                        self.detail_record = None;
                        self.detail_agent = None;
                    }
                    KeyCode::Up | KeyCode::Char('k') => self.scroll_up(),
                    KeyCode::Down | KeyCode::Char('j') => self.scroll_down(),
                    KeyCode::Enter => self.toggle_detail(),
                    KeyCode::Char('d') => {
                        self.detail_record = None;
                        self.detail_agent = None;
                    }
                    KeyCode::Char('t') => {
                        self.time_range = self.time_range.next();
                        self.apply_filters();
                    }
                    KeyCode::Char('e') => {
                        self.export_json();
                    }
                    _ => {}
                }

                // Clear export message after any key press
                if self.export_msg.is_some() {
                    self.export_msg = None;
                }
            }

            self.refresh();
        }
        Ok(())
    }

    fn scroll_up(&mut self) {
        match self.active_tab {
            Tab::Records => {
                if self.detail_record.is_some() {
                    return;
                }
                let idx = self.selected_record.unwrap_or(0);
                if idx > 0 {
                    self.selected_record = Some(idx - 1);
                }
            }
            Tab::Agents => {
                if self.detail_agent.is_some() {
                    return;
                }
                let idx = self.selected_agent.unwrap_or(0);
                if idx > 0 {
                    self.selected_agent = Some(idx - 1);
                }
            }
            _ => {}
        }
    }

    fn scroll_down(&mut self) {
        match self.active_tab {
            Tab::Records => {
                if self.detail_record.is_some() {
                    return;
                }
                let idx = self.selected_record.unwrap_or(0);
                if idx + 1 < self.filtered_records.len() {
                    self.selected_record = Some(idx + 1);
                }
            }
            Tab::Agents => {
                if self.detail_agent.is_some() {
                    return;
                }
                let summaries = self.compute_agent_summaries();
                let idx = self.selected_agent.unwrap_or(0);
                if idx + 1 < summaries.len() {
                    self.selected_agent = Some(idx + 1);
                }
            }
            _ => {}
        }
    }

    fn toggle_detail(&mut self) {
        match self.active_tab {
            Tab::Records => {
                if self.detail_record.is_some() {
                    self.detail_record = None;
                } else if let Some(idx) = self
                    .selected_record
                    .filter(|i| *i < self.filtered_records.len())
                {
                    self.detail_record = Some(self.filtered_records[idx].clone());
                }
            }
            Tab::Agents => {
                if self.detail_agent.is_some() {
                    self.detail_agent = None;
                } else if let Some(idx) = self.selected_agent {
                    let summaries = self.compute_agent_summaries();
                    if idx < summaries.len() {
                        self.detail_agent = Some(summaries[idx].agent_id.clone());
                    }
                }
            }
            _ => {}
        }
    }

    fn render(&self, f: &mut Frame) {
        // Overlay rendering takes priority over tabs
        if self.show_help {
            ui::help::render(f, &self.lang);
            return;
        }
        if self.show_config {
            let config = tokenless_stats::TokenlessConfig::load();
            ui::config::render(f, &config, &self.lang);
            return;
        }

        match self.active_tab {
            Tab::Dashboard => {
                ui::dashboard::render(f, &self.summary, &self.filtered_records, &self.lang);
            }
            Tab::Records => {
                if let Some(ref record) = self.detail_record {
                    ui::detail::render(f, record, &self.lang);
                } else {
                    let time_label = match self.time_range {
                        TimeRange::Today => self.lang.time_range_today(),
                        TimeRange::ThisWeek => self.lang.time_range_week(),
                        TimeRange::AllTime => self.lang.time_range_all(),
                    };
                    ui::records::render(
                        f,
                        &self.filtered_records,
                        self.selected_record,
                        &self.filter_text,
                        time_label,
                        self.search_mode,
                        &self.lang,
                    );
                }
            }
            Tab::Agents => {
                if let Some(ref agent_id) = self.detail_agent {
                    let summaries = self.compute_agent_summaries();
                    let summary = summaries.iter().find(|s| s.agent_id == *agent_id);
                    let ops = match summary {
                        Some(_s) => {
                            let mut result = Vec::new();
                            for r in &self.filtered_records {
                                if r.agent_id == *agent_id {
                                    result.push((
                                        r.operation.clone(),
                                        r.before_chars,
                                        r.after_chars,
                                    ));
                                }
                            }
                            result
                        }
                        None => Vec::new(),
                    };
                    let record_count = summary.map_or(0, |s| s.record_count);
                    ui::agent_detail::render(f, agent_id, record_count, &ops, &self.lang);
                } else {
                    let summaries = self.compute_agent_summaries();
                    let total = self.filtered_records.len();
                    ui::agents::render(f, &summaries, self.selected_agent, total, &self.lang);
                }
            }
            Tab::Trends => {
                let daily = self.compute_daily_totals();
                let daily_chars: Vec<u64> = daily.iter().map(|d| d.chars_saved).collect();
                let daily_tokens: Vec<u64> = daily.iter().map(|d| d.tokens_saved).collect();
                let date_labels: Vec<String> = daily.iter().map(|d| d.date.clone()).collect();
                ui::trends::render(f, &daily_chars, &daily_tokens, &date_labels, &self.lang);
            }
        }
    }
}
