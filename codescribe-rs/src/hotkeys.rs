// hotkeys.rs
//
// Purpose: Captures low-level keyboard events on macOS to detect specific
//          hotkey presses (hold Ctrl+Alt, double-tap Option) using rdev.
//          Provides a channel-based API for the main application to consume events.
//
// Dependencies: rdev (cross-platform keyboard/mouse event listening)
//               crossbeam-channel (for event communication)
//
// Key components: HotkeyEvent enum (Hold/Toggle events)
//                 HoldAction enum (Down/Up states)
//                 start() function (spawns listener thread)
//                 stop() function (stops the listener)
//
// Design rationale: Uses rdev for cross-platform global keyboard monitoring.
//                   A channel decouples event detection from event processing
//                   in the main application loop. Supports configurable modifiers
//                   and exclusive mode for precise gesture detection.

use crossbeam_channel::Sender;
use rdev::{listen, EventType, Key};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

// --- Constants ---

/// Double-tap Option timing threshold (milliseconds)
const DOUBLE_OPTION_INTERVAL_MS: u64 = 450;

// Virtual key codes for modifiers (macOS-specific mapping)
const KEYCODE_LEFT_OPTION: u32 = 58;
const KEYCODE_RIGHT_OPTION: u32 = 61;
const KEYCODE_LEFT_SHIFT: u32 = 56;
const KEYCODE_RIGHT_SHIFT: u32 = 60;
const KEYCODE_LEFT_CMD: u32 = 55;
const KEYCODE_RIGHT_CMD: u32 = 54;
const KEYCODE_LEFT_CTRL: u32 = 59;
const KEYCODE_RIGHT_CTRL: u32 = 62;

// --- Types ---

/// Represents the action of a hold gesture
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HoldAction {
    Down,
    Up,
}

/// Hotkey event emitted by the listener
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HotkeyEvent {
    /// Hold gesture detected (press/release configurable modifiers)
    /// The boolean indicates "assistive mode" (Shift was held during the gesture)
    Hold { action: HoldAction, assistive: bool },
    /// Toggle gesture detected (double-tap Option within threshold)
    Toggle,
}

/// Modifier flags for hold gesture detection
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModifierFlags {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub cmd: bool,
}

impl ModifierFlags {
    pub fn new() -> Self {
        Self {
            ctrl: false,
            alt: false,
            shift: false,
            cmd: false,
        }
    }

    pub fn ctrl_only() -> Self {
        Self {
            ctrl: true,
            alt: false,
            shift: false,
            cmd: false,
        }
    }

    pub fn ctrl_alt() -> Self {
        Self {
            ctrl: true,
            alt: true,
            shift: false,
            cmd: false,
        }
    }

    /// Check if the current flags match the required flags
    /// In exclusive mode, only the specified modifiers must be pressed (no extras)
    /// In non-exclusive mode, the required modifiers must be present (extras allowed)
    pub fn matches(&self, required: &ModifierFlags, exclusive: bool) -> bool {
        if exclusive {
            // In exclusive mode, allow Shift as optional assistive modifier
            let base_match = self.ctrl == required.ctrl
                && self.alt == required.alt
                && self.cmd == required.cmd;
            base_match
        } else {
            // Non-exclusive: all required modifiers must be present
            (!required.ctrl || self.ctrl)
                && (!required.alt || self.alt)
                && (!required.shift || self.shift)
                && (!required.cmd || self.cmd)
        }
    }

    pub fn is_assistive(&self) -> bool {
        self.shift
    }

    pub fn label(&self) -> String {
        let mut parts = Vec::new();
        if self.ctrl {
            parts.push("Ctrl");
        }
        if self.alt {
            parts.push("Option");
        }
        if self.shift {
            parts.push("Shift");
        }
        if self.cmd {
            parts.push("Command");
        }
        if parts.is_empty() {
            "Ctrl".to_string()
        } else {
            parts.join("+")
        }
    }
}

impl Default for ModifierFlags {
    fn default() -> Self {
        Self::new()
    }
}

// --- State ---

struct HotkeyState {
    /// Currently pressed modifier keys
    current_modifiers: ModifierFlags,
    /// Required modifiers for hold gesture
    required_modifiers: ModifierFlags,
    /// Exclusive mode: require ONLY the specified modifiers (no extras)
    exclusive_mode: bool,
    /// Last hold gesture state (true = down, false = up)
    last_combo_down: bool,
    /// Assistive mode from last "down" event (preserved for "up")
    last_assistive_mode: bool,
    /// Last Option key down timestamp
    last_option_down_ts: Option<Instant>,
    /// Set of non-modifier keys currently pressed
    non_modifier_keys_down: std::collections::HashSet<u32>,
    /// Block hold gesture until all keys are released
    block_hold_until_clear: bool,
}

impl HotkeyState {
    fn new(required_modifiers: ModifierFlags, exclusive_mode: bool) -> Self {
        Self {
            current_modifiers: ModifierFlags::new(),
            required_modifiers,
            exclusive_mode,
            last_combo_down: false,
            last_assistive_mode: false,
            last_option_down_ts: None,
            non_modifier_keys_down: std::collections::HashSet::new(),
            block_hold_until_clear: false,
        }
    }

