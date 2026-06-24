//! End-to-end CLI tests for the `tokenless` binary.

#![allow(clippy::expect_used)]

use insta::{assert_snapshot, with_settings};

fn tokenless_command() -> assert_cmd::Command {
    assert_cmd::Command::cargo_bin("tokenless").expect("tokenless binary should build for tests")
}

fn sanitized_command_output(args: &[&str]) -> String {
    let output = tokenless_command()
        .env("HOME", "/tmp/tokenless-e2e-home")
        .env_remove("TOKENLESS_STATS_DB")
        .args(args)
        .output()
        .expect("command should run");

    assert!(
        output.status.success(),
        "command {:?} failed: {}",
        args,
        String::from_utf8_lossy(&output.stderr)
    );

    sanitize_output(&String::from_utf8_lossy(&output.stdout))
}

fn sanitize_output(output: &str) -> String {
    // CWD during `cargo test` is the crate directory, but the binary
    // path is relative to the workspace root. Walk up to workspace root.
    let crate_dir = std::env::current_dir()
        .ok()
        .map(|path| path.to_string_lossy().into_owned())
        .unwrap_or_default();
    let ws_root = if crate_dir.ends_with("/crates/tokenless-cli") {
        crate_dir.replace("/crates/tokenless-cli", "")
    } else {
        crate_dir.clone()
    };

    // Normalize cargo-llvm-cov's separate target directory so coverage runs
    // produce the same snapshot output as plain `cargo test`.
    let output = output.replace("target/llvm-cov-target/", "target/");

    let output = output
        .replace(std::env::consts::EXE_SUFFIX, "")
        .replace("/tmp/tokenless-e2e-home", "$HOME")
        .replace("/private/tmp/tokenless-e2e-home", "$HOME");

    // Replace workspace root with $CWD (before home_dir).
    let output = if ws_root.is_empty() {
        output
    } else {
        output.replace(&ws_root, "$CWD")
    };

    // Normalize home directory paths.
    let home = dirs::home_dir().unwrap_or_default();
    let home_str = home.to_string_lossy();
    let output = output.replace(&*home_str, "$HOME");

    // Normalize RTK installation status (depends on local environment)
    let output = output
        .replace(
            "✅ RTK installed (command rewriting available)",
            "[RTK_STATUS]",
        )
        .replace(
            "ℹ️  RTK not installed (command rewriting disabled; other features work fine)",
            "[RTK_STATUS]",
        );

    // Normalize PATH status (depends on local environment)
    let output = output
        .replace(
            "✅ in PATH: $HOME/.tokenfleet-ai/bin/tokenless",
            "[PATH_STATUS]",
        )
        .replace(
            "⚠️  not found in PATH — add to your shell config",
            "[PATH_STATUS]",
        );

    // Normalize RTK install tip (appears in doctor when RTK not installed)
    let output = output.replace(
        "💡 Tip: Install RTK for command rewriting: https://github.com/RTK/rink\n\n",
        "[RTK_TIP]",
    );

    // Normalize demo RTK status note
    let output = output.replace("\n\n   (RTK not installed; showing expected output)", "");

    output.replace("/private", "")
}

#[test]
fn test_should_show_doctor_diagnostics() {
    let output = sanitized_command_output(&["doctor"]);
    assert!(output.contains("tokenless doctor"));
    assert!(output.contains("Stats recording"));
    assert_snapshot!("doctor", output);
}

#[test]
fn test_should_return_zero_for_status() {
    tokenless_command()
        .env("HOME", "/tmp/tokenless-e2e-home")
        .env_remove("TOKENLESS_STATS_DB")
        .arg("status")
        .assert()
        .success();
}

#[test]
fn test_should_emit_non_empty_demo_output() {
    let output = sanitized_command_output(&["demo"]);
    assert!(!output.trim().is_empty());
    assert_snapshot!("demo", output);
}

#[test]
fn test_should_compress_schema_from_stdin() {
    let input = r#"{"function":{"name":"sum","description":"Add two numbers","parameters":{"type":"object","properties":{"a":{"type":"number"},"b":{"type":"number"}}}}}"#;
    let output = tokenless_command()
        .env("HOME", "/tmp/tokenless-e2e-home")
        .env_remove("TOKENLESS_STATS_DB")
        .arg("compress-schema")
        .write_stdin(input)
        .output()
        .expect("compress-schema should produce output");

    assert!(
        output.status.success(),
        "compress-schema failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = sanitize_output(&String::from_utf8_lossy(&output.stdout));
    assert!(stdout.contains("\"function\""));
    assert_snapshot!("compress_schema_pipe", stdout);
}

#[test]
fn test_should_return_zero_for_stats_summary() {
    tokenless_command()
        .env("HOME", "/tmp/tokenless-e2e-home")
        .env_remove("TOKENLESS_STATS_DB")
        .args(["stats", "summary"])
        .assert()
        .success();
}

#[test]
fn test_should_show_help_for_stats_delete() {
    let output = sanitized_command_output(&["stats", "delete", "--help"]);
    assert!(output.contains("Delete specific records from stats"));
    assert_snapshot!("stats_delete_help", output);
}

#[test]
fn test_should_list_all_subcommands_in_help() {
    let output = sanitized_command_output(&["--help"]);
    for command in [
        "compress-schema",
        "compress-response",
        "compress-auto",
        "compress-toon",
        "decompress-toon",
        "stats",
        "rewrite",
        "hook",
        "init",
        "env-check",
        "mcp",
        "demo",
        "doctor",
        "status",
        "tui",
    ] {
        assert!(output.contains(command), "missing subcommand {command}");
    }
    assert_snapshot!("help", output);
}

#[test]
fn test_should_accept_secure_default_global_flag() {
    let output = sanitized_command_output(&["--secure-default", "--help"]);
    assert!(output.contains("--secure-default"));
    with_settings!({ description => "secure-default global flag should be accepted" }, {
        assert_snapshot!("secure_default_help", output);
    });
}
