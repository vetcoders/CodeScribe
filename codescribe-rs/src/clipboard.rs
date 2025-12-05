// clipboard.rs
//
// Purpose: Provides clipboard operations and paste simulation for macOS
//
// Dependencies: arboard (clipboard access), enigo (keyboard simulation)
//
// Key Components:
// - paste_text: Save clipboard, set new text, simulate Cmd+V, restore clipboard
// - set_clipboard: Set clipboard content without paste simulation
// - get_clipboard: Retrieve current clipboard content
//
// Design Rationale: Uses arboard for cross-platform clipboard access and enigo
// for keyboard event simulation. Implements clipboard save/restore pattern to
// preserve user's clipboard after paste operations. Includes configurable delay
// for clipboard restoration to avoid race conditions.

use anyhow::{Context, Result};
use enigo::{Enigo, Key, Keyboard, Settings};
use std::thread;
use std::time::Duration;
use tracing::{debug, info, warn};

/// Delay in milliseconds before restoring the original clipboard content
/// Can be overridden via RESTORE_CLIPBOARD_DELAY_MS environment variable
const DEFAULT_RESTORE_DELAY_MS: u64 = 200;

/// Gets the clipboard restore delay from environment or uses default
fn get_restore_delay() -> Duration {
    let delay_ms = std::env::var("RESTORE_CLIPBOARD_DELAY_MS")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(DEFAULT_RESTORE_DELAY_MS);
    Duration::from_millis(delay_ms)
}

/// Checks if clipboard restore is enabled via environment variable
fn is_restore_enabled() -> bool {
    std::env::var("RESTORE_CLIPBOARD")
        .ok()
        .map(|v| {
            let lower = v.to_lowercase();
            !matches!(lower.as_str(), "0" | "false" | "no" | "off")
        })
        .unwrap_or(true) // Default: enabled
}

/// Sets the clipboard content without simulating paste
///
/// # Arguments
/// * `text` - The text to copy to clipboard
///
/// # Errors
/// Returns error if clipboard operation fails
pub fn set_clipboard(text: &str) -> Result<()> {
    if text.is_empty() {
        warn!("Attempted to set clipboard with empty text");
        return Ok(());
    }

    let mut clipboard = arboard::Clipboard::new().context("Failed to initialize clipboard")?;
    clipboard
        .set_text(text)
        .context("Failed to set clipboard text")?;

    debug!("Clipboard set successfully ({} chars)", text.len());
    Ok(())
}

/// Gets the current clipboard content
///
/// # Errors
/// Returns error if clipboard operation fails or clipboard is empty
pub fn get_clipboard() -> Result<String> {
    let mut clipboard = arboard::Clipboard::new().context("Failed to initialize clipboard")?;
    let text = clipboard
        .get_text()
        .context("Failed to get clipboard text")?;

    debug!("Retrieved clipboard content ({} chars)", text.len());
    Ok(text)
}

/// Pastes text into the currently active application
///
/// This function implements a sophisticated paste operation:
/// 1. Saves current clipboard content (if restore is enabled)
/// 2. Sets clipboard to new text
/// 3. Simulates Cmd+V keypress
/// 4. Waits briefly for paste to complete
/// 5. Simulates Right Arrow to deselect pasted text
/// 6. Restores original clipboard content after configurable delay
///
/// The clipboard restore can be disabled by setting RESTORE_CLIPBOARD=0
/// The restore delay can be configured via RESTORE_CLIPBOARD_DELAY_MS
///
/// # Arguments
/// * `text` - The text to paste
///
/// # Errors
/// Returns error if clipboard or keyboard simulation fails
///
/// # Platform Support
/// Currently macOS-only. Uses Cmd modifier for paste simulation.
pub fn paste_text(text: &str) -> Result<()> {
    if text.is_empty() {
        warn!("Paste called with empty text");
        return Ok(());
    }

    info!("Pasting text: '{}...' ({} chars)", &text.chars().take(50).collect::<String>(), text.len());

    // 1. Save current clipboard content if restore is enabled
    let original_clipboard = if is_restore_enabled() {
        match get_clipboard() {
            Ok(content) => {
                debug!("Saved original clipboard ({} chars)", content.len());
                Some(content)
            }
            Err(e) => {
                warn!("Could not save original clipboard: {}", e);
                None
            }
        }
    } else {
        None
    };

    // 2. Set clipboard to new text
    set_clipboard(text).context("Failed to set clipboard for paste")?;
    info!("Text successfully copied to clipboard");

    // 3. Simulate Cmd+V keypress
    let mut enigo = Enigo::new(&Settings::default()).context("Failed to initialize keyboard simulator")?;

    // Use Meta key (Cmd on macOS)
    enigo.key(Key::Meta, enigo::Direction::Press)
        .context("Failed to press Cmd key")?;
    thread::sleep(Duration::from_millis(10));

    enigo.key(Key::Unicode('v'), enigo::Direction::Click)
        .context("Failed to press V key")?;
    thread::sleep(Duration::from_millis(10));

    enigo.key(Key::Meta, enigo::Direction::Release)
        .context("Failed to release Cmd key")?;

    info!("Command+V keypress simulated successfully");

    // 4. Wait for paste to settle
    thread::sleep(Duration::from_millis(50));

    // 5. Simulate Right Arrow to deselect pasted text
    // This prevents the restored clipboard from replacing the pasted text
    enigo.key(Key::RightArrow, enigo::Direction::Click)
        .context("Failed to press Right Arrow key")?;
    debug!("Cleared selection (moved cursor to end)");

    // 6. Optional: restore previous clipboard after delay
    if let Some(original) = original_clipboard {
        let delay = get_restore_delay();
        thread::spawn(move || {
            thread::sleep(delay);
            if let Err(e) = set_clipboard(&original) {
                warn!("Failed to restore clipboard: {}", e);
            } else {
                info!("Clipboard restored to previous contents");
            }
        });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_set_and_get_clipboard() {
        let test_text = "Test clipboard content";
        set_clipboard(test_text).expect("Failed to set clipboard");

        let retrieved = get_clipboard().expect("Failed to get clipboard");
        assert_eq!(retrieved, test_text);
    }

    #[test]
    fn test_empty_clipboard_warning() {
        // Should not panic, just log warning
        let result = set_clipboard("");
        assert!(result.is_ok());
    }

    #[test]
    fn test_restore_delay_default() {
        std::env::remove_var("RESTORE_CLIPBOARD_DELAY_MS");
        assert_eq!(get_restore_delay(), Duration::from_millis(DEFAULT_RESTORE_DELAY_MS));
    }

    #[test]
    fn test_restore_delay_custom() {
        std::env::set_var("RESTORE_CLIPBOARD_DELAY_MS", "500");
        assert_eq!(get_restore_delay(), Duration::from_millis(500));
        std::env::remove_var("RESTORE_CLIPBOARD_DELAY_MS");
    }

    #[test]
    fn test_restore_enabled_default() {
        std::env::remove_var("RESTORE_CLIPBOARD");
        assert!(is_restore_enabled());
    }

    #[test]
    fn test_restore_disabled() {
        std::env::set_var("RESTORE_CLIPBOARD", "0");
        assert!(!is_restore_enabled());
        std::env::remove_var("RESTORE_CLIPBOARD");
    }
}
