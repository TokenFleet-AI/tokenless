//! `tokenless init` — Install tokenless hooks for AI coding agents.
//!
//! Supported agents: claude (default), cursor, windsurf, cline, kilocode,
//! antigravity, augment, hermes, pi, gemini, opencode.

use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

/// Tokenless hook configuration.
pub struct InitConfig {
    /// Install globally vs project-local.
    pub global: bool,
}

/// Target agent for hook installation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Agent {
    /// Claude Code (default).
    Claude,
    /// Cursor editor.
    Cursor,
    /// Windsurf IDE (Cascade).
    Windsurf,
    /// Cline / Roo Code (VS Code).
    Cline,
    /// Kilo Code.
    Kilocode,
    /// Google Antigravity.
    Antigravity,
    /// Augment / Auggie.
    Augment,
    /// Hermes CLI.
    Hermes,
    /// Pi coding agent.
    Pi,
    /// Gemini CLI.
    Gemini,
    /// OpenCode VS Code extension.
    Opencode,
    /// GitHub Copilot (VS Code + CLI).
    Copilot,
}

const CLAUDE_HOOKS_JSON: &str = r#"{
  "env": {
    "RTK_SKIP_HOOK_CHECK": "1"
  },
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Bash",
        "hooks": [
          {
            "type": "command",
            "command": "tokenless hook rewrite claude"
          }
        ]
      }
    ],
    "PostToolUse": [
      {
        "matcher": "*",
        "hooks": [
          {
            "type": "command",
            "command": "tokenless hook compress"
          }
        ]
      }
    ]
  }
}"#;

#[cfg(test)]
/// Default hooks JSON for tests only.
const DEFAULT_HOOKS_JSON: &str = r#"{
  "env": {
    "RTK_SKIP_HOOK_CHECK": "1"
  },
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Bash",
        "hooks": [
          {
            "type": "command",
            "command": "tokenless hook rewrite claude"
          }
        ]
      }
    ],
    "PostToolUse": [
      {
        "matcher": "*",
        "hooks": [
          {
            "type": "command",
            "command": "tokenless hook compress"
          }
        ]
      }
    ]
  }
}"#;

const CURSOR_HOOKS_JSON: &str = r#"{
  "version": 1,
  "hooks": {
    "preToolUse": [
      {
        "command": "tokenless hook rewrite cursor",
        "matcher": "Shell"
      }
    ]
  }
}"#;

const GEMINI_HOOK_SCRIPT: &str = "#!/usr/bin/env bash
# tokenless Gemini CLI BeforeTool hook
exec tokenless hook rewrite gemini
";

const COPILOT_HOOK_JSON: &str = r#"{
  "hooks": {
    "PreToolUse": [
      {
        "type": "command",
        "command": "tokenless hook rewrite copilot",
        "cwd": ".",
        "timeout": 5
      }
    ]
  }
}"#;

fn rtk_rules(agent_name: &str) -> String {
    format!(
        r#"# RTK - Rust Token Killer ({agent_name})

**Usage**: Token-optimized CLI proxy for shell commands.

## Rule

Always prefix shell commands with `rtk` to minimize token consumption.

Examples:

```bash
rtk git status
rtk cargo test
rtk ls src/
rtk grep "pattern" src/
rtk find "*.rs" .
rtk docker ps
rtk gh pr list
```

## Meta Commands

```bash
rtk gain              # Show token savings
rtk gain --history    # Command history with savings
rtk discover          # Find missed RTK opportunities
rtk proxy <cmd>       # Run raw (no filtering, for debugging)
```

## Why

RTK filters and compresses command output before it reaches the LLM context, saving 60-90% tokens on common operations. Always use `rtk <cmd>` instead of raw commands.
"#
    )
}

