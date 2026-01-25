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
/// Auto-creates a default config if none is selected and flavor is being set (not cleared).
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

    // Determine if we need to auto-create a config
    // Only create if setting a flavor (Some), not when clearing (None)
    let needs_auto_create = state
        .new_session_dialog_state
        .launch_context
        .selected_config_index
        .is_none()
        && flavor.is_some();

    // Auto-create config if needed
    if needs_auto_create {
        state
            .new_session_dialog_state
            .launch_context
            .create_and_select_default_config();
        if let Some(config) = state
            .new_session_dialog_state
            .launch_context
            .selected_config()
        {
            tracing::info!(
                "Auto-created config '{}' for flavor selection",
                config.config.name
            );
        }
        // Now selected_config_index is Some, pointing to new config
    }

    // Apply the flavor to state
    state
        .new_session_dialog_state
        .launch_context
        .set_flavor(flavor.clone());
    state.new_session_dialog_state.close_modal();

    // Determine if we should auto-save
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
/// Auto-creates a default config if none is selected and dart-defines are being set (not cleared).
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

    // Determine if we need to auto-create a config
    // Only create if adding defines (non-empty), not when clearing (empty vec)
    let needs_auto_create = state
        .new_session_dialog_state
        .launch_context
        .selected_config_index
        .is_none()
        && !defines.is_empty();

    // Auto-create config if needed
    if needs_auto_create {
        state
            .new_session_dialog_state
            .launch_context
            .create_and_select_default_config();
        if let Some(config) = state
            .new_session_dialog_state
            .launch_context
            .selected_config()
        {
            tracing::info!(
                "Auto-created config '{}' for dart-defines",
                config.config.name
            );
        }
        // Now selected_config_index is Some, pointing to new config
    }

    // Apply the dart-defines to state
    state
        .new_session_dialog_state
        .launch_context
        .set_dart_defines(defines.clone());
    state
        .new_session_dialog_state
        .close_dart_defines_modal_with_changes();

    // Determine if we should auto-save
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

