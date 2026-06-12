//! `tokenless init` — Install tokenless hooks for AI coding agents.
//!
//! Supported agents: claude (default), cursor, windsurf, cline, kilocode,
//! antigravity, augment, hermes, pi, gemini, opencode, copilot, codex.

pub mod user_detect;

use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

/// Tokenless hook configuration.
pub struct InitConfig {
    /// Install globally vs project-local.
    pub global: bool,
    /// Enable debug logging for compress hook (~/.tokenfleet-ai/tokenless/compress-debug.log).
    pub debug: bool,
    /// If Some(true), install compress hook.
    /// If Some(false), skip compress hook.
    /// If None (legacy), treated as true.
    pub compress: Option<bool>,
    /// Whether passthrough mode is enabled.
    /// When true, hooks pass through original content unchanged but still
    /// record compress logs for baseline measurement.
    pub passthrough: bool,
    /// User name for statistics attribution (auto-detected from git config or OS).
    pub user_name: Option<String>,
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
    /// OpenAI Codex CLI (AGENTS.md + RTK.md rules, no hooks).
    Codex,
}

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
            "command": "tokenless hook rewrite --target claude --project test-project"
          }
        ]
      }
    ],
    "PostToolUse": [
      {
        "matcher": "^(?!Bash$).*",
        "hooks": [
          {
            "type": "command",
            "command": "tokenless hook compress --semantic --target claude --project test-project"
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
        "command": "tokenless hook rewrite --target cursor",
        "matcher": "Shell"
      }
    ],
    "postToolUse": [
      {
        "command": "tokenless hook compress --semantic --target cursor",
        "matcher": "*"
      }
    ]
  }
}"#;

const GEMINI_HOOK_SCRIPT: &str = "#!/usr/bin/env bash
# tokenless Gemini CLI BeforeTool hook
exec tokenless hook rewrite --target gemini
";

const GEMINI_HOOK_COMPRESS_SCRIPT: &str = "#!/usr/bin/env bash
# tokenless Gemini CLI AfterTool hook
exec tokenless hook compress --semantic --target gemini
";

