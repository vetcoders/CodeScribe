//! CodeScribe - Speech-to-text tray app for macOS
//!
//! This is the library interface for CodeScribe components.
//! The main binary is in `main.rs`.

pub mod audio;

// Re-export commonly used types
pub use audio::{Recorder, RecorderConfig, RecorderDiagnostics};
