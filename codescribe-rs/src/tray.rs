//! System tray icon and menu for CodeScribe
//!
//! Provides visual status feedback and menu controls via macOS menu bar icon.

use anyhow::Result;
use muda::{Menu, MenuItem, PredefinedMenuItem, Submenu};
use tray_icon::{
    menu::MenuEvent, Icon, TrayIconBuilder,
};
use tracing::{debug, info};

/// Status of the CodeScribe system, reflected in tray icon glyph
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayStatus {
    /// Idle, waiting for activation
    Idle,
    /// Actively listening/recording
    Listening,
    /// Processing/transcribing
    Thinking,
    /// Successfully completed
    Success,
}

impl TrayStatus {
    /// Get the unicode glyph for this status
    pub fn glyph(&self) -> &'static str {
        match self {
            TrayStatus::Idle => "•",
            TrayStatus::Listening => "◉",
            TrayStatus::Thinking => "…",
            TrayStatus::Success => "✓",
        }
    }

    /// Create an icon from this status
    fn to_icon(&self) -> Result<Icon> {
        // Create a minimal 16x16 RGBA icon (transparent)
        // TODO: Generate proper icon images or use pre-made icon files
        // For now, using transparent icons - the tooltip will show status

        let rgba = vec![0u8; 16 * 16 * 4];

        Icon::from_rgba(rgba, 16, 16)
            .map_err(|e| anyhow::anyhow!("Failed to create icon: {}", e))
    }
}

/// Build the tray menu
fn build_menu() -> Result<Menu> {
    let menu = Menu::new();

    // Status label (disabled)
    let status_item = MenuItem::new("Status: Ready", false, None);
    menu.append(&status_item)?;

    // Separator
    menu.append(&PredefinedMenuItem::separator())?;

    // Language submenu
    let lang_menu = Submenu::new("Language", true);
    let lang_auto = MenuItem::new("Auto", true, None);
    let lang_polish = MenuItem::new("Polish", true, None);
    let lang_english = MenuItem::new("English", true, None);

    lang_menu.append(&lang_auto)?;
    lang_menu.append(&lang_polish)?;
    lang_menu.append(&lang_english)?;
    menu.append(&lang_menu)?;

    // Separator
    menu.append(&PredefinedMenuItem::separator())?;

    // Quit
    let quit_item = MenuItem::new("Quit", true, None);
    menu.append(&quit_item)?;

    Ok(menu)
}

/// Update the tray icon to reflect current status
///
/// Note: This is a placeholder for now. In the future, we'll need to implement
/// a channel-based communication system to update the tray from other threads.
pub fn set_status(_status: TrayStatus) -> Result<()> {
    // TODO: Implement channel-based status updates
    // For now, this is a no-op since TrayIcon is not Send/Sync
    debug!("set_status called (not yet implemented for thread safety)");
    Ok(())
}

/// Run the tray application (blocking)
///
/// This function creates the system tray icon, sets up the menu,
/// and runs the event loop. It will block until the application quits.
pub fn run() -> Result<()> {
    info!("Initializing system tray...");

    // Build the menu
    let menu = build_menu()?;

    // Create initial icon
    let icon = TrayStatus::Idle.to_icon()?;

    // Build the tray icon
    let _tray_icon = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("CodeScribe - •")
        .with_icon(icon)
        .build()?;

    info!("System tray initialized");

    // Get menu event receiver
    let menu_channel = MenuEvent::receiver();

    // Run event loop
    info!("Starting tray event loop...");
    info!("Press Quit in the tray menu to exit");

    loop {
        // Check for menu events
        if let Ok(event) = menu_channel.try_recv() {
            debug!("Menu event received: {:?}", event);

            // For now, just log all events
            // TODO: Implement proper event handling based on menu item IDs
            // The challenge is that muda 0.15 doesn't expose IDs easily

            info!("Menu event: {:?}", event);

            // Check if this might be a quit event
            // We'll need to implement a proper way to detect this
        }

        // Sleep to avoid busy-waiting
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_glyphs() {
        assert_eq!(TrayStatus::Idle.glyph(), "•");
        assert_eq!(TrayStatus::Listening.glyph(), "◉");
        assert_eq!(TrayStatus::Thinking.glyph(), "…");
        assert_eq!(TrayStatus::Success.glyph(), "✓");
    }

    #[test]
    fn test_icon_creation() {
        let icon = TrayStatus::Idle.to_icon();
        assert!(icon.is_ok());
    }
}
