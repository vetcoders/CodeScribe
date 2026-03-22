uniffi::setup_scaffolding!();

use std::sync::{Arc, Mutex, OnceLock, RwLock};

use vista_kernel::{
    audio::streaming_recorder::StreamingRecorder,
    pipeline::{EngineEvent, EventSink},
};

// ── Tokio runtime (lazy, process-global) ─────────────────────────────────

static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();

fn runtime() -> &'static tokio::runtime::Runtime {
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .thread_name("vista-ffi")
            .build()
            .expect("vista-ffi: failed to create Tokio runtime")
    })
}

// ── Error ────────────────────────────────────────────────────────────────

#[derive(uniffi::Error, Debug)]
pub enum VistaError {
    SystemError { msg: String },
    ConfigError { msg: String },
    AudioError { msg: String },
    ModelError { msg: String },
}

impl std::fmt::Display for VistaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SystemError { msg }
            | Self::ConfigError { msg }
            | Self::AudioError { msg }
            | Self::ModelError { msg } => write!(f, "{msg}"),
        }
    }
}

// ── Enums ────────────────────────────────────────────────────────────────

#[derive(uniffi::Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum VistaLanguage {
    Polish,
    English,
}

#[derive(uniffi::Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum VistaAiMode {
    Formatting,
    Assistive,
}

#[derive(uniffi::Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum VistaStatusSignal {
    Thinking,
    Error,
}

// ── Records ──────────────────────────────────────────────────────────────

#[derive(uniffi::Record, Debug, Clone)]
pub struct VistaConfig {
    pub whisper_language: VistaLanguage,
    pub use_local_stt: bool,
    pub local_model: String,
    pub hold_exclusive: bool,
    pub beep_on_start: bool,
    pub sound_volume: f32,
    pub ai_formatting_enabled: bool,
}

#[derive(uniffi::Record, Debug, Clone)]
pub struct TranscriptionResult {
    pub text: String,
    pub language: String,
}

// ── Callback interface ───────────────────────────────────────────────────

#[uniffi::export(with_foreign)]
pub trait VistaEventListener: Send + Sync {
    fn on_transcription_preview(&self, text: String);
    fn on_transcription_final(&self, text: String, language: String);
    fn on_status_changed(&self, signal: VistaStatusSignal);
    fn on_error(&self, msg: String);
}

// ── Type conversions ─────────────────────────────────────────────────────

impl From<qube_support::config::Language> for VistaLanguage {
    fn from(lang: qube_support::config::Language) -> Self {
        match lang {
            qube_support::config::Language::Polish => Self::Polish,
            qube_support::config::Language::English => Self::English,
        }
    }
}

impl From<VistaLanguage> for qube_support::config::Language {
    fn from(lang: VistaLanguage) -> Self {
        match lang {
            VistaLanguage::Polish => Self::Polish,
            VistaLanguage::English => Self::English,
        }
    }
}

struct VistaPipelineEventSink {
    listener: Arc<dyn VistaEventListener>,
    language: String,
}

impl VistaPipelineEventSink {
    fn new(listener: Arc<dyn VistaEventListener>, language: String) -> Self {
        Self { listener, language }
    }
}

impl EventSink for VistaPipelineEventSink {
    fn on_event(&self, event: &EngineEvent) {
        match event {
            EngineEvent::Preview { text, .. } | EngineEvent::Correction { text, .. } => {
                self.listener.on_transcription_preview(text.clone());
            }
            EngineEvent::UtteranceFinal { text, .. } => {
                self.listener
                    .on_transcription_final(text.clone(), self.language.clone());
            }
            EngineEvent::Warning { message, .. } => {
                self.listener.on_status_changed(VistaStatusSignal::Error);
                self.listener.on_error(message.clone());
            }
            _ => {}
        }
    }
}

// ── Engine (main FFI object) ─────────────────────────────────────────────

#[derive(uniffi::Object)]
pub struct VistaEngine {
    listener: RwLock<Option<Arc<dyn VistaEventListener>>>,
    recorder: Mutex<Option<StreamingRecorder>>,
}

