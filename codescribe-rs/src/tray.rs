//! System tray icon and menu for CodeScribe
//!
//! Provides visual status feedback and menu controls via macOS menu bar icon.
//! Uses tao event loop for proper macOS integration.
//!
//! Note: Some menu event handlers are not yet wired up (pending integration)
#![allow(dead_code)]

use anyhow::Result;
use crossbeam_channel::{unbounded, Receiver, Sender, TryRecvError};
use image::{imageops::FilterType, GenericImageView};
use muda::{CheckMenuItem, Menu, MenuId, MenuItem, PredefinedMenuItem, Submenu};
use std::sync::OnceLock;
use std::time::{Duration, Instant};
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tracing::{debug, info};
use tray_icon::{menu::MenuEvent, Icon, TrayIconBuilder};

/// Embedded CodeScribe logo icon (resized for menu bar)
/// Place icon.png in codescribe-rs/assets/ directory
const ICON_BYTES: &[u8] = include_bytes!("../assets/icon.png");

/// Menu bar icon size (44x44 for Retina, 22x22 logical)
const ICON_SIZE: u32 = 44;

/// Load the custom CodeScribe icon, optionally tinted by status
fn load_custom_icon(status: TrayStatus) -> Result<Icon> {
    let img = image::load_from_memory(ICON_BYTES)
        .map_err(|e| anyhow::anyhow!("Failed to load icon: {}", e))?;

    // Resize to menu bar size (44x44 for Retina)
    let resized = img.resize_exact(ICON_SIZE, ICON_SIZE, FilterType::Lanczos3);
    let (width, height) = resized.dimensions();
    let mut rgba = resized.to_rgba8().into_raw();

    // Apply color tint based on status (multiply blend)
    let (tint_r, tint_g, tint_b): (f32, f32, f32) = match status {
        TrayStatus::Idle => (1.0, 1.0, 1.0), // No tint (white/original)
        TrayStatus::Listening => (1.0, 0.3, 0.3), // Red tint
        TrayStatus::Thinking => (0.3, 0.6, 1.0), // Blue tint
        TrayStatus::Success => (0.3, 1.0, 0.5), // Green tint
    };

    // Apply tint to each pixel
    for pixel in rgba.chunks_exact_mut(4) {
        pixel[0] = (pixel[0] as f32 * tint_r).min(255.0) as u8;
        pixel[1] = (pixel[1] as f32 * tint_g).min(255.0) as u8;
        pixel[2] = (pixel[2] as f32 * tint_b).min(255.0) as u8;
        // Alpha channel (pixel[3]) unchanged
    }

    Icon::from_rgba(rgba, width, height)
        .map_err(|e| anyhow::anyhow!("Failed to create icon: {}", e))
}

/// Create a simple colored circle icon as fallback
fn create_fallback_icon(status: TrayStatus) -> Result<Icon> {
    const SIZE: u32 = 22;
    const RADIUS: i32 = 10;
    const CENTER: i32 = 11;

    let (r, g, b) = match status {
        TrayStatus::Idle => (100u8, 100, 100),  // Gray
        TrayStatus::Listening => (220, 60, 60), // Red
        TrayStatus::Thinking => (60, 130, 220), // Blue
        TrayStatus::Success => (60, 200, 100),  // Green
    };

    let mut rgba = vec![0u8; (SIZE * SIZE * 4) as usize];

    for y in 0..SIZE as i32 {
        for x in 0..SIZE as i32 {
            let dx = x - CENTER;
            let dy = y - CENTER;
            if dx * dx + dy * dy <= RADIUS * RADIUS {
                let idx = ((y as u32 * SIZE + x as u32) * 4) as usize;
                rgba[idx] = r;
                rgba[idx + 1] = g;
                rgba[idx + 2] = b;
                rgba[idx + 3] = 255;
            }
        }
    }

    Icon::from_rgba(rgba, SIZE, SIZE)
        .map_err(|e| anyhow::anyhow!("Failed to create fallback icon: {}", e))
}