const COPILOT_HOOK_JSON: &str = r#"{
  "hooks": {
    "PreToolUse": [
      {
        "type": "command",
        "command": "tokenless hook rewrite --target copilot",
        "cwd": ".",
        "timeout": 5
      }
    ],
    "PostToolUse": [
      {
        "type": "command",
        "command": "tokenless hook compress --semantic --target copilot",
        "cwd": ".",
        "timeout": 10
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
        Agent::Codex => init_codex(config),
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

/// Detect the project name for the current working directory.
///
/// Priority: CLI override > git remote > Cargo.toml > package.json > dirname fallback.
fn detect_project_name(cwd: &Path, cli_override: Option<&str>) -> String {
    // 1. Explicit CLI override — highest priority
    if let Some(name) = cli_override {
        return name.to_string();
    }

    // 2. git remote get-url origin → extract repo name
    if let Ok(output) = std::process::Command::new("git")
        .args(["remote", "get-url", "origin"])
        .current_dir(cwd)
        .output()
    {
        if output.status.success() {
            let url = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if let Some(name) = extract_repo_name(&url) {
                return name;
            }
        }
    }

    // 3. Cargo.toml [package].name
    if let Some(name) = read_manifest_field(cwd.join("Cargo.toml"), "package.name") {
        return name;
    }

    // 4. package.json "name"
    if let Some(name) = read_manifest_field(cwd.join("package.json"), "name") {
        return name;
    }

    // 5. Fallback: current directory basename
    cwd.file_name()
        .map(|n| n.to_string_lossy().to_string())
        .filter(|n| n != "/" && !n.is_empty())
        .unwrap_or_else(|| "(unclassified)".to_string())
}

/// Extract the repository name from a git remote URL.
///
/// Handles HTTPS, SSH, and git:// URL formats.
fn extract_repo_name(url: &str) -> Option<String> {
    let url = url.strip_suffix(".git").unwrap_or(url);
    let name = url.rsplit('/').next()?;
    let name = name.split(':').next_back()?;
    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
}

/// Read a field from a JSON or TOML manifest file using simple line-based parsing.
fn read_manifest_field(path: std::path::PathBuf, field_path: &str) -> Option<String> {
    let content = std::fs::read_to_string(&path).ok()?;

    match field_path {
        "package.name" => {
            // Simple line-based parse for Cargo.toml: name = "value"
            for line in content.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with("name") {
                    if let Some(val) = trimmed.split('=').nth(1) {
                        let val = val.trim().trim_matches('"').trim_matches('\'');
                        if !val.is_empty() {
                            return Some(val.to_string());
                        }
                    }
                }
            }
            None
        }
        "name" => {
            // Simple parse for package.json: "name": "value"
            for line in content.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with("\"name\"") || trimmed.starts_with("\"name ") {
                    if let Some(val) = trimmed.split(':').nth(1) {
                        let val = val
                            .trim()
                            .trim_matches('"')
                            .trim_matches('\'')
                            .trim_end_matches(',');
                        if !val.is_empty() {
                            return Some(val.to_string());
                        }
                    }
                }
            }
            None
        }
        _ => None,
    }
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

    // Project flag: only for project-local installs.
    // Global hooks apply to all projects, so --project is omitted
    // and the hook auto-detects the project at runtime.
    let project_flag = if config.global {
        String::new()
    } else {
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let project_name = detect_project_name(&cwd, None);
        format!(" --project {project_name}")
    };

    let debug_flag = if config.debug { " --debug" } else { "" };
    let user_flag = if let Some(ref user) = config.user_name {
        format!(" --user-name {user}")
    } else {
        String::new()
    };
    let hooks_json = if config.compress == Some(false) {
        // Only PreToolUse rewrite hook, no PostToolUse compress hook
        format!(
            r#"{{
  "env": {{ "RTK_SKIP_HOOK_CHECK": "1" }},
  "hooks": {{
    "PreToolUse": [
      {{
        "matcher": "Bash",
        "hooks": [
          {{ "type": "command", "command": "tokenless hook rewrite --target claude{project}{user}" }}
        ]
      }}
    ]
  }}
}}"#,
            project = project_flag,
            user = user_flag,
        )
    } else {
        format!(
            r#"{{
  "env": {{ "RTK_SKIP_HOOK_CHECK": "1" }},
  "hooks": {{
    "PreToolUse": [
      {{
        "matcher": "Bash",
        "hooks": [
          {{ "type": "command", "command": "tokenless hook rewrite --target claude{project}{user}" }}
        ]
      }}
    ],
    "PostToolUse": [
      {{
        "matcher": "^(?!Bash$).*",
        "hooks": [
          {{ "type": "command", "command": "tokenless hook compress --semantic --target claude{project}{debug}{user}" }}
        ]
      }}
    ]
  }}
}}"#,
            project = project_flag,
            debug = debug_flag,
            user = user_flag,
        )
    };

    merge_into_settings(&settings_path, &hooks_json)?;
    let scope = if config.global { "global" } else { "project" };
    println!("[tokenless] Installed hooks for Claude Code ({scope})");
    if config.global {
        println!("  project: auto-detect (global)");
    } else {
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let project_name = detect_project_name(&cwd, None);
        println!("  project: {project_name}");
    }
    println!(
        "  compress: {}",
        if config.compress != Some(false) {
            "enabled"
        } else {
            "disabled"
        }
    );
    if config.passthrough {
        println!("  passthrough: enabled (hooks pass-through, logs only)");
    }
    if config.debug {
        println!("  debug: ~/.tokenfleet-ai/tokenless/compress-debug.log");
    }
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
    // Write the hook wrapper scripts
    let hooks_dir = base.join("hooks");
    let hook_script = hooks_dir.join("tokenless-hook-gemini.sh");
    write_file(&hook_script, GEMINI_HOOK_SCRIPT)?;
    let compress_script = hooks_dir.join("tokenless-hook-compress-gemini.sh");
    write_file(&compress_script, GEMINI_HOOK_COMPRESS_SCRIPT)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        for script in &[&hook_script, &compress_script] {
            if let Ok(meta) = std::fs::metadata(script) {
                let mut perms = meta.permissions();
                perms.set_mode(0o755);
                let _ = std::fs::set_permissions(script, perms);
            }
        }
    }
    // Write settings.json with BeforeTool + AfterTool hooks
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
            "command": "{rewrite}"
          }}
        ]
      }}
    ],
    "AfterTool": [
      {{
        "matcher": "*",
        "hooks": [
          {{
            "type": "command",
            "command": "{compress}"
          }}
        ]
      }}
    ]
  }}
}}"#,
        rewrite = hook_script.display(),
        compress = compress_script.display(),
    );
    merge_into_settings(&settings_path, &gemini_hooks_json)?;
    let scope = if config.global { "global" } else { "project" };
    println!("[tokenless] Installed hooks for Gemini CLI ({scope})");
    println!("  {}", path_display(&settings_path));
    println!("  {}", path_display(&hook_script));
    println!("  {}", path_display(&compress_script));
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

