use std::env;
use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use directories::BaseDirs;
use serde::{Deserialize, Serialize};

pub mod models;
pub mod prompts;

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    #[default]
    Polish,
    English,
}

impl Language {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Polish => "pl",
            Self::English => "en",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WorkMode {
    Dictation,
    Formatting,
    Assistive,
}

impl WorkMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Dictation => "dictation",
            Self::Formatting => "formatting",
            Self::Assistive => "assistive",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ShortcutBinding {
    HoldFn,
    HoldCtrl,
    HoldCtrlAlt,
    HoldCtrlShift,
    HoldCtrlCmd,
    DoubleCtrl,
    DoubleLeftOption,
    DoubleRightOption,
    Disabled,
}

impl ShortcutBinding {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::HoldFn => "hold_fn",
            Self::HoldCtrl => "hold_ctrl",
            Self::HoldCtrlAlt => "hold_ctrl_alt",
            Self::HoldCtrlShift => "hold_ctrl_shift",
            Self::HoldCtrlCmd => "hold_ctrl_cmd",
            Self::DoubleCtrl => "double_ctrl",
            Self::DoubleLeftOption => "double_left_option",
            Self::DoubleRightOption => "double_right_option",
            Self::Disabled => "disabled",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserSettings {
    #[serde(default = "default_dictation_binding")]
    dictation_binding: ShortcutBinding,
    #[serde(default = "default_formatting_binding")]
    formatting_binding: ShortcutBinding,
    #[serde(default = "default_assistive_binding")]
    assistive_binding: ShortcutBinding,
    #[serde(default)]
    pub hold_exclusive: Option<bool>,
    #[serde(default)]
    pub typing_cps: Option<f32>,
    #[serde(default)]
    pub use_local_stt: Option<bool>,
    #[serde(default)]
    chat_zoom: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub whisper_language: Language,
    pub use_local_stt: bool,
    pub local_model: String,
    pub stt_endpoint: Option<String>,
    pub stt_api_key: Option<String>,
    pub audio_input_device: Option<String>,
    pub hold_start_delay_ms: u32,
    pub hold_exclusive: bool,
    pub beep_on_start: bool,
    pub sound_volume: f32,
    pub ai_formatting_enabled: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            whisper_language: Language::Polish,
            use_local_stt: true,
            local_model: models::DEFAULT_MODEL.to_string(),
            stt_endpoint: None,
            stt_api_key: None,
            audio_input_device: None,
            hold_start_delay_ms: 300,
            hold_exclusive: true,
            beep_on_start: true,
            sound_volume: 1.0,
            ai_formatting_enabled: true,
        }
    }
}

impl Config {
    pub fn config_dir() -> PathBuf {
        let requested = env::var("CODESCRIBE_DATA_DIR")
            .ok()
            .map(PathBuf::from)
            .or_else(|| BaseDirs::new().map(|dirs| dirs.home_dir().join(".codescribe")))
            .unwrap_or_else(|| PathBuf::from(".codescribe"));

        let _ = fs::create_dir_all(&requested);
        requested.canonicalize().unwrap_or(requested)
    }

    pub fn load() -> Self {
        let mut config = Self::default();
        let settings = UserSettings::load();

        if let Ok(value) = env::var("WHISPER_LANGUAGE") {
            config.whisper_language = match value.trim().to_ascii_lowercase().as_str() {
                "en" | "english" => Language::English,
                _ => Language::Polish,
            };
        }

        if let Ok(value) = env::var("USE_LOCAL_STT") {
            config.use_local_stt = parse_bool(&value).unwrap_or(true);
        } else if let Some(value) = settings.use_local_stt {
            config.use_local_stt = value;
        }

        if let Ok(value) = env::var("LOCAL_MODEL") {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                config.local_model = trimmed.to_string();
            }
        }

        config.stt_endpoint = env_non_empty("STT_ENDPOINT");
        config.stt_api_key = env_non_empty("STT_API_KEY");
        config.audio_input_device = env_non_empty("AUDIO_INPUT_DEVICE");

        if let Ok(value) = env::var("HOLD_START_DELAY_MS")
            && let Ok(parsed) = value.trim().parse::<u32>()
        {
            config.hold_start_delay_ms = parsed.clamp(100, 2000);
        }

