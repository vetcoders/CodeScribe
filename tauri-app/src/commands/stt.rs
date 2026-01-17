//! STT Commands - Speech-to-Text
//!
//! These commands are DEPRECATED - use IPC commands instead.
//! CLI manages the embedded Whisper model, GUI should use:
//! - ipc_get_status() to check CLI availability
//! - Direct recording through CLI's hotkey system
//!
//! Keeping these stubs for backwards compatibility with existing UI.
//!
//! Created by M&K (c)2026 VetCoders

use crate::state::AppState;
use std::path::PathBuf;

/// Transcribe audio file (DEPRECATED - CLI manages Whisper)
///
/// This function now uses the embedded Whisper singleton.
/// For best results, use CLI's IPC commands.
#[tauri::command]
pub async fn transcribe_audio(
    _state: tauri::State<'_, AppState>,
    audio_path: String,
) -> Result<String, String> {
    let audio_path = PathBuf::from(&audio_path);
    if !audio_path.exists() {
        return Err(format!("Audio file not found: {}", audio_path.display()));
    }

    // Use embedded Whisper singleton
    codescribe::whisper::init().map_err(|e| format!("Failed to init Whisper: {}", e))?;

    let result = codescribe::whisper::transcribe_file(&audio_path, None)
        .map_err(|e| format!("Transcription failed: {}", e))?;

    Ok(result)
}

/// Transcribe with streaming (DEPRECATED - use CLI)
#[tauri::command]
pub async fn transcribe_audio_streaming(
    _state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
    audio_path: String,
) -> Result<String, String> {
    use tauri::Emitter;

    let audio_path = PathBuf::from(&audio_path);
    if !audio_path.exists() {
        return Err(format!("Audio file not found: {}", audio_path.display()));
    }

    // Use embedded Whisper singleton
    codescribe::whisper::init().map_err(|e| format!("Failed to init Whisper: {}", e))?;

    // Load audio
    let (samples, sample_rate) = codescribe::audio::load_audio_file(&audio_path)
        .map_err(|e| format!("Failed to load audio: {}", e))?;

    // Transcribe with streaming callback
    let app_clone = app.clone();
    let callback = move |chunk: &str| {
        let _ = app_clone.emit("transcript_chunk", chunk);
    };

    let result =
        codescribe::whisper::transcribe_streaming(&samples, sample_rate, None, Some(&callback))
            .map_err(|e| format!("Transcription failed: {}", e))?;

    // Emit final result
    let _ = app.emit("transcription_complete", &result);

    Ok(result)
}

/// Get available models (returns embedded model info)
#[tauri::command]
pub fn get_available_models(_state: tauri::State<'_, AppState>) -> Vec<String> {
    // With embedded model, there's only one option
    if codescribe::whisper::embedded::is_embedded_available() {
        vec!["embedded (large-v3-turbo-q8)".to_string()]
    } else {
        vec!["large-v3-turbo".to_string()]
    }
}

/// Get current model name
#[tauri::command]
pub fn get_current_model(_state: tauri::State<'_, AppState>) -> String {
    if codescribe::whisper::embedded::is_embedded_available() {
        "embedded".to_string()
    } else {
        "large-v3-turbo".to_string()
    }
}
