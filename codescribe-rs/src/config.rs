//! Configuration module for CodeScribe Rust app.
//!
//! Manages persistent settings shared between the Rust frontend and Python backend.
//! Settings are stored in `$HOME/.CodeScribe/settings.json` by default, ensuring
//! both components read/write the exact same configuration file.

use anyhow::{Context, Result};
use directories::BaseDirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// CodeScribe configuration structure.
///
/// This struct mirrors the Python `VistaSettings` dataclass but includes
/// additional Rust-specific fields for hotkey handling and audio processing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    // Shared settings with Python backend
    /// Language preference: "auto", "pl", or "en"
    #[serde(default = "default_language")]
    pub language: String,

    /// Whether AI formatting is enabled for transcriptions
    #[serde(default)]
    pub ai_formatting_enabled: bool,

    /// AI provider: "harmony" or "ollama"
    #[serde(default = "default_ai_provider")]
    pub ai_provider: String,

    /// Maximum tokens for regular AI completions
    #[serde(default = "default_ai_max_tokens")]
    pub ai_max_tokens: i32,

    /// Maximum tokens for assistive AI completions
    #[serde(default = "default_ai_assistive_max_tokens")]
    pub ai_assistive_max_tokens: i32,

    // Rust-specific settings (not used by Python backend)
    /// Modifier keys for hold-to-talk (e.g., "ctrl", "ctrl+alt")
    #[serde(default = "default_hold_mods")]
    pub hold_mods: String,

    /// Whether the hold key is exclusive (no other keys can be pressed)
    #[serde(default)]
    pub hold_exclusive: bool,

    /// Delay in milliseconds before starting recording after holding key
    #[serde(default = "default_hold_start_delay_ms")]
    pub hold_start_delay_ms: u64,

    /// Interval in milliseconds for detecting double Option/Alt key press
    #[serde(default = "default_double_option_interval_ms")]
    pub double_option_interval_ms: u64,

    /// Silence threshold in decibels (-60 to 0)
    #[serde(default = "default_silence_db")]
    pub silence_db: f32,

    /// Silence hang time in seconds before stopping recording
    #[serde(default = "default_silence_hang_sec")]
    pub silence_hang_sec: f32,

    /// Whether to play a beep sound when recording starts
    #[serde(default = "default_beep_on_start")]
    pub beep_on_start: bool,

    /// Backend ports to try connecting to (in priority order)
    #[serde(default = "default_backend_ports")]
    pub backend_ports: Vec<u16>,
}

// Default value functions
fn default_language() -> String {
    "auto".to_string()
}

fn default_ai_provider() -> String {
    "harmony".to_string()
}

fn default_ai_max_tokens() -> i32 {
    512
}

fn default_ai_assistive_max_tokens() -> i32 {
    2048
}

fn default_hold_mods() -> String {
    "ctrl".to_string()
}

fn default_hold_start_delay_ms() -> u64 {
    800
}

fn default_double_option_interval_ms() -> u64 {
    450
}

fn default_silence_db() -> f32 {
    -45.0
}

fn default_silence_hang_sec() -> f32 {
    0.8
}

fn default_beep_on_start() -> bool {
    true
}

fn default_backend_ports() -> Vec<u16> {
    vec![8237, 7237, 6237, 5237]
}

impl Default for Config {
    fn default() -> Self {
        Self {
            language: default_language(),
            ai_formatting_enabled: false,
            ai_provider: default_ai_provider(),
            ai_max_tokens: default_ai_max_tokens(),
            ai_assistive_max_tokens: default_ai_assistive_max_tokens(),
            hold_mods: default_hold_mods(),
            hold_exclusive: false,
            hold_start_delay_ms: default_hold_start_delay_ms(),
            double_option_interval_ms: default_double_option_interval_ms(),
            silence_db: default_silence_db(),
            silence_hang_sec: default_silence_hang_sec(),
            beep_on_start: default_beep_on_start(),
            backend_ports: default_backend_ports(),
        }
    }
}

impl Config {
    /// Load configuration from disk or return defaults.
    ///
    /// Reads from `$HOME/.CodeScribe/settings.json` or the path specified
    /// in `CODESCRIBE_SETTINGS_PATH` environment variable.
    ///
    /// If the file doesn't exist or is malformed, returns default configuration
    /// without raising an error.
    pub fn load() -> Self {
        let path = Self::settings_path();

        if !path.exists() {
            return Self::default();
        }

        match fs::read_to_string(&path) {
            Ok(contents) => match serde_json::from_str::<Config>(&contents) {
                Ok(mut config) => {
                    config.sanitize();
                    config
                }
                Err(_) => Self::default(),
            },
            Err(_) => Self::default(),
        }
    }

