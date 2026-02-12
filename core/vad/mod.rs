//! Voice Activity Detection (VAD) module using Silero neural network.
//!
//! Custom wrapper that uses a shared ort runtime (no dependency conflicts).
//!
//! ## Quick Start
//!
//! ```ignore
//! use codescribe_core::vad;
//!
//! // Create a local VAD instance at your audio sample rate
//! let mut vad = vad::AccumulatingVad::new(44100)?;
//!
//! // Feed audio chunks — returns speech probability (0.0–1.0)
//! let prob = vad.feed(&audio_samples);
//! ```
//!
//! ## Architecture
//!
//! Each consumer owns its own `AccumulatingVad` (or raw `SileroVad`).
//! No global singletons. Silero VAD requires 16kHz audio —
//! `AccumulatingVad` handles resampling and chunk accumulation internally.
//!
//! Created by M&K (c)2026 VetCoders

pub mod config;
pub mod embedded;
pub mod install;
pub mod silero_ort;

pub use config::VadConfig;
pub use install::{
    SILERO_VAD_FILE, SILERO_VAD_URL, ensure_downloaded_to_user_dir, user_model_path,
    user_models_dir,
};
pub use silero_ort::{AccumulatingVad, Resampler, SileroVad, VAD_SAMPLE_RATE, default_model_path};

/// Expected sample rate for VAD (Silero requires 16kHz)
pub const SAMPLE_RATE: u32 = VAD_SAMPLE_RATE;

/// Recommended chunk size in samples (512 = 32ms at 16kHz)
pub const CHUNK_SIZE: usize = 512;
