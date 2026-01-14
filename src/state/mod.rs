//! State module - conversation tracking and history
//!
//! ## Submodules
//!
//! - `conversation` - Voice Chat session tracking (previous_response_id)
//! - `history` - Transcript history management (~/.codescribe/transcriptions/)
//!
//! Created by M&K (c)2026 VetCoders

pub mod conversation;
pub mod history;

// Re-export main types (public API for tauri-app)
#[allow(unused_imports)] // Public API for external consumers
pub use conversation::{
    get_previous_response_id, reset_conversation, set_response_id,
};
#[allow(unused_imports)] // Public API for external consumers
pub use history::{
    HistoryEntry, latest_entry, open_history_folder, recent_entries, save_audio,
    save_entry, save_entry_with_timestamp,
};
