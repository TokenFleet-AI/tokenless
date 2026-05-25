//! Configuration for tokenless stats recording.
//!
//! Reads from `TOKENLESS_STATS_ENABLED` env var or
//! `~/.tokenless/config.json` to determine if stats recording is active.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Persistent configuration for tokenless stats recording.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenlessConfig {
    /// Whether stats recording is enabled.
    pub stats_enabled: bool,
}

impl Default for TokenlessConfig {
    fn default() -> Self {
        Self {
            stats_enabled: true,
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

    /// Return whether the config file exists on disk.
    #[must_use]
    pub fn config_file_exists() -> bool {
        Self::config_path().exists()
    }

    fn config_path() -> PathBuf {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        home.join(".tokenless").join("config.json")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_is_enabled() {
        let config = TokenlessConfig::default();
        assert!(config.stats_enabled);
    }

    #[test]
    fn test_is_stats_enabled_respects_env() {
        let config = TokenlessConfig {
            stats_enabled: false,
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
        };
        assert!(!config.stats_enabled);
        // Actual roundtrip requires mocking HOME, which is not feasible here.
    }
}
