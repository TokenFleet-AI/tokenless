//! Handler for `tokenless stats` subcommands.

use std::{
    collections::BTreeMap,
    io::{self, Write},
};

use rtk_registry::{Classification, classify_command};
use tokenless_stats::{
    OperationType, StatsRecord, StatsSummary, TokenlessConfig, parse_time_range,
};

use crate::shared::open_recorder;

fn format_summary(records: &[StatsRecord], title: Option<&str>) -> String {
    let mut output = String::new();

    if let Some(title) = title {
        output.push_str(title);
        output.push('\n');
        output.push_str(&"=".repeat(title.len()));
        output.push('\n');
    }

    if records.is_empty() {
        output.push_str("No records found.\n");
        return output;
    }

    let mut by_operation: BTreeMap<&str, Vec<&StatsRecord>> = BTreeMap::new();
    for record in records {
        by_operation
            .entry(record.operation.as_str())
            .or_default()
            .push(record);
    }

    let mut total_before_chars = 0_usize;
    let mut total_after_chars = 0_usize;
    let mut total_before_tokens = 0_usize;
    let mut total_after_tokens = 0_usize;

    for (operation_name, group) in &by_operation {
        let before_chars: usize = group.iter().map(|record| record.before_chars).sum();
        let after_chars: usize = group.iter().map(|record| record.after_chars).sum();
        let before_tokens: usize = group.iter().map(|record| record.before_tokens).sum();
        let after_tokens: usize = group.iter().map(|record| record.after_tokens).sum();

        let chars_saved = before_chars.saturating_sub(after_chars);
        let tokens_saved = before_tokens.saturating_sub(after_tokens);
        let chars_pct = percentage(chars_saved, before_chars);
        let tokens_pct = percentage(tokens_saved, before_tokens);

        total_before_chars += before_chars;
        total_after_chars += after_chars;
        total_before_tokens += before_tokens;
        total_after_tokens += after_tokens;

        output.push_str(&format!(
            "{operation_name}\n  Count: {count}\n  Chars: {before_chars} → {after_chars} (-{chars_saved}, {chars_pct:.1}%)\n  Tokens: {before_tokens} → {after_tokens} (-{tokens_saved}, {tokens_pct:.1}%)\n\n",
            count = group.len(),
            chars_saved = format_number(chars_saved),
            tokens_saved = format_number(tokens_saved),
        ));
    }

    let total_chars_saved = total_before_chars.saturating_sub(total_after_chars);
    let total_tokens_saved = total_before_tokens.saturating_sub(total_after_tokens);
    let total_chars_pct = percentage(total_chars_saved, total_before_chars);
    let total_tokens_pct = percentage(total_tokens_saved, total_before_tokens);

    output.push_str(&format!(
        "Total\n  Count: {count}\n  Chars: {before_chars} → {after_chars} (-{chars_saved}, {chars_pct:.1}%)\n  Tokens: {before_tokens} → {after_tokens} (-{tokens_saved}, {tokens_pct:.1}%)\n",
        count = records.len(),
        before_chars = format_number(total_before_chars),
        after_chars = format_number(total_after_chars),
        chars_saved = format_number(total_chars_saved),
        chars_pct = total_chars_pct,
        before_tokens = format_number(total_before_tokens),
        after_tokens = format_number(total_after_tokens),
        tokens_saved = format_number(total_tokens_saved),
        tokens_pct = total_tokens_pct,
    ));

    output
}

