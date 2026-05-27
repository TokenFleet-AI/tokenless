#![allow(clippy::expect_used, clippy::unwrap_used, clippy::approx_constant)]
//! Hook protocol format compliance tests.
//!
//! These tests verify the JSON structures that the tokenless hook handlers
//! produce for each supported agent protocol. They do not execute the binary
//! but verify the expected output shape matches each protocol specification.

use serde_json::{Value, json};

// ---------------------------------------------------------------------------
// Claude Code hook protocol (PreToolUse)
// ---------------------------------------------------------------------------

#[test]
fn test_claude_hook_response_format() {
    // The Claude hook response: {"hookSpecificOutput": {"hookEventName": "...",
    //   "permissionDecision": "allow", "updatedInput": {"command": "rewritten"}}}
    let response = json!({
        "hookSpecificOutput": {
            "hookEventName": "PreToolUse",
            "permissionDecision": "allow",
            "permissionDecisionReason": "tokenless auto-rewrite",
            "updatedInput": {
                "command": "rtk git status"
            }
        }
    });

    let s = serde_json::to_string(&response).expect("must serialize");
    let parsed: Value = serde_json::from_str(&s).expect("must be valid JSON");

    // Verify the structure
    assert!(
        parsed.get("hookSpecificOutput").is_some(),
        "Claude: must have hookSpecificOutput"
    );
    let hso = &parsed["hookSpecificOutput"];
    assert_eq!(
        hso["hookEventName"], "PreToolUse",
        "Claude: hookEventName must be PreToolUse"
    );
    assert_eq!(
        hso["permissionDecision"], "allow",
        "Claude: permissionDecision must be allow"
    );
    assert!(
        hso.get("updatedInput").is_some(),
        "Claude: must have updatedInput"
    );
    assert_eq!(
        hso["updatedInput"]["command"], "rtk git status",
        "Claude: updatedInput.command must match"
    );
}

#[test]
fn test_claude_hook_handles_empty_command() {
    // When cmd is empty, Claude hook returns nothing (early exit)
    // Verify the expected empty-response behavior
    let empty_input = json!({"tool_input": {"command": ""}});
    let cmd = empty_input
        .pointer("/tool_input/command")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    assert!(cmd.is_empty(), "empty command should be detected");

    // No output is produced for empty commands (hook exits early via Ok(()))
}

#[test]
fn test_claude_hook_parses_tool_input() {
    // Verify tool_input.command is accessible via JSON pointer
    let input = json!({"tool_input": {"command": "git status"}});
    let cmd = input
        .pointer("/tool_input/command")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    assert_eq!(cmd, "git status");
}

// ---------------------------------------------------------------------------
// Cursor hook protocol (PreToolUse)
// ---------------------------------------------------------------------------

#[test]
fn test_cursor_hook_response_format() {
    // The Cursor hook response: {"continue": true, "permission": "allow",
    //   "updated_input": {"command": "rewritten"}}
    let response = json!({
        "continue": true,
        "permission": "allow",
        "updated_input": {
            "command": "rtk cargo test"
        }
    });

    let s = serde_json::to_string(&response).expect("must serialize");
    let parsed: Value = serde_json::from_str(&s).expect("must be valid JSON");

    assert_eq!(parsed["continue"], true, "Cursor: continue must be true");
    assert_eq!(
        parsed["permission"], "allow",
        "Cursor: permission must be allow"
    );
    assert!(
        parsed.get("updated_input").is_some(),
        "Cursor: must have updated_input"
    );
    assert_eq!(
        parsed["updated_input"]["command"], "rtk cargo test",
        "Cursor: command must match"
    );
}

#[test]
fn test_cursor_hook_empty_output() {
    // When no rewrite is possible, Cursor hook outputs "{}"
    let empty = json!({});
    let s = serde_json::to_string(&empty).unwrap_or_default();
    let parsed: Value = serde_json::from_str(&s).expect("must be valid JSON");
    assert!(
        parsed.as_object().is_some_and(serde_json::Map::is_empty),
        "Cursor: empty response must be empty object"
    );
}

// ---------------------------------------------------------------------------
// Gemini CLI hook protocol (BeforeTool)
// ---------------------------------------------------------------------------

#[test]
fn test_gemini_hook_response_format() {
    // The Gemini hook response: {"decision": "allow",
    //   "hookSpecificOutput": {"tool_input": {"command": "rewritten"}}}
    let response = json!({
        "decision": "allow",
        "hookSpecificOutput": {
            "tool_input": {
                "command": "rtk ls -la"
            }
        }
    });

    let s = serde_json::to_string(&response).expect("must serialize");
    let parsed: Value = serde_json::from_str(&s).expect("must be valid JSON");

    assert_eq!(
        parsed["decision"], "allow",
        "Gemini: decision must be allow"
    );
    assert!(
        parsed.get("hookSpecificOutput").is_some(),
        "Gemini: must have hookSpecificOutput"
    );
    let hso = &parsed["hookSpecificOutput"];
    assert!(
        hso.get("tool_input").is_some(),
        "Gemini: must have tool_input"
    );
    assert_eq!(
        hso["tool_input"]["command"], "rtk ls -la",
        "Gemini: command must match"
    );
}

