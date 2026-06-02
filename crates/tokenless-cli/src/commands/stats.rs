//! Handler for `tokenless stats` subcommands.

use std::{
    collections::BTreeMap,
    io::{self, Write},
};

use rtk_registry::{Classification, classify_command};
use tokenless_stats::{
    OperationType, TokenlessConfig, format_diff, format_list, format_rewrites, format_show,
    format_summary, parse_time_range,
};

use crate::shared::open_recorder;

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
