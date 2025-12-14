//! Configuration module for CodeScribe Rust app.
//!
//! Manages persistent settings with dual-layer storage:
//! 1. .env file for all configuration (primary source)
//! 2. settings.json for backwards compatibility
//!
//! Settings are stored in `$HOME/.codescribe/` directory by default.
//! .env file takes precedence over settings.json when both exist.
//!
//! Note: Global config API not yet wired up to main.rs (pending integration)
#![allow(dead_code)]

use directories::BaseDirs;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::{OnceLock, RwLock};

/// Thread-safe global configuration instance
static GLOBAL_CONFIG: OnceLock<RwLock<Config>> = OnceLock::new();

/// Modifier key combinations for hold-to-talk
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HoldMods {
    #[default]
    Ctrl,
    CtrlAlt,
    CtrlShift,
    CtrlCmd,
}

impl HoldMods {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Ctrl => "ctrl",
            Self::CtrlAlt => "ctrl_alt",
            Self::CtrlShift => "ctrl_shift",
            Self::CtrlCmd => "ctrl_cmd",
        }
    }

    /// Human-readable label for menu display
    pub fn label(&self) -> &'static str {
        match self {
            Self::Ctrl => "Ctrl only (Formatting)",
            Self::CtrlAlt => "Ctrl+Option",
            Self::CtrlShift => "Ctrl+Shift (AI)",
            Self::CtrlCmd => "Ctrl+Command",
        }
    }
}

impl FromStr for HoldMods {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "ctrl" => Ok(Self::Ctrl),
            "ctrl_alt" | "ctrl+alt" => Ok(Self::CtrlAlt),
            "ctrl_shift" | "ctrl+shift" => Ok(Self::CtrlShift),
            "ctrl_cmd" | "ctrl+cmd" => Ok(Self::CtrlCmd),
            _ => Err(format!("Unknown HoldMods: {}", s)),
        }
    }
}

/// Toggle trigger options
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ToggleTrigger {
    #[default]
    DoubleOption,
    DoubleRightOption,
    None,
}

impl ToggleTrigger {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::DoubleOption => "double_option",
            Self::DoubleRightOption => "double_ralt",
            Self::None => "none",
        }
    }

    /// Human-readable label for menu display
    pub fn label(&self) -> &'static str {
        match self {
            Self::DoubleOption => "double option",
            Self::DoubleRightOption => "double right option",
            Self::None => "disabled",
        }
    }
}

impl FromStr for ToggleTrigger {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "double_option" => Ok(Self::DoubleOption),
            "double_ralt" | "double_right_option" => Ok(Self::DoubleRightOption),
            "none" | "disabled" => Ok(Self::None),
            _ => Err(format!("Unknown ToggleTrigger: {}", s)),
        }
    }
}

/// Language options for Whisper transcription
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    #[default]
    Auto,
    Polish,
    English,
}

impl Language {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Polish => "pl",
            Self::English => "en",
        }
    }
}

impl FromStr for Language {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "auto" => Ok(Self::Auto),
            "pl" | "polish" => Ok(Self::Polish),
            "en" | "english" => Ok(Self::English),
            _ => Err(format!("Unknown Language: {}", s)),
        }
    }
}

/// AI provider options
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AiProvider {
    #[default]
    Harmony,
    Ollama,
}

impl AiProvider {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Harmony => "harmony",
            Self::Ollama => "ollama",
        }
    }
}

impl FromStr for AiProvider {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "harmony" => Ok(Self::Harmony),
            "ollama" => Ok(Self::Ollama),
            _ => Err(format!("Unknown AiProvider: {}", s)),
        }
    }
}

