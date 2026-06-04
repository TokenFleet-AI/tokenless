//! Configuration for tokenless stats recording.
//!
//! Reads from `TOKENLESS_STATS_ENABLED` env var or
//! `~/.tokenless/config.json` to determine if stats recording is active.

use std::fmt;
use std::io;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Persistent configuration for tokenless stats recording.
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct TokenlessConfig {
    /// Whether stats recording is enabled.
    #[serde(default = "default_true")]
    pub stats_enabled: bool,
    /// Whether experimental mode is enabled (format router, enhanced TOON,
    /// semantic compression, diff hook, TUI, MCP, cache).
    /// Default: `true`. Set to `false` to use only core compression
    /// (schema + response + basic TOON).
    #[serde(default = "default_true")]
    pub experimental_mode: bool,

    /// Detected user name for attribution.
    /// Priority: git config user.name > OS username.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_name: Option<String>,

    /// Detected user email for attribution.
    /// Priority: git config user.email.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_email: Option<String>,

    /// Whether compress hook is enabled.
    /// `None` = not explicitly configured (defaults to `true` at runtime).
    /// `Some(true)` = enabled, `Some(false)` = disabled.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compress_enabled: Option<bool>,

    /// Timestamp of last `tokenless init` run (ISO 8601).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_init_at: Option<String>,

    /// Whether passthrough mode is enabled.
    /// When enabled, hooks pass through original content unchanged but still
    /// record compress logs (with `saved_tokens` = 0) for cost comparison.
    /// Default: `false` (normal compression behavior).
    #[serde(default)]
    pub passthrough_mode: bool,
}

/// Serde default for boolean `true` fields.
const fn default_true() -> bool {
    true
}

impl fmt::Debug for TokenlessConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let user_name_display = if self.user_name.is_some() {
            "[redacted]"
        } else {
            "None"
        };
        let user_email_display = if self.user_email.is_some() {
            "[redacted]"
        } else {
            "None"
        };
        f.debug_struct("TokenlessConfig")
            .field("stats_enabled", &self.stats_enabled)
            .field("experimental_mode", &self.experimental_mode)
            .field("user_name", &user_name_display)
            .field("user_email", &user_email_display)
            .field("compress_enabled", &self.compress_enabled)
            .field("last_init_at", &self.last_init_at)
            .field("passthrough_mode", &self.passthrough_mode)
            .finish()
    }
}

impl Default for TokenlessConfig {
    fn default() -> Self {
        Self {
            stats_enabled: true,
            experimental_mode: true,
            user_name: None,
            user_email: None,
            compress_enabled: None,
            last_init_at: None,
            passthrough_mode: false,
        }
    }
}

impl TokenlessConfig {
    /// Load configuration from the default config file, or return defaults.
    #[must_use]
    #[allow(clippy::disallowed_methods)]
    pub fn load() -> Self {
        let path = Self::config_path();
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    /// Save configuration to the default config file using atomic write
    /// (write to temporary file, then rename).
    ///
    /// # Errors
    ///
    /// Returns an I/O error if serialization fails or the file cannot be
    /// written.
    #[allow(clippy::disallowed_methods)]
    pub fn save(&self) -> Result<(), io::Error> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(self)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;
        // Atomic write: write to tmp file, then rename
        let tmp_path = path.with_extension("json.tmp");
        std::fs::write(&tmp_path, &content)?;
        std::fs::rename(&tmp_path, &path)?;
        Ok(())
    }

    /// Check if stats recording is enabled, respecting env override.
    #[must_use]
    pub fn is_stats_enabled(&self) -> bool {
        if let Ok(val) = std::env::var("TOKENLESS_STATS_ENABLED") {
            return val == "1" || val.eq_ignore_ascii_case("true");
        }
        self.stats_enabled
    }

    /// Check if experimental mode is enabled, respecting env override.
    ///
    /// Set `TOKENLESS_EXPERIMENTAL=0` or `TOKENLESS_EXPERIMENTAL=false` to
    /// disable all experimental features (format router, enhanced TOON,
    /// semantic compression, diff hook, TUI, MCP, cache).
    #[must_use]
    pub fn is_experimental_enabled(&self) -> bool {
        if let Ok(val) = std::env::var("TOKENLESS_EXPERIMENTAL") {
            return val == "1" || val.eq_ignore_ascii_case("true");
        }
        self.experimental_mode
    }

    /// Check if compress hook should be installed.
    ///
    /// Returns `true` when `compress_enabled` is `None` (default) or
    /// `Some(true)`.
    #[must_use]
    pub fn is_compress_enabled(&self) -> bool {
        self.compress_enabled.unwrap_or(true)
    }

