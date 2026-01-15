//! NewSessionDialog launch context handlers
//!
//! Handles config, mode, flavor selection and the launch action.

use crate::app::handler::{UpdateAction, UpdateResult};
use crate::app::state::AppState;

/// Handle mode cycle forward (Right arrow on Mode field)
pub fn handle_mode_next(state: &mut AppState) -> UpdateResult {
    use crate::tui::widgets::{DialogPane, LaunchContextField};

    if state.new_session_dialog_state.active_pane == DialogPane::Right
        && state.new_session_dialog_state.active_field == LaunchContextField::Mode
    {
        // Check if mode is editable
        if !state.new_session_dialog_state.is_mode_editable() {
            return UpdateResult::none();
        }

        state.new_session_dialog_state.cycle_mode();

        // Trigger auto-save if FDemon config
        if let Some(config_idx) = state.new_session_dialog_state.selected_config {
            if let Some(config) = state
                .new_session_dialog_state
                .configs
                .configs
                .get(config_idx)
            {
                use crate::config::ConfigSource;
                if config.source == ConfigSource::FDemon {
                    return UpdateResult::action(UpdateAction::AutoSaveConfig {
                        configs: state.new_session_dialog_state.configs.clone(),
                    });
                }
            }
        }
    }
    UpdateResult::none()
}

/// Handle mode cycle backward (Left arrow on Mode field)
pub fn handle_mode_prev(state: &mut AppState) -> UpdateResult {
    use crate::tui::widgets::{DialogPane, LaunchContextField};

    if state.new_session_dialog_state.active_pane == DialogPane::Right
        && state.new_session_dialog_state.active_field == LaunchContextField::Mode
    {
        // Check if mode is editable
        if !state.new_session_dialog_state.is_mode_editable() {
            return UpdateResult::none();
        }

        state.new_session_dialog_state.cycle_mode_reverse();

        // Trigger auto-save if FDemon config
        if let Some(config_idx) = state.new_session_dialog_state.selected_config {
            if let Some(config) = state
                .new_session_dialog_state
                .configs
                .configs
                .get(config_idx)
            {
                use crate::config::ConfigSource;
                if config.source == ConfigSource::FDemon {
                    return UpdateResult::action(UpdateAction::AutoSaveConfig {
                        configs: state.new_session_dialog_state.configs.clone(),
                    });
                }
            }
        }
    }
    UpdateResult::none()
}

/// Handle config selection from fuzzy modal
pub fn handle_config_selected(state: &mut AppState, config_name: String) -> UpdateResult {
    // Find config index by name
    let idx = state
        .new_session_dialog_state
        .configs
        .configs
        .iter()
        .position(|c| c.display_name == config_name);

    state.new_session_dialog_state.select_config(idx);
    state.new_session_dialog_state.close_fuzzy_modal();
    UpdateResult::none()
}

/// Handle flavor selection from fuzzy modal
pub fn handle_flavor_selected(state: &mut AppState, flavor: Option<String>) -> UpdateResult {
    use crate::config::ConfigSource;

    // Check if flavor is editable
    if !state.new_session_dialog_state.is_flavor_editable() {
        return UpdateResult::none();
    }

    // Determine if we should auto-save (must check before mutating state)
    let should_auto_save = if let Some(config_idx) = state.new_session_dialog_state.selected_config
    {
        if let Some(config) = state
            .new_session_dialog_state
            .configs
            .configs
            .get(config_idx)
        {
            config.source == ConfigSource::FDemon
        } else {
            false
        }
    } else {
        false
    };

    state.new_session_dialog_state.flavor = flavor.unwrap_or_default();
    state.new_session_dialog_state.close_fuzzy_modal();

    // Trigger auto-save if needed
    if should_auto_save {
        return UpdateResult::action(UpdateAction::AutoSaveConfig {
            configs: state.new_session_dialog_state.configs.clone(),
        });
    }

    UpdateResult::none()
}

/// Handle dart defines updated from modal
pub fn handle_dart_defines_updated(
    state: &mut AppState,
    defines: Vec<crate::tui::widgets::DartDefine>,
) -> UpdateResult {
    use crate::config::ConfigSource;

    // Check if dart defines are editable
    if !state.new_session_dialog_state.are_dart_defines_editable() {
        return UpdateResult::none();
    }

    // Determine if we should auto-save (must check before mutating state)
    let should_auto_save = if let Some(config_idx) = state.new_session_dialog_state.selected_config
    {
        if let Some(config) = state
            .new_session_dialog_state
            .configs
            .configs
            .get(config_idx)
        {
            config.source == ConfigSource::FDemon
        } else {
            false
        }
    } else {
        false
    };

    state.new_session_dialog_state.dart_defines = defines;
    state.new_session_dialog_state.close_dart_defines_modal();

    // Trigger auto-save if needed
    if should_auto_save {
        return UpdateResult::action(UpdateAction::AutoSaveConfig {
            configs: state.new_session_dialog_state.configs.clone(),
        });
    }

    UpdateResult::none()
}

/// Handle launch button activation
pub fn handle_launch(state: &mut AppState) -> UpdateResult {
    use crate::tui::widgets::TargetTab;

    // Check which tab is active
    let active_tab = state.new_session_dialog_state.target_tab;

    // Get selected device based on active tab
    let device = if active_tab == TargetTab::Connected {
        state
            .new_session_dialog_state
            .selected_connected_device()
            .cloned()
    } else {
        None // Cannot launch bootable devices directly
    };

    if let Some(device) = device {
        // Build dart_defines as Vec<String> in "key=value" format
        let dart_defines: Vec<String> = state
            .new_session_dialog_state
            .dart_defines
            .iter()
            .map(|d| format!("{}={}", d.key, d.value))
            .collect();

        // Get config name if one is selected
        let config_name = state
            .new_session_dialog_state
            .selected_config
            .and_then(|idx| {
                state
                    .new_session_dialog_state
                    .configs
                    .configs
                    .get(idx)
                    .map(|c| c.display_name.clone())
            });

        let flavor = if state.new_session_dialog_state.flavor.is_empty() {
            None
        } else {
            Some(state.new_session_dialog_state.flavor.clone())
        };

        return UpdateResult::action(UpdateAction::LaunchFlutterSession {
            device,
            mode: state.new_session_dialog_state.mode,
            flavor,
            dart_defines,
            config_name,
        });
    } else {
        // Provide context-specific error message
        let error_msg = match active_tab {
            TargetTab::Bootable => {
                if state.new_session_dialog_state.connected_devices.is_empty() {
                    "No connected devices. Boot a device first, or switch to Connected tab."
                } else {
                    "Switch to Connected tab to select a running device for launch."
                }
            }
            TargetTab::Connected => {
                if state.new_session_dialog_state.connected_devices.is_empty() {
                    "No connected devices. Connect a device or start an emulator."
                } else {
                    "Please select a device from the list."
                }
            }
        };

        state
            .new_session_dialog_state
            .set_error(error_msg.to_string());
    }

    UpdateResult::none()
}

/// Handle config auto-save success
pub fn handle_config_saved(_state: &mut AppState) -> UpdateResult {
    // Config auto-save completed successfully
    // Could add a transient notification here if desired
    UpdateResult::none()
}

/// Handle config auto-save failure
pub fn handle_config_save_failed(state: &mut AppState, error: String) -> UpdateResult {
    // Config auto-save failed
    tracing::warn!("Failed to auto-save config: {}", error);
    state
        .new_session_dialog_state
        .set_error(format!("Failed to save config: {}", error));
    UpdateResult::none()
}
