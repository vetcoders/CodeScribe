//! Submenu building functions for the tray menu
//!
//! Each function builds a specific submenu and returns its IDs.

use anyhow::Result;
use muda::{CheckMenuItem, MenuId, MenuItem, PredefinedMenuItem, Submenu};

use crate::tray::state::HOTKEYS_MENU_ITEMS;
use crate::tray::types::HotkeysMenuItems;

// Type aliases
pub type HotkeysMenuIds = (MenuId, MenuId);

/// Build the Hold Hotkeys submenu
pub fn build_hold_hotkeys_submenu() -> Result<(Submenu, HotkeysMenuIds)> {
    let hold_menu = Submenu::new("Hotkeys", true);

    // Read from Config (source of truth for initial state)
    let config = crate::config::Config::load();
    let current_trigger = config.toggle_trigger;
    let toggle_enabled = current_trigger != crate::config::ToggleTrigger::None;

    let hold_summary = MenuItem::new(
        "Hold Ctrl: RAW | +Shift: Chat | +Cmd: Selection",
        false,
        None,
    );
    hold_menu.append(&hold_summary)?;

    let reset_item = MenuItem::new("Reset hotkeys (recommended)", true, None);
    let reset_id = reset_item.id().clone();
    hold_menu.append(&reset_item)?;
    hold_menu.append(&PredefinedMenuItem::separator())?;

    let toggle_label = MenuItem::new(
        format!(
            "Right Option toggle (assistive): {}",
            if toggle_enabled { "ON" } else { "OFF" }
        ),
        false,
        None,
    );
    hold_menu.append(&toggle_label)?;

    let toggle_assistive = CheckMenuItem::new(
        "Enable right Option toggle (assistive)",
        true,
        toggle_enabled,
        None,
    );
    let toggle_assistive_id = toggle_assistive.id().clone();
    hold_menu.append(&toggle_assistive)?;

    HOTKEYS_MENU_ITEMS.with(|items_cell| {
        *items_cell.borrow_mut() = Some(HotkeysMenuItems {
            toggle_assistive,
            toggle_label,
        });
    });

    Ok((hold_menu, (toggle_assistive_id, reset_id)))
}
