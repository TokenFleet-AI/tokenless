//! Formatting helpers for displaying statistics records and summaries.

use std::collections::BTreeMap;

use crate::record::StatsRecord;

/// Format a full summary, optionally with a header title.
#[must_use]
pub fn format_summary(records: &[StatsRecord], title: Option<&str>) -> String {
    let mut output = String::new();

    if let Some(t) = title {
        output.push_str(t);
        output.push('\n');
        output.push_str(&"=".repeat(t.len()));
        output.push('\n');
    }

    if records.is_empty() {
        output.push_str("No records found.\n");
        return output;
    }

    // Group by operation type
    let mut by_op: BTreeMap<&str, Vec<&StatsRecord>> = BTreeMap::new();
    for record in records {
        by_op
            .entry(record.operation.as_str())
            .or_default()
            .push(record);
    }

    let mut total_before_chars: usize = 0;
    let mut total_after_chars: usize = 0;
    let mut total_before_tokens: usize = 0;
    let mut total_after_tokens: usize = 0;

    for (op_name, group) in &by_op {
        let bc: usize = group.iter().map(|r| r.before_chars).sum();
        let ac: usize = group.iter().map(|r| r.after_chars).sum();
        let bt: usize = group.iter().map(|r| r.before_tokens).sum();
        let at: usize = group.iter().map(|r| r.after_tokens).sum();

        let chars_saved = bc.saturating_sub(ac);
        let tokens_saved = bt.saturating_sub(at);
        let chars_pct = if bc > 0 {
            (chars_saved as f64 / bc as f64) * 100.0
        } else {
            0.0
        };
        let tokens_pct = if bt > 0 {
            (tokens_saved as f64 / bt as f64) * 100.0
        } else {
            0.0
        };

        total_before_chars += bc;
        total_after_chars += ac;
        total_before_tokens += bt;
        total_after_tokens += at;

        output.push_str(&format!(
            "{op_name}\n  Count: {count}\n  Chars: {bc} → {ac} (-{cs}, {cp:.1}%)\n  Tokens: {bt} \
             → {at} (-{ts}, {tp:.1}%)\n\n",
            count = group.len(),
            cp = chars_pct,
            tp = tokens_pct,
            cs = format_number(chars_saved),
            ts = format_number(tokens_saved),
        ));
    }

    // Overall totals
    let total_cs = total_before_chars.saturating_sub(total_after_chars);
    let total_ts = total_before_tokens.saturating_sub(total_after_tokens);
    let total_cp = if total_before_chars > 0 {
        (total_cs as f64 / total_before_chars as f64) * 100.0
    } else {
        0.0
    };
    let total_tp = if total_before_tokens > 0 {
        (total_ts as f64 / total_before_tokens as f64) * 100.0
    } else {
        0.0
    };

    output.push_str(&format!(
        "Total\n  Count: {count}\n  Chars: {bc} → {ac} (-{cs}, {cp:.1}%)\n  Tokens: {bt} → {at} \
         (-{ts}, {tp:.1}%)\n",
        count = records.len(),
        bc = format_number(total_before_chars),
        ac = format_number(total_after_chars),
        cs = format_number(total_cs),
        cp = total_cp,
        bt = format_number(total_before_tokens),
        at = format_number(total_after_tokens),
        ts = format_number(total_ts),
        tp = total_tp,
    ));

    output
}

/// Format a list of recent records.
#[must_use]
pub fn format_list(records: &[StatsRecord], limit: usize) -> String {
    let mut output = String::new();
    output.push_str(&format!(
        "Recent {} records (most recent first):\n\n",
        records.len().min(limit)
    ));
    for record in records.iter().take(limit) {
        output.push_str(&record.format_summary_line());
        output.push('\n');
    }
    output
}

