//! Handler for `tokenless env-check`.

use crate::env_check;

/// Handle `tokenless env-check`.
pub(crate) fn handle(
    tool: Option<String>,
    all: bool,
    fix: bool,
    checklist: bool,
    json: bool,
) -> Result<(), (String, i32)> {
    env_check::run(tool.as_deref(), all, fix, checklist, json)
}