fn format_list(records: &[StatsRecord], limit: usize) -> String {
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

fn format_show(record: &StatsRecord) -> String {
    let mut output = String::new();
    output.push_str(&format!(
        "Record ID: {id}\nTimestamp: {timestamp}\nOperation: {operation}\nAgent: {agent}\n\n",
        id = record.id,
        timestamp = record.timestamp.format("%Y-%m-%d %H:%M:%S"),
        operation = record.operation.as_str(),
        agent = record.agent_id,
    ));

    if let Some(session_id) = &record.session_id {
        output.push_str(&format!("Session: {session_id}\n"));
    }
    if let Some(tool_use_id) = &record.tool_use_id {
        output.push_str(&format!("ToolUse: {tool_use_id}\n"));
    }
    if let Some(source_pid) = record.source_pid {
        output.push_str(&format!("PID: {source_pid}\n"));
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
        "\nBefore: {before_chars} chars, {before_tokens} tokens\nAfter: {after_chars} chars, {after_tokens} tokens\nSaved: {chars_saved} chars (-{chars_pct:.1}%), {tokens_saved} tokens (-{tokens_pct:.1}%)\n\n",
        before_chars = record.before_chars,
        before_tokens = record.before_tokens,
        after_chars = record.after_chars,
        after_tokens = record.after_tokens,
        chars_saved = record.chars_saved(),
        chars_pct = record.chars_percent(),
        tokens_saved = record.tokens_saved(),
        tokens_pct = record.tokens_percent(),
    ));

    if let Some(text) = &record.before_text {
        output.push_str("--- Before ---\n");
        output.push_str(text);
        output.push('\n');
    }
    if let Some(text) = &record.after_text {
        output.push_str("--- After ---\n");
        output.push_str(text);
        output.push('\n');
    }

    output
}

fn format_rewrites(entries: &[(&str, usize, Option<f64>)], limit: usize, offset: usize) -> String {
    let mut output = String::from("Rewrite Commands Breakdown\n");
    output.push_str("==========================\n\n");

    if entries.is_empty() {
        output.push_str("No rewrite records found.\n");
        return output;
    }

    let total_commands = entries.len();
    let total_rewrites: usize = entries.iter().map(|(_, count, _)| count).sum();
    let shown = entries.iter().skip(offset).take(limit).count();
    let total_pages = total_commands.div_ceil(limit);
    let current_page = offset / limit + 1;

    output.push_str(&format!(
        "  {total_commands} commands, {total_rewrites} rewrites (page {current_page}/{total_pages})\n\n",
    ));

    for (command, count, savings) in entries.iter().skip(offset).take(limit) {
        match savings {
            Some(savings_pct) => {
                output.push_str(&format!("  {count:>5}  {command:<40} ~{savings_pct:.0}%\n"));
            }
            None => output.push_str(&format!("  {count:>5}  {command}\n")),
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

fn format_diff(records: &[StatsRecord], since: &str, until: &str) -> String {
    let summary = StatsSummary::from_records(records);
    let mut output = String::new();

    output.push_str("Tokenless Savings Report\n");
    output.push_str("========================\n");
    output.push_str(&format!("Period: {since} → {until}\n\n"));

    if records.is_empty() {
        output.push_str("No records found for this period.\n");
        return output;
    }

    output.push_str(&format!("Total records:   {}\n", summary.total_records));
    output.push_str(&format!(
        "Chars:   {} → {}  (-{}, {:.1}%)\n",
        format_number(summary.total_before_chars),
        format_number(summary.total_after_chars),
        format_number(summary.chars_saved()),
        summary.chars_percent(),
    ));
    output.push_str(&format!(
        "Tokens:  {} → {}  (-{}, {:.1}%)\n",
        format_number(summary.total_before_tokens),
        format_number(summary.total_after_tokens),
        format_number(summary.tokens_saved()),
        summary.tokens_percent(),
    ));

    let estimated_cost = summary.tokens_saved() as f64 * 8.0 / 1_000_000.0;
    output.push_str(&format!("Est. cost saved: ~${estimated_cost:.2}\n"));

    let mut by_agent: BTreeMap<&str, (usize, usize)> = BTreeMap::new();
    for record in records {
        let agent_entry = by_agent.entry(&record.agent_id).or_default();
        agent_entry.0 += record.before_tokens.saturating_sub(record.after_tokens);
        agent_entry.1 += 1;
    }
    if by_agent.len() > 1 {
        output.push_str("\nPer-Agent:\n");
        for (agent, (tokens, count)) in &by_agent {
            output.push_str(&format!(
                "  {agent}: -{} tokens ({count} ops)\n",
                format_number(*tokens),
            ));
        }
    }

    output
}

fn percentage(saved: usize, total: usize) -> f64 {
    if total == 0 {
        0.0
    } else {
        (saved as f64 / total as f64) * 100.0
    }
}

fn format_number(value: usize) -> String {
    let digits = value.to_string();
    let mut formatted = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            formatted.push(',');
        }
        formatted.push(digit);
    }
    formatted
}

pub(crate) fn stats_summary(
    limit: Option<usize>,
    project: Option<String>,
    namespace: Option<String>,
) -> Result<(), (String, i32)> {
    let recorder = open_recorder()?;
    let records = recorder
        .records_filtered(None, None, project.as_deref(), namespace.as_deref(), limit)
        .map_err(|e| (format!("Failed to query records: {e}"), 1))?;

    let title = match (&project, &namespace) {
        (Some(p), Some(ns)) => format!("Stats — project: {p}, namespace: {ns}"),
        (Some(p), None) => format!("Stats — project: {p}"),
        (None, Some(ns)) => format!("Stats — namespace: {ns}"),
        (None, None) => "Tokenless Statistics Summary".to_string(),
    };
    println!("{}", format_summary(&records, Some(&title)));
    Ok(())
}

pub(crate) fn stats_list(
    limit: usize,
    project: Option<String>,
    namespace: Option<String>,
) -> Result<(), (String, i32)> {
    let recorder = open_recorder()?;
    let records = recorder
        .records_filtered(
            None,
            None,
            project.as_deref(),
            namespace.as_deref(),
            Some(limit),
        )
        .map_err(|e| (format!("Failed to query records: {e}"), 1))?;
    println!("{}", format_list(&records, limit));
    Ok(())
}

pub(crate) fn stats_show(id: i64) -> Result<(), (String, i32)> {
    let recorder = open_recorder()?;
    let record = recorder
        .record_by_id(id)
        .map_err(|e| (format!("Failed to query record: {e}"), 1))?
        .ok_or_else(|| (format!("Record not found: {id}"), 1))?;
    println!("{}", format_show(&record));
    Ok(())
}

pub(crate) fn stats_clear(yes: bool) -> Result<(), (String, i32)> {
    if !yes {
        print!("Are you sure you want to clear all statistics? [y/N] ");
        let _ = io::stdout().flush();
        let mut input = String::new();
        if io::stdin().read_line(&mut input).unwrap_or(0) == 0 {
            println!("Cancelled.");
            return Ok(());
        }
        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Cancelled.");
            return Ok(());
        }
    }
    let recorder = open_recorder()?;
    recorder
        .clear()
        .map_err(|e| (format!("Failed to clear: {e}"), 1))?;
    println!("Statistics cleared.");
    Ok(())
}

