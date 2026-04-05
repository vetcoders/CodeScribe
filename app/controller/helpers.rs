//! Controller helper functions
//!
//! Session state management and utility functions.

use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::RwLock;
use tracing::warn;

use crate::config::Config;
use codescribe_core::demux::{DemuxEvent, StreamingTagParser};

/// Global flag for current session mode.
/// true = assistive (chat UI), false = non-assistive (simple transcription overlay)
/// This is set before recording starts and checked by the delta callback.
static IS_ASSISTIVE_SESSION: AtomicBool = AtomicBool::new(false);

/// Global flag for conversation mode (full-duplex Moshi).
/// When true, audio is routed to ConversationEngine instead of Whisper.
static IS_CONVERSATION_SESSION: AtomicBool = AtomicBool::new(false);

/// Set the current session mode (called before recording starts)
pub fn set_assistive_session(is_assistive: bool) {
    IS_ASSISTIVE_SESSION.store(is_assistive, Ordering::SeqCst);
}

/// Check if current session is assistive mode
pub fn is_assistive_session() -> bool {
    IS_ASSISTIVE_SESSION.load(Ordering::SeqCst)
}

/// Set conversation mode flag (Moshi full-duplex)
pub fn set_conversation_session(is_conversation: bool) {
    IS_CONVERSATION_SESSION.store(is_conversation, Ordering::SeqCst);
}

/// Check if current session is conversation mode (Moshi)
pub fn is_conversation_session() -> bool {
    IS_CONVERSATION_SESSION.load(Ordering::SeqCst)
}

/// Route transcription delta to the active overlay.
///
/// Contract:
/// - Assistive sessions stream into Agent overlay chat bubbles.
/// - Non-assistive sessions stream into Dictation/Transcription overlay preview.
pub fn route_transcription_delta(delta: &str) {
    if is_assistive_session() {
        crate::voice_chat_ui::append_voice_chat_user_delta(delta);
    } else {
        // Non-assistive: live dictation preview in ephemeral overlay
        crate::ui::overlay::append_transcription_delta(delta);
    }
}

/// DeltaSink that routes deltas to the active UI overlay.
///
/// Uses `is_assistive_session()` to decide: chat bubble vs transcription overlay.
/// Plugs into `PresentationEmitter` → `BufferedEmitter` → delta chain.
pub struct RoutingDeltaSink;

impl codescribe_core::pipeline::contracts::DeltaSink for RoutingDeltaSink {
    fn apply(&self, delta: &codescribe_core::pipeline::contracts::TranscriptDelta) {
        route_transcription_delta(&delta.delta);
    }
}

/// Setup the voice chat send callback with config
pub fn setup_voice_chat_send_callback(config: Arc<RwLock<Config>>) {
    let callback_config = Arc::clone(&config);
    crate::voice_chat_ui::set_voice_chat_send_callback(Some(Arc::new(move |text: String| {
        let config = Arc::clone(&callback_config);
        tokio::spawn(async move {
            crate::voice_chat_ui::update_voice_chat_status("Sending...");
            crate::voice_chat_ui::set_voice_chat_sending(true);

            let lang_str = {
                let cfg = config.read().await;
                cfg.whisper_language
            };

            // Chat overlay should always stream assistant deltas when provider supports SSE.
            let use_streaming = true;
            let streamed_any_delta = Arc::new(AtomicBool::new(false));

            let response_router = if use_streaming {
                let streamed_any_delta = Arc::clone(&streamed_any_delta);
                let on_visible_text = Arc::new(move |delta: &str| {
                    if !delta.trim().is_empty() {
                        streamed_any_delta.store(true, Ordering::SeqCst);
                    }
                    crate::voice_chat_ui::append_voice_chat_assistant_delta(delta);
                }) as Arc<dyn Fn(&str) + Send + Sync>;
                Some(AssistiveResponseRouter::new(on_visible_text, true))
            } else {
                None
            };
            let delta_callback = response_router.as_ref().map(|router| router.callback());

            let result = crate::ai_formatting::format_text_with_status_channels(
                &text,
                Some(lang_str.as_str()),
                true,
                delta_callback,
                None,
            )
            .await;

            if let Some(router) = response_router.as_ref() {
                router.flush();
            }

            match result.status {
                crate::ai_formatting::AiFormatStatus::Applied => {
                    crate::voice_chat_ui::update_voice_chat_status("AI Response:");
                    if use_streaming && streamed_any_delta.load(Ordering::SeqCst) {
                        crate::voice_chat_ui::finalize_voice_chat_assistant_message();
                    } else {
                        crate::voice_chat_ui::set_voice_chat_text(&result.text);
                    }
                    if let Some(reasoning_text) = result.reasoning_text.clone() {
                        crate::voice_chat_ui::add_voice_chat_system_message(&format!(
                            "Reasoning summary:\n{}",
                            reasoning_text
                        ));
                    }
                }
                crate::ai_formatting::AiFormatStatus::Failed => {
                    crate::voice_chat_ui::update_voice_chat_status("AI Failed");
                    crate::voice_chat_ui::add_voice_chat_error_message("AI Failed");
                }
                crate::ai_formatting::AiFormatStatus::Skipped => {
                    crate::voice_chat_ui::set_voice_chat_sending(false);
                }
            }
        });
    })));
}

