//! Conversation-state regression coverage after the monolith split.
//!
//! The old Moshi runtime is no longer linked into `vista-kernel`; the durable
//! conversation surface that remains here is response-id tracking per AI mode.

use serial_test::serial;
use vista_kernel::state::conversation::{
    AiMode, get_previous_response_id_for_mode, has_active_conversation, reset_conversation,
    reset_conversation_for_mode, set_response_id_for_mode,
};

#[test]
#[serial]
fn test_conversation_starts_empty() {
    reset_conversation();
    assert_eq!(get_previous_response_id_for_mode(AiMode::Formatting), None);
    assert_eq!(get_previous_response_id_for_mode(AiMode::Assistive), None);
    assert!(!has_active_conversation());
}

#[test]
#[serial]
fn test_conversation_tracks_modes_independently() {
    reset_conversation();

    set_response_id_for_mode(AiMode::Formatting, "fmt-123".to_string());
    assert_eq!(
        get_previous_response_id_for_mode(AiMode::Formatting),
        Some("fmt-123".to_string())
    );
    assert_eq!(get_previous_response_id_for_mode(AiMode::Assistive), None);

    set_response_id_for_mode(AiMode::Assistive, "ast-456".to_string());
    assert_eq!(
        get_previous_response_id_for_mode(AiMode::Formatting),
        Some("fmt-123".to_string())
    );
    assert_eq!(
        get_previous_response_id_for_mode(AiMode::Assistive),
        Some("ast-456".to_string())
    );
    assert!(has_active_conversation());
}

#[test]
#[serial]
fn test_mode_specific_reset_keeps_other_lane() {
    reset_conversation();

    set_response_id_for_mode(AiMode::Formatting, "fmt-keep".to_string());
    set_response_id_for_mode(AiMode::Assistive, "ast-drop".to_string());

    reset_conversation_for_mode(AiMode::Assistive);

    assert_eq!(
        get_previous_response_id_for_mode(AiMode::Formatting),
        Some("fmt-keep".to_string())
    );
    assert_eq!(get_previous_response_id_for_mode(AiMode::Assistive), None);
    assert!(has_active_conversation());
}

#[test]
#[serial]
fn test_full_reset_clears_all_conversation_state() {
    reset_conversation();

    set_response_id_for_mode(AiMode::Formatting, "fmt".to_string());
    set_response_id_for_mode(AiMode::Assistive, "ast".to_string());
    assert!(has_active_conversation());

    reset_conversation();

    assert_eq!(get_previous_response_id_for_mode(AiMode::Formatting), None);
    assert_eq!(get_previous_response_id_for_mode(AiMode::Assistive), None);
    assert!(!has_active_conversation());
}