/// CodeScribe configuration structure.
///
/// This struct contains all configuration options for the app.
/// Values are loaded from .env file (primary) or settings.json (fallback).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    // ===== Hotkeys =====
    /// Modifier keys for hold-to-talk
    #[serde(default)]
    pub hold_mods: HoldMods,

    /// Whether to ignore extra modifiers when hold key is pressed
    #[serde(default)]
    pub hold_exclusive: bool,

    /// Toggle trigger method (double Option, double RAlt, or none)
    #[serde(default)]
    pub toggle_trigger: ToggleTrigger,

    /// Delay in milliseconds before starting recording after holding key
    #[serde(default = "default_hold_start_delay_ms")]
    pub hold_start_delay_ms: u64,

    // ===== Language =====
    /// Whisper language preference
    #[serde(default)]
    pub whisper_language: Language,

    // ===== AI Formatting =====
    /// Whether AI formatting is enabled for transcriptions
    #[serde(default)]
    pub ai_formatting_enabled: bool,

    /// AI provider for formatting
    #[serde(default)]
    pub ai_provider: AiProvider,

    /// Maximum tokens for regular AI completions
    #[serde(default = "default_ai_max_tokens")]
    pub ai_max_tokens: i32,

    /// Maximum tokens for assistive AI completions
    #[serde(default = "default_ai_assistive_max_tokens")]
    pub ai_assistive_max_tokens: i32,

    // ===== UI =====
    /// Whether to show tray icon glyph
    #[serde(default = "default_show_tray_glyph")]
    pub show_tray_glyph: bool,

    /// Whether to show hold indicator badge
    #[serde(default = "default_hold_indicator")]
    pub hold_indicator: bool,

    /// Size of hold indicator badge in pixels
    #[serde(default = "default_hold_badge_size")]
    pub hold_badge_size: u32,

    /// X offset of hold indicator badge
    #[serde(default = "default_hold_badge_offset_x")]
    pub hold_badge_offset_x: i32,

    /// Y offset of hold indicator badge
    #[serde(default = "default_hold_badge_offset_y")]
    pub hold_badge_offset_y: i32,

    // ===== Sound =====
    /// Whether to play a beep sound when recording starts
    #[serde(default = "default_beep_on_start")]
    pub beep_on_start: bool,

    /// System sound name to play (e.g., "Tink", "Pop")
    #[serde(default = "default_sound_name")]
    pub sound_name: String,

    /// Sound volume (0.0 to 1.0)
    #[serde(default = "default_sound_volume")]
    pub sound_volume: f32,

    // ===== History =====
    /// Whether to keep transcription history
    #[serde(default = "default_history_enabled")]
    pub history_enabled: bool,

    // ===== Backends =====
    /// Whisper server URL
    #[serde(default = "default_whisper_server_url")]
    pub whisper_server_url: String,

    /// LLM server URL
    #[serde(default = "default_llm_server_url")]
    pub llm_server_url: String,

    /// Ollama host URL
    #[serde(default = "default_ollama_host")]
    pub ollama_host: String,

    /// Ollama model name
    #[serde(default = "default_ollama_model")]
    pub ollama_model: String,

    // ===== Clipboard =====
    /// Whether to restore previous clipboard after paste
    #[serde(default = "default_restore_clipboard")]
    pub restore_clipboard: bool,

    /// Delay in milliseconds before restoring clipboard
    #[serde(default = "default_restore_clipboard_delay_ms")]
    pub restore_clipboard_delay_ms: u64,

    // ===== System =====
    /// Whether to start app at login
    #[serde(default)]
    pub start_at_login: bool,

    // ===== Legacy =====
    /// Backend ports to try connecting to (legacy, for backwards compatibility)
    #[serde(default = "default_backend_ports")]
    pub backend_ports: Vec<u16>,

    /// Silence threshold in decibels (legacy)
    #[serde(default = "default_silence_db")]
    pub silence_db: f32,

    /// Silence hang time in seconds (legacy)
    #[serde(default = "default_silence_hang_sec")]
    pub silence_hang_sec: f32,
}

// ===== Default value functions =====

fn default_hold_start_delay_ms() -> u64 {
    800
}

fn default_ai_max_tokens() -> i32 {
    512
}

fn default_ai_assistive_max_tokens() -> i32 {
    2048
}

fn default_show_tray_glyph() -> bool {
    true
}

