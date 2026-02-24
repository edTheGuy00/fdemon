//! NewSessionDialog dart defines modal handlers
//!
//! Handles the key-value editor modal for dart defines.

use crate::handler::UpdateResult;
use crate::message::Message;
use crate::state::AppState;

/// Handle opening dart defines modal.
///
/// Copies the current dart defines from the launch context into the modal
/// working copy, sorted alphabetically by key for consistent presentation.
pub fn handle_open_dart_defines_modal(state: &mut AppState) -> UpdateResult {
    // Copy current dart defines into modal state
    state.new_session_dialog_state.open_dart_defines_modal();

    // Sort defines alphabetically by key for consistent display
    if let Some(ref mut modal) = state.new_session_dialog_state.dart_defines_modal {
        modal.defines.sort_by(|a, b| a.key.cmp(&b.key));
    }

    UpdateResult::none()
}

/// Handle closing dart defines modal and persisting changes.
///
/// Reads the working copy from the modal, applies it to the launch context,
/// triggers auto-save for editable FDemon configurations, then dismisses the
/// modal. If the modal is unexpectedly absent, emits a warning and returns
/// without modifying state.
pub fn handle_close_dart_defines_modal(
    state: &mut AppState,
    update_fn: fn(&mut AppState, Message) -> UpdateResult,
) -> UpdateResult {
    // Get defines from modal before closing
    let defines = if let Some(modal) = state.new_session_dialog_state.dart_defines_modal.as_ref() {
        modal.defines.clone()
    } else {
        tracing::warn!("dart defines modal closed with no editing_config_idx — changes discarded");
        state.new_session_dialog_state.dart_defines_modal = None;
        return UpdateResult::none();
    };

    // Use the new dart defines updated message which handles auto-save
    update_fn(
        state,
        Message::NewSessionDialogDartDefinesUpdated { defines },
    )
}

/// Handle cancelling dart defines modal and discarding all changes.
///
/// Closes the modal without applying any changes to the launch context.
/// No auto-save is triggered.
pub fn handle_cancel_dart_defines_modal(state: &mut AppState) -> UpdateResult {
    state.new_session_dialog_state.dart_defines_modal = None;
    UpdateResult::none()
}

/// Handle pane switch in dart defines modal
pub fn handle_dart_defines_switch_pane(state: &mut AppState) -> UpdateResult {
    if let Some(ref mut modal) = state.new_session_dialog_state.dart_defines_modal {
        modal.switch_pane();
    }
    UpdateResult::none()
}

/// Handle navigation up in dart defines list
pub fn handle_dart_defines_up(state: &mut AppState) -> UpdateResult {
    if let Some(ref mut modal) = state.new_session_dialog_state.dart_defines_modal {
        use crate::new_session_dialog::DartDefinesPane;
        if modal.active_pane == DartDefinesPane::List {
            modal.navigate_up();
        }
    }
    UpdateResult::none()
}

/// Handle navigation down in dart defines list
pub fn handle_dart_defines_down(state: &mut AppState) -> UpdateResult {
    if let Some(ref mut modal) = state.new_session_dialog_state.dart_defines_modal {
        use crate::new_session_dialog::DartDefinesPane;
        if modal.active_pane == DartDefinesPane::List {
            modal.navigate_down();
        }
    }
    UpdateResult::none()
}

/// Handle confirm action in dart defines modal
pub fn handle_dart_defines_confirm(state: &mut AppState) -> UpdateResult {
    if let Some(ref mut modal) = state.new_session_dialog_state.dart_defines_modal {
        use crate::new_session_dialog::{DartDefinesEditField, DartDefinesPane};
        match modal.active_pane {
            DartDefinesPane::List => {
                // Load selected item into edit form
                modal.load_selected_into_edit();
            }
            DartDefinesPane::Edit => {
                // Activate current button or confirm field
                match modal.edit_field {
                    DartDefinesEditField::Key | DartDefinesEditField::Value => {
                        // Move to next field
                        modal.next_field();
                    }
                    DartDefinesEditField::Save => {
                        if !modal.save_edit() {
                            // Save failed (key is empty) - return focus to Key field
                            modal.edit_field = DartDefinesEditField::Key;
                        }
                    }
                    DartDefinesEditField::Delete => {
                        modal.delete_selected();
                    }
                }
            }
        }
    }
    UpdateResult::none()
}

/// Handle field navigation in dart defines edit pane
pub fn handle_dart_defines_next_field(state: &mut AppState) -> UpdateResult {
    if let Some(ref mut modal) = state.new_session_dialog_state.dart_defines_modal {
        use crate::new_session_dialog::DartDefinesPane;
        if modal.active_pane == DartDefinesPane::Edit {
            modal.next_field();
        }
    }
    UpdateResult::none()
}

/// Handle character input in dart defines edit pane
pub fn handle_dart_defines_input(state: &mut AppState, c: char) -> UpdateResult {
    if let Some(ref mut modal) = state.new_session_dialog_state.dart_defines_modal {
        use crate::new_session_dialog::DartDefinesPane;
        if modal.active_pane == DartDefinesPane::Edit {
            modal.input_char(c);
        }
    }
    UpdateResult::none()
}

/// Handle backspace in dart defines edit pane
pub fn handle_dart_defines_backspace(state: &mut AppState) -> UpdateResult {
    if let Some(ref mut modal) = state.new_session_dialog_state.dart_defines_modal {
        use crate::new_session_dialog::DartDefinesPane;
        if modal.active_pane == DartDefinesPane::Edit {
            modal.backspace();
        }
    }
    UpdateResult::none()
}

/// Handle save action in dart defines edit pane
pub fn handle_dart_defines_save(state: &mut AppState) -> UpdateResult {
    if let Some(ref mut modal) = state.new_session_dialog_state.dart_defines_modal {
        modal.save_edit();
    }
    UpdateResult::none()
}

/// Handle delete action in dart defines edit pane
pub fn handle_dart_defines_delete(state: &mut AppState) -> UpdateResult {
    if let Some(ref mut modal) = state.new_session_dialog_state.dart_defines_modal {
        modal.delete_selected();
    }
    UpdateResult::none()
}
