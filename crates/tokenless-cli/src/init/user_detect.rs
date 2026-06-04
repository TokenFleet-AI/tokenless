//! User identity detection for `tokenless init`.
//!
//! Auto-detects user name and email from git config, falling back
//! to OS username when git is unavailable.

use std::path::Path;

/// Maximum byte length for sanitized identity values.
const MAX_IDENTITY_BYTES: usize = 256;

/// Bidirectional control characters to reject.
const BIDI_CONTROLS: &[char] = &[
    '\u{202A}', '\u{202B}', '\u{202C}', '\u{202D}', '\u{202E}', '\u{2066}', '\u{2067}', '\u{2068}',
    '\u{2069}',
];

/// Zero-width and invisible characters to strip.
const ZERO_WIDTH_CHARS: &[char] = &['\u{200B}', '\u{200C}', '\u{200D}', '\u{FEFF}', '\u{00AD}'];

/// Source of detected user identity information.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum IdentitySource {
    /// From `git config user.name` or `git config user.email`.
    GitConfig,
    /// From OS environment ($USER, $LOGNAME, whoami).
    OsUser,
    /// Not detected — fallback.
    #[default]
    Unknown,
}

/// Detected user identity for attribution.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UserIdentity {
    /// User's display name.
    pub name: Option<String>,
    /// User's email address.
    pub email: Option<String>,
    /// Source of the name field.
    pub name_source: IdentitySource,
    /// Source of the email field.
    pub email_source: IdentitySource,
}

/// Sanitize a detected identity string: trim, reject control/bidi chars,
/// strip zero-width chars, truncate at UTF-8 boundary.
#[must_use]
pub fn sanitize_identity(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    // Reject C0 control characters (all, including tab)
    if trimmed.chars().any(|c| c.is_control()) {
        return None;
    }
    // Reject bidirectional control characters
    if trimmed.chars().any(|c| BIDI_CONTROLS.contains(&c)) {
        return None;
    }
    // Strip zero-width/invisible characters
    let cleaned: String = trimmed
        .chars()
        .filter(|c| !ZERO_WIDTH_CHARS.contains(c))
        .collect();
    if cleaned.is_empty() {
        return None;
    }
    // Truncate at valid UTF-8 boundary
    let bytes = cleaned.as_bytes();
    if bytes.len() > MAX_IDENTITY_BYTES {
        let mut end = MAX_IDENTITY_BYTES;
        while end > 0 && (bytes[end] & 0xC0) == 0x80 {
            end -= 1;
        }
        Some(String::from_utf8_lossy(&bytes[..end]).to_string())
    } else {
        Some(cleaned)
    }
}

/// Run `git config <key>` and return trimmed stdout, or `None` on failure.
/// Supports `TEST_GIT_CONFIG_<KEY_UNDERSCORED>` env var override for testing.
fn git_config(key: &str, global: bool) -> Option<String> {
    git_config_with_env(key, global, |k| std::env::var(k).ok())
}

/// Core `git config` lookup with an injectable environment reader for testing.
fn git_config_with_env<F>(key: &str, global: bool, env_lookup: F) -> Option<String>
where
    F: Fn(&str) -> Option<String>,
{
    // Test override via env var
    let env_key = format!("TEST_GIT_CONFIG_{}", key.to_uppercase().replace('.', "_"));
    if let Some(val) = env_lookup(&env_key) {
        return if val.is_empty() { None } else { Some(val) };
    }

    let mut cmd = std::process::Command::new("git");
    cmd.arg("config");
    if global {
        cmd.arg("--global");
    }
    cmd.arg(key);

    cmd.output().ok().and_then(|output| {
        if output.status.success() {
            let value = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if value.is_empty() { None } else { Some(value) }
        } else {
            None
        }
    })
}

/// Get the OS username from environment or `whoami`.
fn os_username() -> Option<String> {
    std::env::var("USER")
        .ok()
        .filter(|s| !s.is_empty())
        .or_else(|| std::env::var("LOGNAME").ok().filter(|s| !s.is_empty()))
        .or_else(|| {
            std::process::Command::new("whoami")
                .output()
                .ok()
                .and_then(|o| {
                    if o.status.success() {
                        let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
                        if s.is_empty() { None } else { Some(s) }
                    } else {
                        None
                    }
                })
        })
}

