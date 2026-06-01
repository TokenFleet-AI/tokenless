//! Locale / i18n support for tokenless TUI.
//!
//! Provides all user-facing strings in Chinese (default) and English.

use tokenless_stats::OperationType;

/// Supported UI languages.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Lang {
    /// 中文（默认）
    Zh,
    /// English
    En,
}

impl Lang {
    /// Resolve language from env var `TOKENLESS_LANG` or default to Zh.
    #[must_use]
    pub fn from_env() -> Self {
        match std::env::var("TOKENLESS_LANG").as_deref() {
            Ok("en" | "EN" | "en-US" | "en-US.UTF-8") => Lang::En,
            _ => Lang::Zh,
        }
    }

    #[must_use]
    pub fn tab_dashboard(&self) -> &'static str {
        match self {
            Lang::Zh => "▣ 仪表盘",
            Lang::En => "▣ Dashboard",
        }
    }

    #[must_use]
    pub fn tab_dashboard_inactive(&self) -> &'static str {
        match self {
            Lang::Zh => "  仪表盘",
            Lang::En => "  Dashboard",
        }
    }

    #[must_use]
    pub fn tab_records(&self) -> &'static str {
        match self {
            Lang::Zh => "▣ 记录",
            Lang::En => "▣ Records",
        }
    }

    #[must_use]
    pub fn tab_records_inactive(&self) -> &'static str {
        match self {
            Lang::Zh => "  记录",
            Lang::En => "  Records",
        }
    }

    #[must_use]
    pub fn op_label(&self, op: &OperationType) -> &'static str {
        match (self, op) {
            (Lang::Zh, OperationType::CompressResponse) => "响应压缩",
            (Lang::En, OperationType::CompressResponse) => "compress-response",
            (Lang::Zh, OperationType::CompressSchema) => "模式压缩",
            (Lang::En, OperationType::CompressSchema) => "compress-schema",
            (Lang::Zh, OperationType::CompressToon) => "TOON编码",
            (Lang::En, OperationType::CompressToon) => "compress-toon",
            (Lang::Zh, OperationType::RewriteCommand) => "命令重写",
            (Lang::En, OperationType::RewriteCommand) => "rewrite-command",
        }
    }

    #[must_use]
    pub fn stat_total_saved(&self) -> &'static str {
        match self {
            Lang::Zh => "总节省",
            Lang::En => "Total Saved",
        }
    }

    #[must_use]
    pub fn stat_total_records(&self) -> &'static str {
        match self {
            Lang::Zh => "总记录数",
            Lang::En => "Total Records",
        }
    }

    #[must_use]
    pub fn stat_avg_savings(&self) -> &'static str {
        match self {
            Lang::Zh => "平均节省",
            Lang::En => "Avg Savings",
        }
    }

    #[must_use]
    pub fn section_breakdown(&self) -> &'static str {
        match self {
            Lang::Zh => " 各操作节省 ",
            Lang::En => " Per-Operation Savings ",
        }
    }

    #[must_use]
    pub fn section_recent(&self) -> &'static str {
        match self {
            Lang::Zh => " 最近活动 ",
            Lang::En => " Recent Activity ",
        }
    }

    #[must_use]
    pub fn tab_agents(&self) -> &'static str {
        match self {
            Lang::Zh => "▣ 代理",
            Lang::En => "▣ Agents",
        }
    }

    #[must_use]
    pub fn tab_agents_inactive(&self) -> &'static str {
        match self {
            Lang::Zh => "  代理",
            Lang::En => "  Agents",
        }
    }

    #[must_use]
    pub fn tab_trends(&self) -> &'static str {
        match self {
            Lang::Zh => "▣ 趋势",
            Lang::En => "▣ Trends",
        }
    }

    #[must_use]
    pub fn tab_trends_inactive(&self) -> &'static str {
        match self {
            Lang::Zh => "  趋势",
            Lang::En => "  Trends",
        }
    }

    #[must_use]
    pub fn dashboard_status_bar(&self) -> &'static str {
        match self {
            Lang::Zh => "[Tab:切换]  [?:帮助]  [c:配置]  [↑↓:滚动]  [Enter:详情]  [q:退出]",
            Lang::En => "[Tab:switch]  [?:help]  [c:config]  [↑↓:scroll]  [Enter:detail]  [q:quit]",
        }
    }

    #[must_use]
    pub fn records_info(&self, count: usize, total: usize, has_filter: bool) -> String {
        match (self, has_filter) {
            (Lang::Zh, true) => format!(" 记录: {count}/{total} (已筛选) "),
            (Lang::Zh, false) => format!(" 记录: {total}条 "),
            (Lang::En, true) => format!(" Records: {count}/{total} (filtered) "),
            (Lang::En, false) => format!(" Records: {total} "),
        }
    }

    #[must_use]
    pub fn records_col_id(&self) -> &'static str {
        match self {
            Lang::Zh => "编号",
            Lang::En => "ID",
        }
    }

    #[must_use]
    pub fn records_col_time(&self) -> &'static str {
        match self {
            Lang::Zh => "时间",
            Lang::En => "Timestamp",
        }
    }

    #[must_use]
    pub fn records_col_op(&self) -> &'static str {
        match self {
            Lang::Zh => "操作",
            Lang::En => "Operation",
        }
    }

    #[must_use]
    pub fn records_col_agent(&self) -> &'static str {
        match self {
            Lang::Zh => "代理",
            Lang::En => "Agent",
        }
    }

    #[must_use]
    pub fn records_col_before(&self) -> &'static str {
        match self {
            Lang::Zh => "压缩前",
            Lang::En => "Before",
        }
    }

    #[must_use]
    pub fn records_col_after(&self) -> &'static str {
        match self {
            Lang::Zh => "压缩后",
            Lang::En => "After",
        }
    }

    #[must_use]
    pub fn records_col_savings(&self) -> &'static str {
        match self {
            Lang::Zh => "节省",
            Lang::En => "Savings",
        }
    }

    #[must_use]
    pub fn records_status_bar(&self) -> &'static str {
        match self {
            Lang::Zh => {
                "[Tab:切换] [↑↓:导航] [Enter:详情] [/:搜索] [t:时间范围] [e:导出] [d:返回] [q:退出]"
            }
            Lang::En => {
                "[Tab:switch] [↑↓:navigate] [Enter:detail] [/:search] [t:time range] [e:export] \
                 [d:back] [q:quit]"
            }
        }
    }

    // ── Agents tab ──

    #[must_use]
    pub fn agents_header(&self, count: usize) -> String {
        match self {
            Lang::Zh => format!(" 代理 (共{count}个) "),
            Lang::En => format!(" Agents ({count} total) "),
        }
    }

    #[must_use]
    pub fn agents_col_agent(&self) -> &'static str {
        match self {
            Lang::Zh => "代理",
            Lang::En => "Agent",
        }
    }

    #[must_use]
    pub fn agents_col_records(&self) -> &'static str {
        match self {
            Lang::Zh => "记录数",
            Lang::En => "Records",
        }
    }

    #[must_use]
    pub fn agents_col_chars_saved(&self) -> &'static str {
        match self {
            Lang::Zh => "字符节省",
            Lang::En => "Chars Saved",
        }
    }

    #[must_use]
    pub fn agents_col_tokens_saved(&self) -> &'static str {
        match self {
            Lang::Zh => "Token节省",
            Lang::En => "Tokens Saved",
        }
    }

    #[must_use]
    pub fn agents_status_bar(&self) -> &'static str {
        match self {
            Lang::Zh => "[Tab:切换] [↑↓:导航] [Enter:操作详情] [d:返回] [q:退出]",
            Lang::En => "[Tab:switch] [↑↓:navigate] [Enter:ops detail] [d:back] [q:quit]",
        }
    }

    #[must_use]
    pub fn agent_detail_header(&self, agent: &str, records: usize) -> String {
        match self {
            Lang::Zh => format!(" 代理: {agent} — 记录数: {records} "),
            Lang::En => format!(" Agent: {agent} — Records: {records} "),
        }
    }

    #[must_use]
    pub fn agent_detail_status_bar(&self) -> &'static str {
        match self {
            Lang::Zh => "[d:返回代理列表] [q:退出]",
            Lang::En => "[d:back to agents] [q:quit]",
        }
    }

    // ── Trends tab ──

    #[must_use]
    pub fn trends_header_chars(&self) -> &'static str {
        match self {
            Lang::Zh => " 每日字符节省 ",
            Lang::En => " Daily Chars Saved ",
        }
    }

    #[must_use]
    pub fn trends_header_tokens(&self) -> &'static str {
        match self {
            Lang::Zh => " 每日Token节省 ",
            Lang::En => " Daily Tokens Saved ",
        }
    }

    #[must_use]
    pub fn trends_status_bar(&self) -> &'static str {
        match self {
            Lang::Zh => "[Tab:切换] [q:退出]",
            Lang::En => "[Tab:switch] [q:quit]",
        }
    }

    #[must_use]
    pub fn trends_no_data(&self) -> &'static str {
        match self {
            Lang::Zh => " 暂无趋势数据 ",
            Lang::En => " No trend data ",
        }
    }

    // ── Search / filter ──

    #[must_use]
    pub fn search_prompt(&self) -> &'static str {
        match self {
            Lang::Zh => " 搜索: ",
            Lang::En => " Search: ",
        }
    }

    #[must_use]
    pub fn search_no_results(&self) -> &'static str {
        match self {
            Lang::Zh => " 无匹配记录 ",
            Lang::En => " No matching records ",
        }
    }

    #[must_use]
    pub fn time_range_today(&self) -> &'static str {
        match self {
            Lang::Zh => "今天",
            Lang::En => "Today",
        }
    }

    #[must_use]
    pub fn time_range_week(&self) -> &'static str {
        match self {
            Lang::Zh => "本周",
            Lang::En => "This Week",
        }
    }

    #[must_use]
    pub fn time_range_all(&self) -> &'static str {
        match self {
            Lang::Zh => "全部",
            Lang::En => "All Time",
        }
    }

    // ── Export ──

    #[must_use]
    pub fn export_success(&self, path: &str) -> String {
        match self {
            Lang::Zh => format!(" 导出成功: {path} "),
            Lang::En => format!(" Export successful: {path} "),
        }
    }

    #[must_use]
    pub fn export_error(&self, msg: &str) -> String {
        match self {
            Lang::Zh => format!(" 导出失败: {msg} "),
            Lang::En => format!(" Export failed: {msg} "),
        }
    }

    #[must_use]
    pub fn detail_header(&self, id: i64, op: &str, agent: &str, pct: f64) -> String {
        match self {
            Lang::Zh => format!("  记录 #{id} — {op} — 代理: {agent} — 节省: {pct}%  [d:返回]"),
            Lang::En => {
                format!("  Record #{id} — {op} — Agent: {agent} — Savings: {pct}%  [d:back]")
            }
        }
    }

    #[must_use]
    pub fn detail_before(&self) -> &'static str {
        match self {
            Lang::Zh => " 压缩前 ",
            Lang::En => " Before ",
        }
    }

    #[must_use]
    pub fn detail_after(&self) -> &'static str {
        match self {
            Lang::Zh => " 压缩后 ",
            Lang::En => " After ",
        }
    }

    #[must_use]
    pub fn detail_no_text(&self) -> &'static str {
        match self {
            Lang::Zh => "(无文本记录)",
            Lang::En => "(no text recorded)",
        }
    }

    #[must_use]
    pub fn detail_status_bar(&self) -> &'static str {
        match self {
            Lang::Zh => "[d:返回记录列表]  [q:退出]",
            Lang::En => "[d:back to records]  [q:quit]",
        }
    }

    /// Localize the default "cli" agent, pass through others.
    /// "cli" → "命令行" (中文) / "CLI" (English)
    #[must_use]
    pub fn agent_label<'a>(&self, agent: &'a str) -> &'a str {
        match (self, agent) {
            (Lang::Zh, "cli") => "命令行",
            (Lang::En, "cli") => "CLI",
            _ => agent,
        }
    }

    // ── Help overlay ──

    #[must_use]
    pub fn help_title(&self) -> &'static str {
        match self {
            Lang::Zh => "帮助",
            Lang::En => "Help",
        }
    }

    /// Return the display label for a given key identifier.
    #[must_use]
    pub fn help_key(&self, key: &str) -> &'static str {
        match key {
            "tab" => "Tab / \u{2190}\u{2192}",
            "up_down" => "\u{2191}\u{2193}",
            "enter" => "Enter",
            "slash" => "/",
            "t" => "t",
            "e" => "e",
            "c" => "c",
            "question" => "?",
            "q" => "q",
            _ => "?",
        }
    }

    /// Return the localized description for a given action identifier.
    #[must_use]
    pub fn help_action(&self, action: &str) -> &'static str {
        match (self, action) {
            (Lang::Zh, "switch_tabs") => "切换标签页",
            (Lang::En, "switch_tabs") => "switch tabs",
            (Lang::Zh, "navigate") => "滚动",
            (Lang::En, "navigate") => "scroll",
            (Lang::Zh, "detail") => "查看详情",
            (Lang::En, "detail") => "detail view",
            (Lang::Zh, "search") => "搜索",
            (Lang::En, "search") => "search",
            (Lang::Zh, "time_range") => "切换时间范围",
            (Lang::En, "time_range") => "time range",
            (Lang::Zh, "export") => "导出",
            (Lang::En, "export") => "export",
            (Lang::Zh, "config") => "配置面板",
            (Lang::En, "config") => "config panel",
            (Lang::Zh, "help") => "帮助",
            (Lang::En, "help") => "help",
            (Lang::Zh, "quit") => "退出",
            (Lang::En, "quit") => "quit",
            _ => match self {
                Lang::Zh => "未知操作",
                Lang::En => "unknown action",
            },
        }
    }

    #[must_use]
    pub fn help_dismiss(&self) -> &'static str {
        match self {
            Lang::Zh => "按 ? 或 Esc 关闭",
            Lang::En => "Press ? or Esc to close",
        }
    }

    // ── Config panel ──

    #[must_use]
    pub fn config_title(&self) -> &'static str {
        match self {
            Lang::Zh => "配置",
            Lang::En => "Config",
        }
    }

    #[must_use]
    pub fn config_stats(&self) -> &'static str {
        match self {
            Lang::Zh => "统计记录",
            Lang::En => "Stats Recording",
        }
    }

    #[must_use]
    pub fn config_cache(&self) -> &'static str {
        match self {
            Lang::Zh => "缓存大小",
            Lang::En => "Cache Size",
        }
    }

    #[must_use]
    pub fn config_threshold(&self) -> &'static str {
        match self {
            Lang::Zh => "差分阈值",
            Lang::En => "Diff Threshold",
        }
    }

    #[must_use]
    pub fn config_enabled(&self) -> &'static str {
        match self {
            Lang::Zh => "已启用",
            Lang::En => "Enabled",
        }
    }

    #[must_use]
    pub fn config_disabled(&self) -> &'static str {
        match self {
            Lang::Zh => "已禁用",
            Lang::En => "Disabled",
        }
    }

    #[must_use]
    pub fn config_dismiss(&self) -> &'static str {
        match self {
            Lang::Zh => "按 c 关闭",
            Lang::En => "Press c to close",
        }
    }
}
