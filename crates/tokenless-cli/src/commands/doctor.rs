//! `tokenless doctor` — one-command environment diagnostic.
//!
//! Checks: binary availability, PATH, RTK, agent configs, hooks, stats, and
//! recent record health.

use std::path::PathBuf;

use crate::shared::{get_home_dir, get_tokenless_dir, open_recorder};

/// Run the `tokenless doctor` diagnostic.
#[allow(
    clippy::too_many_lines,
    clippy::cast_precision_loss,
    clippy::unnecessary_wraps
)]
pub fn doctor() -> Result<(), (String, i32)> {
    println!("🔍 tokenless doctor — environment diagnostic\n");
    println!("{:=<50}", "");

    // 1. Binary executable check
    let exe_path = std::env::current_exe()
        .ok()
        .map_or_else(|| "unknown".to_string(), |p| p.display().to_string());
    println!("✅ tokenless binary: {exe_path}");

    // 2. PATH check — is tokenless in PATH?
    let output = std::process::Command::new("which")
        .arg("tokenless")
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
            } else {
                None
            }
        });
    match output {
        Some(p) => println!("✅ in PATH: {p}"),
        None => println!("⚠️  not found in PATH — add to your shell config"),
    }

    // 3. RTK installation
    let rtk_ok = crate::shared::rtk_available();
    if rtk_ok {
        println!("✅ RTK installed (command rewriting available)");
    } else {
        println!("ℹ️  RTK not installed (command rewriting disabled; other features work fine)");
    }

    // 4. Tokenless directory
    let dir = get_tokenless_dir();
    println!("📁 tokenless dir: {}", dir.display());
    if dir.exists() {
        println!("   ✅ directory exists");
    } else {
        println!("   ⚠️  directory does not exist — hooks may not be installed");
    }

    // 5. Agent config file check
    let home_dir = get_home_dir();
    let home = PathBuf::from(&home_dir);
    let agents: &[(&str, Vec<PathBuf>)] = &[
        (
            "Claude Code",
            vec![PathBuf::from(".claude").join("settings.json")],
        ),
        ("Cursor", vec![PathBuf::from(".cursor").join("hooks.json")]),
        (
            "Windsurf",
            vec![PathBuf::from(".windsurf").join("hooks.json")],
        ),
        ("Cline", vec![PathBuf::from(".clinerules").join("rtk.json")]),
        (
            "Gemini",
            vec![PathBuf::from(".gemini").join("settings.json")],
        ),
        (
            "Copilot",
            vec![
                PathBuf::from(".github")
                    .join("hooks")
                    .join("rtk-rewrite.json"),
            ],
        ),
        (
            "Codex",
            vec![PathBuf::from("AGENTS.md"), PathBuf::from("RTK.md")],
        ),
    ];
    let mut found = 0u32;
    for (_name, paths) in agents {
        let any = paths.iter().any(|p| home.join(p).exists());
        if any {
            found += 1;
        }
    }
    println!("\n🤖 Agent configs found: {found}/{}", agents.len());
    for (agent_name, paths) in agents {
        let any = paths.iter().any(|p| home.join(p).exists());
        if any {
            println!("   ✅ {agent_name}");
        }
    }

    // 6. Stats status
    let config = tokenless_stats::TokenlessConfig::load();
    let stats_enabled = config.is_stats_enabled();
    println!(
        "\n📊 Stats recording: {}",
        if stats_enabled { "ENABLED" } else { "DISABLED" }
    );

    // 7. DB health
    match open_recorder() {
        Ok(recorder) => {
            match recorder.count() {
                Ok(count) => println!("   Record count: {count}"),
                Err(e) => println!("   ⚠️  Failed to count records: {e}"),
            }
            match recorder.db_info() {
                Ok(info) => {
                    if let Some(first) = &info.first_record {
                        println!("   First record:  {first}");
                    }
                    if let Some(last) = &info.last_record {
                        println!("   Last record:   {last}");
                    }
                    if let Some(bytes) = info.size_bytes {
                        println!(
                            "   DB file size:  {} bytes ({:.1} KB)",
                            bytes,
                            bytes as f64 / 1024.0
                        );
                    }
                }
                Err(e) => println!("   ⚠️  Failed to read DB info: {e}"),
            }
        }
        Err(e) => println!("   ⚠️  Cannot open stats DB: {}", e.0),
    }

    // 8. Recent record check
    if let Ok(recorder) = open_recorder() {
        match recorder.all_records(Some(1)) {
            Ok(records) if records.is_empty() => {
                println!("\n💡 No stats records yet — compression hasn't run.");
                println!("   Use `tokenless demo` to create sample records.");
            }
            Ok(records) => {
                let r = &records[0];
                let age = chrono::Local::now().signed_duration_since(r.timestamp);
                let age_str = if age.num_hours() > 0 {
                    format!("{}h ago", age.num_hours())
                } else if age.num_minutes() > 0 {
                    format!("{}m ago", age.num_minutes())
                } else {
                    "just now".to_string()
                };
                println!("\n📈 Most recent record: {age_str}");
                println!("   Operation: {:?}", r.operation);
                println!("   Agent: {}", r.agent_id);
                if let Some(ref proj) = r.project {
                    println!("   Project: {proj}");
                }
            }
            Err(e) => tracing::debug!("stats query failed in doctor: {e}"),
        }
    }

    // 9. Experimental mode
    let experimental = crate::shared::is_experimental_enabled();
    println!(
        "\n🔬 Experimental mode: {}",
        if experimental {
            "ENABLED (format router, semantic, TUI, MCP)"
        } else {
            "DISABLED (core compression only)"
        }
    );

    // 10. Overall verdict
    println!("\n{:=<50}", "");
    if !rtk_ok {
        println!("\n💡 Tip: Install RTK for command rewriting: https://github.com/RTK/rink");
    }
    if !stats_enabled {
        println!("\n💡 Tip: Enable stats with `tokenless stats enable`");
    }
    if found == 0 {
        println!(
            "\n💡 Tip: Install hooks with `tokenless init` or `tokenless init --agent cursor`"
        );
    }
    println!("\n✅ Doctor check complete.");
    Ok(())
}