// ── Codex CLI ───────────────────────────────────────────────
//
// Codex has no hook protocol. Integration is via AGENTS.md + RTK.md
// rules files (same pattern as RTK project hooks/codex/).

/// RTK awareness content for Codex CLI.
const CODEX_RULES: &str = r#"# RTK - Rust Token Killer (Codex CLI)

**Usage**: Token-optimized CLI proxy for shell commands.

## Rule

Always prefix shell commands with `rtk`.

Examples:

```bash
rtk git status
rtk cargo test
rtk npm run build
rtk pytest -q
```

## Meta Commands

```bash
rtk gain            # Token savings analytics
rtk gain --history  # Recent command savings history
rtk proxy <cmd>     # Run raw command without filtering
```

## Verification

```bash
rtk --version
rtk gain
which rtk
```
"#;

const RTK_MD: &str = "RTK.md";
const AGENTS_MD: &str = "AGENTS.md";
const RTK_MD_REF: &str = "@RTK.md";

/// Resolve the Codex config directory.
///
/// Priority: `$CODEX_HOME` → `~/.codex/`.
fn resolve_codex_dir() -> PathBuf {
    resolve_codex_dir_from(
        std::env::var_os("CODEX_HOME").map(PathBuf::from),
        dirs::home_dir(),
    )
}

/// Resolve Codex dir from explicit env + home fallback (testable).
fn resolve_codex_dir_from(codex_home: Option<PathBuf>, home_dir: Option<PathBuf>) -> PathBuf {
    if let Some(path) = codex_home.filter(|p| !p.as_os_str().is_empty()) {
        return path;
    }
    home_dir
        .map(|home| home.join(".codex"))
        .unwrap_or_else(|| PathBuf::from(".codex"))
}

/// Generate the `@RTK.md` reference string.
///
/// In global mode, Codex resolves `@` references relative to CWD, not
/// relative to AGENTS.md location. Use an absolute path for safety.
fn codex_rtk_md_ref(codex_dir: &Path) -> String {
    format!("@{}", codex_dir.join(RTK_MD).display())
}

/// Add an `@RTK.md` reference to AGENTS.md, preserving existing content.
///
/// Returns `Ok(true)` if the reference was added, `Ok(false)` if it
/// already existed.
fn add_ref_to_agents_md(agents_md: &Path, rtk_ref: &str) -> Result<bool, String> {
    let content = if agents_md.exists() {
        std::fs::read_to_string(agents_md)
            .map_err(|e| format!("Failed to read {}: {e}", path_display(agents_md)))?
    } else {
        String::new()
    };

    // Idempotent: skip if reference already present
    if content.contains(rtk_ref) || content.contains(RTK_MD_REF) {
        return Ok(false);
    }

    let new_content = if content.is_empty() {
        format!("{rtk_ref}\n")
    } else {
        format!("{}\n\n{rtk_ref}\n", content.trim())
    };

    write_file(agents_md, &new_content)?;
    Ok(true)
}

