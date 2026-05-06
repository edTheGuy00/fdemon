//! Click handlers for the NewSessionDialog (Phase 5).
//!
//! These functions are dispatched from `handler/update.rs` when a click
//! produces an absolute-index `Message` (see `Message::NewSessionDialog*At`,
//! `Message::NewSessionDialogFocusField`). They mutate state and emit
//! follow-up messages to chain into the existing relative-navigation flow.

use crate::handler::UpdateResult;
use crate::message::Message;
use crate::new_session_dialog::LaunchContextField;
use crate::state::AppState;

/// Stub. Body added in Phase 5 Task 09.
pub fn handle_select_device_at(_state: &mut AppState, _index: usize) -> UpdateResult {
    // TODO(Phase 5 Task 09): set target_selector.selected_index = index for the active tab,
    // emit Message::NewSessionDialogDeviceSelect as a follow-up.
    let _ = Message::NewSessionDialogDeviceSelect;
    UpdateResult::none()
}

/// Stub. Body added in Phase 5 Task 09.
pub fn handle_focus_field(_state: &mut AppState, _field: LaunchContextField) -> UpdateResult {
    // TODO(Phase 5 Task 09): set launch_context.focused_field = field,
    // emit Message::NewSessionDialogFieldActivate as a follow-up.
    let _ = Message::NewSessionDialogFieldActivate;
    UpdateResult::none()
}

/// Stub. Body added in Phase 5 Task 09.
pub fn handle_fuzzy_select_at(_state: &mut AppState, _index: usize) -> UpdateResult {
    // TODO(Phase 5 Task 09): set fuzzy_modal.selected_index = index,
    // emit Message::NewSessionDialogFuzzyConfirm as a follow-up.
    let _ = Message::NewSessionDialogFuzzyConfirm;
    UpdateResult::none()
}
