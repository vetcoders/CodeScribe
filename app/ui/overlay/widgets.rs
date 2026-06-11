//! Unlocked AppKit widget mutators: action buttons, tooltips, hint label,
//! status field, spinner, and text-view editability.
//!
//! Every function here takes an `OverlaySnapshot` and MUST be called outside
//! the `OVERLAY_STATE` lock — see `state::OverlaySnapshot` for the deadlock
//! rationale.

use objc::runtime::Object;
use objc::{msg_send, sel, sel_impl};

use super::state::{OverlaySnapshot, TranscriptionActionContractMode, auto_hide_delay_secs};
use crate::ui::shared::status::{UiStatus, status_from_detail};
use crate::ui_helpers::{Id, set_hidden, set_text, set_text_view_string, set_tooltip};

/// Show/hide action buttons. Call ONLY outside the `OVERLAY_STATE` lock.
pub(super) fn set_action_buttons_visible_unlocked(snap: &OverlaySnapshot, visible: bool) {
    if let Some(copy_ptr) = snap.copy_button {
        unsafe {
            set_hidden(copy_ptr as Id, !visible);
        }
    }
    if let Some(augment_ptr) = snap.augment_button {
        unsafe {
            set_hidden(augment_ptr as Id, !visible);
        }
    }
    if let Some(save_ptr) = snap.save_button {
        unsafe {
            set_hidden(save_ptr as Id, !visible);
        }
    }
}

/// Show/hide commit button. Call ONLY outside the `OVERLAY_STATE` lock.
pub(super) fn set_recording_button_visible_unlocked(snap: &OverlaySnapshot, visible: bool) {
    if let Some(commit_ptr) = snap.commit_button {
        unsafe {
            set_hidden(commit_ptr as Id, !visible);
        }
    }
}

fn action_contract_source_label(mode: TranscriptionActionContractMode) -> &'static str {
    match mode {
        TranscriptionActionContractMode::Raw => "RAW",
        TranscriptionActionContractMode::AiFormat => "AI-FORMAT",
    }
}

pub(super) fn copy_action_tooltip(mode: TranscriptionActionContractMode) -> &'static str {
    match mode {
        TranscriptionActionContractMode::Raw => "Copy RAW transcript",
        TranscriptionActionContractMode::AiFormat => "Copy last-pass/formatted transcript",
    }
}

pub(super) fn augment_action_tooltip(mode: TranscriptionActionContractMode) -> &'static str {
    match mode {
        TranscriptionActionContractMode::Raw => "Open Agent overlay and hand off RAW transcript",
        TranscriptionActionContractMode::AiFormat => {
            "Open Agent overlay and hand off last-pass/formatted transcript"
        }
    }
}

/// Tooltip for the `[Format]` action (decision-mode). ADR 2026-05-28 Faza 1:
/// Format is an on-demand AI polish + paste — it works on the RAW transcript too,
/// it is NOT the old "Save closes" no-op. (Completes the Save->Format rename: the
/// creation-time tooltip was being clobbered back to the old Save text on every
/// action-contract refresh.)
fn format_action_tooltip(_mode: TranscriptionActionContractMode) -> &'static str {
    "Format the transcript with AI, then paste"
}

pub(super) fn decision_hint_text(
    mode: TranscriptionActionContractMode,
    display_status: &str,
    include_auto_hide: bool,
) -> String {
    let mode_label = action_contract_source_label(mode);
    // Action-driven contract (ADR 2026-05-28 Faza 1): [Format] polishes via AI + pastes,
    // [Copy] copies, [Agent] hands off. No more "Save closes" — Format is a real action.
    let base = if display_status.is_empty() {
        format!("Dictation overlay | {} | Format · Copy · Agent", mode_label)
    } else {
        format!(
            "Dictation overlay | {} | {} | Format · Copy · Agent",
            mode_label, display_status
        )
    };
    if include_auto_hide {
        format!("{base} | Auto-hide {}s", auto_hide_delay_secs())
    } else {
        base
    }
}