fn default_hold_indicator() -> bool {
    true
}

fn default_hold_badge_size() -> u32 {
    12
}

fn default_hold_badge_offset_x() -> i32 {
    10
}

fn default_hold_badge_offset_y() -> i32 {
    -10
}

fn default_beep_on_start() -> bool {
    true
}

fn default_sound_name() -> String {
    "Tink".to_string()
}

fn default_sound_volume() -> f32 {
    1.0
}

fn default_history_enabled() -> bool {
    true
}

fn default_whisper_server_url() -> String {
    "http://localhost:8237".to_string()
}

fn default_llm_server_url() -> String {
    "http://localhost:8237".to_string()
}

fn default_ollama_host() -> String {
    "http://localhost:11434".to_string()
}

fn default_ollama_model() -> String {
    "llama3.2".to_string()
}

fn default_restore_clipboard() -> bool {
    true
}

fn default_restore_clipboard_delay_ms() -> u64 {
    1000
}

fn default_backend_ports() -> Vec<u16> {
    vec![8237, 7237, 6237, 5237]
}

fn default_silence_db() -> f32 {
    -45.0
}

fn default_silence_hang_sec() -> f32 {
    0.8
}

impl Default for Config {
    fn default() -> Self {
        Self {
            hold_mods: HoldMods::default(),
            hold_exclusive: false,
            toggle_trigger: ToggleTrigger::default(),
            hold_start_delay_ms: default_hold_start_delay_ms(),
            whisper_language: Language::default(),
            ai_formatting_enabled: false,
            ai_provider: AiProvider::default(),
            ai_max_tokens: default_ai_max_tokens(),
            ai_assistive_max_tokens: default_ai_assistive_max_tokens(),
            show_tray_glyph: default_show_tray_glyph(),
            hold_indicator: default_hold_indicator(),
            hold_badge_size: default_hold_badge_size(),
            hold_badge_offset_x: default_hold_badge_offset_x(),
            hold_badge_offset_y: default_hold_badge_offset_y(),
            beep_on_start: default_beep_on_start(),
            sound_name: default_sound_name(),
            sound_volume: default_sound_volume(),
            history_enabled: default_history_enabled(),
            whisper_server_url: default_whisper_server_url(),
            llm_server_url: default_llm_server_url(),
            ollama_host: default_ollama_host(),
            ollama_model: default_ollama_model(),
            restore_clipboard: default_restore_clipboard(),
            restore_clipboard_delay_ms: default_restore_clipboard_delay_ms(),
            start_at_login: false,
            backend_ports: default_backend_ports(),
            silence_db: default_silence_db(),
            silence_hang_sec: default_silence_hang_sec(),
        }
    }
}

impl Config {
    /// Load configuration from disk or environment.
    ///
    /// Priority order:
    /// 1. Environment variables
    /// 2. .env file in config directory
    /// 3. settings.json (legacy)
    /// 4. Default values
    ///
    /// If the files don't exist or are malformed, returns default configuration
    /// without raising an error.
    pub fn load() -> Self {
        // Load .env file if it exists
        let env_path = Self::env_path();
        if env_path.exists() {
            let _ = dotenvy::from_path(&env_path);
        }

        let mut config = Self::default();

        // Try loading from settings.json (legacy)
        let json_path = Self::settings_path();
        if json_path.exists() {
            if let Ok(contents) = fs::read_to_string(&json_path) {
                if let Ok(json_config) = serde_json::from_str::<Config>(&contents) {
                    config = json_config;
                }
            }
        }

        // Override with environment variables
        config.load_from_env();
        config.sanitize();
        config
    }

