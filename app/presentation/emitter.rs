//! Event-driven presentation emitter.
//!
//! Converts `EngineEvent`s into user-facing output by delegating to
//! `BufferedEmitter` (typing animation, delta encoding) from core.
//!
//! This is the app-layer replacement for directly coupling the pipeline
//! to `BufferedEmitter`. The engine says "here's a preview", and this
//! module decides when/how to show it.
//!
//! Created by M&K (c)2026 VetCoders

use std::sync::Arc;

use codescribe_core::pipeline::contracts::{DeltaSink, EngineEvent, EventSink};
use codescribe_core::pipeline::streaming::BufferedEmitter;
use tokio::sync::Mutex;
use tracing::{debug, info};

/// Presentation emitter — bridges `EngineEvent`s to `BufferedEmitter`.
///
/// Implements `EventSink` so it can be plugged directly into `transcription_session`.
/// Internally manages the `BufferedEmitter` tick loop for typing animation.
pub struct PresentationEmitter {
    emitter: Arc<Mutex<BufferedEmitter>>,
    emitter_handle: Option<tokio::task::JoinHandle<()>>,
    /// Optional callback for completed utterances (used by Toggle mode).
    utterance_callback: Option<Arc<dyn Fn(String) + Send + Sync>>,
    /// Optional callback for VAD stop detection.
    vad_stop_callback: Option<Arc<dyn Fn() + Send + Sync>>,
    vad_stop_emitted: std::sync::atomic::AtomicBool,
}

impl PresentationEmitter {
    pub fn new(
        transcript_buffer: Arc<Mutex<String>>,
        delta_callback: Option<Arc<dyn DeltaSink>>,
        stream_log_path: Option<std::path::PathBuf>,
    ) -> Self {
        let emitter = Arc::new(Mutex::new(BufferedEmitter::new(
            transcript_buffer,
            delta_callback,
            stream_log_path,
        )));

        let emitter_clone = emitter.clone();
        let emitter_handle = Some(tokio::spawn(
            codescribe_core::pipeline::streaming::emitter_tick_loop(emitter_clone),
        ));

        Self {
            emitter,
            emitter_handle,
            utterance_callback: None,
            vad_stop_callback: None,
            vad_stop_emitted: std::sync::atomic::AtomicBool::new(false),
        }
    }

    pub fn set_utterance_callback(&mut self, cb: Option<Arc<dyn Fn(String) + Send + Sync>>) {
        self.utterance_callback = cb;
    }

    pub fn set_vad_stop_callback(&mut self, cb: Option<Arc<dyn Fn() + Send + Sync>>) {
        self.vad_stop_callback = cb;
    }

    /// Signal the emitter to finish and wait for the tick loop to complete.
    pub async fn finish(&mut self) {
        {
            let mut guard = self.emitter.lock().await;
            guard.finish();
        }

        if let Some(handle) = self.emitter_handle.take()
            && let Err(e) = handle.await
        {
            tracing::error!("Emitter tick loop failed: {}", e);
        }
    }
}

impl EventSink for PresentationEmitter {
    fn on_event(&self, event: &EngineEvent) {
        match event {
            EngineEvent::VadStart { .. } => {
                if !self
                    .vad_stop_emitted
                    .swap(true, std::sync::atomic::Ordering::SeqCst)
                    && let Some(cb) = &self.vad_stop_callback
                {
                    cb();
                }
            }
            EngineEvent::Preview { text, .. } => {
                // Push as segment to the buffered emitter for typing animation.
                // The emitter handles tokenization and paced output.
                let emitter = self.emitter.clone();
                let text = text.clone();
                tokio::spawn(async move {
                    let mut guard = emitter.lock().await;
                    guard.push_segment(text);
                });
            }
            EngineEvent::Correction { text, .. } => {
                let emitter = self.emitter.clone();
                let text = text.clone();
                tokio::spawn(async move {
                    let mut guard = emitter.lock().await;
                    guard.push_correction(text);
                });
            }
            EngineEvent::UtteranceFinal { text, .. } => {
                if let Some(cb) = &self.utterance_callback {
                    let payload = text.trim();
                    if !payload.is_empty() {
                        cb(payload.to_string());
                    }
                }
            }
            EngineEvent::Drop { kind, text, reason } => {
                debug!(
                    "Engine dropped: {:?} — {} (text: '{}')",
                    kind,
                    reason,
                    text.chars().take(50).collect::<String>()
                );
            }
            EngineEvent::Stats {
                hallucination_drops,
                semantic_gate_drops,
                corrections_applied,
                total_utterances,
                dropped_audio_chunks,
            } => {
                info!(
                    "Session stats: utterances={}, hallucinations={}, semantic_gate={}, corrections={}, dropped_chunks={}",
                    total_utterances,
                    hallucination_drops,
                    semantic_gate_drops,
                    corrections_applied,
                    dropped_audio_chunks,
                );
            }
            EngineEvent::Warning { code, message } => {
                tracing::warn!("Engine warning [{}]: {}", code, message);
            }
            _ => {}
        }
    }
}