    fn reset(&mut self) {
        self.current_modifiers = ModifierFlags::new();
        self.last_combo_down = false;
        self.last_assistive_mode = false;
        self.last_option_down_ts = None;
        self.non_modifier_keys_down.clear();
        self.block_hold_until_clear = false;
    }

    /// Cancel the hold gesture (force "up" event)
    fn cancel_hold(&mut self, tx: &Sender<HotkeyEvent>, _reason: &str) {
        if self.last_combo_down {
            let _ = tx.send(HotkeyEvent::Hold {
                action: HoldAction::Up,
                assistive: self.last_assistive_mode,
            });
            self.last_combo_down = false;
        }
        self.block_hold_until_clear = true;
    }
}

// --- Helper Functions ---

fn is_modifier_key(key: &Key) -> bool {
    matches!(
        key,
        Key::ControlLeft
            | Key::ControlRight
            | Key::Alt
            | Key::AltGr
            | Key::ShiftLeft
            | Key::ShiftRight
            | Key::MetaLeft
            | Key::MetaRight
    )
}

fn update_modifiers_from_key(modifiers: &mut ModifierFlags, key: &Key, pressed: bool) {
    match key {
        Key::ControlLeft | Key::ControlRight => modifiers.ctrl = pressed,
        Key::Alt | Key::AltGr => modifiers.alt = pressed,
        Key::ShiftLeft | Key::ShiftRight => modifiers.shift = pressed,
        Key::MetaLeft | Key::MetaRight => modifiers.cmd = pressed,
        _ => {}
    }
}

// --- Public API ---

/// Start the global hotkey listener
///
/// Spawns a background thread that listens for keyboard events and sends
/// HotkeyEvent messages through the provided channel.
///
/// # Arguments
/// * `tx` - Channel sender for emitting hotkey events
/// * `required_modifiers` - Modifier flags required for hold gesture
/// * `exclusive_mode` - If true, only the specified modifiers must be pressed
///
/// # Returns
/// * `Ok(())` if the listener started successfully
/// * `Err(String)` if the listener failed to start
pub fn start(
    tx: Sender<HotkeyEvent>,
    required_modifiers: ModifierFlags,
    exclusive_mode: bool,
) -> Result<(), String> {
    let state = Arc::new(Mutex::new(HotkeyState::new(
        required_modifiers,
        exclusive_mode,
    )));

    let state_clone = Arc::clone(&state);

    // Spawn the rdev listener thread
    thread::spawn(move || {
        let callback = move |event: rdev::Event| {
            handle_event(&state_clone, &tx, event);
        };

        if let Err(error) = listen(callback) {
            eprintln!("Error in rdev listener: {:?}", error);
        }
    });

    Ok(())
}

/// Stop the global hotkey listener (placeholder for future cleanup)
///
/// Note: rdev's `listen()` doesn't provide a direct stop mechanism.
/// The thread will continue until the process exits. Future implementations
/// could use a stop channel to signal the thread to exit.
pub fn stop() {
    // TODO: Implement proper cleanup when rdev supports it
    // For now, the listener thread will exit when the process terminates
}

// --- Event Handler ---

fn handle_event(state: &Arc<Mutex<HotkeyState>>, tx: &Sender<HotkeyEvent>, event: rdev::Event) {
    let mut state = state.lock().unwrap();

    match event.event_type {
        EventType::KeyPress(key) => {
            if is_modifier_key(&key) {
                // Update modifier state
                update_modifiers_from_key(&mut state.current_modifiers, &key, true);

                // Check for hold gesture state change
                handle_hold_gesture(&mut state, tx);

                // Check for double-tap Option toggle
                if matches!(key, Key::Alt | Key::AltGr) {
                    handle_option_tap(&mut state, tx);
                }
            } else {
                // Non-modifier key pressed - cancel hold if active
                if !state.non_modifier_keys_down.is_empty() || state.last_combo_down {
                    state.cancel_hold(tx, "other key pressed");
                }
                // Track non-modifier keys (using a placeholder value since rdev doesn't expose raw codes easily)
                state.non_modifier_keys_down.insert(0); // Simplified tracking
            }
        }
        EventType::KeyRelease(key) => {
            if is_modifier_key(&key) {
                // Update modifier state
                update_modifiers_from_key(&mut state.current_modifiers, &key, false);

                // Check for hold gesture state change
                handle_hold_gesture(&mut state, tx);

                // Clear block flag when all modifiers released
                if !state.current_modifiers.ctrl
                    && !state.current_modifiers.alt
                    && !state.current_modifiers.shift
                    && !state.current_modifiers.cmd
                    && state.non_modifier_keys_down.is_empty()
                {
                    state.block_hold_until_clear = false;
                }
            } else {
                // Non-modifier key released
                state.non_modifier_keys_down.clear(); // Simplified tracking
                if state.non_modifier_keys_down.is_empty()
                    && !state.current_modifiers.ctrl
                    && !state.current_modifiers.alt
                    && !state.current_modifiers.shift
                    && !state.current_modifiers.cmd
                {
                    state.block_hold_until_clear = false;
                }
            }
        }
        _ => {}
    }
}