        if let Ok(value) = env::var("HOLD_EXCLUSIVE") {
            config.hold_exclusive = parse_bool(&value).unwrap_or(true);
        } else if let Some(value) = settings.hold_exclusive {
            config.hold_exclusive = value;
        }

        if let Ok(value) = env::var("BEEP_ON_START") {
            config.beep_on_start = parse_bool(&value).unwrap_or(true);
        }

        if let Ok(value) = env::var("SOUND_VOLUME")
            && let Ok(parsed) = value.trim().parse::<f32>()
        {
            config.sound_volume = parsed.clamp(0.0, 1.0);
        }

        if let Ok(value) = env::var("AI_FORMATTING_ENABLED") {
            config.ai_formatting_enabled = parse_bool(&value).unwrap_or(true);
        }

        config
    }

    pub fn save_to_env(&self, key: &str, value: &str) -> Result<()> {
        // SAFETY: callers already treat config mutation as process-global state.
        unsafe { env::set_var(key, value) };

        match key {
            "HOLD_EXCLUSIVE" => {
                let mut settings = UserSettings::load();
                settings.hold_exclusive = parse_bool(value);
                settings.save()?;
            }
            "CODESCRIBE_TYPING_CPS" => {
                let mut settings = UserSettings::load();
                settings.typing_cps = value.trim().parse::<f32>().ok();
                settings.save()?;
            }
            "USE_LOCAL_STT" => {
                let mut settings = UserSettings::load();
                settings.use_local_stt = parse_bool(value);
                settings.save()?;
            }
            _ => {}
        }

        Ok(())
    }
}

impl Default for UserSettings {
    fn default() -> Self {
        Self {
            dictation_binding: default_dictation_binding(),
            formatting_binding: default_formatting_binding(),
            assistive_binding: default_assistive_binding(),
            hold_exclusive: None,
            typing_cps: None,
            use_local_stt: None,
            chat_zoom: None,
        }
    }
}

impl UserSettings {
    pub fn settings_path() -> PathBuf {
        Config::config_dir().join("settings.json")
    }

    pub fn load() -> Self {
        let path = Self::settings_path();
        let Ok(contents) = fs::read_to_string(&path) else {
            return Self::default();
        };

        serde_json::from_str::<Self>(&contents).unwrap_or_default()
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::settings_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("create settings dir: {}", parent.display()))?;
        }

        let json = serde_json::to_string_pretty(self).context("serialize settings")?;
        fs::write(&path, json).with_context(|| format!("write settings: {}", path.display()))?;
        Ok(())
    }

    pub fn mode_binding_for(&self, mode: WorkMode) -> ShortcutBinding {
        match mode {
            WorkMode::Dictation => self.dictation_binding,
            WorkMode::Formatting => self.formatting_binding,
            WorkMode::Assistive => self.assistive_binding,
        }
    }

    pub fn set_mode_binding(&mut self, mode: WorkMode, binding: ShortcutBinding) {
        match mode {
            WorkMode::Dictation => self.dictation_binding = binding,
            WorkMode::Formatting => self.formatting_binding = binding,
            WorkMode::Assistive => self.assistive_binding = binding,
        }
        let _ = self.save();
    }

    pub fn set_chat_zoom(&mut self, zoom: f32) -> bool {
        let normalized = normalize_zoom(zoom);
        let next = if (normalized - 1.0).abs() < f32::EPSILON {
            None
        } else {
            Some(normalized)
        };

        if approx_zoom_eq(self.chat_zoom, next) {
            return false;
        }

        self.chat_zoom = next;
        let _ = self.save();
        true
    }
}

fn default_dictation_binding() -> ShortcutBinding {
    ShortcutBinding::HoldFn
}

fn default_formatting_binding() -> ShortcutBinding {
    ShortcutBinding::DoubleLeftOption
}

fn default_assistive_binding() -> ShortcutBinding {
    ShortcutBinding::DoubleRightOption
}

fn env_non_empty(key: &str) -> Option<String> {
    env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn parse_bool(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    }
}

fn normalize_zoom(value: f32) -> f32 {
    ((value * 100.0).round() / 100.0).clamp(0.5, 3.0)
}

fn approx_zoom_eq(current: Option<f32>, next: Option<f32>) -> bool {
    match (current, next) {
        (None, None) => true,
        (Some(a), Some(b)) => (normalize_zoom(a) - normalize_zoom(b)).abs() < 0.0001,
        _ => false,
    }
}