/// Raw transcript saving is always enabled to avoid data loss.
pub fn raw_save_enabled() -> bool {
    true
}

// ═══════════════════════════════════════════════════════════
// Event-based routing (new pipeline)
// ═══════════════════════════════════════════════════════════

use chrono::SecondsFormat;
use codescribe_core::ipc::{EngineEventWire, IpcEvent, IpcEventPayload};
use codescribe_core::pipeline::contracts::{EngineEvent, EventSink};
use tokio::sync::{broadcast, mpsc};

#[derive(Debug, Default, PartialEq, Eq)]
struct AssistiveStreamEvents {
    visible_text: String,
    spoken_segments: Vec<String>,
}

fn collect_assistive_stream_events(events: Vec<DemuxEvent>) -> AssistiveStreamEvents {
    let mut collected = AssistiveStreamEvents::default();

    for event in events {
        match event {
            DemuxEvent::Text(text) => collected.visible_text.push_str(&text),
            DemuxEvent::Speak(text) => {
                collected.visible_text.push_str(&text);
                if !text.trim().is_empty() {
                    collected.spoken_segments.push(text);
                }
            }
            DemuxEvent::Tool { .. } | DemuxEvent::Partial(_) => {}
        }
    }

    collected
}

fn assistive_tts_available() -> bool {
    codescribe_core::tts::embedded::is_embedded_available()
        || codescribe_core::tts::get_model_path().is_ok()
}

pub(crate) struct AssistiveResponseRouter {
    parser: Arc<StdMutex<StreamingTagParser>>,
    on_visible_text: Arc<dyn Fn(&str) + Send + Sync>,
    speech_tx: Option<mpsc::UnboundedSender<String>>,
}

impl AssistiveResponseRouter {
    pub(crate) fn new(on_visible_text: Arc<dyn Fn(&str) + Send + Sync>, enable_tts: bool) -> Self {
        let speech_tx = if enable_tts && assistive_tts_available() {
            let (tx, mut rx) = mpsc::unbounded_channel::<String>();
            tokio::spawn(async move {
                while let Some(chunk) = rx.recv().await {
                    let text = chunk.trim().to_string();
                    if text.is_empty() {
                        continue;
                    }

                    match tokio::task::spawn_blocking(move || codescribe_core::tts::play(&text))
                        .await
                    {
                        Ok(Ok(())) => {}
                        Ok(Err(error)) => warn!("Assistive TTS playback failed: {}", error),
                        Err(error) => warn!("Assistive TTS task join failed: {}", error),
                    }
                }
            });
            Some(tx)
        } else {
            if enable_tts {
                warn!("Assistive TTS routing disabled: local TTS model is unavailable");
            }
            None
        };

        Self {
            parser: Arc::new(StdMutex::new(StreamingTagParser::new())),
            on_visible_text,
            speech_tx,
        }
    }

    fn dispatch(&self, events: Vec<DemuxEvent>) {
        let collected = collect_assistive_stream_events(events);

        if !collected.visible_text.is_empty() {
            (self.on_visible_text)(&collected.visible_text);
        }

        if let Some(tx) = &self.speech_tx {
            for segment in collected.spoken_segments {
                if tx.send(segment).is_err() {
                    break;
                }
            }
        }
    }

