//! Query parsing helpers for statistics filtering.

/// Parse a human-readable time range string to an RFC 3339 timestamp.
///
/// Supported formats:
/// - `"today"` → start of today (00:00:00 local)
/// - `"yesterday"` → start of yesterday
/// - `"{N}d"` (e.g., `"7d"`) → N days ago
/// - `"YYYY-MM-DD"` → start of that day
/// - `"now"` → current time
///
/// # Panics
///
/// Does not panic; returns `None` for unparsable input.
#[must_use]
pub fn parse_time_range(input: &str) -> Option<String> {
    use chrono::{Datelike, Local, TimeZone};

    let now = Local::now();

    match input {
        "today" => {
            let start = Local
                .with_ymd_and_hms(now.year(), now.month(), now.day(), 0, 0, 0)
                .single()?;
            Some(start.to_rfc3339())
        }
        "yesterday" => {
            let yesterday = now - chrono::Duration::days(1);
            let start = Local
                .with_ymd_and_hms(
                    yesterday.year(),
                    yesterday.month(),
                    yesterday.day(),
                    0,
                    0,
                    0,
                )
                .single()?;
            Some(start.to_rfc3339())
        }
        "now" => Some(now.to_rfc3339()),
        s if s.ends_with('d') => {
            let days: i64 = s.strip_suffix('d')?.parse().ok()?;
            let target = now - chrono::Duration::days(days);
            let start = Local
                .with_ymd_and_hms(target.year(), target.month(), target.day(), 0, 0, 0)
                .single()?;
            Some(start.to_rfc3339())
        }
        // YYYY-MM-DD
        s if s.len() == 10 && s.chars().nth(4) == Some('-') => {
            let parts: Vec<&str> = s.split('-').collect();
            if parts.len() != 3 {
                return None;
            }
            let y: i32 = parts.first()?.parse().ok()?;
            let m: u32 = parts.get(1)?.parse().ok()?;
            let d: u32 = parts.get(2)?.parse().ok()?;
            let start = Local.with_ymd_and_hms(y, m, d, 0, 0, 0).single()?;
            Some(start.to_rfc3339())
        }
        _ => None,
    }
}