    /// Load configuration values from environment variables.
    fn load_from_env(&mut self) {
        // Hotkeys
        if let Ok(val) = std::env::var("HOLD_MODS") {
            if let Ok(mods) = val.parse::<HoldMods>() {
                self.hold_mods = mods;
            }
        }
        if let Ok(val) = std::env::var("HOLD_EXCLUSIVE") {
            self.hold_exclusive = val.parse().unwrap_or(false);
        }
        if let Ok(val) = std::env::var("TOGGLE_TRIGGER") {
            if let Ok(trigger) = val.parse::<ToggleTrigger>() {
                self.toggle_trigger = trigger;
            }
        }
        if let Ok(val) = std::env::var("HOLD_START_DELAY_MS") {
            if let Ok(ms) = val.parse() {
                self.hold_start_delay_ms = ms;
            }
        }

        // Language
        if let Ok(val) = std::env::var("WHISPER_LANGUAGE") {
            if let Ok(lang) = val.parse::<Language>() {
                self.whisper_language = lang;
            }
        }

        // AI Formatting
        if let Ok(val) = std::env::var("AI_FORMATTING_ENABLED") {
            self.ai_formatting_enabled = val.parse().unwrap_or(false);
        }
        if let Ok(val) = std::env::var("AI_PROVIDER") {
            if let Ok(provider) = val.parse::<AiProvider>() {
                self.ai_provider = provider;
            }
        }
        if let Ok(val) = std::env::var("AI_MAX_TOKENS") {
            if let Ok(tokens) = val.parse() {
                self.ai_max_tokens = tokens;
            }
        }
        if let Ok(val) = std::env::var("AI_ASSISTIVE_MAX_TOKENS") {
            if let Ok(tokens) = val.parse() {
                self.ai_assistive_max_tokens = tokens;
            }
        }

        // UI
        if let Ok(val) = std::env::var("SHOW_TRAY_GLYPH") {
            self.show_tray_glyph = val.parse().unwrap_or(true);
        }
        if let Ok(val) = std::env::var("HOLD_INDICATOR") {
            self.hold_indicator = val.parse().unwrap_or(true);
        }
        if let Ok(val) = std::env::var("HOLD_BADGE_SIZE") {
            if let Ok(size) = val.parse() {
                self.hold_badge_size = size;
            }
        }
        if let Ok(val) = std::env::var("HOLD_BADGE_OFFSET_X") {
            if let Ok(offset) = val.parse() {
                self.hold_badge_offset_x = offset;
            }
        }
        if let Ok(val) = std::env::var("HOLD_BADGE_OFFSET_Y") {
            if let Ok(offset) = val.parse() {
                self.hold_badge_offset_y = offset;
            }
        }

        // Sound
        if let Ok(val) = std::env::var("BEEP_ON_START") {
            self.beep_on_start = val.parse().unwrap_or(true);
        }
        if let Ok(val) = std::env::var("SOUND_NAME") {
            self.sound_name = val;
        }
        if let Ok(val) = std::env::var("SOUND_VOLUME") {
            if let Ok(volume) = val.parse() {
                self.sound_volume = volume;
            }
        }

        // History
        if let Ok(val) = std::env::var("HISTORY_ENABLED") {
            self.history_enabled = val.parse().unwrap_or(true);
        }

        // Backends
        if let Ok(val) = std::env::var("WHISPER_SERVER_URL") {
            self.whisper_server_url = val;
        }
        if let Ok(val) = std::env::var("LLM_SERVER_URL") {
            self.llm_server_url = val;
        }
        if let Ok(val) = std::env::var("OLLAMA_HOST") {
            self.ollama_host = val;
        }
        if let Ok(val) = std::env::var("OLLAMA_MODEL") {
            self.ollama_model = val;
        }

        // Clipboard
        if let Ok(val) = std::env::var("RESTORE_CLIPBOARD") {
            self.restore_clipboard = val.parse().unwrap_or(true);
        }
        if let Ok(val) = std::env::var("RESTORE_CLIPBOARD_DELAY_MS") {
            if let Ok(delay) = val.parse() {
                self.restore_clipboard_delay_ms = delay;
            }
        }

        // System
        if let Ok(val) = std::env::var("START_AT_LOGIN") {
            self.start_at_login = val.parse().unwrap_or(false);
        }
    }

