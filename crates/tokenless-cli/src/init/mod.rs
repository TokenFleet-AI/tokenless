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
}

const HOOKS_JSON: &str = r#"{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Bash",
        "hooks": [
          {
            "type": "command",
            "command": "tokenless rewrite {{input}}"
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
            "command": "tokenless compress-response"
          }
        ]
      }
    ]
  }
}"#;

const SH_HOOK_REWRITE: &str = "#!/usr/bin/env bash
# tokenless hook — rewrite commands
exec tokenless rewrite \"$(cat)\"
";

const SH_HOOK_COMPRESS: &str = "#!/usr/bin/env bash
# tokenless hook — compress responses
exec tokenless compress-response
";

/// Run `tokenless init` for the specified agent.
///
/// # Errors
///
/// Returns an error message if config cannot be written.
pub fn run(agent: Agent, config: &InitConfig) -> Result<(), String> {
    match agent {
        Agent::Claude => init_claude(config),
        Agent::Cursor => init_cursor(config),
        Agent::Windsurf => init_generic_agent(".windsurf", config),
        Agent::Cline => init_cline(config),
        Agent::Kilocode => init_generic_agent(".kilocode", config),
        Agent::Antigravity => init_generic_agent(".antigravity", config),
        Agent::Augment => init_generic_agent(".augment", config),
        Agent::Hermes => init_hermes(config),
        Agent::Pi => init_pi(config),
        Agent::Gemini => init_gemini(config),
        Agent::Opencode => init_opencode(config),
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

fn merge_into_settings(settings_path: &Path) -> Result<(), String> {
    let existing = std::fs::read_to_string(settings_path).unwrap_or_default();
    let merged = if existing.is_empty() {
        HOOKS_JSON.to_string()
    } else {
        let mut existing_val: serde_json::Value =
            serde_json::from_str(&existing).map_err(|e| format!("Invalid settings JSON: {e}"))?;
        let new_val: serde_json::Value =
            serde_json::from_str(HOOKS_JSON).map_err(|e| format!("Invalid hooks JSON: {e}"))?;
        #[allow(clippy::collapsible_if)]
        if let Some(obj) = existing_val.as_object_mut() {
            if let Some(new_hooks) = new_val.get("hooks") {
                obj.insert("hooks".to_string(), new_hooks.clone());
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
    merge_into_settings(&settings_path)?;
    let scope = if config.global { "global" } else { "project" };
    println!("[tokenless] Installed hooks for Claude Code ({scope})");
    println!("  {}", path_display(&settings_path));
    Ok(())
}

// ── Cursor ─────────────────────────────────────────────────

fn init_cursor(config: &InitConfig) -> Result<(), String> {
    let base = if config.global {
        home_dir().join(".cursor")
    } else {
        PathBuf::from(".cursor")
    };
    let pre = base.join("hooks/pre_tool_use.sh");
    let post = base.join("hooks/post_tool_use.sh");
    write_file(&pre, SH_HOOK_REWRITE)?;
    write_file(&post, SH_HOOK_COMPRESS)?;
    let scope = if config.global { "global" } else { "project" };
    println!("[tokenless] Installed hooks for Cursor ({scope})");
    println!("  {}", path_display(&pre));
    println!("  {}", path_display(&post));
    Ok(())
}

// ── Generic agent (Windsurf, Kilo, Antigravity, Augment) ──

fn init_generic_agent(dir: &str, config: &InitConfig) -> Result<(), String> {
    let base = if config.global {
        home_dir().join(dir)
    } else {
        PathBuf::from(dir.trim_start_matches('.'))
    };
    let settings_path = base.join("settings.json");
    merge_into_settings(&settings_path)?;
    let name = dir.trim_start_matches('.');
    let scope = if config.global { "global" } else { "project" };
    let capitalized: String = name[..1]
        .to_uppercase()
        .chars()
        .chain(name[1..].chars())
        .collect();
    println!("[tokenless] Installed hooks for {capitalized} ({scope})");
    println!("  {}", path_display(&settings_path));
    Ok(())
}

// ── Cline (VS Code extension) ───────────────────────────────

fn init_cline(config: &InitConfig) -> Result<(), String> {
    // Cline stores config in VS Code's global storage
    let vscode_config = if config.global {
        home_dir().join(".config/Code/User/globalStorage/saoudrizwan.claude-dev/settings")
    } else {
        PathBuf::from(".vscode/globalStorage/saoudrizwan.claude-dev/settings.json")
    };
    merge_into_settings(&vscode_config)?;
    println!("[tokenless] Installed hooks for Cline");
    println!("  {}", path_display(&vscode_config));
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
    let hook_file = base.join("rtk-hook-gemini.sh");
    write_file(&hook_file, SH_HOOK_REWRITE)?;
    let scope = if config.global { "global" } else { "project" };
    println!("[tokenless] Installed hooks for Gemini CLI ({scope})");
    println!("  {}", path_display(&hook_file));
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
        merge_into_settings(&path).unwrap();
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("PreToolUse"));
        assert!(content.contains("PostToolUse"));
        assert!(content.contains("tokenless rewrite"));
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn test_merge_preserves_existing() {
        let path = std::env::temp_dir().join("tokenless-test-merge.json");
        std::fs::write(&path, r#"{"env":{"KEY":"val"}}"#).unwrap();
        merge_into_settings(&path).unwrap();
        let content = std::fs::read_to_string(&path).unwrap();
        let v: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert!(v.get("hooks").is_some());
        assert_eq!(v["env"]["KEY"], "val");
        std::fs::remove_file(&path).ok();
    }
}