fn init_codex(config: &InitConfig) -> Result<(), String> {
    let (agents_md_path, rtk_md_path, codex_dir) = if config.global {
        let dir = resolve_codex_dir();
        (dir.join(AGENTS_MD), dir.join(RTK_MD), dir)
    } else {
        (
            PathBuf::from(AGENTS_MD),
            PathBuf::from(RTK_MD),
            PathBuf::new(),
        )
    };

    // Global mode: create parent dir, use absolute @ reference
    if config.global {
        if let Some(parent) = agents_md_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create {}: {e}", path_display(parent)))?;
        }
    }

    let rtk_ref = if config.global {
        codex_rtk_md_ref(&codex_dir)
    } else {
        RTK_MD_REF.to_string()
    };

    write_file(&rtk_md_path, CODEX_RULES)?;
    let added = add_ref_to_agents_md(&agents_md_path, &rtk_ref)?;

    let scope = if config.global { "global" } else { "project" };
    println!("[tokenless] Installed rules for Codex CLI ({scope})");
    println!("  RTK.md:    {}", path_display(&rtk_md_path));
    if added {
        println!("  AGENTS.md: {rtk_ref} reference added");
    } else {
        println!("  AGENTS.md: {rtk_ref} reference already present");
    }
    if config.global {
        println!(
            "\n  Codex global instructions path: {}",
            path_display(&agents_md_path)
        );
    }
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
        assert!(content.contains("tokenless hook rewrite --target claude"));
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

    // ── project name detection tests (TDD RED) ─────────────────

    #[test]
    fn test_detect_cli_override_wins() {
        let cwd = std::env::temp_dir();
        let name = detect_project_name(&cwd, Some("my-custom-app"));
        assert_eq!(name, "my-custom-app");
    }

    #[test]
    fn test_detect_fallback_to_dirname() {
        let dir = std::env::temp_dir().join("tokenless-test-project");
        let _ = std::fs::create_dir_all(&dir);
        let name = detect_project_name(&dir, None);
        assert_eq!(name, "tokenless-test-project");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_detect_unknown_dir() {
        let name = detect_project_name(std::path::Path::new("/"), None);
        assert_eq!(name, "(unclassified)");
    }

    #[test]
    fn test_extract_repo_name_https() {
        assert_eq!(
            extract_repo_name("https://github.com/user/my-repo.git"),
            Some("my-repo".to_string())
        );
    }

    #[test]
    fn test_extract_repo_name_ssh() {
        assert_eq!(
            extract_repo_name("git@github.com:TokenFleet-AI/tokenless.git"),
            Some("tokenless".to_string())
        );
    }

    #[test]
    fn test_extract_repo_name_no_extension() {
        assert_eq!(
            extract_repo_name("https://github.com/user/repo"),
            Some("repo".to_string())
        );
    }

    // ── Codex tests (TDD RED → GREEN) ──────────────────────────

    /// Helper: run init for Codex against a temp dir.
    fn init_codex_at(dir: &Path, global: bool) -> Result<(), String> {
        let agents_md = dir.join(AGENTS_MD);
        let rtk_md = dir.join(RTK_MD);
        if global {
            fs::create_dir_all(dir).map_err(|e| format!("create dir: {e}"))?;
        }
        write_file(&rtk_md, CODEX_RULES)?;
        let rtk_ref = if global {
            codex_rtk_md_ref(dir)
        } else {
            RTK_MD_REF.to_string()
        };
        add_ref_to_agents_md(&agents_md, &rtk_ref)?;
        Ok(())
    }

    #[test]
    fn test_should_parse_agent_codex() {
        let agent_str = "codex";
        let agent = match agent_str {
            "cursor" => Agent::Cursor,
            "codex" => Agent::Codex,
            _ => Agent::Claude,
        };
        assert_eq!(agent, Agent::Codex);
    }

    #[test]
    fn test_should_init_codex_project_local_writes_rtk_md() {
        let dir = std::env::temp_dir().join("tl-codex-test-project");
        let _ = std::fs::remove_dir_all(&dir);
        let rtk_md = dir.join(RTK_MD);

        init_codex_at(&dir, false).unwrap();

        assert!(rtk_md.exists(), "RTK.md should exist");
        let content = std::fs::read_to_string(&rtk_md).unwrap();
        assert!(content.contains("RTK - Rust Token Killer"));
        assert!(content.contains("rtk git status"));
        assert!(content.contains("Codex CLI"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_should_init_codex_patches_agents_md() {
        let dir = std::env::temp_dir().join("tl-codex-test-agents");
        let _ = std::fs::remove_dir_all(&dir);
        let agents_md = dir.join(AGENTS_MD);

        init_codex_at(&dir, false).unwrap();

        assert!(agents_md.exists(), "AGENTS.md should exist");
        let content = std::fs::read_to_string(&agents_md).unwrap();
        assert!(
            content.contains("@RTK.md"),
            "should contain @RTK.md reference"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_should_init_codex_global_writes_to_codex_home() {
        let dir = std::env::temp_dir().join("tl-codex-test-global");
        let _ = std::fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        init_codex_at(&dir, true).unwrap();

        assert!(dir.join(RTK_MD).exists(), "global RTK.md should exist");
        assert!(
            dir.join(AGENTS_MD).exists(),
            "global AGENTS.md should exist"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_should_init_codex_global_uses_absolute_reference() {
        let dir = std::env::temp_dir().join("tl-codex-test-abs-ref");
        let _ = std::fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        init_codex_at(&dir, true).unwrap();

        let content = std::fs::read_to_string(dir.join(AGENTS_MD)).unwrap();
        let expected_ref = codex_rtk_md_ref(&dir);
        assert!(
            content.contains(&expected_ref),
            "global AGENTS.md should contain absolute @ reference: {expected_ref}"
        );
        assert!(
            expected_ref != RTK_MD_REF,
            "global reference must be absolute path, not relative"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_should_init_codex_idempotent() {
        let dir = std::env::temp_dir().join("tl-codex-test-idempotent");
        let _ = std::fs::remove_dir_all(&dir);

        // First run
        init_codex_at(&dir, false).unwrap();
        let agents_before = std::fs::read_to_string(dir.join(AGENTS_MD)).unwrap();

        // Second run
        init_codex_at(&dir, false).unwrap();
        let agents_after = std::fs::read_to_string(dir.join(AGENTS_MD)).unwrap();

        assert_eq!(
            agents_before.matches("@RTK.md").count(),
            agents_after.matches("@RTK.md").count(),
            "AGENTS.md ref count should be idempotent"
        );
        assert_eq!(
            agents_before, agents_after,
            "AGENTS.md unchanged on second run"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_should_resolve_codex_dir_from_env() {
        let custom = PathBuf::from("/tmp/custom-codex-home");
        let result = resolve_codex_dir_from(Some(custom.clone()), None);
        assert_eq!(result, custom);
    }

    #[test]
    fn test_should_resolve_codex_dir_fallback_to_dot_codex() {
        let home = PathBuf::from("/Users/testuser");
        let result = resolve_codex_dir_from(None, Some(home.clone()));
        assert_eq!(result, home.join(".codex"));
        let result_empty = resolve_codex_dir_from(Some(PathBuf::new()), Some(home.clone()));
        assert_eq!(result_empty, home.join(".codex"));
    }

    #[test]
    fn test_should_init_codex_preserves_existing_agents_md() {
        let dir = std::env::temp_dir().join("tl-codex-test-preserve");
        let _ = std::fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join(AGENTS_MD),
            "# My Project\n\nSome custom instructions.\n",
        )
        .unwrap();

        init_codex_at(&dir, false).unwrap();

        let content = std::fs::read_to_string(dir.join(AGENTS_MD)).unwrap();
        assert!(
            content.contains("My Project"),
            "should preserve existing content"
        );
        assert!(
            content.contains("custom instructions"),
            "should preserve custom text"
        );
        assert!(content.contains("@RTK.md"), "should add RTK reference");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