    /// Save a single configuration value to .env file.
    ///
    /// This updates the .env file by:
    /// 1. Reading existing content
    /// 2. Updating/adding the specified key
    /// 3. Writing back to file
    ///
    /// # Arguments
    /// * `key` - Environment variable name (e.g., "BEEP_ON_START")
    /// * `value` - Value to save
    pub fn save_to_env(&self, key: &str, value: &str) -> anyhow::Result<()> {
        let env_path = Self::env_path();

        // Ensure config directory exists
        if let Some(parent) = env_path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Read existing .env content
        let mut env_vars = if env_path.exists() {
            Self::parse_env_file(&env_path)?
        } else {
            HashMap::new()
        };

        // Update the specific key
        env_vars.insert(key.to_string(), value.to_string());

        // Write back to file
        Self::write_env_file(&env_path, &env_vars)?;

        Ok(())
    }

    /// Save all configuration values to .env file.
    ///
    /// This overwrites the entire .env file with current configuration state.
    pub fn save_all_to_env(&self) -> anyhow::Result<()> {
        let env_path = Self::env_path();

        // Ensure config directory exists
        if let Some(parent) = env_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let mut env_vars = HashMap::new();

        // Hotkeys
        env_vars.insert("HOLD_MODS".to_string(), self.hold_mods.as_str().to_string());
        env_vars.insert(
            "HOLD_EXCLUSIVE".to_string(),
            self.hold_exclusive.to_string(),
        );
        env_vars.insert(
            "TOGGLE_TRIGGER".to_string(),
            self.toggle_trigger.as_str().to_string(),
        );
        env_vars.insert(
            "HOLD_START_DELAY_MS".to_string(),
            self.hold_start_delay_ms.to_string(),
        );

        // Language
        env_vars.insert(
            "WHISPER_LANGUAGE".to_string(),
            self.whisper_language.as_str().to_string(),
        );

        // AI Formatting
        env_vars.insert(
            "AI_FORMATTING_ENABLED".to_string(),
            self.ai_formatting_enabled.to_string(),
        );
        env_vars.insert(
            "AI_PROVIDER".to_string(),
            self.ai_provider.as_str().to_string(),
        );
        env_vars.insert("AI_MAX_TOKENS".to_string(), self.ai_max_tokens.to_string());
        env_vars.insert(
            "AI_ASSISTIVE_MAX_TOKENS".to_string(),
            self.ai_assistive_max_tokens.to_string(),
        );

        // UI
        env_vars.insert(
            "SHOW_TRAY_GLYPH".to_string(),
            self.show_tray_glyph.to_string(),
        );
        env_vars.insert(
            "HOLD_INDICATOR".to_string(),
            self.hold_indicator.to_string(),
        );
        env_vars.insert(
            "HOLD_BADGE_SIZE".to_string(),
            self.hold_badge_size.to_string(),
        );
        env_vars.insert(
            "HOLD_BADGE_OFFSET_X".to_string(),
            self.hold_badge_offset_x.to_string(),
        );
        env_vars.insert(
            "HOLD_BADGE_OFFSET_Y".to_string(),
            self.hold_badge_offset_y.to_string(),
        );

        // Sound
        env_vars.insert("BEEP_ON_START".to_string(), self.beep_on_start.to_string());
        env_vars.insert("SOUND_NAME".to_string(), self.sound_name.clone());
        env_vars.insert("SOUND_VOLUME".to_string(), self.sound_volume.to_string());

        // History
        env_vars.insert(
            "HISTORY_ENABLED".to_string(),
            self.history_enabled.to_string(),
        );

        // Backends
        env_vars.insert(
            "WHISPER_SERVER_URL".to_string(),
            self.whisper_server_url.clone(),
        );
        env_vars.insert("LLM_SERVER_URL".to_string(), self.llm_server_url.clone());
        env_vars.insert("OLLAMA_HOST".to_string(), self.ollama_host.clone());
        env_vars.insert("OLLAMA_MODEL".to_string(), self.ollama_model.clone());

        // Clipboard
        env_vars.insert(
            "RESTORE_CLIPBOARD".to_string(),
            self.restore_clipboard.to_string(),
        );
        env_vars.insert(
            "RESTORE_CLIPBOARD_DELAY_MS".to_string(),
            self.restore_clipboard_delay_ms.to_string(),
        );

        // System
        env_vars.insert(
            "START_AT_LOGIN".to_string(),
            self.start_at_login.to_string(),
        );

        Self::write_env_file(&env_path, &env_vars)?;

        Ok(())
    }

