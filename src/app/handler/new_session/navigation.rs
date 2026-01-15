//! NewSessionDialog navigation handlers
//!
//! Handles pane/tab switching and field navigation in the NewSessionDialog.

use crate::app::handler::{UpdateAction, UpdateResult};
use crate::app::message::Message;
use crate::app::state::AppState;

/// Handle pane switch (Tab key)
pub fn handle_switch_pane(state: &mut AppState) -> UpdateResult {
    state.new_session_dialog_state.switch_pane();
    UpdateResult::none()
}

/// Handle tab switch (Connected/Bootable tabs)
pub fn handle_switch_tab(
    state: &mut AppState,
    tab: crate::tui::widgets::TargetTab,
) -> UpdateResult {
    // Check if we need to trigger discovery BEFORE switch_tab modifies state
    let needs_bootable_discovery = tab == crate::tui::widgets::TargetTab::Bootable
        && state.new_session_dialog_state.bootable_devices.is_empty()
        && !state.new_session_dialog_state.loading_bootable;

    state.new_session_dialog_state.switch_tab(tab);

    // Trigger bootable device discovery if switching to Bootable tab and not loaded
    if needs_bootable_discovery {
        state.new_session_dialog_state.loading_bootable = true;
        return UpdateResult::action(UpdateAction::DiscoverBootableDevices);
    }
    UpdateResult::none()
}

/// Handle tab toggle (Alt+Tab)
pub fn handle_toggle_tab(state: &mut AppState) -> UpdateResult {
    let new_tab = state.new_session_dialog_state.target_tab.toggle();
    handle_switch_tab(state, new_tab)
}

/// Handle field navigation down (Tab in right pane)
pub fn handle_field_next(state: &mut AppState) -> UpdateResult {
    use crate::tui::widgets::DialogPane;
    if state.new_session_dialog_state.active_pane == DialogPane::Right {
        state.new_session_dialog_state.context_down();
    }
    UpdateResult::none()
}

/// Handle field navigation up (Shift+Tab in right pane)
pub fn handle_field_prev(state: &mut AppState) -> UpdateResult {
    use crate::tui::widgets::DialogPane;
    if state.new_session_dialog_state.active_pane == DialogPane::Right {
        state.new_session_dialog_state.context_up();
    }
    UpdateResult::none()
}

/// Handle field activation (Enter on a field)
pub fn handle_field_activate(
    state: &mut AppState,
    update_fn: fn(&mut AppState, Message) -> UpdateResult,
) -> UpdateResult {
    use crate::tui::widgets::{DialogPane, FuzzyModalType, LaunchContextField};

    if state.new_session_dialog_state.active_pane != DialogPane::Right {
        return UpdateResult::none();
    }

    match state.new_session_dialog_state.active_field {
        LaunchContextField::Config => {
            // Open config fuzzy modal
            return update_fn(
                state,
                Message::NewSessionDialogOpenFuzzyModal {
                    modal_type: FuzzyModalType::Config,
                },
            );
        }

        LaunchContextField::Mode => {
            // Mode uses left/right arrows, Enter moves to next field
            state.new_session_dialog_state.context_down();
        }

        LaunchContextField::Flavor => {
            // Check if flavor is editable based on selected config
            if !state.new_session_dialog_state.is_flavor_editable() {
                // VSCode configs are read-only, skip to next field
                state.new_session_dialog_state.context_down();
                return UpdateResult::none();
            }

            // Open flavor fuzzy modal
            return update_fn(
                state,
                Message::NewSessionDialogOpenFuzzyModal {
                    modal_type: FuzzyModalType::Flavor,
                },
            );
        }

        LaunchContextField::DartDefines => {
            // Check if dart defines are editable
            if !state.new_session_dialog_state.are_dart_defines_editable() {
                // VSCode configs are read-only, skip to next field
                state.new_session_dialog_state.context_down();
                return UpdateResult::none();
            }

            // Open dart defines modal
            return update_fn(state, Message::NewSessionDialogOpenDartDefinesModal);
        }

        LaunchContextField::Launch => {
            // Trigger launch
            return update_fn(state, Message::NewSessionDialogLaunch);
        }
    }

    UpdateResult::none()
}