/// Run the `tokenless status` command — a lightweight daily check.
///
/// Shows current hook status, recent savings, and top operations without the
/// full output of `stats summary`.
#[allow(clippy::unnecessary_wraps, clippy::cast_precision_loss)]
pub fn status() -> Result<(), (String, i32)> {
    use tokenless_stats::TokenlessConfig;

    println!("🟢 tokenless status\n");
    println!("{:=<40}", "");

    // Hook status
    let home_dir = get_home_dir();
    let home = std::path::PathBuf::from(&home_dir);
    let cwd = std::env::current_dir().unwrap_or_else(|_| home.clone());

    let agents: &[(&str, &str, &[&str])] = &[
        ("Claude Code", "claude", &[".claude/settings.json"]),
        ("Cursor", "cursor", &[".cursor/hooks.json"]),
        ("Codex", "codex", &["AGENTS.md", "RTK.md"]),
    ];

    let mut installed: Vec<&str> = Vec::new();
    for (name, _key, paths) in agents {
        let any_local = paths.iter().any(|p| cwd.join(p).exists());
        let any_global = paths.iter().any(|p| home.join(p).exists());
        if any_local || any_global {
            installed.push(name);
        }
    }

    if installed.is_empty() {
        println!("🔌 Hooks: NOT INSTALLED");
        println!("   Run `tokenless init` to set up.");
    } else {
        println!("🔌 Hooks installed for: {}", installed.join(", "));
    }

    // RTK
    if crate::shared::rtk_available() {
        println!("⚡ RTK: available");
    } else {
        println!("⚡ RTK: not installed (command rewriting disabled)");
    }

    // Stats
    let config = TokenlessConfig::load();
    println!(
        "📊 Stats: {}",
        if config.is_stats_enabled() {
            "enabled"
        } else {
            "disabled"
        }
    );

    // Recent 24h savings
    match open_recorder() {
        Ok(recorder) => {
            let now = chrono::Local::now();
            let day_ago = (now - chrono::Duration::hours(24)).to_rfc3339();
            let now_str = now.to_rfc3339();

            match recorder.records_since(Some(&day_ago), Some(&now_str)) {
                Ok(records) if records.is_empty() => {
                    println!("\n📈 Last 24h: no records");
                    println!("   Run `tokenless demo` or trigger a hook to create records.");
                }
                Ok(records) => {
                    let total_saved_bytes: usize = records
                        .iter()
                        .map(|r| r.before_chars.saturating_sub(r.after_chars))
                        .sum();
                    let total_saved_tokens: usize = records
                        .iter()
                        .map(|r| r.before_tokens.saturating_sub(r.after_tokens))
                        .sum();
                    let elapsed = records[0].timestamp.signed_duration_since(
                        records.last().map_or(records[0].timestamp, |r| r.timestamp),
                    );
                    println!("\n📈 Last 24h:");
                    println!("   Records: {}", records.len());
                    println!("   Saved: {total_saved_bytes} bytes / ~{total_saved_tokens} tokens");
                    // Estimated cost ($3/M tokens)
                    let est_cost = total_saved_tokens as f64 / 1_000_000.0 * 3.0;
                    println!("   Estimated savings: ~${est_cost:.2}");

                    if !elapsed.is_zero() {
                        println!(
                            "   Time range: {} records over {}",
                            records.len(),
                            if elapsed.num_hours() > 0 {
                                format!("{}h", elapsed.num_hours())
                            } else {
                                format!("{}m", elapsed.num_minutes())
                            }
                        );
                    }
                }
                Err(e) => println!("\n📈 Last 24h: query failed ({e})"),
            }
        }
        Err(e) => println!("📈 Stats DB: unavailable ({})", e.0),
    }

    println!("\n{:=<40}", "");
    println!("💡 Tips:");
    println!("   tokenless demo       — see compression in action");
    println!("   tokenless stats summary — full statistics breakdown");
    println!("   tokenless doctor     — full environment diagnostic");
    Ok(())
}