    /// Get the effective user name, falling back to `"unknown"`.
    #[must_use]
    pub fn effective_user_name(&self) -> &str {
        self.user_name.as_deref().unwrap_or("unknown")
    }

    /// Update user identity fields in-place (for init flow).
    pub fn set_user_identity(&mut self, name: Option<String>, email: Option<String>) {
        self.user_name = name;
        self.user_email = email;
    }

    /// Check if passthrough mode is enabled, respecting env override.
    ///
    /// Set `TOKENLESS_PASSTHROUGH=1` or `TOKENLESS_PASSTHROUGH=true` to
    /// temporarily enable passthrough mode without re-running `init`.
    #[must_use]
    pub fn is_passthrough_enabled(&self) -> bool {
        if let Ok(val) = std::env::var("TOKENLESS_PASSTHROUGH") {
            return val == "1" || val.eq_ignore_ascii_case("true");
        }
        self.passthrough_mode
    }

    /// Return whether the config file exists on disk.
    #[must_use]
    pub fn config_file_exists() -> bool {
        Self::config_path().exists()
    }

    fn config_path() -> PathBuf {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        home.join(".tokenfleet-ai")
            .join("tokenless")
            .join("config.json")
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_default_is_enabled() {
        let config = TokenlessConfig::default();
        assert!(config.stats_enabled);
        assert!(config.experimental_mode);
    }

    #[test]
    fn test_passthrough_mode_default_false() {
        let config = TokenlessConfig::default();
        assert!(!config.passthrough_mode);
        assert!(!config.is_passthrough_enabled());
    }

    #[test]
    fn test_passthrough_mode_enabled() {
        let config = TokenlessConfig {
            passthrough_mode: true,
            ..Default::default()
        };
        assert!(config.is_passthrough_enabled());
    }

    #[test]
    fn test_deserialize_old_config_no_new_fields() {
        let json = r#"{"statsEnabled":true,"experimentalMode":false}"#;
        let config: TokenlessConfig = serde_json::from_str(json).expect("valid JSON");
        assert!(config.stats_enabled);
        assert!(!config.experimental_mode);
        assert!(config.user_name.is_none());
        assert!(config.user_email.is_none());
        assert!(config.compress_enabled.is_none());
        assert!(config.last_init_at.is_none());
        assert!(!config.passthrough_mode); // default false
    }

    #[test]
    fn test_serialize_skips_none_fields() {
        let config = TokenlessConfig::default();
        let json = serde_json::to_string(&config).expect("serialize");
        assert!(json.contains("statsEnabled"));
        assert!(json.contains("experimentalMode"));
        // Optional fields should be absent when None
        assert!(!json.contains("userName"));
        assert!(!json.contains("userEmail"));
        assert!(!json.contains("compressEnabled"));
        assert!(!json.contains("lastInitAt"));
    }

    #[test]
    fn test_compress_enabled_defaults_true() {
        let config = TokenlessConfig::default();
        assert!(config.is_compress_enabled());
    }

    #[test]
    fn test_compress_enabled_explicit_false() {
        let config = TokenlessConfig {
            compress_enabled: Some(false),
            ..Default::default()
        };
        assert!(!config.is_compress_enabled());
    }

    #[test]
    fn test_effective_user_name_fallback() {
        let config = TokenlessConfig::default();
        assert_eq!(config.effective_user_name(), "unknown");
    }

    #[test]
    fn test_effective_user_name_returns_value() {
        let config = TokenlessConfig {
            user_name: Some("Alice".to_string()),
            ..Default::default()
        };
        assert_eq!(config.effective_user_name(), "Alice");
    }

    #[test]
    fn test_debug_redacts_user_info() {
        let config = TokenlessConfig {
            user_name: Some("Alice".to_string()),
            user_email: Some("alice@example.com".to_string()),
            ..Default::default()
        };
        let debug_str = format!("{config:?}");
        assert!(!debug_str.contains("Alice"), "user_name must be redacted");
        assert!(
            !debug_str.contains("alice@example.com"),
            "user_email must be redacted"
        );
        assert!(
            debug_str.contains("[redacted]"),
            "should show redacted marker"
        );
    }

    #[test]
    fn test_set_user_identity() {
        let mut config = TokenlessConfig::default();
        config.set_user_identity(Some("Bob".to_string()), Some("bob@test.com".to_string()));
        assert_eq!(config.user_name.as_deref(), Some("Bob"));
        assert_eq!(config.user_email.as_deref(), Some("bob@test.com"));
    }

    #[test]
    fn test_deserialize_missing_experimental_mode_defaults_true() {
        let json = r#"{"statsEnabled":true}"#;
        let config: TokenlessConfig = serde_json::from_str(json).expect("valid JSON");
        assert!(config.experimental_mode);
        assert!(config.is_experimental_enabled());
    }
}