impl VistaEngine {
    fn cloned_listener(&self) -> Result<Option<Arc<dyn VistaEventListener>>, VistaError> {
        self.listener
            .read()
            .map(|guard| guard.clone())
            .map_err(|e| VistaError::SystemError { msg: e.to_string() })
    }

    fn resolve_pipeline_language(&self, requested: Option<&str>) -> String {
        requested
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .unwrap_or_else(
                || match qube_support::config::Config::load().whisper_language {
                    qube_support::config::Language::Polish => "pl".to_string(),
                    qube_support::config::Language::English => "en".to_string(),
                },
            )
    }
}

#[uniffi::export]
impl VistaEngine {
    // ── Constructor ──────────────────────────────────────────────────

    #[uniffi::constructor]
    pub fn new() -> Arc<Self> {
        let _ = runtime();
        Arc::new(Self {
            listener: RwLock::new(None),
            recorder: Mutex::new(None),
        })
    }

    // ── Event listener ───────────────────────────────────────────────

    pub fn set_event_listener(&self, listener: Arc<dyn VistaEventListener>) {
        if let Ok(mut guard) = self.listener.write() {
            *guard = Some(listener);
        }
    }

    pub fn remove_event_listener(&self) {
        if let Ok(mut guard) = self.listener.write() {
            *guard = None;
        }
    }

    // ── Model management ─────────────────────────────────────────────

    pub fn init_model(&self) -> Result<(), VistaError> {
        if !qube_stt::stt::whisper::singleton::is_initialized() {
            qube_stt::stt::whisper::singleton::init()
                .map_err(|e| VistaError::ModelError { msg: e.to_string() })?;
        }
        Ok(())
    }

    pub fn is_model_loaded(&self) -> bool {
        qube_stt::stt::whisper::singleton::is_initialized()
    }

    // ── Config ───────────────────────────────────────────────────────

    pub fn load_config(&self) -> VistaConfig {
        let cfg = qube_support::config::Config::load();
        VistaConfig {
            whisper_language: cfg.whisper_language.into(),
            use_local_stt: cfg.use_local_stt,
            local_model: cfg.local_model,
            hold_exclusive: cfg.hold_exclusive,
            beep_on_start: cfg.beep_on_start,
            sound_volume: cfg.sound_volume,
            ai_formatting_enabled: cfg.ai_formatting_enabled,
        }
    }

    pub fn update_config(&self, key: String, value: String) -> Result<(), VistaError> {
        let cfg = qube_support::config::Config::load();
        cfg.save_to_env(&key, &value)
            .map_err(|e| VistaError::ConfigError { msg: e.to_string() })
    }

    pub fn config_dir(&self) -> String {
        qube_support::config::Config::config_dir()
            .to_string_lossy()
            .to_string()
    }

    // ── Transcription (file) ─────────────────────────────────────────

    pub async fn transcribe_file(
        &self,
        audio_path: String,
    ) -> Result<TranscriptionResult, VistaError> {
        let path = std::path::PathBuf::from(&audio_path);
        let (samples, sample_rate) = qube_stt::audio::load_audio_file(&path)
            .map_err(|e| VistaError::AudioError { msg: e.to_string() })?;

        self.init_model()?;

        let engine_mux = qube_stt::stt::whisper::singleton::engine()
            .map_err(|e| VistaError::ModelError { msg: e.to_string() })?;
        let mut engine = engine_mux
            .lock()
            .map_err(|e| VistaError::SystemError { msg: e.to_string() })?;

        let language = engine
            .detect_language(&samples, sample_rate)
            .map_err(|e| VistaError::SystemError { msg: e.to_string() })?;

        let text = engine
            .transcribe_long_with_language(&samples, sample_rate, Some(&language))
            .map_err(|e| VistaError::SystemError { msg: e.to_string() })?;

        Ok(TranscriptionResult { text, language })
    }

    // ── Recording pipeline ───────────────────────────────────────────

