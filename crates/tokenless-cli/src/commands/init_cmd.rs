//! Handler for `tokenless init`.

use tokenless_stats::TokenlessConfig;

use crate::init;

/// Handle `tokenless init`.
pub(crate) fn handle(
    global: bool,
    agent: String,
    debug: bool,
    compress: bool,
    no_compress: bool,
    passthrough: bool,
) -> Result<(), (String, i32)> {
    let agent = match agent.as_str() {
        "cursor" => init::Agent::Cursor,
        "windsurf" => init::Agent::Windsurf,
        "cline" => init::Agent::Cline,
        "kilocode" => init::Agent::Kilocode,
        "antigravity" => init::Agent::Antigravity,
        "augment" => init::Agent::Augment,
        "hermes" => init::Agent::Hermes,
        "pi" => init::Agent::Pi,
        "gemini" => init::Agent::Gemini,
        "opencode" => init::Agent::Opencode,
        "copilot" => init::Agent::Copilot,
        "codex" => init::Agent::Codex,
        _ => init::Agent::Claude,
    };

    // Resolve compress flag from CLI
    let compress_flag = if no_compress {
        Some(false)
    } else if compress {
        Some(true)
    } else {
        None
    };

    // Load and update persistent config
    let mut config = TokenlessConfig::load();

    // Detect user identity (skip if current_dir fails)
    let user_name = if let Ok(cwd) = std::env::current_dir() {
        let identity = crate::init::user_detect::detect_user_identity(&cwd);
        config.set_user_identity(identity.name.clone(), identity.email);
        identity.name
    } else {
        None
    };

    // Resolve compress preference
    let resolved_compress = compress_flag.unwrap_or_else(|| config.is_compress_enabled());
    config.compress_enabled = Some(resolved_compress);

    // Persist passthrough mode
    config.passthrough_mode = passthrough;

    // Record init timestamp
    config.last_init_at = Some(chrono::Utc::now().to_rfc3339());

    // Persist config
    config
        .save()
        .map_err(|e| (format!("Failed to save config: {e}"), 1))?;

    let init_config = init::InitConfig {
        global,
        debug,
        compress: Some(resolved_compress),
        passthrough,
        user_name,
    };

    init::run(agent, &init_config).map_err(|e| (e, 1))
}
