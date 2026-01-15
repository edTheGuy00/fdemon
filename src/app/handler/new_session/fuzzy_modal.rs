//! NewSessionDialog fuzzy modal handlers
//!
//! Handles fuzzy search modal for config and flavor selection.

use crate::app::handler::UpdateResult;
use crate::app::message::Message;
use crate::app::new_session_dialog::FuzzyModalType;
use crate::app::state::AppState;
use tracing::warn;

/// Handle opening fuzzy modal
pub fn handle_open_fuzzy_modal(state: &mut AppState, modal_type: FuzzyModalType) -> UpdateResult {
    // Prevent opening a modal when another is already open
    if state.new_session_dialog_state.has_modal_open() {
        warn!("Cannot open fuzzy modal while another modal is open");
        return UpdateResult::none();
    }

    match modal_type {
        FuzzyModalType::Config => {
            state.new_session_dialog_state.open_config_modal();
            // Initial filter with empty query (show all)
            apply_fuzzy_filter(state);
        }
        FuzzyModalType::Flavor => {
            // TODO: Get flavors from project analysis
            // For now, use any existing flavor as suggestion
            let mut flavors = Vec::new();
            if let Some(ref flavor) = state.new_session_dialog_state.launch_context.flavor {
                if !flavor.is_empty() {
                    flavors.push(flavor.clone());
                }
            }
            state.new_session_dialog_state.open_flavor_modal(flavors);
            // Initial filter with empty query (show all)
            apply_fuzzy_filter(state);
        }
    };
    UpdateResult::none()
}

/// Handle closing fuzzy modal
pub fn handle_close_fuzzy_modal(state: &mut AppState) -> UpdateResult {
    state.new_session_dialog_state.close_modal();
    UpdateResult::none()
}

/// Handle fuzzy modal navigation up
pub fn handle_fuzzy_up(state: &mut AppState) -> UpdateResult {
    if let Some(ref mut modal) = state.new_session_dialog_state.fuzzy_modal {
        modal.navigate_up();
    }
    UpdateResult::none()
}

/// Handle fuzzy modal navigation down
pub fn handle_fuzzy_down(state: &mut AppState) -> UpdateResult {
    if let Some(ref mut modal) = state.new_session_dialog_state.fuzzy_modal {
        modal.navigate_down();
    }
    UpdateResult::none()
}

/// Handle fuzzy modal confirm selection
pub fn handle_fuzzy_confirm(
    state: &mut AppState,
    update_fn: fn(&mut AppState, Message) -> UpdateResult,
) -> UpdateResult {
    if let Some(ref modal) = state.new_session_dialog_state.fuzzy_modal {
        if let Some(value) = modal.selected_value() {
            match modal.modal_type {
                FuzzyModalType::Config => {
                    // Use the new config selected message
                    return update_fn(
                        state,
                        Message::NewSessionDialogConfigSelected { config_name: value },
                    );
                }
                FuzzyModalType::Flavor => {
                    // Use the new flavor selected message which handles auto-save
                    return update_fn(
                        state,
                        Message::NewSessionDialogFlavorSelected {
                            flavor: if value.is_empty() { None } else { Some(value) },
                        },
                    );
                }
            }
        }
    }
    state.new_session_dialog_state.close_modal();
    UpdateResult::none()
}

/// Handle fuzzy modal character input
pub fn handle_fuzzy_input(state: &mut AppState, c: char) -> UpdateResult {
    if let Some(ref mut modal) = state.new_session_dialog_state.fuzzy_modal {
        modal.input_char(c);
        apply_fuzzy_filter(state);
    }
    UpdateResult::none()
}

/// Handle fuzzy modal backspace
pub fn handle_fuzzy_backspace(state: &mut AppState) -> UpdateResult {
    if let Some(ref mut modal) = state.new_session_dialog_state.fuzzy_modal {
        modal.backspace();
        apply_fuzzy_filter(state);
    }
    UpdateResult::none()
}

/// Handle fuzzy modal clear query
pub fn handle_fuzzy_clear(state: &mut AppState) -> UpdateResult {
    if let Some(ref mut modal) = state.new_session_dialog_state.fuzzy_modal {
        modal.clear_query();
        apply_fuzzy_filter(state);
    }
    UpdateResult::none()
}

/// Apply fuzzy filter to current modal state
fn apply_fuzzy_filter(state: &mut AppState) {
    if let Some(ref mut modal) = state.new_session_dialog_state.fuzzy_modal {
        // Import the filter function from TUI layer
        use crate::tui::widgets::new_session_dialog::fuzzy_modal::fuzzy_filter;

        let query = &modal.query;
        let items = &modal.items;
        let filtered = fuzzy_filter(query, items);
        modal.update_filter(filtered);
    }
}
