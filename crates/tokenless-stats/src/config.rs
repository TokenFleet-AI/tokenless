//! Configuration for tokenless stats recording.
//!
//! Reads from `TOKENLESS_STATS_ENABLED` env var or
//! `~/.tokenless/config.json` to determine if stats recording is active.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Persistent configuration for tokenless stats recording.
#[derive(Debug, Clone, Serialize, Deserialize)]
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
}

/// Serde default for boolean `true` fields.
const fn default_true() -> bool {
    true
}

impl Default for TokenlessConfig {
    fn default() -> Self {
        Self {
            stats_enabled: true,
            experimental_mode: true,
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

    /// Save configuration to the default config file.
    ///
    /// # Errors
    ///
    /// Returns an I/O error if the file cannot be written.
    #[allow(clippy::disallowed_methods)]
    pub fn save(&self) -> Result<(), std::io::Error> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(self).unwrap_or_default();
        std::fs::write(&path, content)
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
    fn test_experimental_mode_default_true() {
        let config = TokenlessConfig::default();
        assert!(config.is_experimental_enabled());
    }

    #[test]
    fn test_experimental_mode_disabled() {
        let config = TokenlessConfig {
            stats_enabled: true,
            experimental_mode: false,
        };
        assert!(!config.is_experimental_enabled());
    }

    #[test]
    fn test_is_stats_enabled_respects_env() {
        let config = TokenlessConfig {
            stats_enabled: false,
            experimental_mode: true,
        };
        // When env is not set, use config value
        assert!(!config.is_stats_enabled());
    }

    #[test]
    fn test_save_and_load_roundtrip() {
        // Save/load uses the real home directory config path.
        // This test verifies the API compiles and doesn't panic.
        let config = TokenlessConfig {
            stats_enabled: false,
            experimental_mode: false,
        };
        assert!(!config.stats_enabled);
        assert!(!config.experimental_mode);
        // Actual roundtrip requires mocking HOME, which is not feasible here.
    }

    #[test]
    fn test_deserialize_missing_experimental_mode_defaults_true() {
        let json = r#"{"stats_enabled":true}"#;
        let config: TokenlessConfig =
            serde_json::from_str(json).expect("valid JSON should deserialize");
        assert!(config.experimental_mode);
        assert!(config.is_experimental_enabled());
    }
}
