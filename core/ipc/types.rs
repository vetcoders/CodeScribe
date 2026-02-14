use serde::{Deserialize, Serialize};

use crate::pipeline::contracts::{DropKind, EngineEvent, TranscriptSegment};

#[derive(Debug, Serialize, Deserialize)]
pub enum IpcCommand {
    // Config
    GetConfig,
    SaveConfig {
        config: Box<crate::config::Config>,
    },
    ReloadRuntimeConfig,

    // Prompts
    GetPrompt {
        prompt_type: String,
    },
    SavePrompt {
        prompt_type: String,
        content: String,
    },
    ResetPrompt {
        prompt_type: String,
    },

    // AI / Chat
    SendMessage {
        message: String,
    },
    ResetContext,
    FormatTranscript {
        text: String,
        language: Option<String>,
        assistive: bool,
    },
    TranscribeFile {
        path: String,
    },

    // Status
    GetStatus,

    // Recording
    StartRecording {
        assistive: bool,
    },
    StopRecording,

    // Event stream
    Subscribe,
    Unsubscribe,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum IpcResponse {
    Config(Box<crate::config::Config>),
    Prompt(String),
    Message(String),
    Status(AppStatus),
    Ok,
    Error(String),
    Event(IpcEvent),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppStatus {
    pub state: String, // "idle", "recording", "busy"
    pub ai_formatting: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpcEvent {
    pub timestamp: String, // RFC3339 UTC
    #[serde(flatten)]
    pub payload: IpcEventPayload,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event")]
pub enum IpcEventPayload {
    #[serde(rename = "engine")]
    Engine(EngineEventWire),
    #[serde(rename = "state_change")]
    StateChange { from: String, to: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EngineEventWire {
    VadStart {
        speech_prob: f32,
        ts_ms: u64,
    },
    VadEnd {
        speech_prob: f32,
        ts_ms: u64,
    },
    VadFallback {
        max_prob: f32,
        samples: usize,
    },
    Preview {
        rev: u64,
        text: String,
    },
    Correction {
        rev: u64,
        text: String,
        previous_text: String,
    },
    UtteranceFinal {
        utterance_id: u64,
        text: String,
        start_ts: f32,
        end_ts: f32,
        segments: Vec<TranscriptSegment>,
    },
    Drop {
        kind: String,
        text: String,
        reason: String,
    },
    Stats {
        dropped_audio_chunks: u64,
        hallucination_drops: u64,
        semantic_gate_drops: u64,
        filtered_empty_drops: u64,
        corrections_applied: u64,
        total_utterances: u64,
    },
    Warning {
        code: String,
        message: String,
    },
}

impl From<&EngineEvent> for EngineEventWire {
    fn from(value: &EngineEvent) -> Self {
        match value {
            EngineEvent::VadStart { speech_prob, ts_ms } => Self::VadStart {
                speech_prob: *speech_prob,
                ts_ms: *ts_ms,
            },
            EngineEvent::VadEnd { speech_prob, ts_ms } => Self::VadEnd {
                speech_prob: *speech_prob,
                ts_ms: *ts_ms,
            },
            EngineEvent::VadFallback { max_prob, samples } => Self::VadFallback {
                max_prob: *max_prob,
                samples: *samples,
            },
            EngineEvent::Preview { rev, text } => Self::Preview {
                rev: *rev,
                text: text.clone(),
            },
            EngineEvent::Correction {
                rev,
                text,
                previous_text,
            } => Self::Correction {
                rev: *rev,
                text: text.clone(),
                previous_text: previous_text.clone(),
            },
            EngineEvent::UtteranceFinal {
                utterance_id,
                text,
                start_ts,
                end_ts,
                segments,
                ..
            } => Self::UtteranceFinal {
                utterance_id: *utterance_id,
                text: text.clone(),
                start_ts: *start_ts,
                end_ts: *end_ts,
                segments: segments.clone(),
            },
            EngineEvent::Drop { kind, text, reason } => Self::Drop {
                kind: drop_kind_to_wire(kind).to_string(),
                text: text.clone(),
                reason: reason.clone(),
            },
            EngineEvent::Stats {
                dropped_audio_chunks,
                hallucination_drops,
                semantic_gate_drops,
                filtered_empty_drops,
                corrections_applied,
                total_utterances,
            } => Self::Stats {
                dropped_audio_chunks: *dropped_audio_chunks,
                hallucination_drops: *hallucination_drops,
                semantic_gate_drops: *semantic_gate_drops,
                filtered_empty_drops: *filtered_empty_drops,
                corrections_applied: *corrections_applied,
                total_utterances: *total_utterances,
            },
            EngineEvent::Warning { code, message } => Self::Warning {
                code: code.clone(),
                message: message.clone(),
            },
        }
    }
}

fn drop_kind_to_wire(kind: &DropKind) -> &'static str {
    match kind {
        DropKind::Hallucination => "hallucination",
        DropKind::SemanticGate => "semantic_gate",
        DropKind::OverlapEmpty => "overlap_empty",
        DropKind::FilteredEmpty => "filtered_empty",
    }
}