/// Refresh action contract tooltips/hints. Call ONLY outside the `OVERLAY_STATE` lock.
pub(super) fn refresh_action_contract_ui_unlocked(
    snap: &OverlaySnapshot,
    mode: TranscriptionActionContractMode,
    include_auto_hide_hint: bool,
) {
    if let Some(copy_ptr) = snap.copy_button {
        unsafe {
            set_tooltip(copy_ptr as Id, copy_action_tooltip(mode));
        }
    }
    if let Some(augment_ptr) = snap.augment_button {
        unsafe {
            set_tooltip(augment_ptr as Id, augment_action_tooltip(mode));
        }
    }
    if let Some(save_ptr) = snap.save_button {
        // `save_button` slot now holds the [Format] button (ADR 2026-05-28 Faza 1).
        // It must advertise the format action, not the old Save "close" semantics —
        // otherwise this refresh clobbers the creation-time tooltip back to "Save".
        unsafe {
            set_tooltip(save_ptr as Id, format_action_tooltip(mode));
        }
    }
    if let Some(label_ptr) = snap.auto_hide_label {
        unsafe {
            if include_auto_hide_hint {
                let hint = decision_hint_text(mode, &snap.display_status, true);
                set_text(label_ptr as Id, &hint);
                set_tooltip(label_ptr as Id, "Transcription overlay action contract");
                set_hidden(label_ptr as Id, false);
            } else {
                set_hidden(label_ptr as Id, true);
            }
        }
    }
}

/// Show/hide auto-hide hint. Call ONLY outside the `OVERLAY_STATE` lock.
pub(super) fn set_auto_hide_hint_visible_unlocked(
    snap: &OverlaySnapshot,
    mode: TranscriptionActionContractMode,
    visible: bool,
) {
    refresh_action_contract_ui_unlocked(snap, mode, visible);
}

pub(super) fn overlay_status_label(kind: UiStatus) -> &'static str {
    match kind {
        UiStatus::Idle => "Idle",
        UiStatus::Listening => "Listening",
        UiStatus::Processing => "Thinking",
        UiStatus::Error => "Error",
    }
}

/// Update status label + spinner. Call ONLY outside the `OVERLAY_STATE` lock.
pub(super) fn set_status_message_unlocked(snap: &OverlaySnapshot, msg: &str, allow_spinner: bool) {
    let status_kind = status_from_detail(msg);
    let status_text = overlay_status_label(status_kind);

    if let Some(status_ptr) = snap.status_field {
        unsafe {
            set_text(status_ptr as Id, status_text);
            set_hidden(status_ptr as Id, false);
            let status_color = status_kind.text_color();
            let _: () = msg_send![status_ptr as Id, setTextColor: status_color];

            let detail = if msg.trim().is_empty() {
                "Status: Idle".to_string()
            } else {
                format!("Status: {}", msg.trim())
            };
            set_tooltip(status_ptr as Id, &detail);
        }
    }

    let _ = crate::tray::update_tray_status(status_kind.to_tray());

    let show_spinner = allow_spinner && status_kind == UiStatus::Processing;
    if let Some(spinner_ptr) = snap.progress_indicator {
        unsafe {
            set_hidden(spinner_ptr as Id, !show_spinner);
            if show_spinner {
                let _: () =
                    msg_send![spinner_ptr as Id, startAnimation: std::ptr::null::<Object>()];
            } else {
                let _: () = msg_send![spinner_ptr as Id, stopAnimation: std::ptr::null::<Object>()];
            }
        }
    }
}

pub(super) fn set_text_view_editable_unlocked(snap: &OverlaySnapshot, editable: bool) {
    if let Some(text_view_ptr) = snap.text_view {
        unsafe {
            let text_view = text_view_ptr as Id;
            let _: () = msg_send![text_view, setEditable: editable];
            let _: () = msg_send![text_view, setSelectable: true];
        }
    }
}

/// Update the overlay text content. Call ONLY outside the `OVERLAY_STATE` lock.
pub(super) fn update_overlay_text_unlocked(text_view_ptr: Option<usize>, visible_text: &str) {
    if let Some(tv_ptr) = text_view_ptr {
        unsafe {
            set_text_view_string(tv_ptr as Id, visible_text);
        }
    }
}

// NOTE: update_overlay_text_and_layout and maybe_resize_overlay_layout were removed.
// Their logic is now inlined into callers using the extract-drop-execute pattern
// to prevent deadlocks. See append_transcription_delta_impl, set_transcription_text_impl, etc.

/// Reset status to idle. Call ONLY outside the `OVERLAY_STATE` lock.
pub(super) fn reset_overlay_to_idle_unlocked(snap: &OverlaySnapshot) {
    set_status_message_unlocked(snap, "Idle", false);
}

/// Toggle recording status indicator. Call ONLY outside the `OVERLAY_STATE` lock.
pub(super) fn set_recording_status_unlocked(snap: &OverlaySnapshot, show: bool) {
    if show {
        set_status_message_unlocked(snap, "Listening", false);
        return;
    }
    reset_overlay_to_idle_unlocked(snap);
}