fn handle_hold_gesture(state: &mut HotkeyState, tx: &Sender<HotkeyEvent>) {
    // Skip if blocked or non-modifier keys are pressed
    if state.block_hold_until_clear || !state.non_modifier_keys_down.is_empty() {
        if !state.non_modifier_keys_down.is_empty() && state.last_combo_down {
            state.cancel_hold(tx, "other key pressed");
        }
        return;
    }

    // Check if current modifiers match required modifiers
    let combo_now = state
        .current_modifiers
        .matches(&state.required_modifiers, state.exclusive_mode);

    // Detect state change
    if combo_now != state.last_combo_down {
        let is_assistive = if combo_now {
            // "down" event - check if Shift is currently pressed
            let assistive = state.current_modifiers.is_assistive();
            state.last_assistive_mode = assistive; // Remember for "up" event
            assistive
        } else {
            // "up" event - use remembered assistive mode
            state.last_assistive_mode
        };

        let action = if combo_now {
            HoldAction::Down
        } else {
            HoldAction::Up
        };

        let _ = tx.send(HotkeyEvent::Hold {
            action,
            assistive: is_assistive,
        });

        state.last_combo_down = combo_now;
    }
}

fn handle_option_tap(state: &mut HotkeyState, tx: &Sender<HotkeyEvent>) {
    let now = Instant::now();

    // Check if other modifiers are pressed (ignore Option with other modifiers)
    if state.current_modifiers.ctrl
        || state.current_modifiers.shift
        || state.current_modifiers.cmd
    {
        state.last_option_down_ts = None;
        return;
    }

    // Check for double-tap
    if let Some(last_down) = state.last_option_down_ts {
        let elapsed = now.duration_since(last_down);
        if elapsed <= Duration::from_millis(DOUBLE_OPTION_INTERVAL_MS) {
            // Double-tap detected!
            let _ = tx.send(HotkeyEvent::Toggle);
            state.last_option_down_ts = None; // Reset
            return;
        }
    }

    // Record this Option press
    state.last_option_down_ts = Some(now);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_modifier_flags_ctrl_only() {
        let flags = ModifierFlags::ctrl_only();
        assert!(flags.ctrl);
        assert!(!flags.alt);
        assert!(!flags.shift);
        assert!(!flags.cmd);
    }

    #[test]
    fn test_modifier_flags_ctrl_alt() {
        let flags = ModifierFlags::ctrl_alt();
        assert!(flags.ctrl);
        assert!(flags.alt);
        assert!(!flags.shift);
        assert!(!flags.cmd);
    }

    #[test]
    fn test_matches_exclusive_mode() {
        let required = ModifierFlags::ctrl_only();
        let current = ModifierFlags {
            ctrl: true,
            alt: false,
            shift: false,
            cmd: false,
        };
        assert!(current.matches(&required, true));

        // With Shift (assistive mode) - should still match in exclusive mode
        let current_with_shift = ModifierFlags {
            ctrl: true,
            alt: false,
            shift: true,
            cmd: false,
        };
        assert!(current_with_shift.matches(&required, true));

        // Extra modifier (Alt) - should NOT match in exclusive mode
        let current_with_extra = ModifierFlags {
            ctrl: true,
            alt: true,
            shift: false,
            cmd: false,
        };
        assert!(!current_with_extra.matches(&required, true));
    }

    #[test]
    fn test_matches_non_exclusive_mode() {
        let required = ModifierFlags::ctrl_only();
        let current = ModifierFlags {
            ctrl: true,
            alt: true, // Extra modifier allowed in non-exclusive mode
            shift: false,
            cmd: false,
        };
        assert!(current.matches(&required, false));
    }

    #[test]
    fn test_is_assistive() {
        let flags = ModifierFlags {
            ctrl: true,
            alt: true,
            shift: true,
            cmd: false,
        };
        assert!(flags.is_assistive());

        let flags_no_shift = ModifierFlags {
            ctrl: true,
            alt: true,
            shift: false,
            cmd: false,
        };
        assert!(!flags_no_shift.is_assistive());
    }

    #[test]
    fn test_label() {
        let flags = ModifierFlags::ctrl_alt();
        assert_eq!(flags.label(), "Ctrl+Option");

        let flags_all = ModifierFlags {
            ctrl: true,
            alt: true,
            shift: true,
            cmd: true,
        };
        assert_eq!(flags_all.label(), "Ctrl+Option+Shift+Command");
    }

    #[test]
    fn test_is_modifier_key() {
        assert!(is_modifier_key(&Key::ControlLeft));
        assert!(is_modifier_key(&Key::Alt));
        assert!(is_modifier_key(&Key::ShiftLeft));
        assert!(is_modifier_key(&Key::MetaLeft));
        assert!(!is_modifier_key(&Key::KeyA));
        assert!(!is_modifier_key(&Key::Space));
    }
}
