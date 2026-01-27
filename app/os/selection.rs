//! Selection/context capture for assistive mode (macOS)
//!
//! POC goal:
//! - If user has selected text in the frontmost app, include it as context for Assistive mode.
//! - Avoid clipboard pollution by snapshot+restore.
//! - Best-effort only: failure should never break recording/transcription.

use std::time::Duration;

use tracing::{debug, warn};

use crate::os::clipboard::{self, ClipboardSnapshot};

#[derive(Debug, Clone, Default)]
pub struct AssistiveContext {
    pub frontmost_app: Option<String>,
    pub selected_text: Option<String>,
}

fn env_flag(key: &str, default: bool) -> bool {
    std::env::var(key)
        .ok()
        .map(|v| {
            let v = v.to_lowercase();
            !matches!(v.as_str(), "0" | "false" | "no" | "off")
        })
        .unwrap_or(default)
}

fn env_usize(key: &str, default: usize) -> usize {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(default)
}

/// Capture best-effort context for assistive mode.
///
/// Env knobs (POC):
/// - `ASSISTIVE_CONTEXT_ENABLED` (default: 1)
/// - `ASSISTIVE_CONTEXT_MAX_CHARS` (default: 5000)
/// - `ASSISTIVE_CONTEXT_INCLUDE_APP` (default: 1)
pub fn capture_assistive_context() -> AssistiveContext {
    // Unit tests should not trigger osascript / clipboard / event simulation.
    if cfg!(test) {
        return AssistiveContext::default();
    }

    if !env_flag("ASSISTIVE_CONTEXT_ENABLED", true) {
        return AssistiveContext::default();
    }

    let max_chars = env_usize("ASSISTIVE_CONTEXT_MAX_CHARS", 5000);
    let include_app = env_flag("ASSISTIVE_CONTEXT_INCLUDE_APP", true);

    let frontmost_app = if include_app {
        frontmost_app_name()
    } else {
        None
    };

    // Avoid capturing from ourselves (frontmost can temporarily become CodeScribe)
    if matches!(
        frontmost_app.as_deref(),
        Some("CodeScribe") | Some("codescribe")
    ) {
        debug!("Assistive context: frontmost is CodeScribe, skipping selection capture");
        return AssistiveContext { frontmost_app, selected_text: None };
    }

    let selected_text = copy_selected_text_from_frontmost(max_chars);

    debug!(
        "Assistive context captured (app_present={}, selected_chars={})",
        frontmost_app.is_some(),
        selected_text.as_ref().map(|s| s.len()).unwrap_or(0)
    );

    AssistiveContext {
        frontmost_app,
        selected_text,
    }
}

/// Build the LLM input for assistive mode, including optional selection context.
pub fn build_assistive_input(user_voice_text: &str, ctx: &AssistiveContext) -> String {
    let mut out = String::new();

    if let Some(app) = ctx.frontmost_app.as_deref()
        && !app.trim().is_empty()
    {
        out.push_str("Frontmost app: ");
        out.push_str(app.trim());
        out.push('\n');
    }

    if let Some(sel) = ctx.selected_text.as_deref() {
        let sel = sel.trim();
        if !sel.is_empty() {
            out.push_str("Selected text (context):\n");
            out.push_str("----\n");
            out.push_str(sel);
            out.push_str("\n----\n\n");
        }
    }

    out.push_str("User (voice): ");
    out.push_str(user_voice_text.trim());
    out.push('\n');

    out
}

#[cfg(target_os = "macos")]
fn frontmost_app_name() -> Option<String> {
    use std::process::Command;

    // This is best-effort. It may fail if System Events is restricted.
    let output = Command::new("osascript")
        .args([
            "-e",
            r#"tell application "System Events" to name of first application process whose frontmost is true"#,
        ])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let s = String::from_utf8_lossy(&output.stdout).trim().to_string();
    (!s.is_empty()).then_some(s)
}

#[cfg(not(target_os = "macos"))]
fn frontmost_app_name() -> Option<String> {
    None
}

#[cfg(target_os = "macos")]
fn copy_selected_text_from_frontmost(max_chars: usize) -> Option<String> {
    // Snapshot current clipboard
    let snapshot = ClipboardSnapshot::capture().ok();
    let prev_text = snapshot.as_ref().and_then(|s| s.text.clone());

    // Trigger copy (Cmd+C) in the frontmost app.
    if let Err(e) = clipboard::simulate_cmd_c() {
        warn!("Assistive context: failed to simulate Cmd+C: {}", e);
        return None;
    }

    // Give the app a moment to update the pasteboard.
    std::thread::sleep(Duration::from_millis(80));

    let mut copied = match clipboard::get_clipboard() {
        Ok(t) => t,
        Err(e) => {
            debug!("Assistive context: clipboard read failed: {}", e);
            String::new()
        }
    };

    // Always restore clipboard snapshot (best-effort), regardless of user restore settings.
    if let Some(snapshot) = snapshot {
        if let Err(e) = snapshot.restore() {
            debug!("Assistive context: clipboard restore failed: {}", e);
        }
    }

    copied = copied.trim().to_string();
    if copied.is_empty() {
        return None;
    }

    // Avoid sending stale clipboard as "selection context".
    if let Some(prev) = prev_text {
        if copied == prev.trim() {
            debug!("Assistive context: clipboard unchanged; treating as no selection");
            return None;
        }
    }

    if copied.len() > max_chars {
        copied.truncate(max_chars);
        copied.push('…');
    }

    Some(copied)
}

#[cfg(not(target_os = "macos"))]
fn copy_selected_text_from_frontmost(_max_chars: usize) -> Option<String> {
    None
}