/// Detect user identity from git config and OS environment.
///
/// Priority:
/// 1. `git config --global user.name` -> `git config user.name` (repo scope)
/// 2. `git config --global user.email` -> `git config user.email` (repo scope)
/// 3. OS username ($USER -> $LOGNAME -> whoami) — for name only
///
/// Detection failure never panics; fields remain `None` and source is `Unknown`.
#[must_use]
pub fn detect_user_identity(cwd: &Path) -> UserIdentity {
    let _ = cwd; // reserved for future use (e.g. repo-scoped git config)
    let mut identity = UserIdentity::default();

    // Detect name
    if let Some(name) = git_config("user.name", true)
        .or_else(|| git_config("user.name", false))
        .and_then(|s| sanitize_identity(&s))
    {
        identity.name = Some(name);
        identity.name_source = IdentitySource::GitConfig;
    } else if let Some(name) = os_username().and_then(|s| sanitize_identity(&s)) {
        identity.name = Some(name);
        identity.name_source = IdentitySource::OsUser;
    }

    // Detect email
    if let Some(email) = git_config("user.email", true)
        .or_else(|| git_config("user.email", false))
        .and_then(|s| sanitize_identity(&s))
    {
        identity.email = Some(email);
        identity.email_source = IdentitySource::GitConfig;
    }

    identity
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// Build a mock env-lookup closure from a `HashMap`.
    fn mock_env<'a>(map: &'a HashMap<&'a str, &'a str>) -> impl Fn(&str) -> Option<String> + 'a {
        |k: &str| map.get(k).map(|v| (*v).to_string())
    }

    // ── sanitize_identity ──

    #[test]
    fn test_sanitize_valid_name() {
        assert_eq!(sanitize_identity("John Doe"), Some("John Doe".to_string()));
    }

    #[test]
    fn test_sanitize_empty_string() {
        assert_eq!(sanitize_identity("   "), None);
    }

    #[test]
    fn test_sanitize_empty_input() {
        assert_eq!(sanitize_identity(""), None);
    }

    #[test]
    fn test_sanitize_null_byte() {
        assert_eq!(sanitize_identity("user\x00name"), None);
    }

    #[test]
    fn test_sanitize_escape_char() {
        assert_eq!(sanitize_identity("name\x1b"), None);
    }

    #[test]
    fn test_sanitize_rejects_bidi_override() {
        assert_eq!(sanitize_identity("user\u{202E}malicious"), None);
    }

    #[test]
    fn test_sanitize_strips_zero_width_space() {
        let result = sanitize_identity("user\u{200B}name");
        assert_eq!(result.as_deref(), Some("username"));
    }

    #[test]
    fn test_sanitize_truncates_long_string() {
        let long = "a".repeat(300);
        let result = sanitize_identity(&long);
        assert!(result.is_some());
        assert!(result.unwrap().len() <= 256);
    }

    #[test]
    fn test_sanitize_only_invisible_chars_returns_none() {
        assert_eq!(sanitize_identity("\u{200B}\u{200C}\u{200D}"), None);
    }

    #[test]
    fn test_sanitize_tab_is_filtered() {
        // Tab is a control char, should be rejected
        assert_eq!(sanitize_identity("name\twith\ttabs"), None);
    }

    // ── UserIdentity ──

    #[test]
    fn test_user_identity_default_all_none() {
        let id = UserIdentity::default();
        assert!(id.name.is_none());
        assert!(id.email.is_none());
        assert_eq!(id.name_source, IdentitySource::Unknown);
        assert_eq!(id.email_source, IdentitySource::Unknown);
    }

    // ── git_config with env override ──

    #[test]
    fn test_git_config_test_env_override() {
        let mut env = HashMap::new();
        env.insert("TEST_GIT_CONFIG_USER_NAME", "TestUser");
        let lookup = mock_env(&env);
        let result = git_config_with_env("user.name", true, lookup);
        assert_eq!(result, Some("TestUser".to_string()));
    }

    #[test]
    fn test_git_config_test_env_override_empty() {
        let mut env = HashMap::new();
        env.insert("TEST_GIT_CONFIG_USER_NAME", "");
        let lookup = mock_env(&env);
        let result = git_config_with_env("user.name", true, lookup);
        assert_eq!(result, None);
    }

    #[test]
    fn test_git_config_no_env_falls_through() {
        let env: HashMap<&str, &str> = HashMap::new();
        let lookup = mock_env(&env);
        // When no TEST_GIT_CONFIG_* env is set, git_config_with_env runs `git config`.
        // On systems without git user config, this returns None.
        let result = git_config_with_env("user.name", true, lookup);
        // We can't assert on the exact value (it depends on the machine),
        // but the function should not panic.
        let _ = result;
    }
}