/// Status of the CodeScribe system, reflected in tray icon
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
    /// Get the human-readable tooltip for this status
    pub fn tooltip(&self) -> String {
        match self {
            TrayStatus::Idle => "CodeScribe - Ready".to_string(),
            TrayStatus::Listening => "CodeScribe - Recording...".to_string(),
            TrayStatus::Thinking => "CodeScribe - Processing...".to_string(),
            TrayStatus::Success => "CodeScribe - Done!".to_string(),
        }
    }

    /// Create an icon from this status using the custom CodeScribe logo
    /// Falls back to simple circle if custom icon fails
    fn to_icon(self) -> Result<Icon> {
        load_custom_icon(self).or_else(|e| {
            debug!("Custom icon failed, using fallback: {}", e);
            create_fallback_icon(self)
        })
    }
}

/// Menu events that can be sent to the main controller
#[derive(Debug, Clone)]
pub enum TrayMenuEvent {
    // Top-level actions
    ToggleHotkeys,
    StartAtLogin(bool),
    Quit,

    // Language submenu
    SetLanguage(Language),

    // Formatting submenu
    SetFormattingProvider(FormattingProvider),

    // Hold Hotkeys submenu
    SetHoldMods(HoldMods),
    ToggleHoldExclusive,
    SetToggleTrigger(ToggleTrigger),

    // History submenu
    ToggleHistory,
    CopyLatestToClipboard,
    OpenHistoryFolder,
    SelectHistoryEntry(usize),

    // Appearance submenu
    ToggleStatusGlyph,
    RefreshTrayIcon,

    // Feedback submenu
    ToggleStartSound,
    SetSoundType(SoundType),
    SetVolume,