pub(crate) fn stats_rewrites(
    limit: usize,
    offset: usize,
    project: Option<String>,
) -> Result<(), (String, i32)> {
    let recorder = open_recorder()?;
    let all = recorder
        .records_filtered(None, None, project.as_deref(), None, None)
        .map_err(|e| (format!("Failed to query records: {e}"), 1))?;
    let rewrites: Vec<_> = all
        .iter()
        .filter(|r| r.operation == OperationType::RewriteCommand)
        .collect();
    let mut by_cmd: BTreeMap<&str, usize> = BTreeMap::new();
    for r in &rewrites {
        if let Some(ref before) = r.before_text {
            *by_cmd.entry(before.as_str()).or_default() += 1;
        }
    }
    let mut entries: Vec<_> = by_cmd
        .into_iter()
        .map(|(cmd, count)| {
            let savings = match classify_command(cmd) {
                Classification::Supported {
                    estimated_savings_pct,
                    ..
                } => Some(estimated_savings_pct),
                _ => None,
            };
            (cmd, count, savings)
        })
        .collect();
    entries.sort_by_key(|a| std::cmp::Reverse(a.1));
    let slice: Vec<(&str, usize, Option<f64>)> =
        entries.iter().map(|(c, n, s)| (*c, *n, *s)).collect();
    println!("{}", format_rewrites(&slice, limit, offset));
    Ok(())
}

pub(crate) fn stats_status() -> Result<(), (String, i32)> {
    let config = TokenlessConfig::load();
    let source = if std::env::var("TOKENLESS_STATS_ENABLED").is_ok() {
        "env override"
    } else if TokenlessConfig::config_file_exists() {
        "config file"
    } else {
        "default"
    };
    let state = if config.is_stats_enabled() {
        "ENABLED"
    } else {
        "DISABLED"
    };
    let exp_source = if std::env::var("TOKENLESS_EXPERIMENTAL").is_ok() {
        "env override"
    } else {
        source
    };
    let exp_state = if config.is_experimental_enabled() {
        "ENABLED"
    } else {
        "DISABLED"
    };
    println!("Stats recording: {state} (via {source})");
    println!("Experimental mode: {exp_state} (via {exp_source})");
    Ok(())
}

