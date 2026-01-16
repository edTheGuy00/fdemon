//! NewSessionDialog navigation handlers
//!
//! Handles pane/tab switching and field navigation in the NewSessionDialog.

use crate::app::handler::{UpdateAction, UpdateResult};
use crate::app::message::Message;
use crate::app::state::{AppState, UiMode};

/// Handle pane switch (Tab key)
pub fn handle_switch_pane(state: &mut AppState) -> UpdateResult {
    state.new_session_dialog_state.toggle_pane_focus();
    UpdateResult::none()
}

/// Handle tab switch (Connected/Bootable tabs)
pub fn handle_switch_tab(
    state: &mut AppState,
    tab: crate::tui::widgets::TargetTab,
) -> UpdateResult {
    // Check if we need to trigger discovery BEFORE switch_tab modifies state
    let needs_bootable_discovery = tab == crate::tui::widgets::TargetTab::Bootable
        && state
            .new_session_dialog_state
            .target_selector
            .ios_simulators
            .is_empty()
        && state
            .new_session_dialog_state
            .target_selector
            .android_avds
            .is_empty()
        && !state
            .new_session_dialog_state
            .target_selector
            .bootable_loading;

    state.new_session_dialog_state.target_selector.set_tab(tab);

    // Trigger bootable device discovery if switching to Bootable tab and not loaded
    if needs_bootable_discovery {
        state
            .new_session_dialog_state
            .target_selector
            .bootable_loading = true;
        return UpdateResult::action(UpdateAction::DiscoverBootableDevices);
    }
    UpdateResult::none()
}

/// Handle tab toggle (Alt+Tab)
pub fn handle_toggle_tab(state: &mut AppState) -> UpdateResult {
    let new_tab = state
        .new_session_dialog_state
        .target_selector
        .active_tab
        .toggle();
    handle_switch_tab(state, new_tab)
}

/// Handle field navigation down (Tab in right pane)
pub fn handle_field_next(state: &mut AppState) -> UpdateResult {
    use crate::app::new_session_dialog::DialogPane;
    if state.new_session_dialog_state.focused_pane == DialogPane::LaunchContext {
        let current = state.new_session_dialog_state.launch_context.focused_field;
        state.new_session_dialog_state.launch_context.focused_field = current.next();
    }
    UpdateResult::none()
}

/// Handle field navigation up (Shift+Tab in right pane)
pub fn handle_field_prev(state: &mut AppState) -> UpdateResult {
    use crate::app::new_session_dialog::DialogPane;
    if state.new_session_dialog_state.focused_pane == DialogPane::LaunchContext {
        let current = state.new_session_dialog_state.launch_context.focused_field;
        state.new_session_dialog_state.launch_context.focused_field = current.prev();
    }
    UpdateResult::none()
}

/// Handle field activation (Enter on a field)
pub fn handle_field_activate(
    state: &mut AppState,
    update_fn: fn(&mut AppState, Message) -> UpdateResult,
) -> UpdateResult {
    use crate::app::new_session_dialog::{DialogPane, FuzzyModalType, LaunchContextField};

    if state.new_session_dialog_state.focused_pane != DialogPane::LaunchContext {
        return UpdateResult::none();
    }

    let current_field = state.new_session_dialog_state.launch_context.focused_field;
    match current_field {
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
            let next = current_field.next();
            state.new_session_dialog_state.launch_context.focused_field = next;
        }

        LaunchContextField::Flavor => {
            // Check if flavor is editable based on selected config
            if !state
                .new_session_dialog_state
                .launch_context
                .is_flavor_editable()
            {
                // VSCode configs are read-only, skip to next field
                let next = current_field.next();
                state.new_session_dialog_state.launch_context.focused_field = next;
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
            if !state
                .new_session_dialog_state
                .launch_context
                .are_dart_defines_editable()
            {
                // VSCode configs are read-only, skip to next field
                let next = current_field.next();
                state.new_session_dialog_state.launch_context.focused_field = next;
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

/// Opens the new session dialog and triggers device discovery.
///
/// Loads launch configurations from the project path and initializes
/// the dialog state. If no configurations are found, defaults are used.
pub fn handle_open_new_session_dialog(state: &mut AppState) -> UpdateResult {
    // Load configs with error handling
    let configs = crate::config::load_all_configs(&state.project_path);

    // Log warning if no configs found (not an error, just informational)
    if configs.configs.is_empty() {
        tracing::info!("No launch configurations found, using defaults");
    }

    // Show the dialog
    state.show_new_session_dialog(configs);

    // Trigger device discovery
    UpdateResult::action(UpdateAction::DiscoverDevices)
}

/// Closes the new session dialog and returns to the appropriate UI mode.
///
/// If sessions are running, returns to Normal mode. Otherwise, remains
/// in Normal mode (as startup state).
pub fn handle_close_new_session_dialog(state: &mut AppState) -> UpdateResult {
    state.hide_new_session_dialog();

    // Return to appropriate UI mode based on session state
    if state.session_manager.has_running_sessions() {
        state.ui_mode = UiMode::Normal;
    } else {
        // No sessions, stay in startup mode
        state.ui_mode = UiMode::Normal;
    }

    UpdateResult::none()
}

/// Handles the Escape key in the new session dialog.
///
/// Priority order:
/// 1. Close fuzzy modal if open
/// 2. Close dart defines modal if open (saves changes)
/// 3. Close dialog if sessions exist
/// 4. Quit if no sessions (in Startup mode, nowhere else to go)
pub fn handle_new_session_dialog_escape(state: &mut AppState) -> UpdateResult {
    // Priority 1: Close fuzzy modal
    if state.new_session_dialog_state.is_fuzzy_modal_open() {
        state.new_session_dialog_state.fuzzy_modal = None;
        return UpdateResult::none();
    }

    // Priority 2: Close dart defines modal (with save)
    if state.new_session_dialog_state.is_dart_defines_modal_open() {
        state
            .new_session_dialog_state
            .close_dart_defines_modal_with_changes();
        return UpdateResult::none();
    }

    // Priority 3: Close dialog (only if sessions exist)
    if state.session_manager.has_running_sessions() {
        return UpdateResult::message(Message::CloseNewSessionDialog);
    }

    // Priority 4: No sessions in Startup mode - quit immediately
    // There's nowhere else to go, so Escape should exit the app
    UpdateResult::message(Message::Quit)
}