/// Run `tokenless init` for the specified agent.
///
/// # Errors
///
/// Returns an error message if config cannot be written.
pub fn run(agent: Agent, config: &InitConfig) -> Result<(), String> {
    match agent {
        Agent::Claude => init_claude(config),
        Agent::Cursor => init_cursor(config),
        Agent::Windsurf => init_windsurf(config),
        Agent::Cline => init_cline(config),
        Agent::Kilocode => init_kilocode(config),
        Agent::Antigravity => init_antigravity(config),
        Agent::Augment => init_augment(config),
        Agent::Hermes => init_hermes(config),
        Agent::Pi => init_pi(config),
        Agent::Gemini => init_gemini(config),
        Agent::Opencode => init_opencode(config),
        Agent::Copilot => init_copilot(config),
    }
}

// ── helpers ────────────────────────────────────────────────

fn home_dir() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("."))
}

fn path_display(p: &Path) -> String {
    p.display().to_string()
}

fn write_file(path: &Path, content: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        #[allow(clippy::disallowed_methods)]
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create {}: {e}", path_display(parent)))?;
    }
    #[allow(clippy::disallowed_methods)]
    {
        let mut f = fs::File::create(path)
            .map_err(|e| format!("Failed to write {}: {e}", path_display(path)))?;
        f.write_all(content.as_bytes())
            .map_err(|e| format!("Failed to write {}: {e}", path_display(path)))?;
    }
    Ok(())
}

fn merge_into_settings(settings_path: &Path, hooks_json: &str) -> Result<(), String> {
    let existing = std::fs::read_to_string(settings_path).unwrap_or_default();
    let merged = if existing.is_empty() {
        hooks_json.to_string()
    } else {
        let mut existing_val: serde_json::Value =
            serde_json::from_str(&existing).map_err(|e| format!("Invalid settings JSON: {e}"))?;
        let new_val: serde_json::Value =
            serde_json::from_str(hooks_json).map_err(|e| format!("Invalid hooks JSON: {e}"))?;
        #[allow(clippy::collapsible_if)]
        if let Some(obj) = existing_val.as_object_mut() {
            if let Some(new_hooks) = new_val.get("hooks") {
                obj.insert("hooks".to_string(), new_hooks.clone());
            }
            // Merge the env block so RTK_SKIP_HOOK_CHECK is set without polluting the LLM context.
            if let Some(new_env) = new_val.get("env") {
                let existing_env = obj
                    .entry("env".to_string())
                    .or_insert_with(|| serde_json::json!({}));
                if let (Some(dst), Some(src)) = (existing_env.as_object_mut(), new_env.as_object())
                {
                    for (k, v) in src {
                        dst.insert(k.clone(), v.clone());
                    }
                }
            }
        }
        serde_json::to_string_pretty(&existing_val)
            .map_err(|e| format!("Serialization error: {e}"))?
    };
    write_file(settings_path, &merged)?;
    Ok(())
}

// ── Claude Code ────────────────────────────────────────────

fn claude_dir(global: bool) -> PathBuf {
    if global {
        home_dir().join(".claude")
    } else {
        PathBuf::from(".claude")
    }
}

fn init_claude(config: &InitConfig) -> Result<(), String> {
    let dir = claude_dir(config.global);
    let settings_path = dir.join("settings.json");
    merge_into_settings(&settings_path, CLAUDE_HOOKS_JSON)?;
    let scope = if config.global { "global" } else { "project" };
    println!("[tokenless] Installed hooks for Claude Code ({scope})");
    println!("  {}", path_display(&settings_path));
    Ok(())
}

// ── Windsurf ───────────────────────────────────────────────

fn init_windsurf(config: &InitConfig) -> Result<(), String> {
    let path = PathBuf::from(".windsurfrules");
    write_file(&path, &rtk_rules("Windsurf"))?;
    let scope = if config.global { "global" } else { "project" };
    println!("[tokenless] Installed rules for Windsurf ({scope})");
    println!("  {}", path_display(&path));
    Ok(())
}

// ── Cline ──────────────────────────────────────────────────

fn init_cline(config: &InitConfig) -> Result<(), String> {
    let path = PathBuf::from(".clinerules");
    write_file(&path, &rtk_rules("Cline"))?;
    let scope = if config.global { "global" } else { "project" };
    println!("[tokenless] Installed rules for Cline ({scope})");
    println!("  {}", path_display(&path));
    Ok(())
}

