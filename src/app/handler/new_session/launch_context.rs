//! NewSessionDialog launch context handlers
//!
//! Handles config, mode, flavor selection and the launch action.

use crate::app::handler::{UpdateAction, UpdateResult};
use crate::app::state::AppState;

/// Cycles the Flutter mode forward (Debug → Profile → Release).
///
/// Only applies when the Mode field is focused in the LaunchContext pane.
/// Triggers auto-save for editable FDemon configurations.
pub fn handle_mode_next(state: &mut AppState) -> UpdateResult {
    use crate::app::new_session_dialog::{DialogPane, LaunchContextField};

    if state.new_session_dialog_state.focused_pane == DialogPane::LaunchContext
        && state.new_session_dialog_state.launch_context.focused_field == LaunchContextField::Mode
    {
        // Check if mode is editable
        if !state
            .new_session_dialog_state
            .launch_context
            .is_mode_editable()
        {
            return UpdateResult::none();
        }

        // Cycle mode
        state.new_session_dialog_state.launch_context.mode =
            match state.new_session_dialog_state.launch_context.mode {
                crate::config::FlutterMode::Debug => crate::config::FlutterMode::Profile,
                crate::config::FlutterMode::Profile => crate::config::FlutterMode::Release,
                crate::config::FlutterMode::Release => crate::config::FlutterMode::Debug,
            };

        // Trigger auto-save if FDemon config
        if let Some(config_idx) = state
            .new_session_dialog_state
            .launch_context
            .selected_config_index
        {
            if let Some(config) = state
                .new_session_dialog_state
                .launch_context
                .configs
                .configs
                .get(config_idx)
            {
                use crate::config::ConfigSource;
                if config.source == ConfigSource::FDemon {
                    return UpdateResult::action(UpdateAction::AutoSaveConfig {
                        configs: state
                            .new_session_dialog_state
                            .launch_context
                            .configs
                            .clone(),
                    });
                }
            }
        }
    }
    UpdateResult::none()
}

/// Cycles the Flutter mode backward (Release → Profile → Debug).
///
/// Only applies when the Mode field is focused in the LaunchContext pane.
/// Triggers auto-save for editable FDemon configurations.
pub fn handle_mode_prev(state: &mut AppState) -> UpdateResult {
    use crate::app::new_session_dialog::{DialogPane, LaunchContextField};

    if state.new_session_dialog_state.focused_pane == DialogPane::LaunchContext
        && state.new_session_dialog_state.launch_context.focused_field == LaunchContextField::Mode
    {
        // Check if mode is editable
        if !state
            .new_session_dialog_state
            .launch_context
            .is_mode_editable()
        {
            return UpdateResult::none();
        }

        // Cycle mode backwards
        state.new_session_dialog_state.launch_context.mode =
            match state.new_session_dialog_state.launch_context.mode {
                crate::config::FlutterMode::Debug => crate::config::FlutterMode::Release,
                crate::config::FlutterMode::Profile => crate::config::FlutterMode::Debug,
                crate::config::FlutterMode::Release => crate::config::FlutterMode::Profile,
            };

        // Trigger auto-save if FDemon config
        if let Some(config_idx) = state
            .new_session_dialog_state
            .launch_context
            .selected_config_index
        {
            if let Some(config) = state
                .new_session_dialog_state
                .launch_context
                .configs
                .configs
                .get(config_idx)
            {
                use crate::config::ConfigSource;
                if config.source == ConfigSource::FDemon {
                    return UpdateResult::action(UpdateAction::AutoSaveConfig {
                        configs: state
                            .new_session_dialog_state
                            .launch_context
                            .configs
                            .clone(),
                    });
                }
            }
        }
    }
    UpdateResult::none()
}

/// Handles configuration selection from the fuzzy modal.
///
/// Applies the selected configuration and closes the modal.
pub fn handle_config_selected(state: &mut AppState, config_name: String) -> UpdateResult {
    state
        .new_session_dialog_state
        .launch_context
        .select_config_by_name(&config_name);
    state.new_session_dialog_state.close_modal();
    UpdateResult::none()
}