    // Permissions submenu
    CheckPermissions,
    OpenAccessibilitySettings,
    OpenMicrophoneSettings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    Auto,
    Polish,
    English,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormattingProvider {
    Harmony,
    Ollama,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HoldMods {
    Ctrl,
    CtrlOption,
    CtrlShift,
    CtrlCommand,
}

impl HoldMods {
    fn label(&self) -> &str {
        match self {
            HoldMods::Ctrl => "Ctrl only (Formatting)",
            HoldMods::CtrlOption => "Ctrl+Option",
            HoldMods::CtrlShift => "Ctrl+Shift (AI)",
            HoldMods::CtrlCommand => "Ctrl+Command",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToggleTrigger {
    DoubleOption,
    DoubleRightOption,
    Disabled,
}

impl ToggleTrigger {
    fn label(&self) -> &str {
        match self {
            ToggleTrigger::DoubleOption => "double option",
            ToggleTrigger::DoubleRightOption => "double right option",
            ToggleTrigger::Disabled => "disabled",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SoundType {
    Tink,
    Pop,
}

/// Menu item IDs for tracking all clickable items
struct MenuIds {
    // Top-level
    enable_hotkeys: MenuId,
    start_at_login: MenuId,
    quit: MenuId,

    // Language submenu
    lang_auto: MenuId,
    lang_polish: MenuId,
    lang_english: MenuId,

    // Formatting submenu
    fmt_harmony: MenuId,
    fmt_ollama: MenuId,

    // Hold Hotkeys submenu
    hold_ctrl: MenuId,
    hold_ctrl_opt: MenuId,
    hold_ctrl_shift: MenuId,
    hold_ctrl_cmd: MenuId,
    hold_exclusive: MenuId,
    toggle_double_opt: MenuId,
    toggle_double_ralt: MenuId,
    toggle_disabled: MenuId,

    // History submenu
    history_save: MenuId,
    history_copy_latest: MenuId,
    history_open_folder: MenuId,

    // Appearance submenu
    appearance_glyph: MenuId,
    appearance_refresh: MenuId,

    // Feedback submenu
    feedback_start_sound: MenuId,
    feedback_sound_tink: MenuId,
    feedback_sound_pop: MenuId,
    feedback_set_volume: MenuId,

    // Permissions submenu
    perm_check: MenuId,
    perm_accessibility: MenuId,
    perm_microphone: MenuId,
}

/// Build the complete tray menu with all submenus
fn build_menu() -> Result<(Menu, MenuIds)> {
    let menu = Menu::new();

    // 1. Status: Ready (disabled label)
    let status_item = MenuItem::new("Status: Ready", false, None);
    menu.append(&status_item)?;

    // 2. Enable Hotkeys (checkbox toggle)
    let enable_hotkeys = CheckMenuItem::new("Enable Hotkeys", true, true, None);
    let enable_hotkeys_id = enable_hotkeys.id().clone();
    menu.append(&enable_hotkeys)?;

    // 3. Separator
    menu.append(&PredefinedMenuItem::separator())?;

    // 4. Language submenu
    let lang_menu = Submenu::new("Language", true);
    let lang_auto = CheckMenuItem::new("✓ Auto", true, true, None);
    let lang_auto_id = lang_auto.id().clone();
    let lang_polish = CheckMenuItem::new("Polish (PL)", true, false, None);
    let lang_polish_id = lang_polish.id().clone();
    let lang_english = CheckMenuItem::new("English (EN)", true, false, None);
    let lang_english_id = lang_english.id().clone();

    lang_menu.append(&lang_auto)?;
    lang_menu.append(&lang_polish)?;
    lang_menu.append(&lang_english)?;
    menu.append(&lang_menu)?;

    // 5. Formatting submenu
    let fmt_menu = Submenu::new("Formatting", true);
    let fmt_provider_label = MenuItem::new("Provider", false, None);
    fmt_menu.append(&fmt_provider_label)?;
    let fmt_harmony = CheckMenuItem::new("✓ Harmony", true, true, None);
    let fmt_harmony_id = fmt_harmony.id().clone();
    let fmt_ollama = CheckMenuItem::new("Ollama", true, false, None);
    let fmt_ollama_id = fmt_ollama.id().clone();

    fmt_menu.append(&fmt_harmony)?;
    fmt_menu.append(&fmt_ollama)?;
    menu.append(&fmt_menu)?;

    // 6. Hold Hotkeys submenu
    let hold_menu = Submenu::new("Hold Hotkeys", true);

    // Current: [label]
    let hold_current_label = MenuItem::new("Current: Ctrl only (Formatting)", false, None);
    hold_menu.append(&hold_current_label)?;
    hold_menu.append(&PredefinedMenuItem::separator())?;

    // Hold modifier options
    let hold_ctrl = CheckMenuItem::new("Hold: Ctrl only (Formatting)", true, true, None);
    let hold_ctrl_id = hold_ctrl.id().clone();
    let hold_ctrl_opt = CheckMenuItem::new("Hold: Ctrl+Option", true, false, None);
    let hold_ctrl_opt_id = hold_ctrl_opt.id().clone();
    let hold_ctrl_shift = CheckMenuItem::new("Hold: Ctrl+Shift (AI)", true, false, None);
    let hold_ctrl_shift_id = hold_ctrl_shift.id().clone();
    let hold_ctrl_cmd = CheckMenuItem::new("Hold: Ctrl+Command", true, false, None);
    let hold_ctrl_cmd_id = hold_ctrl_cmd.id().clone();

    hold_menu.append(&hold_ctrl)?;
    hold_menu.append(&hold_ctrl_opt)?;
    hold_menu.append(&hold_ctrl_shift)?;
    hold_menu.append(&hold_ctrl_cmd)?;
    hold_menu.append(&PredefinedMenuItem::separator())?;

    // Exclusive checkbox
    let hold_exclusive = CheckMenuItem::new("Exclusive (ignore extra modifiers)", true, true, None);
    let hold_exclusive_id = hold_exclusive.id().clone();
    hold_menu.append(&hold_exclusive)?;
    hold_menu.append(&PredefinedMenuItem::separator())?;

    // Toggle trigger options
    let toggle_label = MenuItem::new("Toggle: double option", false, None);
    hold_menu.append(&toggle_label)?;
    let toggle_double_opt = CheckMenuItem::new("Use double Option (⌥⌥)", true, true, None);
    let toggle_double_opt_id = toggle_double_opt.id().clone();
    let toggle_double_ralt = CheckMenuItem::new("Use double Right Option", true, false, None);
    let toggle_double_ralt_id = toggle_double_ralt.id().clone();
    let toggle_disabled = CheckMenuItem::new("Disable toggle", true, false, None);
    let toggle_disabled_id = toggle_disabled.id().clone();

    hold_menu.append(&toggle_double_opt)?;
    hold_menu.append(&toggle_double_ralt)?;
    hold_menu.append(&toggle_disabled)?;

    menu.append(&hold_menu)?;

    // 7. History submenu
    let history_menu = Submenu::new("History", true);

    let history_latest_label = MenuItem::new("Latest: (none)", false, None);
    history_menu.append(&history_latest_label)?;
    history_menu.append(&PredefinedMenuItem::separator())?;

    let history_save = CheckMenuItem::new("Save transcripts to History", true, true, None);
    let history_save_id = history_save.id().clone();
    history_menu.append(&history_save)?;
    history_menu.append(&PredefinedMenuItem::separator())?;

    // Placeholder for recent entries (would be dynamically populated)
    let placeholder_entry = MenuItem::new("(no recent entries)", false, None);
    history_menu.append(&placeholder_entry)?;
    history_menu.append(&PredefinedMenuItem::separator())?;

    let history_copy_latest = MenuItem::new("Copy Latest to Clipboard", true, None);
    let history_copy_latest_id = history_copy_latest.id().clone();
    let history_open_folder = MenuItem::new("Open History Folder", true, None);
    let history_open_folder_id = history_open_folder.id().clone();

    history_menu.append(&history_copy_latest)?;
    history_menu.append(&history_open_folder)?;

    menu.append(&history_menu)?;

    // 8. Appearance submenu
    let appearance_menu = Submenu::new("Appearance", true);

    let appearance_glyph = CheckMenuItem::new("Show status glyph next to icon", true, true, None);
    let appearance_glyph_id = appearance_glyph.id().clone();
    appearance_menu.append(&appearance_glyph)?;
    appearance_menu.append(&PredefinedMenuItem::separator())?;

    let appearance_refresh = MenuItem::new("Refresh Tray Icon", true, None);
    let appearance_refresh_id = appearance_refresh.id().clone();
    appearance_menu.append(&appearance_refresh)?;

    menu.append(&appearance_menu)?;

    // 9. Feedback submenu
    let feedback_menu = Submenu::new("Feedback", true);

    let feedback_start_sound = CheckMenuItem::new("Enable Start Sound", true, true, None);
    let feedback_start_sound_id = feedback_start_sound.id().clone();
    feedback_menu.append(&feedback_start_sound)?;
    feedback_menu.append(&PredefinedMenuItem::separator())?;

    let feedback_sound_tink = CheckMenuItem::new("Sound: Tink", true, true, None);
    let feedback_sound_tink_id = feedback_sound_tink.id().clone();
    let feedback_sound_pop = CheckMenuItem::new("Sound: Pop", true, false, None);
    let feedback_sound_pop_id = feedback_sound_pop.id().clone();
    feedback_menu.append(&feedback_sound_tink)?;
    feedback_menu.append(&feedback_sound_pop)?;

    let feedback_set_volume = MenuItem::new("Set Volume...", true, None);
    let feedback_set_volume_id = feedback_set_volume.id().clone();
    feedback_menu.append(&feedback_set_volume)?;

    menu.append(&feedback_menu)?;

    // 10. Permissions submenu
    let permissions_menu = Submenu::new("Permissions", true);

    // Status display using permission check functions
    let ax_status = if crate::permissions::check_accessibility()
        == crate::permissions::PermissionStatus::Granted
    {
        "✓"
    } else {
        "✗"
    };
    let mic_status = match crate::permissions::check_microphone() {
        crate::permissions::PermissionStatus::Granted => "✓",
        crate::permissions::PermissionStatus::NotDetermined => "?",
        _ => "✗",
    };
    let perm_status_label = MenuItem::new(
        format!("AX: {} | Mic: {}", ax_status, mic_status),
        false,
        None,
    );
    permissions_menu.append(&perm_status_label)?;
    permissions_menu.append(&PredefinedMenuItem::separator())?;

    let perm_check = MenuItem::new("Check Permissions Now", true, None);
    let perm_check_id = perm_check.id().clone();
    permissions_menu.append(&perm_check)?;
    permissions_menu.append(&PredefinedMenuItem::separator())?;

    let perm_accessibility = MenuItem::new("Open Accessibility Settings", true, None);
    let perm_accessibility_id = perm_accessibility.id().clone();
    permissions_menu.append(&perm_accessibility)?;

    let perm_microphone = MenuItem::new("Open Microphone Settings", true, None);
    let perm_microphone_id = perm_microphone.id().clone();
    permissions_menu.append(&perm_microphone)?;

    menu.append(&permissions_menu)?;

    // 11. Separator
    menu.append(&PredefinedMenuItem::separator())?;

    // 12. Start at Login (checkbox)
    let start_at_login = CheckMenuItem::new("Start at Login", true, false, None);
    let start_at_login_id = start_at_login.id().clone();
    menu.append(&start_at_login)?;

    // 13. Quit
    let quit_item = MenuItem::new("Quit", true, None);
    let quit_id = quit_item.id().clone();
    menu.append(&quit_item)?;

    Ok((
        menu,
        MenuIds {
            enable_hotkeys: enable_hotkeys_id,
            start_at_login: start_at_login_id,
            quit: quit_id,
            lang_auto: lang_auto_id,
            lang_polish: lang_polish_id,
            lang_english: lang_english_id,
            fmt_harmony: fmt_harmony_id,
            fmt_ollama: fmt_ollama_id,
            hold_ctrl: hold_ctrl_id,
            hold_ctrl_opt: hold_ctrl_opt_id,
            hold_ctrl_shift: hold_ctrl_shift_id,
            hold_ctrl_cmd: hold_ctrl_cmd_id,
            hold_exclusive: hold_exclusive_id,
            toggle_double_opt: toggle_double_opt_id,
            toggle_double_ralt: toggle_double_ralt_id,
            toggle_disabled: toggle_disabled_id,
            history_save: history_save_id,
            history_copy_latest: history_copy_latest_id,
            history_open_folder: history_open_folder_id,
            appearance_glyph: appearance_glyph_id,
            appearance_refresh: appearance_refresh_id,
            feedback_start_sound: feedback_start_sound_id,
            feedback_sound_tink: feedback_sound_tink_id,
            feedback_sound_pop: feedback_sound_pop_id,
            feedback_set_volume: feedback_set_volume_id,
            perm_check: perm_check_id,
            perm_accessibility: perm_accessibility_id,
            perm_microphone: perm_microphone_id,
        },
    ))
}

/// Global channel for status updates (crossbeam for sync safety)
static STATUS_CHANNEL: OnceLock<Sender<TrayStatus>> = OnceLock::new();

/// Global channel for menu events
static MENU_EVENT_CHANNEL: OnceLock<Sender<TrayMenuEvent>> = OnceLock::new();

/// Update the tray icon to reflect current status
pub fn update_tray_status(status: TrayStatus) -> Result<()> {
    if let Some(sender) = STATUS_CHANNEL.get() {
        sender
            .send(status)
            .map_err(|e| anyhow::anyhow!("Failed to send tray status: {}", e))?;
        debug!("Tray status update sent: {:?}", status);
        Ok(())
    } else {
        debug!("Tray status channel not initialized yet");
        Ok(())
    }
}

/// Get a receiver for menu events (call once from main controller)
pub fn menu_event_receiver() -> Result<Receiver<TrayMenuEvent>> {
    let (tx, rx) = unbounded();
    MENU_EVENT_CHANNEL
        .set(tx)
        .map_err(|_| anyhow::anyhow!("Menu event channel already initialized"))?;
    Ok(rx)
}

/// Send a menu event to the main controller
fn send_menu_event(event: TrayMenuEvent) {
    if let Some(sender) = MENU_EVENT_CHANNEL.get() {
        if let Err(e) = sender.send(event) {
            debug!("Failed to send menu event: {}", e);
        }
    }
}

/// Handle menu item click and send appropriate event
fn handle_menu_event(event_id: &MenuId, menu_ids: &MenuIds) {
    // Top-level actions
    if event_id == &menu_ids.enable_hotkeys {
        send_menu_event(TrayMenuEvent::ToggleHotkeys);
    } else if event_id == &menu_ids.start_at_login {
        // Determine new state (would need to query checkbox state in real implementation)
        send_menu_event(TrayMenuEvent::StartAtLogin(true));
    } else if event_id == &menu_ids.quit {
        send_menu_event(TrayMenuEvent::Quit);
    }
    // Language submenu
    else if event_id == &menu_ids.lang_auto {
        send_menu_event(TrayMenuEvent::SetLanguage(Language::Auto));
    } else if event_id == &menu_ids.lang_polish {
        send_menu_event(TrayMenuEvent::SetLanguage(Language::Polish));
    } else if event_id == &menu_ids.lang_english {
        send_menu_event(TrayMenuEvent::SetLanguage(Language::English));
    }
    // Formatting submenu
    else if event_id == &menu_ids.fmt_harmony {
        send_menu_event(TrayMenuEvent::SetFormattingProvider(
            FormattingProvider::Harmony,
        ));
    } else if event_id == &menu_ids.fmt_ollama {
        send_menu_event(TrayMenuEvent::SetFormattingProvider(
            FormattingProvider::Ollama,
        ));
    }
    // Hold Hotkeys submenu
    else if event_id == &menu_ids.hold_ctrl {
        send_menu_event(TrayMenuEvent::SetHoldMods(HoldMods::Ctrl));
    } else if event_id == &menu_ids.hold_ctrl_opt {
        send_menu_event(TrayMenuEvent::SetHoldMods(HoldMods::CtrlOption));
    } else if event_id == &menu_ids.hold_ctrl_shift {
        send_menu_event(TrayMenuEvent::SetHoldMods(HoldMods::CtrlShift));
    } else if event_id == &menu_ids.hold_ctrl_cmd {
        send_menu_event(TrayMenuEvent::SetHoldMods(HoldMods::CtrlCommand));
    } else if event_id == &menu_ids.hold_exclusive {
        send_menu_event(TrayMenuEvent::ToggleHoldExclusive);
    } else if event_id == &menu_ids.toggle_double_opt {
        send_menu_event(TrayMenuEvent::SetToggleTrigger(ToggleTrigger::DoubleOption));
    } else if event_id == &menu_ids.toggle_double_ralt {
        send_menu_event(TrayMenuEvent::SetToggleTrigger(
            ToggleTrigger::DoubleRightOption,
        ));
    } else if event_id == &menu_ids.toggle_disabled {
        send_menu_event(TrayMenuEvent::SetToggleTrigger(ToggleTrigger::Disabled));
    }
    // History submenu
    else if event_id == &menu_ids.history_save {
        send_menu_event(TrayMenuEvent::ToggleHistory);
    } else if event_id == &menu_ids.history_copy_latest {
        send_menu_event(TrayMenuEvent::CopyLatestToClipboard);
    } else if event_id == &menu_ids.history_open_folder {
        send_menu_event(TrayMenuEvent::OpenHistoryFolder);
    }
    // Appearance submenu
    else if event_id == &menu_ids.appearance_glyph {
        send_menu_event(TrayMenuEvent::ToggleStatusGlyph);
    } else if event_id == &menu_ids.appearance_refresh {
        send_menu_event(TrayMenuEvent::RefreshTrayIcon);
    }
    // Feedback submenu
    else if event_id == &menu_ids.feedback_start_sound {
        send_menu_event(TrayMenuEvent::ToggleStartSound);
    } else if event_id == &menu_ids.feedback_sound_tink {
        send_menu_event(TrayMenuEvent::SetSoundType(SoundType::Tink));
    } else if event_id == &menu_ids.feedback_sound_pop {
        send_menu_event(TrayMenuEvent::SetSoundType(SoundType::Pop));
    } else if event_id == &menu_ids.feedback_set_volume {
        send_menu_event(TrayMenuEvent::SetVolume);
    }
    // Permissions submenu
    else if event_id == &menu_ids.perm_check {
        send_menu_event(TrayMenuEvent::CheckPermissions);
        // Also log current status immediately
        crate::permissions::check_all_permissions();
    } else if event_id == &menu_ids.perm_accessibility {
        send_menu_event(TrayMenuEvent::OpenAccessibilitySettings);
        // Open System Settings > Privacy & Security > Accessibility
        #[cfg(target_os = "macos")]
        {
            use std::process::Command;
            let _ = Command::new("open")
                .arg(
                    "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility",
                )
                .spawn();
        }
    } else if event_id == &menu_ids.perm_microphone {
        send_menu_event(TrayMenuEvent::OpenMicrophoneSettings);
        // Open System Settings > Privacy & Security > Microphone
        #[cfg(target_os = "macos")]
        {
            use std::process::Command;
            let _ = Command::new("open")
                .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone")
                .spawn();
        }
    }
}

/// Run the tray application (blocking)
///
/// Uses tao event loop for proper macOS integration.
/// Optionally accepts a HotkeyManager to process hotkey events in the same loop.
pub fn run() -> Result<()> {
    run_with_hotkeys(None)
}

/// Run the tray application with optional hotkey manager
///
/// The hotkey manager must be created on main thread before calling this.
pub fn run_with_hotkeys(hotkey_manager: Option<crate::hotkeys::HotkeyManager>) -> Result<()> {
    info!("Initializing system tray...");

    // Create channel for status updates (crossbeam for sync safety)
    let (status_tx, status_rx): (Sender<TrayStatus>, Receiver<TrayStatus>) = unbounded();
    STATUS_CHANNEL
        .set(status_tx)
        .map_err(|_| anyhow::anyhow!("Status channel already initialized"))?;

    // Build event loop (must be on main thread for macOS)
    let event_loop = EventLoopBuilder::new().build();

    // Build the menu and get IDs
    let (menu, menu_ids) = build_menu()?;

    // Create initial icon
    let initial_status = TrayStatus::Idle;
    let icon = initial_status.to_icon()?;

    // Build the tray icon
    let tray_icon = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip(initial_status.tooltip())
        .with_icon(icon)
        .build()?;

    info!("System tray initialized");

    // Get menu event receiver
    let menu_channel = MenuEvent::receiver();

    if hotkey_manager.is_some() {
        info!("Global hotkeys enabled");
    }

    info!("Starting tray event loop...");
    info!("Press Quit in the tray menu to exit");

    // Poll interval for checking channels
    let poll_interval = Duration::from_millis(100);

    // Run the event loop
    event_loop.run(move |_event, _, control_flow| {
        // Use WaitUntil to avoid busy-waiting while still checking channels
        *control_flow = ControlFlow::WaitUntil(Instant::now() + poll_interval);

        // Process hotkey events (integrated with main event loop for macOS compatibility)
        if let Some(ref hk_manager) = hotkey_manager {
            hk_manager.process_events();
        }

        // Check for status updates (non-blocking)
        match status_rx.try_recv() {
            Ok(new_status) => {
                debug!("Received status update: {:?}", new_status);

                // Update tooltip
                if let Err(e) = tray_icon.set_tooltip(Some(new_status.tooltip())) {
                    debug!("Failed to update tray tooltip: {}", e);
                }

                // Update icon
                if let Ok(new_icon) = new_status.to_icon() {
                    if let Err(e) = tray_icon.set_icon(Some(new_icon)) {
                        debug!("Failed to update tray icon: {}", e);
                    }
                }

                info!("Tray status updated to: {:?}", new_status);
            }
            Err(TryRecvError::Empty) => {}
            Err(TryRecvError::Disconnected) => {
                info!("Status channel closed, exiting");
                *control_flow = ControlFlow::Exit;
            }
        }

        // Check for menu events (non-blocking)
        if let Ok(event) = menu_channel.try_recv() {
            debug!("Menu event: {:?}", event);

            // Handle menu item clicks
            handle_menu_event(&event.id, &menu_ids);

            // Handle Quit specially to exit event loop
            if event.id == menu_ids.quit {
                info!("Quit requested, exiting...");
                *control_flow = ControlFlow::Exit;
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_icon_creation() {
        let icon = TrayStatus::Idle.to_icon();
        assert!(icon.is_ok());
    }

    #[test]
    fn test_status_tooltips() {
        assert_eq!(TrayStatus::Idle.tooltip(), "CodeScribe - Ready");
        assert_eq!(TrayStatus::Listening.tooltip(), "CodeScribe - Recording...");
        assert_eq!(TrayStatus::Thinking.tooltip(), "CodeScribe - Processing...");
        assert_eq!(TrayStatus::Success.tooltip(), "CodeScribe - Done!");
    }

    #[test]
    fn test_hold_mods_labels() {
        assert_eq!(HoldMods::Ctrl.label(), "Ctrl only (Formatting)");
        assert_eq!(HoldMods::CtrlOption.label(), "Ctrl+Option");
        assert_eq!(HoldMods::CtrlShift.label(), "Ctrl+Shift (AI)");
        assert_eq!(HoldMods::CtrlCommand.label(), "Ctrl+Command");
    }

    #[test]
    fn test_toggle_trigger_labels() {
        assert_eq!(ToggleTrigger::DoubleOption.label(), "double option");
        assert_eq!(
            ToggleTrigger::DoubleRightOption.label(),
            "double right option"
        );
        assert_eq!(ToggleTrigger::Disabled.label(), "disabled");
    }
}