    /// Parse .env file into HashMap.
    fn parse_env_file(path: &PathBuf) -> anyhow::Result<HashMap<String, String>> {
        // Path comes from Config::env_path() which is hardcoded to ~/.codescribe/.env
        // nosemgrep: tainted-path
        let contents = fs::read_to_string(path)?;
        let mut vars = HashMap::new();

        for line in contents.lines() {
            let line = line.trim();

            // Skip empty lines and comments
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            // Parse KEY=VALUE
            if let Some((key, value)) = line.split_once('=') {
                let key = key.trim().to_string();
                let value = value
                    .trim()
                    .trim_matches('"')
                    .trim_matches('\'')
                    .to_string();
                vars.insert(key, value);
            }
        }

        Ok(vars)
    }

    /// Write HashMap to .env file.
    fn write_env_file(path: &PathBuf, vars: &HashMap<String, String>) -> anyhow::Result<()> {
        // Path comes from Config::env_path() which is hardcoded to ~/.codescribe/.env
        // nosemgrep: tainted-path
        let mut file = fs::File::create(path)?;

        writeln!(file, "# CodeScribe Configuration")?;
        writeln!(file, "# Generated automatically - edit with care")?;
        writeln!(file)?;

        // Sort keys for consistent output
        let mut keys: Vec<_> = vars.keys().collect();
        keys.sort();

        for key in keys {
            if let Some(value) = vars.get(key) {
                writeln!(file, "{}={}", key, value)?;
            }
        }

        Ok(())
    }

    /// Get the configuration directory path (`$HOME/.codescribe`).
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

        // Default to $HOME/.codescribe (lowercase!)
        BaseDirs::new()
            .map(|dirs| dirs.home_dir().join(".codescribe"))
            .unwrap_or_else(|| PathBuf::from(".codescribe"))
    }

    /// Get the full path to the .env file.
    pub fn env_path() -> PathBuf {
        if let Ok(custom) = std::env::var("CODESCRIBE_ENV_PATH") {
            return PathBuf::from(shellexpand::tilde(&custom).into_owned());
        }

        Self::config_dir().join(".env")
    }

    /// Get the full path to the settings.json file (legacy).
    fn settings_path() -> PathBuf {
        // Check for custom settings path
        if let Ok(custom) = std::env::var("CODESCRIBE_SETTINGS_PATH") {
            return PathBuf::from(shellexpand::tilde(&custom).into_owned());
        }

        Self::config_dir().join("settings.json")
    }

    /// Sanitize configuration values to ensure they're valid.
    fn sanitize(&mut self) {
        // Ensure token limits are reasonable
        if self.ai_max_tokens <= 0 {
            self.ai_max_tokens = 512;
        }
        if self.ai_assistive_max_tokens <= 0 {
            self.ai_assistive_max_tokens = 2048;
        }

        // Validate audio thresholds (legacy)
        if self.silence_db > 0.0 || self.silence_db < -100.0 {
            self.silence_db = -45.0;
        }
        if self.silence_hang_sec <= 0.0 || self.silence_hang_sec > 10.0 {
            self.silence_hang_sec = 0.8;
        }

        // Ensure at least one backend port is configured (legacy)
        if self.backend_ports.is_empty() {
            self.backend_ports = default_backend_ports();
        }

        // Clamp sound volume
        self.sound_volume = self.sound_volume.clamp(0.0, 1.0);

        // Validate badge size
        if self.hold_badge_size < 8 || self.hold_badge_size > 64 {
            self.hold_badge_size = 12;
        }
    }
}