// ── Kilo Code ──────────────────────────────────────────────

fn init_kilocode(config: &InitConfig) -> Result<(), String> {
    let path = PathBuf::from(".kilocode/rules/rtk-rules.md");
    write_file(&path, &rtk_rules("Kilo Code"))?;
    let scope = if config.global { "global" } else { "project" };
    println!("[tokenless] Installed rules for Kilo Code ({scope})");
    println!("  {}", path_display(&path));
    Ok(())
}

// ── Antigravity ────────────────────────────────────────────

fn init_antigravity(config: &InitConfig) -> Result<(), String> {
    let path = PathBuf::from(".agents/rules/antigravity-rtk-rules.md");
    write_file(&path, &rtk_rules("Google Antigravity"))?;
    let scope = if config.global { "global" } else { "project" };
    println!("[tokenless] Installed rules for Antigravity ({scope})");
    println!("  {}", path_display(&path));
    Ok(())
}

// ── Augment ────────────────────────────────────────────────

fn init_augment(config: &InitConfig) -> Result<(), String> {
    let base = if config.global {
        home_dir().join(".augment")
    } else {
        PathBuf::from(".augment")
    };
    let path = base.join("rules/rtk.md");
    write_file(&path, &rtk_rules("Augment"))?;
    let scope = if config.global { "global" } else { "project" };
    println!("[tokenless] Installed rules for Augment ({scope})");
    println!("  {}", path_display(&path));
    Ok(())
}

// ── Cursor ─────────────────────────────────────────────────

fn init_cursor(config: &InitConfig) -> Result<(), String> {
    let base = if config.global {
        home_dir().join(".cursor")
    } else {
        PathBuf::from(".cursor")
    };
    let hooks_json = base.join("hooks.json");
    write_file(&hooks_json, CURSOR_HOOKS_JSON)?;
    let scope = if config.global { "global" } else { "project" };
    println!("[tokenless] Installed hooks for Cursor ({scope})");
    println!("  {}", path_display(&hooks_json));
    Ok(())
}

// ── Hermes CLI ─────────────────────────────────────────────

fn init_hermes(config: &InitConfig) -> Result<(), String> {
    let base = if config.global {
        home_dir().join(".hermes/plugins/tokenless-rewrite")
    } else {
        PathBuf::from(".hermes/plugins/tokenless-rewrite")
    };
    let manifest = base.join("plugin.yaml");
    let plugin_init = base.join("__init__.py");
    write_file(
        &manifest,
        "name: tokenless-rewrite\nversion: \"0.1.0\"\ndescription: Command rewriting via \
         tokenless\nhooks:\n  pre_tool_call:\n    - tokenless rewrite {{command}}\n",
    )?;
    write_file(
        &plugin_init,
        "# tokenless Hermes plugin\nimport subprocess\n\ndef pre_tool_call(ctx):\n    cmd = \
         ctx.get(\"command\", \"\")\n    result = subprocess.run([\"tokenless\", \"rewrite\", \
         cmd], capture_output=True, text=True)\n    if result.returncode == 0 and \
         result.stdout.strip():\n        ctx[\"command\"] = result.stdout.strip()\n",
    )?;
    let scope = if config.global { "global" } else { "project" };
    println!("[tokenless] Installed hooks for Hermes ({scope})");
    println!("  {}", path_display(&base));
    Ok(())
}

// ── Pi coding agent ────────────────────────────────────────

fn init_pi(config: &InitConfig) -> Result<(), String> {
    let base = if config.global {
        home_dir().join(".pi/agent/extensions")
    } else {
        PathBuf::from(".pi/agent/extensions")
    };
    let plugin_file = base.join("tokenless.ts");
    write_file(
        plugin_file.as_path(),
        "// tokenless Pi extension\nexport function preToolUse(command: string): string {\n  \
         const result = await exec(\"tokenless rewrite \" + command);\n  return result || \
         command;\n}\n",
    )?;
    let scope = if config.global { "global" } else { "project" };
    println!("[tokenless] Installed hooks for Pi ({scope})");
    println!("  {}", path_display(&base));
    Ok(())
}