    pub fn start_recording(&self, language: Option<String>) -> Result<(), VistaError> {
        let mut guard = self
            .recorder
            .lock()
            .map_err(|e| VistaError::SystemError { msg: e.to_string() })?;
        if guard.is_some() {
            return Err(VistaError::SystemError {
                msg: "recording already active".to_string(),
            });
        }

        let listener = self.cloned_listener()?;
        let mut recorder =
            StreamingRecorder::new().map_err(|e| VistaError::AudioError { msg: e.to_string() })?;
        if let Some(listener) = listener {
            let sink: Arc<dyn EventSink> = Arc::new(VistaPipelineEventSink::new(
                listener,
                self.resolve_pipeline_language(language.as_deref()),
            ));
            recorder.set_event_sink(Some(sink));
        }

        runtime()
            .block_on(recorder.start_event_session(language))
            .map_err(|e| VistaError::AudioError { msg: e.to_string() })?;
        *guard = Some(recorder);
        Ok(())
    }

    pub fn stop_recording(&self) -> Result<String, VistaError> {
        let mut recorder = {
            let mut guard = self
                .recorder
                .lock()
                .map_err(|e| VistaError::SystemError { msg: e.to_string() })?;
            guard.take().ok_or_else(|| VistaError::SystemError {
                msg: "no active recording".to_string(),
            })?
        };

        let (text, _path) = runtime()
            .block_on(recorder.stop())
            .map_err(|e| VistaError::AudioError { msg: e.to_string() })?;

        Ok(text)
    }

    pub fn is_recording(&self) -> bool {
        self.recorder
            .lock()
            .map(|guard| guard.is_some())
            .unwrap_or(false)
    }

    pub fn start_pipeline(&self, language: Option<String>) -> Result<(), VistaError> {
        self.start_recording(language)
    }

    pub fn stop_pipeline(&self) -> Result<String, VistaError> {
        self.stop_recording()
    }

    // ── AI formatting ────────────────────────────────────────────────

    pub fn is_formatting_available(&self) -> bool {
        vista_kernel::ai_formatting::is_formatting_available()
    }

    pub async fn format_text(&self, text: String, assistive: bool) -> Result<String, VistaError> {
        let handle = runtime().handle().clone();
        handle
            .spawn(async move {
                vista_kernel::ai_formatting::format_text(&text, None, assistive).await
            })
            .await
            .map_err(|e| VistaError::SystemError {
                msg: format!("formatting task failed: {e}"),
            })
    }

    // ── Conversation state ───────────────────────────────────────────

    pub fn reset_conversation(&self) {
        vista_kernel::state::reset_conversation();
    }

    pub fn has_active_conversation(&self) -> bool {
        vista_kernel::state::has_active_conversation()
    }

    pub fn reset_conversation_for_mode(&self, mode: VistaAiMode) {
        let kernel_mode = match mode {
            VistaAiMode::Formatting => vista_kernel::state::AiMode::Formatting,
            VistaAiMode::Assistive => vista_kernel::state::AiMode::Assistive,
        };
        vista_kernel::state::reset_conversation_for_mode(kernel_mode);
    }

    // ── History ──────────────────────────────────────────────────────

    pub fn save_history(&self, text: String) -> String {
        let entry = vista_kernel::state::save_entry(&text);
        entry.path.to_string_lossy().to_string()
    }

    pub fn latest_history_path(&self) -> Result<String, VistaError> {
        let entry = vista_kernel::state::latest_entry()
            .map_err(|e| VistaError::SystemError { msg: e.to_string() })?;
        Ok(entry.path.to_string_lossy().to_string())
    }

    // ── Prompts ──────────────────────────────────────────────────────

    pub fn get_formatting_prompt(&self) -> String {
        qube_support::config::prompts::get_formatting_prompt()
    }

    pub fn get_assistive_prompt(&self) -> String {
        qube_support::config::prompts::get_assistive_prompt()
    }

    // ── Onboarding ───────────────────────────────────────────────────

    pub fn should_show_onboarding(&self) -> bool {
        vista_kernel::should_show_onboarding()
    }
}