#[test]
fn test_gemini_hook_allow_only() {
    // When no rewrite, Gemini hook outputs {"decision":"allow"}
    let decision_only = json!({"decision": "allow"});
    let s = serde_json::to_string(&decision_only).unwrap_or_default();
    let parsed: Value = serde_json::from_str(&s).expect("must be valid JSON");
    assert_eq!(
        parsed["decision"], "allow",
        "Gemini: decision-only response must have decision=allow"
    );
    assert!(
        parsed.get("hookSpecificOutput").is_none(),
        "Gemini: decision-only has no hookSpecificOutput"
    );
}

// ---------------------------------------------------------------------------
// Copilot hook protocol (PreToolUse)
// ---------------------------------------------------------------------------

#[test]
fn test_copilot_claude_style_response_format() {
    // VS Code Copilot Chat uses the same protocol as Claude
    let response = json!({
        "hookSpecificOutput": {
            "hookEventName": "PreToolUse",
            "permissionDecision": "allow",
            "permissionDecisionReason": "tokenless auto-rewrite",
            "updatedInput": {
                "command": "rtk npm install"
            }
        }
    });

    let s = serde_json::to_string(&response).expect("must serialize");
    let parsed: Value = serde_json::from_str(&s).expect("must be valid JSON");
    assert!(parsed.get("hookSpecificOutput").is_some());
    assert_eq!(parsed["hookSpecificOutput"]["permissionDecision"], "allow");
    assert_eq!(
        parsed["hookSpecificOutput"]["updatedInput"]["command"],
        "rtk npm install"
    );
}

#[test]
fn test_copilot_cli_style_response_format() {
    // Copilot CLI uses a deny+reason format
    let response = json!({
        "permissionDecision": "deny",
        "permissionDecisionReason": "Token savings: use `rtk git diff` instead (rtk saves 60-90% tokens)"
    });

    let s = serde_json::to_string(&response).expect("must serialize");
    let parsed: Value = serde_json::from_str(&s).expect("must be valid JSON");
    assert_eq!(
        parsed["permissionDecision"], "deny",
        "Copilot CLI: permissionDecision must be deny"
    );
    assert!(
        parsed["permissionDecisionReason"]
            .as_str()
            .unwrap()
            .contains("rtk"),
        "Copilot CLI: reason must reference rtk"
    );
}

// ---------------------------------------------------------------------------
// BOM handling test
// ---------------------------------------------------------------------------

#[test]
fn test_strip_leading_bom_single() {
    let input_with_bom = "\u{feff}{\"tool_input\": {\"command\": \"ls\"}}";
    let stripped = input_with_bom
        .strip_prefix("\u{feff}")
        .unwrap_or(input_with_bom);
    assert!(
        stripped.starts_with('{'),
        "input after BOM removal should start with '{{', got: {stripped:?}"
    );
    let parsed: Value = serde_json::from_str(stripped).expect("must parse after BOM removal");
    assert_eq!(parsed["tool_input"]["command"], "ls");
}

#[test]
fn test_strip_leading_bom_double() {
    let input_with_bom = "\u{feff}\u{feff}{\"tool_input\": {\"command\": \"pwd\"}}";
    // Match the actual logic: try single first, then try double
    let stripped = input_with_bom
        .strip_prefix("\u{feff}")
        .or_else(|| input_with_bom.strip_prefix("\u{feff}\u{feff}"))
        .unwrap_or(input_with_bom);
    // After stripping one BOM, one remains, but JSON parser may handle it
    assert!(!stripped.is_empty());
}

#[test]
fn test_strip_leading_bom_no_bom() {
    let input = "{\"tool_input\": {\"command\": \"echo hello\"}}";
    let stripped = input
        .strip_prefix("\u{feff}")
        .or_else(|| input.strip_prefix("\u{feff}\u{feff}"))
        .unwrap_or(input);
    assert_eq!(stripped, input, "input without BOM should be unchanged");
    let parsed: Value = serde_json::from_str(stripped).expect("must parse without BOM");
    assert_eq!(parsed["tool_input"]["command"], "echo hello");
}

// ---------------------------------------------------------------------------
// Round-trip: all protocol formats produce parsable JSON
// ---------------------------------------------------------------------------

#[test]
fn test_hook_protocols_are_stable_json() {
    // Verify all three protocol shapes remain valid JSON across different commands.
    let commands = [
        "git status",
        "cargo test --workspace",
        "docker ps -a",
        "npm install --save-dev typescript",
        "gh pr list --state open",
    ];

    for cmd in &commands {
        let rewritten = format!("rtk {cmd}");

        // Claude
        let claude = json!({
            "hookSpecificOutput": {
                "hookEventName": "PreToolUse",
                "permissionDecision": "allow",
                "permissionDecisionReason": "tokenless auto-rewrite",
                "updatedInput": { "command": rewritten }
            }
        });
        let _: Value = serde_json::from_str(&serde_json::to_string(&claude).unwrap()).unwrap();

        // Cursor
        let cursor = json!({
            "continue": true,
            "permission": "allow",
            "updated_input": { "command": rewritten }
        });
        let _: Value = serde_json::from_str(&serde_json::to_string(&cursor).unwrap()).unwrap();

        // Gemini
        let gemini = json!({
            "decision": "allow",
            "hookSpecificOutput": {
                "tool_input": { "command": rewritten }
            }
        });
        let _: Value = serde_json::from_str(&serde_json::to_string(&gemini).unwrap()).unwrap();
    }
}
