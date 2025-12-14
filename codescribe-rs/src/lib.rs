//! CodeScribe - Speech-to-text tray app for macOS
//!
//! This is the library interface for CodeScribe components.
//! The main binary is in `main.rs`.

// Allow unexpected cfgs from objc crate's msg_send! macro
#![allow(unexpected_cfgs)]

pub mod audio;
pub mod clipboard;
pub mod config;
pub mod settings;

#[cfg(target_os = "macos")]
pub mod ui;

// Re-export commonly used types
pub use audio::{Recorder, RecorderConfig, RecorderDiagnostics};

#[cfg(target_os = "macos")]
pub use ui::{
    focused_element_accepts_text, get_caret_position, get_cursor_position, hide_hold_badge,
    show_hold_badge, show_hold_badge_with_config, HoldBadgeConfig,
};
