//! Handler for `tokenless env-check`.

use crate::env_check;

/// Handle `tokenless env-check`.
#[allow(clippy::too_many_arguments, clippy::fn_params_excessive_bools)]
pub(crate) fn handle(
    tool: Option<&str>,
    all: bool,
    fix: bool,
    checklist: bool,
    json: bool,
) -> Result<(), (String, i32)> {
    env_check::run(tool, all, fix, checklist, json)
}
