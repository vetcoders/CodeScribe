//! System sound playback for macOS
//!
//! Provides simple sound feedback using macOS system sounds.

use tracing::debug;

/// Play a system sound by name
///
/// # Arguments
/// * `name` - Name of the system sound (e.g., "Tink", "Pop", "Glass")
///
/// # Platform Support
/// - macOS: Uses `afplay` with system sounds from `/System/Library/Sounds/`
/// - Other platforms: No-op (silent)
///
/// # Examples
/// ```no_run
/// play_sound("Tink");  // Plays confirmation beep
/// play_sound("Pop");   // Plays pop sound
/// ```
#[cfg(target_os = "macos")]
pub fn play_sound(name: &str) {
    use std::process::Command;

    debug!("Playing system sound: {}", name);

    let path = format!("/System/Library/Sounds/{}.aiff", name);

    // Spawn afplay in background, don't wait for completion
    match Command::new("afplay").arg(&path).spawn() {
        Ok(_) => debug!("Sound playback started: {}", name),
        Err(e) => debug!("Failed to play sound {}: {}", name, e),
    }
}

#[cfg(not(target_os = "macos"))]
pub fn play_sound(_name: &str) {
    // No-op on non-macOS platforms
}