// ── Gemini CLI ─────────────────────────────────────────────

fn init_gemini(config: &InitConfig) -> Result<(), String> {
    let base = if config.global {
        home_dir().join(".gemini")
    } else {
        PathBuf::from(".gemini")
    };
    // Write the hook wrapper script
    let hooks_dir = base.join("hooks");
    let hook_script = hooks_dir.join("tokenless-hook-gemini.sh");
    write_file(&hook_script, GEMINI_HOOK_SCRIPT)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = std::fs::metadata(&hook_script) {
            let mut perms = meta.permissions();
            perms.set_mode(0o755);
            let _ = std::fs::set_permissions(&hook_script, perms);
        }
    }
    // Write settings.json with BeforeTool hook (Gemini uses BeforeTool, not PreToolUse)
    let settings_path = base.join("settings.json");
    let gemini_hooks_json = format!(
        r#"{{
  "hooks": {{
    "BeforeTool": [
      {{
        "matcher": "run_shell_command",
        "hooks": [
          {{
            "type": "command",
            "command": "{}"
          }}
        ]
      }}
    ]
  }}
}}"#,
        hook_script.display()
    );
    merge_into_settings(&settings_path, &gemini_hooks_json)?;
    let scope = if config.global { "global" } else { "project" };
    println!("[tokenless] Installed hooks for Gemini CLI ({scope})");
    println!("  {}", path_display(&settings_path));
    println!("  {}", path_display(&hook_script));
    Ok(())
}

// ── Copilot ───────────────────────────────────────────────

fn init_copilot(config: &InitConfig) -> Result<(), String> {
    let hooks_dir = PathBuf::from(".github").join("hooks");
    let hooks_json = hooks_dir.join("rtk-rewrite.json");
    write_file(&hooks_json, COPILOT_HOOK_JSON)?;
    let scope = if config.global { "global" } else { "project" };
    println!("[tokenless] Installed hooks for GitHub Copilot ({scope})");
    println!("  {}", path_display(&hooks_json));
    Ok(())
}

// ── OpenCode ───────────────────────────────────────────────

fn init_opencode(config: &InitConfig) -> Result<(), String> {
    if !config.global {
        return Err(
            "OpenCode plugin is global-only. Use: tokenless init --global --agent opencode"
                .to_string(),
        );
    }
    let base = home_dir().join(".opencode/plugins/tokenless");
    let plugin_json = base.join("plugin.json");
    write_file(
        &plugin_json,
        "{\"name\":\"tokenless-rewrite\",\"version\":\"0.1.0\",\"hooks\":{\"before_tool_call\":{\"\
         exec\":\"tokenless rewrite {{command}}\"},\"tool_result_persist\":{\"exec\":\"tokenless \
         compress-response\"}}}",
    )?;
    println!("[tokenless] Installed hooks for OpenCode (global)");
    println!("  {}", path_display(&plugin_json));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_merge_into_new_settings() {
        let path = std::env::temp_dir().join("tokenless-test-settings.json");
        // Start empty — will be created
        let _ = std::fs::remove_file(&path);
        merge_into_settings(&path, DEFAULT_HOOKS_JSON).unwrap();
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("PreToolUse"));
        assert!(content.contains("PostToolUse"));
        assert!(content.contains("tokenless hook rewrite claude"));
        assert!(content.contains("RTK_SKIP_HOOK_CHECK"));
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn test_merge_preserves_existing() {
        let path = std::env::temp_dir().join("tokenless-test-merge.json");
        std::fs::write(&path, r#"{"env":{"KEY":"val"}}"#).unwrap();
        merge_into_settings(&path, DEFAULT_HOOKS_JSON).unwrap();
        let content = std::fs::read_to_string(&path).unwrap();
        let v: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert!(v.get("hooks").is_some());
        assert_eq!(v["env"]["KEY"], "val");
        assert_eq!(v["env"]["RTK_SKIP_HOOK_CHECK"], "1");
        std::fs::remove_file(&path).ok();
    }
}