/// Format a detailed view of a single record (with full text content).
#[must_use]
pub fn format_show(record: &StatsRecord) -> String {
    let mut output = String::new();
    output.push_str(&format!(
        "Record ID: {id}\nTimestamp: {ts}\nOperation: {op}\nAgent: {agent}\n\n",
        id = record.id,
        ts = record.timestamp.format("%Y-%m-%d %H:%M:%S"),
        op = record.operation.as_str(),
        agent = record.agent_id,
    ));

    if let Some(ref sid) = record.session_id {
        output.push_str(&format!("Session: {sid}\n"));
    }
    if let Some(ref tuid) = record.tool_use_id {
        output.push_str(&format!("ToolUse: {tuid}\n"));
    }
    if let Some(pid) = record.source_pid {
        output.push_str(&format!("PID: {pid}\n"));
    }
    output.push_str(&format!(
        "Experimental: {}\n",
        if record.experimental_mode {
            "yes"
        } else {
            "no"
        }
    ));

    output.push_str(&format!(
        "\nBefore: {bc} chars, {bt} tokens\nAfter: {ac} chars, {at} tokens\nSaved: {cs} chars \
         (-{cp:.1}%), {ts} tokens (-{tp:.1}%)\n\n",
        bc = record.before_chars,
        bt = record.before_tokens,
        ac = record.after_chars,
        at = record.after_tokens,
        cs = record.chars_saved(),
        cp = record.chars_percent(),
        ts = record.tokens_saved(),
        tp = record.tokens_percent(),
    ));

    if let Some(ref text) = record.before_text {
        output.push_str("--- Before ---\n");
        output.push_str(text);
        output.push('\n');
    }
    if let Some(ref text) = record.after_text {
        output.push_str("--- After ---\n");
        output.push_str(text);
        output.push('\n');
    }

    output
}

/// Format a breakdown of rewrite-command records grouped by original command.
///
/// `entries` is a list of `(command, count, savings_pct)` tuples, pre-sorted by count descending.
#[must_use]
pub fn format_rewrites(
    entries: &[(&str, usize, Option<f64>)],
    limit: usize,
    offset: usize,
) -> String {
    let mut output = String::from("Rewrite Commands Breakdown\n");
    output.push_str("==========================\n\n");

    if entries.is_empty() {
        output.push_str("No rewrite records found.\n");
        return output;
    }

    let total_commands = entries.len();
    let total_rewrites: usize = entries.iter().map(|(_, c, _)| c).sum();
    let shown = entries.iter().skip(offset).take(limit).count();
    let total_pages = total_commands.div_ceil(limit);
    let current_page = offset / limit + 1;

    output.push_str(&format!(
        "  {total_commands} commands, {total_rewrites} rewrites (page \
         {current_page}/{total_pages})\n\n",
    ));

    for (cmd, count, savings) in entries.iter().skip(offset).take(limit) {
        match savings {
            Some(pct) => output.push_str(&format!("  {count:>5}  {cmd:<40} ~{pct:.0}%\n")),
            None => output.push_str(&format!("  {count:>5}  {cmd}\n")),
        }
    }

    if offset + shown < total_commands {
        let remaining = total_commands - offset - shown;
        output.push_str(&format!(
            "\n  ... {remaining} more command(s). Use --offset {} for next page.\n",
            offset + limit
        ));
    }

    output
}