/// Handles flavor selection from the fuzzy modal.
///
/// Applies the selected flavor and closes the modal.
/// Triggers auto-save for editable FDemon configurations.
pub fn handle_flavor_selected(state: &mut AppState, flavor: Option<String>) -> UpdateResult {
    use crate::config::ConfigSource;

    // Check if flavor is editable
    if !state
        .new_session_dialog_state
        .launch_context
        .is_flavor_editable()
    {
        return UpdateResult::none();
    }

    // Determine if we should auto-save (must check before mutating state)
    let should_auto_save = if let Some(config_idx) = state
        .new_session_dialog_state
        .launch_context
        .selected_config_index
    {
        if let Some(config) = state
            .new_session_dialog_state
            .launch_context
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

    state
        .new_session_dialog_state
        .launch_context
        .set_flavor(flavor);
    state.new_session_dialog_state.close_modal();

    // Trigger auto-save if needed
    if should_auto_save {
        return UpdateResult::action(UpdateAction::AutoSaveConfig {
            configs: state
                .new_session_dialog_state
                .launch_context
                .configs
                .clone(),
        });
    }

    UpdateResult::none()
}

/// Handles dart defines updates from the modal.
///
/// Applies the updated dart defines and closes the modal.
/// Triggers auto-save for editable FDemon configurations.
pub fn handle_dart_defines_updated(
    state: &mut AppState,
    defines: Vec<crate::tui::widgets::DartDefine>,
) -> UpdateResult {
    use crate::config::ConfigSource;

    // Check if dart defines are editable
    if !state
        .new_session_dialog_state
        .launch_context
        .are_dart_defines_editable()
    {
        return UpdateResult::none();
    }

    // Determine if we should auto-save (must check before mutating state)
    let should_auto_save = if let Some(config_idx) = state
        .new_session_dialog_state
        .launch_context
        .selected_config_index
    {
        if let Some(config) = state
            .new_session_dialog_state
            .launch_context
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

    state
        .new_session_dialog_state
        .launch_context
        .set_dart_defines(defines);
    state
        .new_session_dialog_state
        .close_dart_defines_modal_with_changes();

    // Trigger auto-save if needed
    if should_auto_save {
        return UpdateResult::action(UpdateAction::AutoSaveConfig {
            configs: state
                .new_session_dialog_state
                .launch_context
                .configs
                .clone(),
        });
    }

    UpdateResult::none()
}

/// Launches a Flutter session with the current dialog configuration.
///
/// Validates that a device is selected and builds launch parameters
/// from the dialog state. Returns an error to the user if validation fails.
pub fn handle_launch(state: &mut AppState) -> UpdateResult {
    use crate::config::LaunchConfig;

    // Try to build launch params
    if let Some(params) = state.new_session_dialog_state.build_launch_params() {
        // Get device reference without unwrap
        let device = match state.new_session_dialog_state.selected_device() {
            Some(d) => d.clone(),
            None => {
                state
                    .new_session_dialog_state
                    .target_selector
                    .set_error("Device no longer available".to_string());
                return UpdateResult::none();
            }
        };

        // Check if device already has a running session
        if state
            .session_manager
            .find_by_device_id(&device.id)
            .is_some()
        {
            state
                .new_session_dialog_state
                .target_selector
                .set_error(format!(
                    "Device '{}' already has an active session",
                    device.name
                ));
            return UpdateResult::none();
        }

        // Build launch config if we have parameters
        let config = if params.config_name.is_some()
            || params.flavor.is_some()
            || !params.dart_defines.is_empty()
        {
            let mut cfg = LaunchConfig {
                name: params.config_name.unwrap_or_else(|| "Session".to_string()),
                device: device.id.clone(),
                mode: params.mode,
                flavor: params.flavor,
                ..Default::default()
            };

            // Parse dart_defines into HashMap
            for define in params.dart_defines {
                if let Some((key, value)) = define.split_once('=') {
                    cfg.dart_defines.insert(key.to_string(), value.to_string());
                }
            }

            Some(cfg)
        } else {
            None
        };

        // Create session in manager
        let session_result = if let Some(ref cfg) = config {
            state
                .session_manager
                .create_session_with_config(&device, cfg.clone())
        } else {
            state.session_manager.create_session(&device)
        };

        match session_result {
            Ok(session_id) => {
                tracing::info!(
                    "Session created for {} (id: {}, device: {})",
                    device.name,
                    session_id,
                    device.id
                );

                // Auto-switch to the newly created session
                state.session_manager.select_by_id(session_id);

                // Close the dialog and switch to Normal mode
                state.hide_new_session_dialog();
                state.ui_mode = crate::app::state::UiMode::Normal;

                // Return action to spawn session
                return UpdateResult::action(UpdateAction::SpawnSession {
                    session_id,
                    device,
                    config: config.map(Box::new),
                });
            }
            Err(e) => {
                // Max sessions reached or other error
                state
                    .new_session_dialog_state
                    .target_selector
                    .set_error(format!("Failed to create session: {}", e));
                return UpdateResult::none();
            }
        }
    } else {
        // Provide context-specific error message
        use crate::app::new_session_dialog::TargetTab;
        let active_tab = state.new_session_dialog_state.target_selector.active_tab;
        let connected_count = state
            .new_session_dialog_state
            .target_selector
            .connected_devices
            .len();

        let error_msg = match active_tab {
            TargetTab::Bootable => {
                if connected_count == 0 {
                    "No connected devices. Boot a device first, or switch to Connected tab."
                } else {
                    "Switch to Connected tab to select a running device for launch."
                }
            }
            TargetTab::Connected => {
                if connected_count == 0 {
                    "No connected devices. Connect a device or start an emulator."
                } else {
                    "Please select a device from the list."
                }
            }
        };

        state
            .new_session_dialog_state
            .target_selector
            .set_error(error_msg.to_string());
    }

    UpdateResult::none()
}

/// Handles successful configuration auto-save completion.
///
/// Called after FDemon configurations are automatically saved.
pub fn handle_config_saved(_state: &mut AppState) -> UpdateResult {
    // Config auto-save completed successfully
    // Could add a transient notification here if desired
    UpdateResult::none()
}

/// Handles configuration auto-save failure.
///
/// Logs the error and displays an error message to the user.
pub fn handle_config_save_failed(state: &mut AppState, error: String) -> UpdateResult {
    // Config auto-save failed
    tracing::warn!("Failed to auto-save config: {}", error);
    state
        .new_session_dialog_state
        .target_selector
        .set_error(format!("Failed to save config: {}", error));
    UpdateResult::none()
}