    /// Save configuration to disk.
    ///
    /// Writes the configuration to `$HOME/.CodeScribe/settings.json`,
    /// creating the directory if it doesn't exist.
    pub fn save(&self) -> Result<()> {
        let path = Self::settings_path();

        // Ensure the parent directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create config directory: {:?}", parent))?;
        }

        let json = serde_json::to_string_pretty(self)
            .context("Failed to serialize configuration to JSON")?;

        fs::write(&path, json)
            .with_context(|| format!("Failed to write config file: {:?}", path))?;

        Ok(())
    }

    /// Get the configuration directory path (`$HOME/.CodeScribe`).
    ///
    /// Can be overridden with `CODESCRIBE_DATA_DIR` or `CODESCRIBE_APP_DIR`
    /// environment variables.
    pub fn config_dir() -> PathBuf {
        // Check for environment variable overrides
        if let Ok(custom) = std::env::var("CODESCRIBE_DATA_DIR") {
            return PathBuf::from(shellexpand::tilde(&custom).into_owned());
        }

        if let Ok(custom) = std::env::var("CODESCRIBE_APP_DIR") {
            return PathBuf::from(shellexpand::tilde(&custom).into_owned());
        }

        // Default to $HOME/.CodeScribe
        BaseDirs::new()
            .map(|dirs| dirs.home_dir().join(".CodeScribe"))
            .unwrap_or_else(|| PathBuf::from(".CodeScribe"))
    }

    /// Get the full path to the settings file.
    fn settings_path() -> PathBuf {
        // Check for custom settings path
        if let Ok(custom) = std::env::var("CODESCRIBE_SETTINGS_PATH") {
            return PathBuf::from(shellexpand::tilde(&custom).into_owned());
        }

        Self::config_dir().join("settings.json")
    }

    /// Sanitize configuration values to ensure they're valid.
    ///
    /// This mirrors the Python `_sanitize` function to maintain consistency.
    fn sanitize(&mut self) {
        // Normalize language
        self.language = self.language.to_lowercase();
        if self.language.is_empty() {
            self.language = "auto".to_string();
        }

        // Validate AI provider
        self.ai_provider = self.ai_provider.to_lowercase();
        if !matches!(self.ai_provider.as_str(), "harmony" | "ollama") {
            self.ai_provider = "harmony".to_string();
        }

        // Ensure token limits are reasonable
        if self.ai_max_tokens <= 0 {
            self.ai_max_tokens = 512;
        }
        if self.ai_assistive_max_tokens <= 0 {
            self.ai_assistive_max_tokens = 2048;
        }

        // Validate audio thresholds
        if self.silence_db > 0.0 || self.silence_db < -100.0 {
            self.silence_db = -45.0;
        }
        if self.silence_hang_sec <= 0.0 || self.silence_hang_sec > 10.0 {
            self.silence_hang_sec = 0.8;
        }

        // Ensure at least one backend port is configured
        if self.backend_ports.is_empty() {
            self.backend_ports = default_backend_ports();
        }
    }

    /// Update specific fields and save to disk.
    ///
    /// This is a convenience method for updating individual settings
    /// without loading and manually modifying the struct.
    pub fn update<F>(&mut self, updater: F) -> Result<()>
    where
        F: FnOnce(&mut Config),
    {
        updater(self);
        self.sanitize();
        self.save()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.language, "auto");
        assert_eq!(config.ai_provider, "harmony");
        assert_eq!(config.ai_max_tokens, 512);
        assert!(!config.ai_formatting_enabled);
        assert_eq!(config.backend_ports, vec![8237, 7237, 6237, 5237]);
    }

    #[test]
    fn test_sanitize_language() {
        let mut config = Config::default();
        config.language = "PL".to_string();
        config.sanitize();
        assert_eq!(config.language, "pl");

        config.language = "".to_string();
        config.sanitize();
        assert_eq!(config.language, "auto");
    }

    #[test]
    fn test_sanitize_ai_provider() {
        let mut config = Config::default();
        config.ai_provider = "HARMONY".to_string();
        config.sanitize();
        assert_eq!(config.ai_provider, "harmony");

        config.ai_provider = "invalid".to_string();
        config.sanitize();
        assert_eq!(config.ai_provider, "harmony");
    }

    #[test]
    fn test_sanitize_token_limits() {
        let mut config = Config::default();
        config.ai_max_tokens = -1;
        config.sanitize();
        assert_eq!(config.ai_max_tokens, 512);
    }

    #[test]
    fn test_config_dir() {
        let dir = Config::config_dir();
        assert!(dir.to_string_lossy().contains(".CodeScribe"));
    }
}