pub(crate) fn stats_enable() -> Result<(), (String, i32)> {
    let mut config = TokenlessConfig::load();
    config.stats_enabled = true;
    config
        .save()
        .map_err(|e| (format!("Failed to save config: {e}"), 1))?;
    println!("Stats recording enabled.");
    Ok(())
}

pub(crate) fn stats_disable() -> Result<(), (String, i32)> {
    let mut config = TokenlessConfig::load();
    config.stats_enabled = false;
    config
        .save()
        .map_err(|e| (format!("Failed to save config: {e}"), 1))?;
    println!("Stats recording disabled.");
    Ok(())
}

pub(crate) fn stats_experimental_on() -> Result<(), (String, i32)> {
    let mut config = TokenlessConfig::load();
    config.experimental_mode = true;
    config
        .save()
        .map_err(|e| (format!("Failed to save config: {e}"), 1))?;
    println!("Experimental mode enabled (format router, enhanced TOON, semantic, TUI, MCP).");
    Ok(())
}

pub(crate) fn stats_experimental_off() -> Result<(), (String, i32)> {
    let mut config = TokenlessConfig::load();
    config.experimental_mode = false;
    config
        .save()
        .map_err(|e| (format!("Failed to save config: {e}"), 1))?;
    println!("Experimental mode disabled (core compression only: schema + response + basic TOON).");
    Ok(())
}

pub(crate) fn stats_diff(
    since: Option<String>,
    until: Option<String>,
    project: Option<String>,
) -> Result<(), (String, i32)> {
    let recorder = open_recorder()?;
    let until_str = until
        .as_deref()
        .and_then(parse_time_range)
        .unwrap_or_else(|| chrono::Local::now().to_rfc3339());
    let since_str = since
        .as_deref()
        .and_then(parse_time_range)
        .unwrap_or_else(|| {
            let d = chrono::Local::now() - chrono::Duration::days(7);
            d.to_rfc3339()
        });

    let since_label = since_str.clone();
    let until_label = until_str.clone();

    // Use records_filtered for project support, then post-filter by time
    let all_records = recorder
        .records_filtered(None, None, project.as_deref(), None, None)
        .map_err(|e| (format!("Failed to query records: {e}"), 1))?;
    let records: Vec<_> = all_records
        .into_iter()
        .filter(|r| {
            let ts = r.timestamp.to_rfc3339();
            ts >= since_str && ts <= until_str
        })
        .collect();

    println!("{}", format_diff(&records, &since_label, &until_label));
    Ok(())
}

pub(crate) fn stats_delete(
    id: Option<i64>,
    agent: Option<String>,
    before: Option<String>,
    yes: bool,
) -> Result<(), (String, i32)> {
    let recorder = open_recorder()?;

    // Determine what to delete
    let desc: String = if id.is_some() {
        "record #{}".to_string()
    } else if let Some(ref agent_id) = agent {
        let count = recorder
            .count()
            .map_err(|e| (format!("Failed to count records: {e}"), 1))?;
        format!("all records for agent \"{agent_id}\" ({count} total records in DB)")
    } else if let Some(ref date) = before {
        let count = recorder
            .count()
            .map_err(|e| (format!("Failed to count records: {e}"), 1))?;
        format!("all records before {date} ({count} total records in DB)")
    } else {
        return Err(("Must specify --id, --agent, or --before".to_string(), 1));
    };

    // Confirmation
    if !yes {
        print!("Delete {desc}? [y/N] ");
        let _ = io::stdout().flush();
        let mut input = String::new();
        if io::stdin().read_line(&mut input).unwrap_or(0) == 0 {
            println!("Cancelled.");
            return Ok(());
        }
        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Cancelled.");
            return Ok(());
        }
    }

    // Execute delete
    if let Some(record_id) = id {
        recorder
            .delete_by_id(record_id)
            .map_err(|e| (format!("Failed to delete: {e}"), 1))?;
        println!("Deleted record #{record_id}.");
    } else if let Some(ref agent_id) = agent {
        let deleted = recorder
            .delete_by_agent(agent_id)
            .map_err(|e| (format!("Failed to delete: {e}"), 1))?;
        println!("Deleted {deleted} records for agent \"{agent_id}\".");
    } else if let Some(ref date) = before {
        let deleted = recorder
            .delete_before(date)
            .map_err(|e| (format!("Failed to delete: {e}"), 1))?;
        println!("Deleted {deleted} records before {date}.");
    }

    Ok(())
}