/// Handles entry point selection from the fuzzy modal.
///
/// - "(default)" selection clears the entry point (Flutter uses lib/main.dart)
/// - Path selection sets the entry point
/// - Auto-creates FDemon config if none selected and setting a value
/// - Triggers auto-save for FDemon configurations
pub fn handle_entry_point_selected(state: &mut AppState, selected: Option<String>) -> UpdateResult {
    use crate::config::ConfigSource;
    use std::path::PathBuf;

    // Check if field is editable FIRST
    if !state
        .new_session_dialog_state
        .launch_context
        .is_entry_point_editable()
    {
        state.new_session_dialog_state.close_modal();
        return UpdateResult::none();
    }

    // Parse selection into Option<PathBuf>
    let entry_point = selected.filter(|s| s != "(default)").map(PathBuf::from);

    // Determine if we need to auto-create a config
    // Only create if setting an entry point (Some), not when clearing (None)
    let needs_auto_create = state
        .new_session_dialog_state
        .launch_context
        .selected_config_index
        .is_none()
        && entry_point.is_some();

    // Auto-create config if needed
    if needs_auto_create {
        state
            .new_session_dialog_state
            .launch_context
            .create_and_select_default_config();
        if let Some(config) = state
            .new_session_dialog_state
            .launch_context
            .selected_config()
        {
            tracing::info!(
                "Auto-created config '{}' for entry point selection",
                config.config.name
            );
        }
    }

    // Apply the entry point to state
    state
        .new_session_dialog_state
        .launch_context
        .set_entry_point(entry_point);
    state.new_session_dialog_state.close_modal();

    // Determine if we should auto-save
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
            || params.entry_point.is_some()
        {
            let mut cfg = LaunchConfig {
                name: params.config_name.unwrap_or_else(|| "Session".to_string()),
                device: device.id.clone(),
                mode: params.mode,
                flavor: params.flavor,
                entry_point: params.entry_point,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::new_session_dialog::{DartDefine, FuzzyModalType};
    use crate::app::state::{AppState, UiMode};
    use crate::config::priority::SourcedConfig;
    use crate::config::types::{ConfigSource, LaunchConfig};

    #[test]
    fn test_flavor_selected_no_config_creates_default() {
        let mut state = AppState::default();
        state.ui_mode = UiMode::NewSessionDialog;
        // No config selected
        assert!(state
            .new_session_dialog_state
            .launch_context
            .selected_config_index
            .is_none());

        let result = handle_flavor_selected(&mut state, Some("development".to_string()));

        // Config should be created and selected
        assert!(state
            .new_session_dialog_state
            .launch_context
            .selected_config_index
            .is_some());
        let idx = state
            .new_session_dialog_state
            .launch_context
            .selected_config_index
            .unwrap();
        let config = &state
            .new_session_dialog_state
            .launch_context
            .configs
            .configs[idx];

        assert_eq!(config.config.name, "Default");
        assert_eq!(config.source, ConfigSource::FDemon);

        // Verify flavor was set in launch_context state (not config struct)
        assert_eq!(
            state.new_session_dialog_state.launch_context.flavor,
            Some("development".to_string())
        );

        // Should trigger auto-save
        assert!(matches!(
            result.action,
            Some(UpdateAction::AutoSaveConfig { .. })
        ));
    }

    #[test]
    fn test_flavor_cleared_no_config_no_create() {
        let mut state = AppState::default();
        state.ui_mode = UiMode::NewSessionDialog;

        // Clear flavor (set to None) - should NOT create config
        let result = handle_flavor_selected(&mut state, None);

        assert!(state
            .new_session_dialog_state
            .launch_context
            .selected_config_index
            .is_none());
        assert!(state
            .new_session_dialog_state
            .launch_context
            .configs
            .configs
            .is_empty());
        assert!(result.action.is_none());
    }

    #[test]
    fn test_flavor_selected_existing_config_no_create() {
        let mut state = AppState::default();
        state.ui_mode = UiMode::NewSessionDialog;

        // Add and select existing config
        state
            .new_session_dialog_state
            .launch_context
            .configs
            .configs
            .push(SourcedConfig {
                config: LaunchConfig {
                    name: "Existing".to_string(),
                    ..Default::default()
                },
                source: ConfigSource::FDemon,
                display_name: "Existing".to_string(),
            });
        state
            .new_session_dialog_state
            .launch_context
            .selected_config_index = Some(0);

        let _result = handle_flavor_selected(&mut state, Some("staging".to_string()));

        // Should NOT create new config, just update existing
        assert_eq!(
            state
                .new_session_dialog_state
                .launch_context
                .configs
                .configs
                .len(),
            1
        );

        // Verify flavor was set in launch_context state (not config struct)
        assert_eq!(
            state.new_session_dialog_state.launch_context.flavor,
            Some("staging".to_string())
        );
    }

    #[test]
    fn test_flavor_selected_vscode_config_no_save() {
        let mut state = AppState::default();
        state.ui_mode = UiMode::NewSessionDialog;

        // Add VSCode config (read-only)
        state
            .new_session_dialog_state
            .launch_context
            .configs
            .configs
            .push(SourcedConfig {
                config: LaunchConfig {
                    name: "VSCode Config".to_string(),
                    ..Default::default()
                },
                source: ConfigSource::VSCode,
                display_name: "VSCode Config".to_string(),
            });
        state
            .new_session_dialog_state
            .launch_context
            .selected_config_index = Some(0);

        let result = handle_flavor_selected(&mut state, Some("production".to_string()));

        // Should NOT trigger auto-save for VSCode config
        assert!(result.action.is_none());
    }

    #[test]
    fn test_dart_defines_updated_no_config_creates_default() {
        let mut state = AppState::default();
        state.ui_mode = UiMode::NewSessionDialog;
        // No config selected
        assert!(state
            .new_session_dialog_state
            .launch_context
            .selected_config_index
            .is_none());

        let defines = vec![
            DartDefine::new("API_URL", "https://api.dev"),
            DartDefine::new("DEBUG_MODE", "true"),
        ];

        let result = handle_dart_defines_updated(&mut state, defines);

        // Config should be created and selected
        assert!(state
            .new_session_dialog_state
            .launch_context
            .selected_config_index
            .is_some());
        let idx = state
            .new_session_dialog_state
            .launch_context
            .selected_config_index
            .unwrap();
        let config = &state
            .new_session_dialog_state
            .launch_context
            .configs
            .configs[idx];

        assert_eq!(config.config.name, "Default");
        assert_eq!(config.source, ConfigSource::FDemon);

        // Verify dart_defines were set in launch_context state (not config struct)
        let state_defines = &state.new_session_dialog_state.launch_context.dart_defines;
        assert_eq!(state_defines.len(), 2);
        assert_eq!(state_defines[0].key, "API_URL");
        assert_eq!(state_defines[0].value, "https://api.dev");
        assert_eq!(state_defines[1].key, "DEBUG_MODE");
        assert_eq!(state_defines[1].value, "true");

        // Should trigger auto-save
        assert!(matches!(
            result.action,
            Some(UpdateAction::AutoSaveConfig { .. })
        ));
    }

    #[test]
    fn test_dart_defines_cleared_no_config_no_create() {
        let mut state = AppState::default();
        state.ui_mode = UiMode::NewSessionDialog;

        // Clear dart-defines (empty vec) - should NOT create config
        let result = handle_dart_defines_updated(&mut state, vec![]);

        assert!(state
            .new_session_dialog_state
            .launch_context
            .selected_config_index
            .is_none());
        assert!(state
            .new_session_dialog_state
            .launch_context
            .configs
            .configs
            .is_empty());
        assert!(result.action.is_none());
    }

    #[test]
    fn test_dart_defines_updated_existing_config_no_create() {
        let mut state = AppState::default();
        state.ui_mode = UiMode::NewSessionDialog;

        // Add and select existing config
        state
            .new_session_dialog_state
            .launch_context
            .configs
            .configs
            .push(SourcedConfig {
                config: LaunchConfig {
                    name: "Existing".to_string(),
                    ..Default::default()
                },
                source: ConfigSource::FDemon,
                display_name: "Existing".to_string(),
            });
        state
            .new_session_dialog_state
            .launch_context
            .selected_config_index = Some(0);

        let defines = vec![DartDefine::new("ENV", "staging")];

        let _result = handle_dart_defines_updated(&mut state, defines);

        // Should NOT create new config, just update existing
        assert_eq!(
            state
                .new_session_dialog_state
                .launch_context
                .configs
                .configs
                .len(),
            1
        );

        // Verify dart_defines were set in launch_context state (not config struct)
        let state_defines = &state.new_session_dialog_state.launch_context.dart_defines;
        assert_eq!(state_defines.len(), 1);
        assert_eq!(state_defines[0].key, "ENV");
        assert_eq!(state_defines[0].value, "staging");
    }

    #[test]
    fn test_dart_defines_vscode_config_no_save() {
        let mut state = AppState::default();
        state.ui_mode = UiMode::NewSessionDialog;

        // Add VSCode config (read-only)
        state
            .new_session_dialog_state
            .launch_context
            .configs
            .configs
            .push(SourcedConfig {
                config: LaunchConfig {
                    name: "VSCode Config".to_string(),
                    ..Default::default()
                },
                source: ConfigSource::VSCode,
                display_name: "VSCode Config".to_string(),
            });
        state
            .new_session_dialog_state
            .launch_context
            .selected_config_index = Some(0);

        let defines = vec![DartDefine::new("KEY", "value")];

        let result = handle_dart_defines_updated(&mut state, defines);

        // Should NOT trigger auto-save for VSCode config
        assert!(result.action.is_none());
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Entry Point Tests for handle_launch
    // ─────────────────────────────────────────────────────────────────────────

    /// Helper to create a test device
    fn test_device() -> crate::daemon::Device {
        crate::daemon::Device {
            id: "emulator-5554".to_string(),
            name: "Android Emulator".to_string(),
            platform: "android".to_string(),
            emulator: true,
            category: None,
            platform_type: None,
            ephemeral: false,
            emulator_id: None,
        }
    }

    #[test]
    fn test_handle_launch_entry_point_creates_config() {
        use std::path::PathBuf;

        let mut state = AppState::default();
        state.ui_mode = UiMode::NewSessionDialog;

        // Add a connected device and select it
        // Note: selected_index = 1 because index 0 is the group header
        state
            .new_session_dialog_state
            .target_selector
            .connected_devices
            .push(test_device());
        state
            .new_session_dialog_state
            .target_selector
            .selected_index = 1;

        // Set ONLY entry_point (no config, no flavor, no dart_defines)
        // This should trigger config creation
        state.new_session_dialog_state.launch_context.entry_point =
            Some(PathBuf::from("lib/main_dev.dart"));

        let result = handle_launch(&mut state);

        // Should return SpawnSession action with config
        match result.action {
            Some(UpdateAction::SpawnSession { config, .. }) => {
                // Config should be created because entry_point is set
                assert!(
                    config.is_some(),
                    "Config should be created when entry_point is set"
                );
                let cfg = config.unwrap();
                assert_eq!(
                    cfg.entry_point,
                    Some(PathBuf::from("lib/main_dev.dart")),
                    "entry_point should be passed to LaunchConfig"
                );
            }
            _ => panic!("Expected SpawnSession action, got {:?}", result.action),
        }
    }

    #[test]
    fn test_handle_launch_with_entry_point_and_flavor() {
        use std::path::PathBuf;

        let mut state = AppState::default();
        state.ui_mode = UiMode::NewSessionDialog;

        // Add a connected device and select it
        // Note: selected_index = 1 because index 0 is the group header
        state
            .new_session_dialog_state
            .target_selector
            .connected_devices
            .push(test_device());
        state
            .new_session_dialog_state
            .target_selector
            .selected_index = 1;

        // Set both entry_point and flavor
        state.new_session_dialog_state.launch_context.entry_point =
            Some(PathBuf::from("lib/main_staging.dart"));
        state.new_session_dialog_state.launch_context.flavor = Some("staging".to_string());

        let result = handle_launch(&mut state);

        // Should return SpawnSession action with config containing both
        match result.action {
            Some(UpdateAction::SpawnSession { config, .. }) => {
                assert!(config.is_some(), "Config should be created");
                let cfg = config.unwrap();
                assert_eq!(
                    cfg.entry_point,
                    Some(PathBuf::from("lib/main_staging.dart")),
                    "entry_point should be in config"
                );
                assert_eq!(
                    cfg.flavor,
                    Some("staging".to_string()),
                    "flavor should be in config"
                );
            }
            _ => panic!("Expected SpawnSession action, got {:?}", result.action),
        }
    }

    #[test]
    fn test_handle_launch_without_entry_point_no_config() {
        let mut state = AppState::default();
        state.ui_mode = UiMode::NewSessionDialog;

        // Add a connected device and select it
        // Note: selected_index = 1 because index 0 is the group header
        state
            .new_session_dialog_state
            .target_selector
            .connected_devices
            .push(test_device());
        state
            .new_session_dialog_state
            .target_selector
            .selected_index = 1;

        // No entry_point, no flavor, no dart_defines, no config name
        // All launch context fields at defaults

        let result = handle_launch(&mut state);

        // Should return SpawnSession action WITHOUT config
        match result.action {
            Some(UpdateAction::SpawnSession { config, .. }) => {
                assert!(
                    config.is_none(),
                    "Config should NOT be created when no launch params are set"
                );
            }
            _ => panic!("Expected SpawnSession action, got {:?}", result.action),
        }
    }

    #[test]
    fn test_handle_launch_entry_point_from_vscode_config() {
        use std::path::PathBuf;

        let mut state = AppState::default();
        state.ui_mode = UiMode::NewSessionDialog;

        // Add a connected device and select it
        // Note: selected_index = 1 because index 0 is the group header
        state
            .new_session_dialog_state
            .target_selector
            .connected_devices
            .push(test_device());
        state
            .new_session_dialog_state
            .target_selector
            .selected_index = 1;

        // Add VSCode config with entry_point (simulating VSCode's program field)
        state
            .new_session_dialog_state
            .launch_context
            .configs
            .configs
            .push(SourcedConfig {
                config: LaunchConfig {
                    name: "Development".to_string(),
                    entry_point: Some(PathBuf::from("lib/main_dev.dart")),
                    flavor: Some("dev".to_string()),
                    ..Default::default()
                },
                source: ConfigSource::VSCode,
                display_name: "Development (VSCode)".to_string(),
            });

        // Select the config - this should apply entry_point to state
        state
            .new_session_dialog_state
            .launch_context
            .select_config(Some(0));

        // Verify entry_point was applied from config
        assert_eq!(
            state.new_session_dialog_state.launch_context.entry_point,
            Some(PathBuf::from("lib/main_dev.dart"))
        );

        let result = handle_launch(&mut state);

        // Should return SpawnSession with config containing entry_point
        match result.action {
            Some(UpdateAction::SpawnSession { config, .. }) => {
                assert!(config.is_some(), "Config should be created");
                let cfg = config.unwrap();
                assert_eq!(
                    cfg.entry_point,
                    Some(PathBuf::from("lib/main_dev.dart")),
                    "entry_point from VSCode config should be passed through"
                );
            }
            _ => panic!("Expected SpawnSession action, got {:?}", result.action),
        }
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Phase 3 Task 06: Entry Point Activation Handler Tests
    // ─────────────────────────────────────────────────────────────────────────

    // Note: Entry point activation is now handled through fuzzy_modal.rs
    // These tests verify the integration with the modal system

    // ─────────────────────────────────────────────────────────────────────────
    // Phase 3 Task 07: Entry Point Selection Handler Tests
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_entry_point_selected_sets_path() {
        use std::path::PathBuf;

        let mut state = AppState::default();
        state.ui_mode = UiMode::NewSessionDialog;

        // Add FDemon config so auto-save can trigger
        state
            .new_session_dialog_state
            .launch_context
            .configs
            .configs
            .push(SourcedConfig {
                config: LaunchConfig::default(),
                source: ConfigSource::FDemon,
                display_name: "Default".to_string(),
            });
        state
            .new_session_dialog_state
            .launch_context
            .selected_config_index = Some(0);

        let result = handle_entry_point_selected(&mut state, Some("lib/main_dev.dart".to_string()));

        // Entry point should be set
        assert_eq!(
            state.new_session_dialog_state.launch_context.entry_point,
            Some(PathBuf::from("lib/main_dev.dart"))
        );

        // Should trigger auto-save
        assert!(matches!(
            result.action,
            Some(UpdateAction::AutoSaveConfig { .. })
        ));
    }

    #[test]
    fn test_entry_point_selected_default_clears() {
        use std::path::PathBuf;

        let mut state = AppState::default();
        state.ui_mode = UiMode::NewSessionDialog;
        state.new_session_dialog_state.launch_context.entry_point =
            Some(PathBuf::from("lib/old.dart"));

        let _result = handle_entry_point_selected(&mut state, Some("(default)".to_string()));

        // Entry point should be cleared
        assert_eq!(
            state.new_session_dialog_state.launch_context.entry_point,
            None
        );
    }

    #[test]
    fn test_entry_point_selected_none_clears() {
        use std::path::PathBuf;

        let mut state = AppState::default();
        state.ui_mode = UiMode::NewSessionDialog;
        state.new_session_dialog_state.launch_context.entry_point =
            Some(PathBuf::from("lib/old.dart"));

        let _result = handle_entry_point_selected(&mut state, None);

        // Entry point should be cleared
        assert_eq!(
            state.new_session_dialog_state.launch_context.entry_point,
            None
        );
    }

    #[test]
    fn test_entry_point_selected_auto_creates_config() {
        use std::path::PathBuf;

        let mut state = AppState::default();
        state.ui_mode = UiMode::NewSessionDialog;
        // No config selected
        assert!(state
            .new_session_dialog_state
            .launch_context
            .selected_config_index
            .is_none());

        let result = handle_entry_point_selected(&mut state, Some("lib/main_dev.dart".to_string()));

        // Config should be created and selected
        assert!(state
            .new_session_dialog_state
            .launch_context
            .selected_config_index
            .is_some());
        let idx = state
            .new_session_dialog_state
            .launch_context
            .selected_config_index
            .unwrap();
        let config = &state
            .new_session_dialog_state
            .launch_context
            .configs
            .configs[idx];

        assert_eq!(config.config.name, "Default");
        assert_eq!(config.source, ConfigSource::FDemon);

        // Entry point should be set
        assert_eq!(
            state.new_session_dialog_state.launch_context.entry_point,
            Some(PathBuf::from("lib/main_dev.dart"))
        );

        // Should trigger auto-save
        assert!(matches!(
            result.action,
            Some(UpdateAction::AutoSaveConfig { .. })
        ));
    }

    #[test]
    fn test_entry_point_cleared_no_config_no_create() {
        let mut state = AppState::default();
        state.ui_mode = UiMode::NewSessionDialog;

        // Clear entry point (set to default) - should NOT create config
        let result = handle_entry_point_selected(&mut state, Some("(default)".to_string()));

        assert!(state
            .new_session_dialog_state
            .launch_context
            .selected_config_index
            .is_none());
        assert!(state
            .new_session_dialog_state
            .launch_context
            .configs
            .configs
            .is_empty());
        assert!(result.action.is_none());
    }

    #[test]
    fn test_entry_point_selected_vscode_config_no_save() {
        let mut state = AppState::default();
        state.ui_mode = UiMode::NewSessionDialog;

        // Add VSCode config (read-only)
        state
            .new_session_dialog_state
            .launch_context
            .configs
            .configs
            .push(SourcedConfig {
                config: LaunchConfig::default(),
                source: ConfigSource::VSCode,
                display_name: "VSCode".to_string(),
            });
        state
            .new_session_dialog_state
            .launch_context
            .selected_config_index = Some(0);

        let result = handle_entry_point_selected(&mut state, Some("lib/main_dev.dart".to_string()));

        // Should NOT trigger auto-save for VSCode config
        // Note: The handler checks is_entry_point_editable() and returns early
        // Entry point should NOT be set because field is not editable
        assert!(result.action.is_none());
    }

    #[test]
    fn test_entry_point_selected_closes_modal() {
        use crate::app::new_session_dialog::FuzzyModalState;

        let mut state = AppState::default();
        state.ui_mode = UiMode::NewSessionDialog;

        // Simulate modal being open
        state.new_session_dialog_state.fuzzy_modal =
            Some(FuzzyModalState::new(FuzzyModalType::EntryPoint, vec![]));

        handle_entry_point_selected(&mut state, Some("lib/main.dart".to_string()));

        // Modal should be closed
        assert!(state.new_session_dialog_state.fuzzy_modal.is_none());
    }
}