    pub(crate) fn callback(&self) -> crate::ai_formatting::AiStreamCallback {
        let parser = Arc::clone(&self.parser);
        let on_visible_text = Arc::clone(&self.on_visible_text);
        let speech_tx = self.speech_tx.clone();

        Arc::new(move |chunk: &str| {
            let events = {
                let mut parser = parser.lock().unwrap_or_else(|e| e.into_inner());
                parser.feed(chunk)
            };
            let collected = collect_assistive_stream_events(events);

            if !collected.visible_text.is_empty() {
                on_visible_text(&collected.visible_text);
            }

            if let Some(tx) = &speech_tx {
                for segment in collected.spoken_segments {
                    if tx.send(segment).is_err() {
                        break;
                    }
                }
            }
        })
    }

    pub(crate) fn flush(&self) {
        let events = {
            let mut parser = self.parser.lock().unwrap_or_else(|e| e.into_inner());
            parser.flush()
        };
        self.dispatch(events);
    }
}

/// Session-level engine stats snapshot used by controller decisions.
#[derive(Debug, Clone, Default)]
pub(crate) struct SessionEngineStats {
    pub hallucination_drops: u64,
    pub semantic_gate_drops: u64,
    pub filtered_empty_drops: u64,
    pub corrections_applied: u64,
    pub total_utterances: u64,
    pub dropped_audio_chunks: u64,
    pub partial_runs_total: u64,
    pub trigger_utterance_count: u64,
    pub trigger_speech_count: u64,
    pub trigger_watchdog_count: u64,
    pub partial_stale_count: u64,
    pub partial_coalesced_count: u64,
    pub partial_dropped_count: u64,
}

/// Session telemetry captured from `EngineEvent`s.
#[derive(Debug, Clone, Default)]
pub(crate) struct SessionTelemetrySnapshot {
    pub no_speech_reason: Option<String>,
    pub stats: Option<SessionEngineStats>,
}

pub(crate) type SharedSessionTelemetry = Arc<StdMutex<SessionTelemetrySnapshot>>;

pub(crate) fn new_session_telemetry() -> SharedSessionTelemetry {
    Arc::new(StdMutex::new(SessionTelemetrySnapshot::default()))
}

pub(crate) fn reset_session_telemetry(shared: &SharedSessionTelemetry) {
    let mut guard = shared.lock().unwrap_or_else(|e| e.into_inner());
    *guard = SessionTelemetrySnapshot::default();
}

pub(crate) fn snapshot_session_telemetry(
    shared: &SharedSessionTelemetry,
) -> SessionTelemetrySnapshot {
    shared.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

/// Captures `NoSpeech`/`Stats` telemetry for controller-level routing decisions.
pub(crate) struct SessionTelemetrySink {
    shared: SharedSessionTelemetry,
}

impl SessionTelemetrySink {
    pub(crate) fn new(shared: SharedSessionTelemetry) -> Self {
        Self { shared }
    }
}

/// Broadcasts sanitized engine events to IPC subscribers.
pub(crate) struct IpcBroadcastSink {
    tx: broadcast::Sender<IpcEvent>,
}

impl IpcBroadcastSink {
    pub(crate) fn new(tx: broadcast::Sender<IpcEvent>) -> Self {
        Self { tx }
    }
}

impl EventSink for IpcBroadcastSink {
    fn on_event(&self, event: &EngineEvent) {
        let ipc_event = IpcEvent {
            timestamp: chrono::Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),
            payload: IpcEventPayload::Engine(EngineEventWire::from(event)),
        };
        let _ = self.tx.send(ipc_event);
    }
}

