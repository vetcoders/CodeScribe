use std::env;
use std::fs;
use std::path::PathBuf;

use directories::BaseDirs;
use serde::{Deserialize, Serialize};

pub mod models;

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub whisper_language: Language,
    pub use_local_stt: bool,
    pub local_model: String,
    pub stt_endpoint: Option<String>,
    pub stt_api_key: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            whisper_language: Language::Polish,
            use_local_stt: true,
            local_model: models::DEFAULT_MODEL.to_string(),
            stt_endpoint: None,
            stt_api_key: None,
        }
    }
}

impl Config {
    pub fn config_dir() -> PathBuf {
        let dir = BaseDirs::new()
            .map(|dirs| dirs.home_dir().join(".codescribe"))
            .unwrap_or_else(|| PathBuf::from(".codescribe"));
        let _ = fs::create_dir_all(&dir);
        dir
    }

    pub fn load() -> Self {
        let mut config = Self::default();

        if let Ok(value) = env::var("WHISPER_LANGUAGE") {
            config.whisper_language = match value.trim().to_ascii_lowercase().as_str() {
                "en" | "english" => Language::English,
                _ => Language::Polish,
            };
        }

        if let Ok(value) = env::var("USE_LOCAL_STT") {
            config.use_local_stt = parse_bool(&value).unwrap_or(true);
        }

        if let Ok(value) = env::var("LOCAL_MODEL") {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                config.local_model = trimmed.to_string();
            }
        }

        config.stt_endpoint = env::var("STT_ENDPOINT")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        config.stt_api_key = env::var("STT_API_KEY")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());

        config
    }
}

fn parse_bool(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    }
}
