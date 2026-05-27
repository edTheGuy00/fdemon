//! NewSessionDialog launch context handlers
//!
//! Handles config, mode, flavor selection and the launch action.

use crate::handler::{UpdateAction, UpdateResult};
use crate::state::AppState;

/// Cycles the Flutter mode forward (Debug → Profile → Release).
///
/// Only applies when the Mode field is focused in the LaunchContext pane.
/// Triggers auto-save for editable FDemon configurations.
pub fn handle_mode_next(state: &mut AppState) -> UpdateResult {
    use crate::new_session_dialog::{DialogPane, LaunchContextField};

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
    use crate::new_session_dialog::{DialogPane, LaunchContextField};

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

/// Sets the Flutter mode to a specific value.
///
/// Unlike `handle_mode_next` / `handle_mode_prev`, this function accepts an
/// explicit target mode. It is invoked from mouse click regions on the three
/// mode buttons so the user can jump directly to any mode.
///
/// - If the Mode field is not editable, returns `UpdateResult::none()`.
/// - Sets `focused_pane` and `focused_field` so a click also focuses the row,
///   matching the row-level `FocusField` region's effect.
/// - Does **not** short-circuit when `mode == current mode`; clicking the
///   already-selected button is valid (it focuses the field).
/// - Triggers auto-save for editable FDemon configurations.
pub fn handle_set_mode(state: &mut AppState, mode: crate::config::FlutterMode) -> UpdateResult {
    use crate::new_session_dialog::{DialogPane, LaunchContextField};

    // Gate: only proceed when the Mode field is editable
    if !state
        .new_session_dialog_state
        .launch_context
        .is_mode_editable()
    {
        return UpdateResult::none();
    }

    // Focus the LaunchContext pane and the Mode field so clicking a button
    // also moves keyboard focus there.
    state.new_session_dialog_state.focused_pane = DialogPane::LaunchContext;
    state.new_session_dialog_state.launch_context.focused_field = LaunchContextField::Mode;

    // Apply the mode
    state.new_session_dialog_state.launch_context.mode = mode;

    // Trigger auto-save if the selected config is an FDemon config
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
    defines: Vec<crate::new_session_dialog::DartDefine>,
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
/// When ≥1 devices are checked in the target selector, spawns one session per
/// checked device sharing the same launch context. When zero devices are checked,
/// behavior is identical to the legacy single-device path (cursor device).
///
/// Returns an error to the user if validation fails for all devices.
pub fn handle_launch(state: &mut AppState) -> UpdateResult {
    use fdemon_core::strip_ansi_codes;

    // Build the device list: checked set if non-empty, otherwise cursor device.
    let checked: Vec<fdemon_daemon::Device> = state
        .new_session_dialog_state
        .target_selector
        .checked_devices()
        .into_iter()
        .cloned()
        .collect();

    let devices: Vec<fdemon_daemon::Device> = if checked.is_empty() {
        // Single-device path — unchanged behavior: use cursor device.
        let cursor_device = match state.new_session_dialog_state.selected_device() {
            Some(d) => d.clone(),
            None => {
                // No device selected at all — show a context-sensitive error.
                use crate::new_session_dialog::TargetTab;
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
                return UpdateResult::none();
            }
        };
        vec![cursor_device]
    } else {
        checked
    };

    // Fan out: attempt to spawn one session per device.
    let mut actions: Vec<UpdateAction> = Vec::new();
    let mut skipped: Vec<(String, String)> = Vec::new(); // (device_name, reason)
    let mut first_session_id: Option<crate::session::SessionId> = None;
    // Track the first successfully-launched device for save_last_selection.
    let mut first_success: Option<(String, Option<String>)> = None; // (device_id, config_name)

    for device in devices.iter() {
        let params = state
            .new_session_dialog_state
            .build_launch_params_for_device(&device.id);

        match spawn_one(state, device, params) {
            Ok((session_id, action, config_name)) => {
                if first_session_id.is_none() {
                    first_session_id = Some(session_id);
                    first_success = Some((device.id.clone(), config_name));
                }
                actions.push(action);
            }
            Err(reason) => {
                skipped.push((device.name.clone(), reason));
            }
        }
    }

    // If all devices were skipped, surface the error and keep the dialog open.
    if actions.is_empty() {
        let error = summarize_skipped(&skipped);
        state
            .new_session_dialog_state
            .target_selector
            .set_error(error);
        return UpdateResult::none();
    }

    // Persist the first successfully-launched device for future auto-launch.
    // Done here (not inside spawn_one) so that a skipped device-0 is never
    // persisted — only the first device that actually creates a session counts.
    // Failures are non-fatal: a disk or permission issue must not block launch.
    if let Some((dev_id, cfg_name)) = first_success {
        if let Err(e) = crate::config::save_last_selection(
            &state.project_path,
            cfg_name.as_deref(),
            Some(&dev_id),
        ) {
            tracing::warn!("handle_launch: failed to persist last selection: {e}");
        }
    }

    // At least one session was created: switch to the first new session,
    // close the dialog, and clear the checked set.
    if let Some(session_id) = first_session_id {
        state.session_manager.select_by_id(session_id);
    }
    state
        .new_session_dialog_state
        .target_selector
        .clear_checked();
    state.hide_new_session_dialog();
    state.ui_mode = crate::state::UiMode::Normal;

    // Surface a non-fatal warning when some devices were skipped.
    // ANSI-strip names and reasons: device.name can come from `flutter devices`
    // stdout which may contain terminal colour codes.
    if !skipped.is_empty() {
        let launched = actions.len();
        let total = launched + skipped.len();
        let toast_msg = format!(
            "Launched {} of {} device(s). Skipped: {}",
            launched,
            total,
            skipped
                .iter()
                .map(|(name, reason)| format!(
                    "{} ({})",
                    strip_ansi_codes(name),
                    strip_ansi_codes(reason)
                ))
                .collect::<Vec<_>>()
                .join(", ")
        );
        state.push_toast(crate::state::ToastLevel::Warn, toast_msg);
    }

    UpdateResult::actions_vec(actions)
}

/// Try to create a session for `device` using `params` and return the
/// appropriate spawn action (or a reason string on failure).
///
/// Returns `Ok((session_id, action, config_name))` on success, where
/// `config_name` is the name of the launch configuration (if any) that was
/// used — the caller uses this to persist the auto-launch default for the
/// first successfully launched device.
///
/// # Errors
/// Returns `Err(reason)` when the device is skipped due to an active session
/// already running on it, when the session cap is reached, or when the
/// Flutter SDK is not available.
fn spawn_one(
    state: &mut AppState,
    device: &fdemon_daemon::Device,
    params: crate::new_session_dialog::LaunchParams,
) -> Result<(crate::session::SessionId, UpdateAction, Option<String>), String> {
    use crate::config::LaunchConfig;

    // Dedup: skip devices that already have an active session.
    if state
        .session_manager
        .find_active_by_device_id(&device.id)
        .is_some()
    {
        return Err("already has an active session".to_string());
    }

    // Fail fast: resolve the Flutter SDK before creating any session so that
    // a missing SDK never leaves an orphaned Initializing session in the
    // manager. flutter_executable() is state-global — the result is the same
    // for every device in the fan-out loop.
    //
    // Note: the SpawnPreAppSources branch (below) does not consume `flutter`,
    // so resolving it here does not change that branch's behaviour.
    let flutter_opt = state.flutter_executable();

    // Build a LaunchConfig if any non-default parameters are present.
    let config = if params.config_name.is_some()
        || params.flavor.is_some()
        || !params.dart_defines.is_empty()
        || params.entry_point.is_some()
        || !params.extra_args.is_empty()
    {
        let mut cfg = LaunchConfig {
            name: params.config_name.unwrap_or_else(|| "Session".to_string()),
            device: device.id.clone(),
            mode: params.mode,
            flavor: params.flavor,
            entry_point: params.entry_point,
            extra_args: params.extra_args,
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

    // Cap handling: create_session_* enforces MAX_SESSIONS, evicting only the
    // oldest *Stopped* session. Sessions created earlier in THIS fan-out loop are
    // Initializing and therefore never evicted, so already-built actions can never
    // reference an evicted session id. If the eviction policy ever changes to evict
    // active sessions, this loop must be revisited (dangling-action-id risk).
    let devtools = state.settings.devtools.clone();
    let session_result = if let Some(ref cfg) = config {
        state
            .session_manager
            .create_session_with_config_configured(device, cfg.clone(), &devtools)
    } else {
        state
            .session_manager
            .create_session_configured(device, &devtools)
    };

    let session_id = match session_result {
        Ok(id) => id,
        Err(e) => {
            return Err(format!("{}", e));
        }
    };

    tracing::info!(
        "Session created for {} (id: {}, device: {})",
        device.name,
        session_id,
        device.id
    );

    // Decide whether pre-app custom sources need to start before spawning the app.
    // A shared source already running does not need to be re-spawned.
    let needs_pre_app_spawn = state.settings.native_logs.enabled
        && state
            .settings
            .native_logs
            .pre_app_sources()
            .any(|s| !s.shared || !state.is_shared_source_running(&s.name));

    let config_name = config.as_ref().map(|c| c.name.clone());

    let action = if needs_pre_app_spawn {
        UpdateAction::SpawnPreAppSources {
            session_id,
            device: device.clone(),
            config: config.map(Box::new),
            settings: state.settings.native_logs.clone(),
            project_path: state.project_path.clone(),
            running_shared_names: state.running_shared_source_names(),
        }
    } else {
        let Some(flutter) = flutter_opt else {
            // Roll back the session creation: remove_session undoes the insert
            // so the slot is reclaimed immediately rather than waiting for the
            // next capacity eviction (which only reclaims Stopped sessions, not
            // Initializing ones).
            state.session_manager.remove_session(session_id);
            tracing::warn!("handle_launch: no Flutter SDK — cannot spawn session");
            return Err(
                "No Flutter SDK found. Configure sdk_path in .fdemon/config.toml or install Flutter."
                    .to_string(),
            );
        };
        UpdateAction::SpawnSession {
            session_id,
            device: device.clone(),
            config: config.map(Box::new),
            flutter,
        }
    };

    Ok((session_id, action, config_name))
}

/// Format a non-empty list of `(device_name, reason)` skipped entries into a
/// single user-facing error string.
///
/// Both `device_name` and `reason` may originate from `flutter devices` stdout
/// and can therefore contain ANSI escape codes; these are stripped before
/// display so the message is clean in the TUI.
fn summarize_skipped(skipped: &[(String, String)]) -> String {
    use fdemon_core::strip_ansi_codes;

    if skipped.len() == 1 {
        let (name, reason) = &skipped[0];
        return format!(
            "Device '{}' skipped: {}",
            strip_ansi_codes(name),
            strip_ansi_codes(reason)
        );
    }
    let details: Vec<String> = skipped
        .iter()
        .map(|(name, reason)| {
            format!("{} ({})", strip_ansi_codes(name), strip_ansi_codes(reason))
        })
        .collect();
    format!("All selected devices skipped: {}", details.join(", "))
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
    use crate::config::priority::SourcedConfig;
    use crate::config::types::{ConfigSource, LaunchConfig};
    use crate::new_session_dialog::{DartDefine, FuzzyModalType};
    use crate::state::{AppState, UiMode};

    /// Creates an `AppState` pre-loaded with a fake Flutter SDK so that
    /// handlers that call `state.flutter_executable()` can proceed past the
    /// SDK guard in unit tests.
    fn state_with_sdk() -> AppState {
        AppState {
            resolved_sdk: Some(fdemon_daemon::test_utils::fake_flutter_sdk()),
            ..Default::default()
        }
    }

    #[test]
    fn test_flavor_selected_no_config_creates_default() {
        let mut state = AppState {
            ui_mode: UiMode::NewSessionDialog,
            ..Default::default()
        };
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
        let mut state = AppState {
            ui_mode: UiMode::NewSessionDialog,
            ..Default::default()
        };

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
        let mut state = AppState {
            ui_mode: UiMode::NewSessionDialog,
            ..Default::default()
        };

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
        let mut state = AppState {
            ui_mode: UiMode::NewSessionDialog,
            ..Default::default()
        };

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
        let mut state = AppState {
            ui_mode: UiMode::NewSessionDialog,
            ..Default::default()
        };
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
        let mut state = AppState {
            ui_mode: UiMode::NewSessionDialog,
            ..Default::default()
        };

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
        let mut state = AppState {
            ui_mode: UiMode::NewSessionDialog,
            ..Default::default()
        };

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
        let mut state = AppState {
            ui_mode: UiMode::NewSessionDialog,
            ..Default::default()
        };

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
    fn test_device() -> fdemon_daemon::Device {
        fdemon_daemon::Device {
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

        let mut state = state_with_sdk();
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

        let mut state = state_with_sdk();
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
        let mut state = state_with_sdk();
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

        let mut state = state_with_sdk();
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

        let mut state = AppState {
            ui_mode: UiMode::NewSessionDialog,
            ..Default::default()
        };

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

        let mut state = AppState {
            ui_mode: UiMode::NewSessionDialog,
            ..Default::default()
        };
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

        let mut state = AppState {
            ui_mode: UiMode::NewSessionDialog,
            ..Default::default()
        };
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

        let mut state = AppState {
            ui_mode: UiMode::NewSessionDialog,
            ..Default::default()
        };
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
        let mut state = AppState {
            ui_mode: UiMode::NewSessionDialog,
            ..Default::default()
        };

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
        let mut state = AppState {
            ui_mode: UiMode::NewSessionDialog,
            ..Default::default()
        };

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
        use crate::new_session_dialog::FuzzyModalState;

        let mut state = AppState {
            ui_mode: UiMode::NewSessionDialog,
            ..Default::default()
        };

        // Simulate modal being open
        state.new_session_dialog_state.fuzzy_modal =
            Some(FuzzyModalState::new(FuzzyModalType::EntryPoint, vec![]));

        handle_entry_point_selected(&mut state, Some("lib/main.dart".to_string()));

        // Modal should be closed
        assert!(state.new_session_dialog_state.fuzzy_modal.is_none());
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Device reuse guard tests — verify stopped sessions allow reuse, active sessions block
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_handle_launch_allows_device_reuse_when_session_stopped() {
        use fdemon_core::AppPhase;

        let mut state = state_with_sdk();
        state.ui_mode = UiMode::NewSessionDialog;

        // Create a session for the test device, then mark it as stopped
        let device = test_device();
        let id = state
            .session_manager
            .create_session(&device)
            .expect("should create session");
        state.session_manager.get_mut(id).unwrap().session.phase = AppPhase::Stopped;

        // Configure new session dialog to select the same device
        // (selected_index = 1 because index 0 is the group header)
        state
            .new_session_dialog_state
            .target_selector
            .connected_devices
            .push(device);
        state
            .new_session_dialog_state
            .target_selector
            .selected_index = 1;

        let result = handle_launch(&mut state);

        // Stopped session should not block device reuse — SpawnSession action is returned
        assert!(
            result.action.is_some(),
            "Expected SpawnSession action but got none; stopped sessions must allow device reuse"
        );
    }

    #[test]
    fn test_handle_launch_blocks_device_with_running_session() {
        use fdemon_core::AppPhase;

        let mut state = AppState {
            ui_mode: UiMode::NewSessionDialog,
            ..Default::default()
        };

        // Create a session for the test device and set it to Running
        let device = test_device();
        let id = state
            .session_manager
            .create_session(&device)
            .expect("should create session");
        state.session_manager.get_mut(id).unwrap().session.phase = AppPhase::Running;

        // Configure new session dialog to select the same device
        state
            .new_session_dialog_state
            .target_selector
            .connected_devices
            .push(device);
        state
            .new_session_dialog_state
            .target_selector
            .selected_index = 1;

        let result = handle_launch(&mut state);

        // Running session should block device reuse
        assert!(
            result.action.is_none(),
            "Expected no action but got one; running sessions must block device reuse"
        );
        let error = state
            .new_session_dialog_state
            .target_selector
            .error
            .as_ref()
            .expect("Expected error to be set on target_selector");
        assert!(
            error.contains("already has an active session"),
            "Error message should mention active session, got: {error}"
        );
    }

    #[test]
    fn test_handle_launch_blocks_device_with_initializing_session() {
        // Default phase for a new session is Initializing — it should block device reuse

        let mut state = AppState {
            ui_mode: UiMode::NewSessionDialog,
            ..Default::default()
        };

        // Create a session — default phase is Initializing
        let device = test_device();
        state
            .session_manager
            .create_session(&device)
            .expect("should create session");

        // Configure new session dialog to select the same device
        state
            .new_session_dialog_state
            .target_selector
            .connected_devices
            .push(device);
        state
            .new_session_dialog_state
            .target_selector
            .selected_index = 1;

        let result = handle_launch(&mut state);

        // Initializing session should block device reuse
        assert!(
            result.action.is_none(),
            "Expected no action but got one; initializing sessions must block device reuse"
        );
    }

    #[test]
    fn test_handle_launch_allows_device_reuse_when_session_quitting() {
        use fdemon_core::AppPhase;

        let mut state = state_with_sdk();
        state.ui_mode = UiMode::NewSessionDialog;

        // Create a session for the test device, then mark it as Quitting
        let device = test_device();
        let id = state
            .session_manager
            .create_session(&device)
            .expect("should create session");
        state.session_manager.get_mut(id).unwrap().session.phase = AppPhase::Quitting;

        // Configure new session dialog to select the same device
        // (selected_index = 1 because index 0 is the group header)
        state
            .new_session_dialog_state
            .target_selector
            .connected_devices
            .push(device);
        state
            .new_session_dialog_state
            .target_selector
            .selected_index = 1;

        let result = handle_launch(&mut state);

        // Quitting session should not block device reuse — SpawnSession action is returned
        assert!(
            result.action.is_some(),
            "Expected SpawnSession action but got none; quitting sessions must allow device reuse"
        );
    }

    #[test]
    fn test_handle_launch_blocks_device_with_reloading_session() {
        use fdemon_core::AppPhase;

        let mut state = AppState {
            ui_mode: UiMode::NewSessionDialog,
            ..Default::default()
        };

        // Create a session for the test device and set it to Reloading
        let device = test_device();
        let id = state
            .session_manager
            .create_session(&device)
            .expect("should create session");
        state.session_manager.get_mut(id).unwrap().session.phase = AppPhase::Reloading;

        // Configure new session dialog to select the same device
        state
            .new_session_dialog_state
            .target_selector
            .connected_devices
            .push(device);
        state
            .new_session_dialog_state
            .target_selector
            .selected_index = 1;

        let result = handle_launch(&mut state);

        // Reloading session should block device reuse (Reloading is active)
        assert!(
            result.action.is_none(),
            "Expected no action but got one; reloading sessions must block device reuse"
        );
        let error = state
            .new_session_dialog_state
            .target_selector
            .error
            .as_ref()
            .expect("Expected error to be set on target_selector");
        assert!(
            error.contains("already has an active session"),
            "Error message should mention active session, got: {error}"
        );
    }

    // ─────────────────────────────────────────────────────────────────────────
    // extra_args pipeline: handle_launch produces SpawnSession with extra_args
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_handle_launch_extra_args_in_spawn_session_config() {
        let mut state = state_with_sdk();
        state.ui_mode = UiMode::NewSessionDialog;

        // Add a connected device and select it
        // (selected_index = 1 because index 0 is the group header)
        state
            .new_session_dialog_state
            .target_selector
            .connected_devices
            .push(test_device());
        state
            .new_session_dialog_state
            .target_selector
            .selected_index = 1;

        // Set extra_args directly on launch context state (as if populated from a config)
        state.new_session_dialog_state.launch_context.extra_args =
            vec!["--dart-define-from-file=env.json".to_string()];

        let result = handle_launch(&mut state);

        match result.action {
            Some(UpdateAction::SpawnSession { config, .. }) => {
                assert!(
                    config.is_some(),
                    "Config should be created because extra_args is non-empty"
                );
                let cfg = config.unwrap();
                assert_eq!(
                    cfg.extra_args,
                    vec!["--dart-define-from-file=env.json".to_string()],
                    "extra_args should be passed through to LaunchConfig"
                );
            }
            _ => panic!("Expected SpawnSession action, got {:?}", result.action),
        }
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Pre-app custom sources: handle_launch gating
    // (pre-app-custom-sources Phase 1, Task 05)
    // ─────────────────────────────────────────────────────────────────────────

    /// Helper: build a `CustomSourceConfig` with `start_before_app = true`.
    fn pre_app_source(name: &str) -> crate::config::types::CustomSourceConfig {
        crate::config::types::CustomSourceConfig {
            name: name.to_string(),
            command: "server".to_string(),
            args: vec![],
            format: fdemon_core::types::OutputFormat::Raw,
            working_dir: None,
            env: std::collections::HashMap::new(),
            start_before_app: true,
            shared: false,
            ready_check: None,
        }
    }

    #[test]
    fn test_handle_launch_returns_spawn_pre_app_when_pre_app_sources() {
        let mut state = AppState {
            ui_mode: UiMode::NewSessionDialog,
            ..Default::default()
        };

        // Enable native logs with a pre-app source
        state.settings.native_logs.enabled = true;
        state
            .settings
            .native_logs
            .custom_sources
            .push(pre_app_source("test-server"));

        // Select a device
        state
            .new_session_dialog_state
            .target_selector
            .connected_devices
            .push(test_device());
        state
            .new_session_dialog_state
            .target_selector
            .selected_index = 1;

        let result = handle_launch(&mut state);

        assert!(
            matches!(result.action, Some(UpdateAction::SpawnPreAppSources { .. })),
            "Expected SpawnPreAppSources when pre-app sources are configured, got {:?}",
            result.action
        );
    }

    #[test]
    fn test_handle_launch_returns_spawn_session_when_no_pre_app_sources() {
        let mut state = state_with_sdk();
        state.ui_mode = UiMode::NewSessionDialog;

        // Enable native logs but no pre-app sources
        state.settings.native_logs.enabled = true;
        // custom_sources is empty by default

        // Select a device
        state
            .new_session_dialog_state
            .target_selector
            .connected_devices
            .push(test_device());
        state
            .new_session_dialog_state
            .target_selector
            .selected_index = 1;

        let result = handle_launch(&mut state);

        assert!(
            matches!(result.action, Some(UpdateAction::SpawnSession { .. })),
            "Expected SpawnSession when no pre-app sources configured, got {:?}",
            result.action
        );
    }

    #[test]
    fn test_handle_launch_returns_spawn_session_when_native_logs_disabled() {
        let mut state = state_with_sdk();
        state.ui_mode = UiMode::NewSessionDialog;

        // Disable native logs even though a pre-app source is defined
        state.settings.native_logs.enabled = false;
        state
            .settings
            .native_logs
            .custom_sources
            .push(pre_app_source("test-server"));

        // Select a device
        state
            .new_session_dialog_state
            .target_selector
            .connected_devices
            .push(test_device());
        state
            .new_session_dialog_state
            .target_selector
            .selected_index = 1;

        let result = handle_launch(&mut state);

        assert!(
            matches!(result.action, Some(UpdateAction::SpawnSession { .. })),
            "Expected SpawnSession when native logs disabled, got {:?}",
            result.action
        );
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Pre-app gate skip: already-running shared sources
    // (pre-app-custom-sources Phase 2, Task 07)
    // ─────────────────────────────────────────────────────────────────────────

    /// Helper: build a shared `CustomSourceConfig` with `start_before_app = true`.
    fn shared_pre_app_source(name: &str) -> crate::config::types::CustomSourceConfig {
        crate::config::types::CustomSourceConfig {
            name: name.to_string(),
            command: "server".to_string(),
            args: vec![],
            format: fdemon_core::types::OutputFormat::Raw,
            working_dir: None,
            env: std::collections::HashMap::new(),
            start_before_app: true,
            shared: true,
            ready_check: None,
        }
    }

    /// Helper: push a `SharedSourceHandle` onto `state.shared_source_handles`
    /// to simulate an already-running shared source.
    fn mark_shared_source_running(state: &mut AppState, name: &str) {
        use crate::session::SharedSourceHandle;
        let (tx, _rx) = tokio::sync::watch::channel(false);
        state.shared_source_handles.push(SharedSourceHandle {
            name: name.to_string(),
            shutdown_tx: std::sync::Arc::new(tx),
            task_handle: None,
            start_before_app: true,
        });
    }

    #[test]
    fn test_launch_skips_gate_when_all_shared_pre_app_running() {
        // Second session scenario: the only pre-app source is shared and
        // already running. The gate should be skipped → SpawnSession.
        let mut state = state_with_sdk();
        state.ui_mode = UiMode::NewSessionDialog;
        state.settings.native_logs.enabled = true;
        state
            .settings
            .native_logs
            .custom_sources
            .push(shared_pre_app_source("logcat"));

        // Simulate the shared source already running
        mark_shared_source_running(&mut state, "logcat");

        // Select a device
        state
            .new_session_dialog_state
            .target_selector
            .connected_devices
            .push(test_device());
        state
            .new_session_dialog_state
            .target_selector
            .selected_index = 1;

        let result = handle_launch(&mut state);

        assert!(
            matches!(result.action, Some(UpdateAction::SpawnSession { .. })),
            "Expected SpawnSession when all shared pre-app sources are already running, got {:?}",
            result.action
        );
    }

    #[test]
    fn test_launch_gates_when_non_shared_pre_app_present() {
        // Non-shared pre-app sources always require the gate regardless of
        // whether any shared sources are running.
        let mut state = AppState {
            ui_mode: UiMode::NewSessionDialog,
            ..Default::default()
        };
        state.settings.native_logs.enabled = true;
        // Add one shared (running) and one non-shared pre-app source
        state
            .settings
            .native_logs
            .custom_sources
            .push(shared_pre_app_source("logcat"));
        state
            .settings
            .native_logs
            .custom_sources
            .push(pre_app_source("my-server"));

        mark_shared_source_running(&mut state, "logcat");

        // Select a device
        state
            .new_session_dialog_state
            .target_selector
            .connected_devices
            .push(test_device());
        state
            .new_session_dialog_state
            .target_selector
            .selected_index = 1;

        let result = handle_launch(&mut state);

        assert!(
            matches!(result.action, Some(UpdateAction::SpawnPreAppSources { .. })),
            "Expected SpawnPreAppSources when a non-shared pre-app source is present, got {:?}",
            result.action
        );
    }

    #[test]
    fn test_launch_gates_when_shared_pre_app_not_yet_running() {
        // First session scenario: the shared source has never been started yet.
        // The gate must fire.
        let mut state = AppState {
            ui_mode: UiMode::NewSessionDialog,
            ..Default::default()
        };
        state.settings.native_logs.enabled = true;
        state
            .settings
            .native_logs
            .custom_sources
            .push(shared_pre_app_source("logcat"));

        // Do NOT mark the source as running

        // Select a device
        state
            .new_session_dialog_state
            .target_selector
            .connected_devices
            .push(test_device());
        state
            .new_session_dialog_state
            .target_selector
            .selected_index = 1;

        let result = handle_launch(&mut state);

        assert!(
            matches!(result.action, Some(UpdateAction::SpawnPreAppSources { .. })),
            "Expected SpawnPreAppSources when shared pre-app source is not yet running, got {:?}",
            result.action
        );
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Symmetric persistence: handle_launch persists device + config selection
    // (consolidate-launch-config Task 02)
    // ─────────────────────────────────────────────────────────────────────────

    /// Helper: create state_with_sdk but also set project_path to a real temp dir
    /// so that save_last_selection can actually write the settings file.
    fn state_with_sdk_and_project(project_dir: &std::path::Path) -> AppState {
        let mut state = AppState::with_settings(project_dir.to_path_buf(), Default::default());
        state.resolved_sdk = Some(fdemon_daemon::test_utils::fake_flutter_sdk());
        state
    }

    #[test]
    fn test_handle_launch_persists_device_id_on_success() {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(tmp.path().join(".fdemon")).expect("create .fdemon");

        let mut state = state_with_sdk_and_project(tmp.path());
        state.ui_mode = UiMode::NewSessionDialog;

        state
            .new_session_dialog_state
            .target_selector
            .connected_devices
            .push(test_device());
        // selected_index = 1 because index 0 is the group header
        state
            .new_session_dialog_state
            .target_selector
            .selected_index = 1;

        // No config selected — ad-hoc device selection
        let result = handle_launch(&mut state);

        assert!(
            result.action.is_some(),
            "Expected a spawn action from handle_launch"
        );

        // settings.local.toml must now contain the device id
        let prefs_path = tmp.path().join(".fdemon/settings.local.toml");
        assert!(
            prefs_path.exists(),
            "settings.local.toml should have been created by save_last_selection"
        );
        let contents = std::fs::read_to_string(&prefs_path).expect("read settings.local.toml");
        assert!(
            contents.contains("emulator-5554"),
            "settings.local.toml should contain the selected device id, got: {contents}"
        );
    }

    #[test]
    fn test_handle_launch_persists_config_name_when_config_selected() {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(tmp.path().join(".fdemon")).expect("create .fdemon");

        let mut state = state_with_sdk_and_project(tmp.path());
        state.ui_mode = UiMode::NewSessionDialog;

        state
            .new_session_dialog_state
            .target_selector
            .connected_devices
            .push(test_device());
        state
            .new_session_dialog_state
            .target_selector
            .selected_index = 1;

        // Select a named config
        state
            .new_session_dialog_state
            .launch_context
            .configs
            .configs
            .push(SourcedConfig {
                config: LaunchConfig {
                    name: "Staging".to_string(),
                    ..Default::default()
                },
                source: ConfigSource::FDemon,
                display_name: "Staging".to_string(),
            });
        state
            .new_session_dialog_state
            .launch_context
            .selected_config_index = Some(0);
        // Set flavor so the config condition in handle_launch fires
        state.new_session_dialog_state.launch_context.flavor = Some("staging".to_string());

        let result = handle_launch(&mut state);

        assert!(
            result.action.is_some(),
            "Expected a spawn action from handle_launch"
        );

        let prefs_path = tmp.path().join(".fdemon/settings.local.toml");
        assert!(
            prefs_path.exists(),
            "settings.local.toml should have been created"
        );
        let contents = std::fs::read_to_string(&prefs_path).expect("read settings.local.toml");
        assert!(
            contents.contains("emulator-5554"),
            "settings.local.toml should contain device id, got: {contents}"
        );
        assert!(
            contents.contains("Staging"),
            "settings.local.toml should contain config name 'Staging', got: {contents}"
        );
    }

    #[test]
    fn test_handle_launch_does_not_persist_on_session_creation_failure() {
        // session_manager already has MAX_SESSIONS slots taken so create_session fails
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(tmp.path().join(".fdemon")).expect("create .fdemon");

        let mut state = state_with_sdk_and_project(tmp.path());
        state.ui_mode = UiMode::NewSessionDialog;

        // Fill all session slots — SessionManager enforces a max of 9
        for i in 0..9 {
            let d = fdemon_daemon::Device {
                id: format!("filler-device-{i}"),
                name: format!("Filler {i}"),
                platform: "android".to_string(),
                emulator: true,
                category: None,
                platform_type: None,
                ephemeral: false,
                emulator_id: None,
            };
            let _ = state.session_manager.create_session(&d);
        }

        // Select the test device through the dialog (slot is full — create_session will fail)
        state
            .new_session_dialog_state
            .target_selector
            .connected_devices
            .push(test_device());
        state
            .new_session_dialog_state
            .target_selector
            .selected_index = 1;

        let result = handle_launch(&mut state);

        // Session creation should fail → no spawn action
        assert!(
            result.action.is_none(),
            "Expected no action when session creation fails"
        );

        // settings.local.toml must NOT be written
        let prefs_path = tmp.path().join(".fdemon/settings.local.toml");
        assert!(
            !prefs_path.exists(),
            "settings.local.toml must NOT be written when session creation fails"
        );
    }

    // ─────────────────────────────────────────────────────────────────────────
    // handle_set_mode tests
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_handle_set_mode_sets_mode_when_editable() {
        let mut state = AppState {
            ui_mode: UiMode::NewSessionDialog,
            ..Default::default()
        };
        // Default mode is Debug; set to Profile
        let result = handle_set_mode(&mut state, crate::config::FlutterMode::Profile);

        assert_eq!(
            state.new_session_dialog_state.launch_context.mode,
            crate::config::FlutterMode::Profile,
        );
        // No config selected → no auto-save action
        assert!(result.action.is_none());
    }

    #[test]
    fn test_handle_set_mode_is_noop_when_not_editable() {
        use crate::config::priority::SourcedConfig;
        use crate::config::types::{ConfigSource, LaunchConfig};

        let mut state = AppState {
            ui_mode: UiMode::NewSessionDialog,
            ..Default::default()
        };

        // Add a VSCode config — mode field is read-only for VSCode configs
        state
            .new_session_dialog_state
            .launch_context
            .configs
            .configs
            .push(SourcedConfig {
                config: LaunchConfig {
                    name: "VSCode Config".to_string(),
                    mode: crate::config::FlutterMode::Debug,
                    ..Default::default()
                },
                source: ConfigSource::VSCode,
                display_name: "VSCode Config".to_string(),
            });
        state
            .new_session_dialog_state
            .launch_context
            .selected_config_index = Some(0);

        let original_mode = state.new_session_dialog_state.launch_context.mode;
        let result = handle_set_mode(&mut state, crate::config::FlutterMode::Release);

        // Mode must remain unchanged
        assert_eq!(
            state.new_session_dialog_state.launch_context.mode,
            original_mode,
        );
        assert!(result.action.is_none());
    }

    #[test]
    fn test_handle_set_mode_returns_auto_save_for_fdemon_config() {
        use crate::config::priority::SourcedConfig;
        use crate::config::types::{ConfigSource, LaunchConfig};

        let mut state = AppState {
            ui_mode: UiMode::NewSessionDialog,
            ..Default::default()
        };

        // Add an FDemon config
        state
            .new_session_dialog_state
            .launch_context
            .configs
            .configs
            .push(SourcedConfig {
                config: LaunchConfig {
                    name: "MyConfig".to_string(),
                    ..Default::default()
                },
                source: ConfigSource::FDemon,
                display_name: "MyConfig".to_string(),
            });
        state
            .new_session_dialog_state
            .launch_context
            .selected_config_index = Some(0);

        let result = handle_set_mode(&mut state, crate::config::FlutterMode::Release);

        assert_eq!(
            state.new_session_dialog_state.launch_context.mode,
            crate::config::FlutterMode::Release,
        );
        assert!(
            matches!(result.action, Some(UpdateAction::AutoSaveConfig { .. })),
            "Expected AutoSaveConfig action for FDemon config"
        );
    }

    #[test]
    fn test_handle_set_mode_returns_none_for_vscode_config() {
        // VSCode config is not editable for mode — early return before auto-save check
        // The is_mode_editable() gate fires first, so no action is returned.
        use crate::config::priority::SourcedConfig;
        use crate::config::types::{ConfigSource, LaunchConfig};

        let mut state = AppState {
            ui_mode: UiMode::NewSessionDialog,
            ..Default::default()
        };

        state
            .new_session_dialog_state
            .launch_context
            .configs
            .configs
            .push(SourcedConfig {
                config: LaunchConfig {
                    name: "VSCode".to_string(),
                    ..Default::default()
                },
                source: ConfigSource::VSCode,
                display_name: "VSCode".to_string(),
            });
        state
            .new_session_dialog_state
            .launch_context
            .selected_config_index = Some(0);

        let result = handle_set_mode(&mut state, crate::config::FlutterMode::Profile);

        assert!(result.action.is_none());
    }

    #[test]
    fn test_handle_set_mode_focuses_mode_field() {
        use crate::new_session_dialog::{DialogPane, LaunchContextField};

        let mut state = AppState {
            ui_mode: UiMode::NewSessionDialog,
            ..Default::default()
        };

        handle_set_mode(&mut state, crate::config::FlutterMode::Release);

        assert_eq!(
            state.new_session_dialog_state.focused_pane,
            DialogPane::LaunchContext,
            "focused_pane should be LaunchContext after handle_set_mode"
        );
        assert_eq!(
            state.new_session_dialog_state.launch_context.focused_field,
            LaunchContextField::Mode,
            "focused_field should be Mode after handle_set_mode"
        );
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Multi-launch fan-out tests (Phase 1, Task 03)
    // ─────────────────────────────────────────────────────────────────────────

    /// Build a device with a given platform (all physical, non-emulator).
    fn make_device(id: &str, name: &str) -> fdemon_daemon::Device {
        fdemon_daemon::Device {
            id: id.to_string(),
            name: name.to_string(),
            platform: "android".to_string(),
            emulator: false,
            category: None,
            platform_type: None,
            ephemeral: false,
            emulator_id: None,
        }
    }

    /// Seed `state` with `devices` in the connected tab and mark them all as
    /// checked.  Returns the seeded device list.
    fn seed_checked_devices(state: &mut AppState, devices: Vec<fdemon_daemon::Device>) {
        let ids: Vec<String> = devices.iter().map(|d| d.id.clone()).collect();
        state
            .new_session_dialog_state
            .target_selector
            .set_connected_devices(devices);
        // selected_index = 1 so the cursor lands on the first device (index 0
        // is the platform group header).
        state
            .new_session_dialog_state
            .target_selector
            .selected_index = 1;
        for id in ids {
            state
                .new_session_dialog_state
                .target_selector
                .checked_device_ids
                .insert(id);
        }
    }

    #[test]
    fn launch_with_two_checked_emits_two_actions() {
        let mut state = state_with_sdk();
        state.ui_mode = UiMode::NewSessionDialog;

        let devices = vec![
            make_device("dev-a", "Device A"),
            make_device("dev-b", "Device B"),
        ];
        seed_checked_devices(&mut state, devices);

        let result = handle_launch(&mut state);

        let actions = result.actions();
        assert_eq!(
            actions.len(),
            2,
            "Two checked devices should produce two spawn actions"
        );
    }

    #[test]
    fn launch_with_none_checked_falls_back_to_cursor_single() {
        let mut state = state_with_sdk();
        state.ui_mode = UiMode::NewSessionDialog;

        // Add two connected devices but check NONE of them.
        let devices = vec![
            make_device("dev-a", "Device A"),
            make_device("dev-b", "Device B"),
        ];
        state
            .new_session_dialog_state
            .target_selector
            .set_connected_devices(devices);
        // Place cursor on the first device (index 1 due to group header).
        state
            .new_session_dialog_state
            .target_selector
            .selected_index = 1;
        // Checked set remains empty.
        assert_eq!(
            state
                .new_session_dialog_state
                .target_selector
                .checked_count(),
            0
        );

        let result = handle_launch(&mut state);

        // Single action for the cursor device only.
        let actions = result.actions();
        assert_eq!(
            actions.len(),
            1,
            "Zero checked devices should fall back to a single cursor-device action"
        );
    }

    #[test]
    fn launch_skips_device_with_active_session() {
        use fdemon_core::AppPhase;

        let mut state = state_with_sdk();
        state.ui_mode = UiMode::NewSessionDialog;

        // Create a Running session for dev-a.
        let dev_a = make_device("dev-a", "Device A");
        let id_a = state
            .session_manager
            .create_session(&dev_a)
            .expect("should create session for dev-a");
        state.session_manager.get_mut(id_a).unwrap().session.phase = AppPhase::Running;

        // Seed both devices as checked.
        let devices = vec![
            make_device("dev-a", "Device A"),
            make_device("dev-b", "Device B"),
        ];
        seed_checked_devices(&mut state, devices);

        let result = handle_launch(&mut state);

        // dev-a is skipped, dev-b launches → exactly one action.
        let actions = result.actions();
        assert_eq!(
            actions.len(),
            1,
            "Device with active session should be skipped; remaining device should launch"
        );

        // A warning toast should be visible.
        assert!(
            !state.toasts.is_empty(),
            "A warn toast should be pushed when a device is skipped"
        );
    }

    #[test]
    fn launch_clears_checked_and_closes_on_success() {
        let mut state = state_with_sdk();
        state.ui_mode = UiMode::NewSessionDialog;

        let devices = vec![
            make_device("dev-a", "Device A"),
            make_device("dev-b", "Device B"),
        ];
        seed_checked_devices(&mut state, devices);

        let result = handle_launch(&mut state);

        assert!(
            !result.actions().is_empty(),
            "Should have at least one action"
        );

        // Dialog must be dismissed after success (ui_mode transitions to Normal).
        assert_eq!(
            state.ui_mode,
            UiMode::Normal,
            "ui_mode should be Normal after a successful launch"
        );

        // Checked set must be cleared.
        assert_eq!(
            state
                .new_session_dialog_state
                .target_selector
                .checked_count(),
            0,
            "Checked set must be cleared after a successful launch"
        );
    }

    #[test]
    fn launch_all_skipped_keeps_dialog_open_with_error() {
        use fdemon_core::AppPhase;

        let mut state = AppState {
            ui_mode: UiMode::NewSessionDialog,
            ..Default::default()
        };

        // Create Running sessions for both devices so both get skipped.
        for (id, name) in [("dev-a", "Device A"), ("dev-b", "Device B")] {
            let d = make_device(id, name);
            let sid = state
                .session_manager
                .create_session(&d)
                .expect("should create session");
            state.session_manager.get_mut(sid).unwrap().session.phase = AppPhase::Running;
        }

        // Seed both as checked.
        let devices = vec![
            make_device("dev-a", "Device A"),
            make_device("dev-b", "Device B"),
        ];
        seed_checked_devices(&mut state, devices);

        let result = handle_launch(&mut state);

        // No actions emitted.
        assert!(
            result.actions().is_empty(),
            "All skipped should produce no actions"
        );

        // Dialog stays open (ui_mode remains NewSessionDialog).
        assert_eq!(
            state.ui_mode,
            UiMode::NewSessionDialog,
            "ui_mode must remain NewSessionDialog when all devices are skipped"
        );

        // An error is set on the target selector.
        assert!(
            state
                .new_session_dialog_state
                .target_selector
                .error
                .is_some(),
            "An error should be set on the target selector when all devices are skipped"
        );
    }

    /// AC1 — No orphaned session: when spawn_one fails due to missing SDK
    /// (SpawnSession branch), the session must be rolled back so the
    /// SessionManager slot count is unchanged.
    #[test]
    fn launch_no_sdk_leaves_no_orphaned_session() {
        // State without SDK — flutter_executable() returns None.
        let mut state = AppState {
            ui_mode: UiMode::NewSessionDialog,
            ..Default::default()
        };
        assert!(state.flutter_executable().is_none(), "pre-condition: no SDK");

        let session_count_before = state.session_manager.len();

        let device = make_device("dev-a", "Device A");
        seed_checked_devices(&mut state, vec![device]);

        let result = handle_launch(&mut state);

        // All devices skipped — no action.
        assert!(result.actions().is_empty(), "no action without SDK");
        // No orphaned Initializing session left behind.
        assert_eq!(
            state.session_manager.len(),
            session_count_before,
            "session count must be unchanged after SDK-check failure"
        );
    }

    /// AC3 — `save_last_selection` persists the first *successfully launched*
    /// device.  When device 0 is skipped (has a Running session) and device 1
    /// succeeds, device 1's id must be written to settings.local.toml.
    #[test]
    fn launch_skipped_primary_persists_second_device() {
        use fdemon_core::AppPhase;

        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(tmp.path().join(".fdemon")).expect("create .fdemon");

        let mut state = state_with_sdk_and_project(tmp.path());
        state.ui_mode = UiMode::NewSessionDialog;

        // device 0 ("dev-a") has an active Running session — it will be skipped.
        let dev_a = make_device("dev-a", "Device A");
        let id_a = state
            .session_manager
            .create_session(&dev_a)
            .expect("create session for dev-a");
        state.session_manager.get_mut(id_a).unwrap().session.phase = AppPhase::Running;

        // Seed both devices as checked.
        let devices = vec![
            make_device("dev-a", "Device A"),
            make_device("dev-b", "Device B"),
        ];
        seed_checked_devices(&mut state, devices);

        let result = handle_launch(&mut state);

        // dev-b should have launched (one action).
        assert_eq!(
            result.actions().len(),
            1,
            "dev-b should launch after dev-a is skipped"
        );

        // settings.local.toml must contain dev-b's id, NOT dev-a's.
        let prefs_path = tmp.path().join(".fdemon/settings.local.toml");
        assert!(
            prefs_path.exists(),
            "settings.local.toml should have been created"
        );
        let contents = std::fs::read_to_string(&prefs_path).expect("read settings.local.toml");
        assert!(
            contents.contains("dev-b"),
            "settings.local.toml should contain dev-b (first successful device), got: {contents}"
        );
        assert!(
            !contents.contains("dev-a"),
            "settings.local.toml must NOT contain dev-a (skipped device), got: {contents}"
        );
    }

    /// M2 — Cap-hit mid-loop: first device fills the last slot; second device
    /// hits `ensure_capacity` Err and is skipped.  The handler should emit
    /// exactly one action, push a warn toast, and close the dialog.
    #[test]
    fn launch_partial_when_cap_hit_mid_loop_emits_toast_no_panic() {
        use fdemon_core::AppPhase;

        let mut state = state_with_sdk();
        state.ui_mode = UiMode::NewSessionDialog;

        // Fill 8 of 9 slots with active (non-evictable) Running sessions.
        for i in 0..8 {
            let d = make_device(&format!("filler-{i}"), &format!("Filler {i}"));
            let sid = state.session_manager.create_session(&d).expect("create");
            state.session_manager.get_mut(sid).unwrap().session.phase = AppPhase::Running;
        }

        // Two fresh checked devices: first fills slot 9, second hits the cap.
        let devices = vec![make_device("dev-a", "Device A"), make_device("dev-b", "Device B")];
        seed_checked_devices(&mut state, devices);

        let result = handle_launch(&mut state);

        assert_eq!(
            result.actions().len(),
            1,
            "exactly one device should launch before the cap"
        );
        assert!(
            !state.toasts.is_empty(),
            "a warn toast should report the skipped device"
        );
        assert_eq!(
            state.ui_mode,
            UiMode::Normal,
            "dialog closes on partial success"
        );
        // No panic reaching this point is itself part of the assertion.
    }
}