pub(crate) fn stats_vacuum() -> Result<(), (String, i32)> {
    let recorder = open_recorder()?;
    let bytes_before = recorder.db_size_bytes();
    if let Some(b) = bytes_before {
        println!("Before VACUUM: {b} bytes");
    }
    recorder
        .vacuum()
        .map_err(|e| (format!("VACUUM failed: {e}"), 1))?;
    let bytes_after = recorder.db_size_bytes();
    if let Some(after_bytes) = bytes_after {
        println!("After VACUUM:  {after_bytes} bytes");
        if let Some(before_bytes) = bytes_before
            && after_bytes < before_bytes
        {
            println!(
                "Freed {} bytes ({}%).",
                before_bytes - after_bytes,
                (100 * (before_bytes - after_bytes)) / before_bytes
            );
        }
    }
    println!("VACUUM complete.");
    Ok(())
}

pub(crate) fn stats_export(output: &str) -> Result<(), (String, i32)> {
    use std::path::Path;
    let recorder = open_recorder()?;
    let count = recorder
        .export_json(Path::new(output))
        .map_err(|e| (format!("Export failed: {e}"), 1))?;
    println!("Exported {count} records to {output}");
    Ok(())
}

/// Generate a shareable weekly/monthly report.
pub(crate) fn stats_share(
    since: Option<String>,
    project: Option<String>,
    format: Option<String>,
) -> Result<(), (String, i32)> {
    let recorder = open_recorder()?;
    let now = chrono::Local::now();
    let since_str = since
        .as_deref()
        .and_then(parse_time_range)
        .unwrap_or_else(|| (now - chrono::Duration::days(7)).to_rfc3339());
    let until_str = now.to_rfc3339();
    let all = recorder
        .records_filtered(None, None, project.as_deref(), None, None)
        .map_err(|e| (format!("Failed to query records: {e}"), 1))?;
    let records: Vec<_> = all
        .into_iter()
        .filter(|r| {
            let ts = r.timestamp.to_rfc3339();
            ts >= since_str && ts <= until_str
        })
        .collect();

    let total_saved_tokens: usize = records
        .iter()
        .map(|r| r.before_tokens.saturating_sub(r.after_tokens))
        .sum();
    let total_saved_bytes: usize = records
        .iter()
        .map(|r| r.before_chars.saturating_sub(r.after_chars))
        .sum();
    let est_cost = total_saved_tokens as f64 / 1_000_000.0 * 3.0;

    // Top agents
    let mut agent_counts: std::collections::BTreeMap<&str, usize> =
        std::collections::BTreeMap::new();
    for r in &records {
        *agent_counts.entry(&r.agent_id).or_default() += 1;
    }

    let fmt = format.unwrap_or_else(|| "terminal".to_string());
    match fmt.as_str() {
        "markdown" => {
            println!("# 📊 Tokenless Weekly Report\n");
            println!(
                "**Period**: {} → {}",
                &since_str[..10.min(since_str.len())],
                &until_str[..10.min(until_str.len())]
            );
            println!("**Records**: {}", records.len());
            println!("**Tokens saved**: ~{total_saved_tokens}");
            println!("**Bytes saved**: {total_saved_bytes}");
            println!("**Est. cost saved**: ~${est_cost:.2}\n");
            println!("## Top Agents\n");
            for (agent, count) in agent_counts.iter().take(5) {
                println!("- `{agent}`: {count} records");
            }
        }
        _ => {
            println!("📊 Tokenless Weekly Report");
            println!("{:=<40}", "");
            println!(
                "Period:   {} → {}",
                &since_str[..10.min(since_str.len())],
                &until_str[..10.min(until_str.len())]
            );
            println!("Records:  {}", records.len());
            println!("Tokens:   ~{total_saved_tokens} saved");
            println!("Bytes:    {total_saved_bytes} saved");
            println!("Cost:     ~${est_cost:.2} saved");
            println!("\nTop Agents:");
            for (agent, count) in agent_counts.iter().take(5) {
                println!("  {agent}: {count}");
            }
        }
    }
    Ok(())
}
