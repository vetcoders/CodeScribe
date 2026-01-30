//! Main menu building logic for the tray menu.
//!
//! Menu structure:
//! - Status line (dynamic)
//! - Copy Last to Clipboard
//! - Show Chat Overlay
//! - Run Onboarding
//! - Open history...
//! - Advanced… ▸ (Hotkeys + diagnostics + quality)
//! - Help / About / Quit

use std::cell::RefCell;

use anyhow::Result;
use muda::accelerator::{Accelerator, Code, Modifiers};
use muda::{Menu, MenuItem, PredefinedMenuItem, Submenu};

use crate::config::Config;
use crate::tray::submenus::build_hold_hotkeys_submenu;
use crate::tray::types::MenuIds;

thread_local! {
    pub static STATUS_MENU_ITEM: RefCell<Option<MenuItem>> = const { RefCell::new(None) };
    pub static QUALITY_MENU_ITEM: RefCell<Option<MenuItem>> = const { RefCell::new(None) };
}

/// Build the tray menu and return (menu, ids).
pub fn build_menu() -> Result<(Menu, MenuIds)> {
    let menu = Menu::new();

    // Status line (disabled, dynamic text)
    let status_item = MenuItem::new("Status: Idle", false, None);
    menu.append(&status_item)?;
    STATUS_MENU_ITEM.with(|cell| {
        *cell.borrow_mut() = Some(status_item);
    });

    // Quick actions
    let copy_last_item = MenuItem::new("Copy Last to Clipboard", true, None);
    let copy_last_id = copy_last_item.id().clone();
    menu.append(&copy_last_item)?;

    let show_overlay_item = MenuItem::new("Show Chat Overlay", true, None);
    let show_overlay_id = show_overlay_item.id().clone();
    menu.append(&show_overlay_item)?;

    let run_onboarding_item = MenuItem::new("Run Onboarding", true, None);
    let run_onboarding_id = run_onboarding_item.id().clone();
    menu.append(&run_onboarding_item)?;

    menu.append(&PredefinedMenuItem::separator())?;

    let open_history_item = MenuItem::new("Open history...", true, None);
    let open_history_id = open_history_item.id().clone();
    menu.append(&open_history_item)?;

    menu.append(&PredefinedMenuItem::separator())?;

    // Advanced submenu
    let advanced_submenu = Submenu::new("Advanced…", true);

    let (hotkeys_submenu, hold_ids) = build_hold_hotkeys_submenu()?;
    advanced_submenu.append(&hotkeys_submenu)?;

    advanced_submenu.append(&PredefinedMenuItem::separator())?;

    let copy_diag_item = MenuItem::new("Copy diagnostics", true, None);
    let copy_diagnostics_id = copy_diag_item.id().clone();
    advanced_submenu.append(&copy_diag_item)?;

    let state = crate::quality_loop::read_daemon_state();
    let quality_label = if !state.available {
        "Quality: unavailable".to_string()
    } else if state.pending_mismatches > 0 {
        format!("Quality: {} pending", state.pending_mismatches)
    } else {
        "Quality: OK".to_string()
    };

    let quality_item = MenuItem::new(&quality_label, true, None);
    let quality_open_report_id = quality_item.id().clone();
    advanced_submenu.append(&quality_item)?;

    QUALITY_MENU_ITEM.with(|cell| {
        *cell.borrow_mut() = Some(quality_item);
    });

    menu.append(&advanced_submenu)?;

    menu.append(&PredefinedMenuItem::separator())?;

    let help_item = MenuItem::new("Help", true, None);
    let help_id = help_item.id().clone();
    menu.append(&help_item)?;

    let about_item = MenuItem::new("About", true, None);
    let about_id = about_item.id().clone();
    menu.append(&about_item)?;

    menu.append(&PredefinedMenuItem::separator())?;

    // Quit (Cmd+Q)
    let quit_accel = Accelerator::new(Some(Modifiers::SUPER), Code::KeyQ);
    let quit_item = MenuItem::new("Quit", true, Some(quit_accel));
    let quit_id = quit_item.id().clone();
    menu.append(&quit_item)?;

    let (
        hold_ctrl_id,
        hold_ctrl_opt_id,
        hold_ctrl_shift_id,
        hold_ctrl_cmd_id,
        hold_exclusive_id,
        toggle_double_opt_id,
        toggle_double_ralt_id,
        toggle_disabled_id,
    ) = hold_ids;

    Ok((
        menu,
        MenuIds {
            copy_last: copy_last_id,
            show_overlay: show_overlay_id,
            run_onboarding: run_onboarding_id,
            open_history: open_history_id,
            copy_diagnostics: copy_diagnostics_id,
            help: help_id,
            about: about_id,
            quit: quit_id,
            hold_ctrl: hold_ctrl_id,
            hold_ctrl_opt: hold_ctrl_opt_id,
            hold_ctrl_shift: hold_ctrl_shift_id,
            hold_ctrl_cmd: hold_ctrl_cmd_id,
            hold_exclusive: hold_exclusive_id,
            toggle_double_opt: toggle_double_opt_id,
            toggle_double_ralt: toggle_double_ralt_id,
            toggle_disabled: toggle_disabled_id,
            quality_open_report: quality_open_report_id,
        },
    ))
}

/// Update the status label in the menu.
/// Must be called from the main thread.
pub fn update_status_label(label: &str) {
    STATUS_MENU_ITEM.with(|cell| {
        if let Some(ref item) = *cell.borrow() {
            item.set_text(label);
        }
    });
}

/// Toggle AI Formatting and persist to config.
/// Note: the tray checkbox was removed, but this is still useful for hotkeys / IPC.
pub fn toggle_ai_formatting() -> bool {
    let current_state = Config::load().ai_formatting_enabled;
    let new_state = !current_state;

    let config = Config::load();
    let _ = config.save_to_env("AI_FORMATTING_ENABLED", if new_state { "1" } else { "0" });

    new_state
}

/// Update the quality label in the menu.
pub fn update_quality_label() {
    let state = crate::quality_loop::read_daemon_state();
    let label = if !state.available {
        "Quality: unavailable".to_string()
    } else if state.pending_mismatches > 0 {
        format!("Quality: {} pending", state.pending_mismatches)
    } else {
        "Quality: OK".to_string()
    };

    QUALITY_MENU_ITEM.with(|cell| {
        if let Some(ref item) = *cell.borrow() {
            item.set_text(&label);
        }
    });
}