/// Format a cumulative diff report for a time range.
#[must_use]
pub fn format_diff(records: &[StatsRecord], since: &str, until: &str) -> String {
    let summary = crate::recorder::StatsSummary::from_records(records);
    let mut output = String::new();

    output.push_str("Tokenless Savings Report\n");
    output.push_str("========================\n");
    output.push_str(&format!("Period: {since} → {until}\n\n"));

    if records.is_empty() {
        output.push_str("No records found for this period.\n");
        return output;
    }

    output.push_str(&format!(
        "Total records:   {count}\n",
        count = summary.total_records
    ));
    output.push_str(&format!(
        "Chars:   {bc} → {ac}  (-{cs}, {cp:.1}%)\n",
        bc = format_number(summary.total_before_chars),
        ac = format_number(summary.total_after_chars),
        cs = format_number(summary.chars_saved()),
        cp = summary.chars_percent(),
    ));
    output.push_str(&format!(
        "Tokens:  {bt} → {at}  (-{ts}, {tp:.1}%)\n",
        bt = format_number(summary.total_before_tokens),
        at = format_number(summary.total_after_tokens),
        ts = format_number(summary.tokens_saved()),
        tp = summary.tokens_percent(),
    ));

    // Estimated cost savings (assuming $3/1M input tokens, $15/1M output tokens blended ~$8/1M)
    let est_cost = summary.tokens_saved() as f64 * 8.0 / 1_000_000.0;
    output.push_str(&format!("Est. cost saved: ~${est_cost:.2}\n"));

    // Per-agent breakdown
    let mut by_agent: std::collections::BTreeMap<&str, (usize, usize)> =
        std::collections::BTreeMap::new();
    for r in records {
        let e = by_agent.entry(&r.agent_id).or_default();
        e.0 += r.before_tokens.saturating_sub(r.after_tokens);
        e.1 += 1;
    }
    if by_agent.len() > 1 {
        output.push_str("\nPer-Agent:\n");
        for (agent, (tokens, count)) in &by_agent {
            output.push_str(&format!(
                "  {agent}: -{tokens} tokens ({count} ops)\n",
                tokens = format_number(*tokens)
            ));
        }
    }

    output
}

/// Parse a human-readable time range string to an RFC 3339 timestamp.
///
/// Supported formats:
/// - `"today"` → start of today (00:00:00 local)
/// - `"yesterday"` → start of yesterday
/// - `"{N}d"` (e.g., `"7d"`) → N days ago
/// - `"YYYY-MM-DD"` → start of that day
/// - `"now"` → current time
///
/// # Panics
///
/// Does not panic; returns `None` for unparsable input.
#[must_use]
pub fn parse_time_range(input: &str) -> Option<String> {
    use chrono::{Datelike, Local, TimeZone};

    let now = Local::now();

    match input {
        "today" => {
            let start = Local
                .with_ymd_and_hms(now.year(), now.month(), now.day(), 0, 0, 0)
                .single()?;
            Some(start.to_rfc3339())
        }
        "yesterday" => {
            let yesterday = now - chrono::Duration::days(1);
            let start = Local
                .with_ymd_and_hms(
                    yesterday.year(),
                    yesterday.month(),
                    yesterday.day(),
                    0,
                    0,
                    0,
                )
                .single()?;
            Some(start.to_rfc3339())
        }
        "now" => Some(now.to_rfc3339()),
        s if s.ends_with('d') => {
            let days: i64 = s.strip_suffix('d')?.parse().ok()?;
            let target = now - chrono::Duration::days(days);
            let start = Local
                .with_ymd_and_hms(target.year(), target.month(), target.day(), 0, 0, 0)
                .single()?;
            Some(start.to_rfc3339())
        }
        // YYYY-MM-DD
        s if s.len() == 10 && s.chars().nth(4) == Some('-') => {
            let parts: Vec<&str> = s.split('-').collect();
            if parts.len() != 3 {
                return None;
            }
            let y: i32 = parts.first()?.parse().ok()?;
            let m: u32 = parts.get(1)?.parse().ok()?;
            let d: u32 = parts.get(2)?.parse().ok()?;
            let start = Local.with_ymd_and_hms(y, m, d, 0, 0, 0).single()?;
            Some(start.to_rfc3339())
        }
        _ => None,
    }
}

/// Format a number with thousands separators.
fn format_number(n: usize) -> String {
    let s = n.to_string();
    let mut result = String::with_capacity(s.len() + s.len() / 3);
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            result.push(',');
        }
        result.push(c);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_summary_empty() {
        let result = format_summary(&[], Some("Test"));
        assert!(result.contains("No records found"));
    }

    #[test]
    fn test_format_number() {
        assert_eq!(format_number(0), "0");
        assert_eq!(format_number(100), "100");
        assert_eq!(format_number(1000), "1,000");
        assert_eq!(format_number(1_000_000), "1,000,000");
    }
}