// ===== Global configuration access =====

/// Initialize global configuration.
///
/// Should be called once at application startup.
pub fn init() {
    let config = Config::load();
    GLOBAL_CONFIG.get_or_init(|| RwLock::new(config));
}

/// Get read access to global configuration.
///
/// # Panics
/// Panics if called before `init()`.
pub fn get() -> std::sync::RwLockReadGuard<'static, Config> {
    GLOBAL_CONFIG
        .get()
        .expect("Config not initialized - call config::init() first")
        .read()
        .expect("Config lock poisoned")
}

/// Update global configuration.
///
/// # Example
/// ```rust,no_run
/// use codescribe::config;
///
/// config::update(|c| {
///     c.beep_on_start = false;
///     c.sound_volume = 0.5;
/// });
/// ```
pub fn update<F>(f: F)
where
    F: FnOnce(&mut Config),
{
    let mut config = GLOBAL_CONFIG
        .get()
        .expect("Config not initialized - call config::init() first")
        .write()
        .expect("Config lock poisoned");

    f(&mut config);
    config.sanitize();
}

/// Save current global configuration to .env file.
pub fn save() -> anyhow::Result<()> {
    let config = get();
    config.save_all_to_env()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.hold_mods, HoldMods::Ctrl);
        assert_eq!(config.whisper_language, Language::Auto);
        assert_eq!(config.ai_provider, AiProvider::Harmony);
        assert_eq!(config.ai_max_tokens, 512);
        assert!(!config.ai_formatting_enabled);
        assert_eq!(config.backend_ports, vec![8237, 7237, 6237, 5237]);
    }

    #[test]
    fn test_hold_mods_parsing() {
        assert_eq!("ctrl".parse::<HoldMods>(), Ok(HoldMods::Ctrl));
        assert_eq!("ctrl_alt".parse::<HoldMods>(), Ok(HoldMods::CtrlAlt));
        assert_eq!("ctrl+shift".parse::<HoldMods>(), Ok(HoldMods::CtrlShift));
        assert!("invalid".parse::<HoldMods>().is_err());
    }

    #[test]
    fn test_language_parsing() {
        assert_eq!("auto".parse::<Language>(), Ok(Language::Auto));
        assert_eq!("pl".parse::<Language>(), Ok(Language::Polish));
        assert_eq!("en".parse::<Language>(), Ok(Language::English));
        assert!("invalid".parse::<Language>().is_err());
    }

    #[test]
    fn test_sanitize_token_limits() {
        let mut config = Config::default();
        config.ai_max_tokens = -1;
        config.sanitize();
        assert_eq!(config.ai_max_tokens, 512);
    }

    #[test]
    fn test_sanitize_sound_volume() {
        let mut config = Config::default();
        config.sound_volume = 1.5;
        config.sanitize();
        assert_eq!(config.sound_volume, 1.0);

        config.sound_volume = -0.5;
        config.sanitize();
        assert_eq!(config.sound_volume, 0.0);
    }

    #[test]
    fn test_config_dir() {
        let dir = Config::config_dir();
        assert!(dir.to_string_lossy().contains(".codescribe"));
    }

    #[test]
    fn test_env_file_parse_write() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        // Create temporary .env file
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "# Comment line").unwrap();
        writeln!(temp_file, "KEY1=value1").unwrap();
        writeln!(temp_file, "KEY2=\"value2\"").unwrap();
        writeln!(temp_file, "").unwrap();
        writeln!(temp_file, "KEY3=value3").unwrap();
        temp_file.flush().unwrap();

        let path = temp_file.path().to_path_buf();
        let vars = Config::parse_env_file(&path).unwrap();

        assert_eq!(vars.get("KEY1"), Some(&"value1".to_string()));
        assert_eq!(vars.get("KEY2"), Some(&"value2".to_string()));
        assert_eq!(vars.get("KEY3"), Some(&"value3".to_string()));
        assert_eq!(vars.len(), 3);
    }
}
