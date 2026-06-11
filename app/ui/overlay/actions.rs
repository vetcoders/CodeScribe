//! Objective-C action-handler bridge: button callbacks (Copy / Agent /
//! Format / Finish) and hover tracking, plus the action-contract text
//! snapshot used by the controller's commit path.

use std::sync::Once;
use std::sync::atomic::Ordering;

use objc::declare::ClassDecl;
use objc::runtime::{Class, Object, Sel};
use objc::{sel, sel_impl};
use tracing::{info, warn};

use super::lifecycle::{hide_transcription_overlay, schedule_auto_hide};
#[cfg(test)]
use super::state::TranscriptionOverlayState;
use super::state::{
    AUTO_HIDE_GENERATION, AUTO_HIDE_PENDING, OVERLAY_STATE, OverlaySnapshot,
    action_text_for_contract,
};
use super::widgets::{set_action_buttons_visible_unlocked, set_status_message_unlocked};
use crate::os::clipboard;
use crate::ui_helpers::{Id, get_text_view_string};

static ACTION_HANDLER_INIT: Once = Once::new();
static mut ACTION_HANDLER_CLASS: *const Class = std::ptr::null();

#[cfg(test)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum AugmentAction {
    CommitLiveSegment,
    HandoffDecisionText(String),
}

pub(super) fn action_handler_class() -> *const Class {
    ACTION_HANDLER_INIT.call_once(|| unsafe {
        let superclass = Class::get("NSObject").unwrap();
        let mut decl = ClassDecl::new("TranscriptionOverlayActionHandler", superclass).unwrap();

        decl.add_method(
            sel!(onCopyTranscript:),
            on_copy_transcript as extern "C" fn(&Object, Sel, Id),
        );
        decl.add_method(
            sel!(onAgentTranscript:),
            on_agent_transcript as extern "C" fn(&Object, Sel, Id),
        );
        decl.add_method(
            sel!(onFormatTranscript:),
            on_format_transcript as extern "C" fn(&Object, Sel, Id),
        );
        decl.add_method(
            sel!(onCommitRecording:),
            on_commit_recording as extern "C" fn(&Object, Sel, Id),
        );
        decl.add_method(
            sel!(mouseEntered:),
            on_mouse_entered as extern "C" fn(&Object, Sel, Id),
        );
        decl.add_method(
            sel!(mouseExited:),
            on_mouse_exited as extern "C" fn(&Object, Sel, Id),
        );

        ACTION_HANDLER_CLASS = decl.register();
    });
    unsafe { ACTION_HANDLER_CLASS }
}

fn current_action_text_snapshot() -> (String, bool, OverlaySnapshot) {
    let (fallback, decision_mode, snap) = {
        let state = OVERLAY_STATE.lock().unwrap_or_else(|e| e.into_inner());
        (
            action_text_for_contract(&state),
            state.decision_mode,
            OverlaySnapshot::from_state(&state),
        )
    };

    if decision_mode && let Some(text_view_ptr) = snap.text_view {
        let edited = unsafe { get_text_view_string(text_view_ptr as Id) };
        return (edited, decision_mode, snap);
    }

    (fallback, decision_mode, snap)
}

#[cfg(test)]
pub(super) fn augment_action_for_state(state: &TranscriptionOverlayState) -> Option<AugmentAction> {
    let text = action_text_for_contract(state);
    if text.trim().is_empty() {
        return None;
    }
    if state.decision_mode {
        Some(AugmentAction::HandoffDecisionText(text))
    } else {
        Some(AugmentAction::CommitLiveSegment)
    }
}

/// Returns the current action-contract text (Raw or AiFormat depending on
/// `state.action_contract_mode`). Used by controller's `commit_segment` to
/// read segment text for save without coupling button handlers to controller
/// state. Returns empty string if overlay state lock is poisoned (recoverable).
pub fn current_segment_text() -> String {
    let (text, _, _) = current_action_text_snapshot();
    text
}

/// Handler: Copy transcript using contract source of truth.
extern "C" fn on_copy_transcript(_this: &Object, _cmd: Sel, _sender: Id) {
    let (text, _, snap) = current_action_text_snapshot();
    if text.is_empty() {
        return;
    }
    if let Err(e) = clipboard::set_clipboard(&text) {
        warn!("Failed to copy transcript: {}", e);
        set_status_message_unlocked(&snap, "Copy failed", true);
        return;
    }

    info!("Copied transcript ({} chars)", text.len());
    hide_transcription_overlay();
}

/// Handler: Agent = hand the whole transcript to the Agent (Emil).
///
/// Decision-mode (post-recording): hands off the complete session transcript to
/// the voice-chat overlay as a single message. Live (mid-recording, legacy) clips
/// and commits the current segment, then augments. ADR 2026-05-28 Faza 1 renames
/// the former "Augment" action to "Agent" — same handoff, clearer contract.
extern "C" fn on_agent_transcript(_this: &Object, _cmd: Sel, _sender: Id) {
    let (text, decision_mode, _) = current_action_text_snapshot();
    if text.trim().is_empty() {
        return;
    }

    if decision_mode {
        crate::ui::voice_chat::show_voice_chat_overlay();
        crate::ui::voice_chat::show_agent_tab();
        crate::ui::voice_chat::handoff_transcript_to_chat(&text);
    } else {
        crate::controller::request_segment_commit_and_augment();
    }
    hide_transcription_overlay();
}

/// Handler: Format = run AI formatting on the decision transcript, then paste.
///
/// ADR 2026-05-28 Faza 1: formatting is a post-recording CHOICE, not something the
/// dictation does mid-stream. The async format + paste runs off the main thread via
/// the controller; the overlay closes immediately.
extern "C" fn on_format_transcript(_this: &Object, _cmd: Sel, _sender: Id) {
    crate::controller::request_format_and_paste();
    hide_transcription_overlay();
}

/// Handler: Commit segment = save WAV + transcript + Quick Notes WITHOUT
/// stopping the recorder. Recording continues; buffer offset advances so the
/// next segment starts from here. Overlay fades out.
extern "C" fn on_commit_recording(_this: &Object, _cmd: Sel, _sender: Id) {
    crate::controller::request_segment_commit();
    hide_transcription_overlay();
}

extern "C" fn on_mouse_entered(_this: &Object, _cmd: Sel, _sender: Id) {
    let (cancel_auto_hide, snap) = {
        let mut state = OVERLAY_STATE.lock().unwrap_or_else(|e| e.into_inner());
        state.hover_active = true;
        let dm = state.decision_mode;
        (dm, OverlaySnapshot::from_state(&state))
    }; // Lock dropped before AppKit calls.
    if cancel_auto_hide {
        set_action_buttons_visible_unlocked(&snap, true);
        AUTO_HIDE_GENERATION.fetch_add(1, Ordering::SeqCst);
        AUTO_HIDE_PENDING.store(false, Ordering::SeqCst);
    }
}

extern "C" fn on_mouse_exited(_this: &Object, _cmd: Sel, _sender: Id) {
    let (decision_mode, snap) = {
        let mut state = OVERLAY_STATE.lock().unwrap_or_else(|e| e.into_inner());
        state.hover_active = false;
        (state.decision_mode, OverlaySnapshot::from_state(&state))
    }; // Lock dropped before AppKit calls.
    if decision_mode {
        set_action_buttons_visible_unlocked(&snap, true);
        schedule_auto_hide();
    } else {
        set_action_buttons_visible_unlocked(&snap, false);
    }
}