impl EventSink for SessionTelemetrySink {
    fn on_event(&self, event: &EngineEvent) {
        let mut guard = self.shared.lock().unwrap_or_else(|e| e.into_inner());
        match event {
            EngineEvent::NoSpeech { reason } => {
                guard.no_speech_reason = Some(reason.clone());
            }
            EngineEvent::Stats {
                hallucination_drops,
                semantic_gate_drops,
                filtered_empty_drops,
                corrections_applied,
                total_utterances,
                dropped_audio_chunks,
                partial_runs_total,
                trigger_utterance_count,
                trigger_speech_count,
                trigger_watchdog_count,
                partial_stale_count,
                partial_coalesced_count,
                partial_dropped_count,
            } => {
                guard.stats = Some(SessionEngineStats {
                    hallucination_drops: *hallucination_drops,
                    semantic_gate_drops: *semantic_gate_drops,
                    filtered_empty_drops: *filtered_empty_drops,
                    corrections_applied: *corrections_applied,
                    total_utterances: *total_utterances,
                    dropped_audio_chunks: *dropped_audio_chunks,
                    partial_runs_total: *partial_runs_total,
                    trigger_utterance_count: *trigger_utterance_count,
                    trigger_speech_count: *trigger_speech_count,
                    trigger_watchdog_count: *trigger_watchdog_count,
                    partial_stale_count: *partial_stale_count,
                    partial_coalesced_count: *partial_coalesced_count,
                    partial_dropped_count: *partial_dropped_count,
                });
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_telemetry_sink_tracks_no_speech_and_stats() {
        let shared = new_session_telemetry();
        let sink = SessionTelemetrySink::new(Arc::clone(&shared));

        sink.on_event(&EngineEvent::NoSpeech {
            reason: "vad_no_speech_detected".to_string(),
        });
        sink.on_event(&EngineEvent::Stats {
            dropped_audio_chunks: 3,
            hallucination_drops: 2,
            semantic_gate_drops: 1,
            filtered_empty_drops: 4,
            corrections_applied: 5,
            total_utterances: 0,
            partial_runs_total: 6,
            trigger_utterance_count: 2,
            trigger_speech_count: 3,
            trigger_watchdog_count: 1,
            partial_stale_count: 7,
            partial_coalesced_count: 8,
            partial_dropped_count: 9,
        });

        let snapshot = snapshot_session_telemetry(&shared);
        assert_eq!(
            snapshot.no_speech_reason.as_deref(),
            Some("vad_no_speech_detected")
        );
        let stats = snapshot.stats.expect("stats should be captured");
        assert_eq!(stats.hallucination_drops, 2);
        assert_eq!(stats.semantic_gate_drops, 1);
        assert_eq!(stats.filtered_empty_drops, 4);
        assert_eq!(stats.corrections_applied, 5);
        assert_eq!(stats.total_utterances, 0);
        assert_eq!(stats.dropped_audio_chunks, 3);
        assert_eq!(stats.partial_runs_total, 6);
        assert_eq!(stats.trigger_utterance_count, 2);
        assert_eq!(stats.trigger_speech_count, 3);
        assert_eq!(stats.trigger_watchdog_count, 1);
        assert_eq!(stats.partial_stale_count, 7);
        assert_eq!(stats.partial_coalesced_count, 8);
        assert_eq!(stats.partial_dropped_count, 9);
    }

    #[test]
    fn test_reset_session_telemetry_clears_snapshot() {
        let shared = new_session_telemetry();
        {
            let mut guard = shared.lock().unwrap_or_else(|e| e.into_inner());
            guard.no_speech_reason = Some("test".to_string());
            guard.stats = Some(SessionEngineStats {
                hallucination_drops: 1,
                ..Default::default()
            });
        }
        reset_session_telemetry(&shared);

        let snapshot = snapshot_session_telemetry(&shared);
        assert!(snapshot.no_speech_reason.is_none());
        assert!(snapshot.stats.is_none());
    }

    #[test]
    fn test_collect_assistive_stream_events_keeps_visible_order_and_spoken_segments() {
        let collected = collect_assistive_stream_events(vec![
            DemuxEvent::Text("Ala ".to_string()),
            DemuxEvent::Speak("ma kota".to_string()),
            DemuxEvent::Tool {
                name: "ignored".to_string(),
                args: "{}".to_string(),
            },
            DemuxEvent::Text(".".to_string()),
        ]);

        assert_eq!(collected.visible_text, "Ala ma kota.");
        assert_eq!(collected.spoken_segments, vec!["ma kota".to_string()]);
    }

    #[test]
    fn test_assistive_response_router_flushes_split_speak_tag() {
        let visible = Arc::new(StdMutex::new(String::new()));
        let on_visible_text = {
            let visible = Arc::clone(&visible);
            Arc::new(move |delta: &str| {
                let mut guard = visible.lock().unwrap_or_else(|e| e.into_inner());
                guard.push_str(delta);
            }) as Arc<dyn Fn(&str) + Send + Sync>
        };
        let router = AssistiveResponseRouter::new(on_visible_text, false);
        let callback = router.callback();

        callback("<speak>To jest");
        callback(" test");
        assert_eq!(*visible.lock().unwrap_or_else(|e| e.into_inner()), "");

        router.flush();
        assert_eq!(
            *visible.lock().unwrap_or_else(|e| e.into_inner()),
            "To jest test"
        );
    }
}
