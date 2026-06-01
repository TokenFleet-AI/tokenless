//! Handler for `tokenless tui`.

use crate::shared::open_recorder;

/// Handle `tokenless tui`.
pub(crate) fn handle(refresh: u64, lang: String) -> Result<(), (String, i32)> {
    let lang = match lang.as_str() {
        "en" => tokenless_tui::Lang::En,
        _ => tokenless_tui::Lang::Zh,
    };
    let recorder = open_recorder()?;
    tokenless_tui::run_tui(recorder, refresh, lang).map_err(|e| (e, 1))
}
