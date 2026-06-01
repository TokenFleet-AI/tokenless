//! Handler for `tokenless init`.

use crate::init;

/// Handle `tokenless init`.
pub(crate) fn handle(global: bool, agent: String) -> Result<(), (String, i32)> {
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
        _ => init::Agent::Claude,
    };
    let config = init::InitConfig { global };
    init::run(agent, &config).map_err(|e| (e, 1))
}
