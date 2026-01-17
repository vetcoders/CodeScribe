//! Window management for CodeScribe Tauri app
//!
//! Handles showing/hiding the main window.
//! These functions are kept for future use when CLI communicates with GUI.
//!
//! Created by M&K (c)2026 VetCoders

#![allow(dead_code)]

use tauri::{AppHandle, Manager};

/// Toggle the main window visibility
///
/// If visible -> hide, if hidden -> show and focus
pub fn toggle_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        if window.is_visible().unwrap_or(false) {
            let _ = window.hide();
        } else {
            let _ = window.show();
            let _ = window.set_focus();
        }
    }
}

/// Show the main window and bring to front
pub fn show_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

/// Hide the main window
pub fn hide_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }
}
