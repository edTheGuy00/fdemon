//! Startup dialog handlers
//!
//! Handles navigation, device selection, config selection, and launch actions.

use crate::app::state::{AppState, DialogSection, UiMode};
use crate::config::FlutterMode;
use tracing::warn;

use super::{UpdateAction, UpdateResult};

/// Handle show startup dialog message
pub fn handle_show_startup_dialog(state: &mut AppState) -> UpdateResult {
    // Load all configs (launch.toml + launch.json)
    let configs = crate::config::load_all_configs(&state.project_path);

    // Show the dialog with configs (will use cache if available - Task 08e)
    state.show_startup_dialog(configs);

    // Trigger device discovery (background refresh if cache exists)
    UpdateResult::action(UpdateAction::DiscoverDevices)
}

/// Handle hide startup dialog message
pub fn handle_hide_startup_dialog(state: &mut AppState) -> UpdateResult {
    // Task 10d: Reset creating_new_config flag on cancel
    if state.startup_dialog_state.creating_new_config {
        state.startup_dialog_state.creating_new_config = false;
        state.startup_dialog_state.dirty = false;
    }
    state.hide_startup_dialog();
    UpdateResult::none()
}

/// Handle startup dialog up message
pub fn handle_startup_dialog_up(state: &mut AppState) -> UpdateResult {
    state.startup_dialog_state.navigate_up();
    UpdateResult::none()
}

/// Handle startup dialog down message
pub fn handle_startup_dialog_down(state: &mut AppState) -> UpdateResult {
    state.startup_dialog_state.navigate_down();
    UpdateResult::none()
}

/// Handle startup dialog next section message
pub fn handle_startup_dialog_next_section(state: &mut AppState) -> UpdateResult {
    state.startup_dialog_state.next_section();
    UpdateResult::none()
}

/// Handle startup dialog previous section message
pub fn handle_startup_dialog_prev_section(state: &mut AppState) -> UpdateResult {
    state.startup_dialog_state.prev_section();
    UpdateResult::none()
}

/// Handle startup dialog next section skip disabled message
pub fn handle_startup_dialog_next_section_skip_disabled(state: &mut AppState) -> UpdateResult {
    let mut next = state.startup_dialog_state.active_section.next();

    // Skip Flavor and DartDefines if they're disabled
    while matches!(next, DialogSection::Flavor | DialogSection::DartDefines) {
        next = next.next();
    }

    state.startup_dialog_state.editing = false;
    state.startup_dialog_state.active_section = next;
    UpdateResult::none()
}

/// Handle startup dialog previous section skip disabled message
pub fn handle_startup_dialog_prev_section_skip_disabled(state: &mut AppState) -> UpdateResult {
    let mut prev = state.startup_dialog_state.active_section.prev();

    // Skip Flavor and DartDefines if they're disabled
    while matches!(prev, DialogSection::Flavor | DialogSection::DartDefines) {
        prev = prev.prev();
    }

    state.startup_dialog_state.editing = false;
    state.startup_dialog_state.active_section = prev;
    UpdateResult::none()
}

/// Handle startup dialog select config message
pub fn handle_startup_dialog_select_config(state: &mut AppState, idx: usize) -> UpdateResult {
    // Task 10b: Use on_config_selected to handle VSCode config field population
    state.startup_dialog_state.on_config_selected(Some(idx));
    UpdateResult::none()
}

/// Handle startup dialog select device message
pub fn handle_startup_dialog_select_device(state: &mut AppState, idx: usize) -> UpdateResult {
    state.startup_dialog_state.selected_device = Some(idx);
    UpdateResult::none()
}

/// Handle startup dialog set mode message
pub fn handle_startup_dialog_set_mode(state: &mut AppState, mode: FlutterMode) -> UpdateResult {
    state.startup_dialog_state.mode = mode;
    UpdateResult::none()
}

/// Handle startup dialog char input message
pub fn handle_startup_dialog_char_input(state: &mut AppState, c: char) -> UpdateResult {
    // Task 10b: Block input on disabled fields (VSCode configs)
    if state.startup_dialog_state.editing && state.startup_dialog_state.flavor_editable() {
        match state.startup_dialog_state.active_section {
            DialogSection::Flavor => {
                state.startup_dialog_state.flavor.push(c);
                // Task 10c: Mark dirty for auto-save
                state.startup_dialog_state.mark_dirty();
            }
            DialogSection::DartDefines => {
                state.startup_dialog_state.dart_defines.push(c);
                // Task 10c: Mark dirty for auto-save
                state.startup_dialog_state.mark_dirty();
            }
            _ => {}
        }
    }
    UpdateResult::none()
}

/// Handle startup dialog backspace message
pub fn handle_startup_dialog_backspace(state: &mut AppState) -> UpdateResult {
    // Task 10b: Block backspace on disabled fields (VSCode configs)
    if state.startup_dialog_state.editing && state.startup_dialog_state.flavor_editable() {
        match state.startup_dialog_state.active_section {
            DialogSection::Flavor => {
                state.startup_dialog_state.flavor.pop();
                // Task 10c: Mark dirty for auto-save
                state.startup_dialog_state.mark_dirty();
            }
            DialogSection::DartDefines => {
                state.startup_dialog_state.dart_defines.pop();
                // Task 10c: Mark dirty for auto-save
                state.startup_dialog_state.mark_dirty();
            }
            _ => {}
        }
    }
    UpdateResult::none()
}

/// Handle startup dialog clear input message
pub fn handle_startup_dialog_clear_input(state: &mut AppState) -> UpdateResult {
    match state.startup_dialog_state.active_section {
        DialogSection::Flavor => {
            state.startup_dialog_state.flavor.clear();
            // Task 10c: Mark dirty for auto-save
            state.startup_dialog_state.mark_dirty();
        }
        DialogSection::DartDefines => {
            state.startup_dialog_state.dart_defines.clear();
            // Task 10c: Mark dirty for auto-save
            state.startup_dialog_state.mark_dirty();
        }
        _ => {}
    }
    UpdateResult::none()
}

/// Handle startup dialog confirm message
pub fn handle_startup_dialog_confirm(state: &mut AppState) -> UpdateResult {
    // Task 10d: Save config before launching if dirty (user clicked launch before debounce)
    if state.startup_dialog_state.dirty {
        if state.startup_dialog_state.creating_new_config {
            // Save new config synchronously before launch
            if !state.startup_dialog_state.flavor.is_empty()
                || !state.startup_dialog_state.dart_defines.is_empty()
            {
                let project_path = state.project_path.clone();
                let dialog = &mut state.startup_dialog_state;

                let new_config = crate::config::LaunchConfig {
                    name: dialog.new_config_name.clone(),
                    device: "auto".to_string(),
                    mode: dialog.mode,
                    flavor: if dialog.flavor.is_empty() {
                        None
                    } else {
                        Some(dialog.flavor.clone())
                    },
                    dart_defines: crate::config::parse_dart_defines(&dialog.dart_defines),
                    ..Default::default()
                };

                if let Ok(()) = crate::config::add_launch_config(&project_path, new_config) {
                    tracing::info!(
                        "Created new config before launch: {}",
                        dialog.new_config_name
                    );
                    dialog.creating_new_config = false;
                }
            }
        } else if let Some(ref config_name) = state.startup_dialog_state.editing_config_name {
            // Save existing config edits before launch
            let project_path = state.project_path.clone();
            let name = config_name.clone();
            let flavor = state.startup_dialog_state.flavor.clone();
            let dart_defines = state.startup_dialog_state.dart_defines.clone();

            let _ =
                crate::config::update_launch_config_field(&project_path, &name, "flavor", &flavor);
            let _ = crate::config::update_launch_config_dart_defines(
                &project_path,
                &name,
                &dart_defines,
            );
        }
        state.startup_dialog_state.mark_saved();
    }

    let dialog = &state.startup_dialog_state;

    // Get selected device (required)
    let device = match dialog.selected_device() {
        Some(d) => d.clone(),
        None => {
            // No device selected - show error
            state.startup_dialog_state.error = Some("Please select a device".to_string());
            return UpdateResult::none();
        }
    };

    // Build config: start from selected config OR create ad-hoc if user entered values
    let config: Option<crate::config::LaunchConfig> = {
        // Check if user entered any custom values
        let has_custom_flavor = !dialog.flavor.is_empty();
        let has_custom_defines = !dialog.dart_defines.is_empty();
        let has_custom_mode = dialog.mode != crate::config::FlutterMode::Debug;

        if let Some(sourced) = dialog.selected_config() {
            // User selected a config - clone and override
            let mut cfg = sourced.config.clone();

            // Override mode
            cfg.mode = dialog.mode;

            // Override flavor if user entered one
            if has_custom_flavor {
                cfg.flavor = Some(dialog.flavor.clone());
            }

            // Override dart-defines if user entered any
            if has_custom_defines {
                cfg.dart_defines = crate::config::parse_dart_defines(&dialog.dart_defines);
            }

            Some(cfg)
        } else if has_custom_flavor || has_custom_defines || has_custom_mode {
            // No config selected but user entered custom values
            // Create an ad-hoc config with the entered values
            Some(crate::config::LaunchConfig {
                name: "Ad-hoc Launch".to_string(),
                device: device.id.clone(),
                mode: dialog.mode,
                flavor: if has_custom_flavor {
                    Some(dialog.flavor.clone())
                } else {
                    None
                },
                dart_defines: if has_custom_defines {
                    crate::config::parse_dart_defines(&dialog.dart_defines)
                } else {
                    std::collections::HashMap::new()
                },
                entry_point: None,
                extra_args: Vec::new(),
                auto_start: false,
            })
        } else {
            // No config, no custom values - bare run
            None
        }
    };

    // Save selection (only if a named config was selected)
    let _ = crate::config::save_last_selection(
        &state.project_path,
        dialog.selected_config().map(|c| c.config.name.as_str()),
        Some(&device.id),
    );

    // Create session
    let result = if let Some(ref cfg) = config {
        state
            .session_manager
            .create_session_with_config(&device, cfg.clone())
    } else {
        state.session_manager.create_session(&device)
    };

    match result {
        Ok(session_id) => {
            state.ui_mode = UiMode::Normal;
            UpdateResult::action(UpdateAction::SpawnSession {
                session_id,
                device,
                config: config.map(Box::new),
            })
        }
        Err(e) => {
            state.startup_dialog_state.error = Some(format!("Failed to create session: {}", e));
            UpdateResult::none()
        }
    }
}

/// Handle save startup dialog config message
pub fn handle_save_startup_dialog_config(state: &mut AppState) -> UpdateResult {
    let dialog = &mut state.startup_dialog_state;

    // Task 10d: Handle new config creation (no config selected, user entered data)
    if dialog.creating_new_config && dialog.dirty {
        // Don't create empty config
        if dialog.flavor.is_empty() && dialog.dart_defines.is_empty() {
            dialog.creating_new_config = false;
            dialog.mark_saved();
            return UpdateResult::none();
        }

        let project_path = state.project_path.clone();
        let new_config = crate::config::LaunchConfig {
            name: dialog.new_config_name.clone(),
            device: "auto".to_string(),
            mode: dialog.mode,
            flavor: if dialog.flavor.is_empty() {
                None
            } else {
                Some(dialog.flavor.clone())
            },
            dart_defines: crate::config::parse_dart_defines(&dialog.dart_defines),
            ..Default::default()
        };

        match crate::config::add_launch_config(&project_path, new_config) {
            Ok(()) => {
                tracing::info!("Created new config: {}", dialog.new_config_name);

                // Reload configs to show the new one
                let reloaded = crate::config::load_all_configs(&project_path);

                // Find the actual name (may have been renamed due to collision)
                let new_idx = reloaded
                    .configs
                    .iter()
                    .position(|c| c.config.name.starts_with(&dialog.new_config_name))
                    .or_else(|| {
                        // Fallback: find last config (most recently added)
                        if !reloaded.configs.is_empty() {
                            Some(reloaded.configs.len() - 1)
                        } else {
                            None
                        }
                    });

                let actual_name = new_idx
                    .and_then(|idx| reloaded.configs.get(idx))
                    .map(|c| c.config.name.clone())
                    .unwrap_or_else(|| dialog.new_config_name.clone());

                dialog.configs = reloaded;
                dialog.selected_config = new_idx;
                dialog.creating_new_config = false;
                dialog.editing_config_name = Some(actual_name);
                dialog.mark_saved();
            }
            Err(e) => {
                warn!("Failed to create config: {}", e);
                // Could show error to user in the future
            }
        }
    }
    // Task 10c: Save FDemon config edits (flavor, dart_defines)
    else if let Some(ref config_name) = dialog.editing_config_name {
        if dialog.dirty {
            let project_path = state.project_path.clone();
            let name = config_name.clone();
            let flavor = dialog.flavor.clone();
            let dart_defines = dialog.dart_defines.clone();

            // Save flavor
            if let Err(e) =
                crate::config::update_launch_config_field(&project_path, &name, "flavor", &flavor)
            {
                warn!("Failed to save flavor: {}", e);
            }

            // Save dart_defines
            if let Err(e) = crate::config::update_launch_config_dart_defines(
                &project_path,
                &name,
                &dart_defines,
            ) {
                warn!("Failed to save dart_defines: {}", e);
            }

            dialog.mark_saved();
        }
    }
    UpdateResult::none()
}

/// Handle startup dialog refresh devices message
pub fn handle_startup_dialog_refresh_devices(state: &mut AppState) -> UpdateResult {
    // Mark as refreshing (shows loading indicator but keeps existing devices)
    state.startup_dialog_state.refreshing = true;

    // Trigger device discovery
    UpdateResult::action(UpdateAction::DiscoverDevices)
}

/// Handle startup dialog jump to section message
pub fn handle_startup_dialog_jump_to_section(
    state: &mut AppState,
    section: DialogSection,
) -> UpdateResult {
    state.startup_dialog_state.jump_to_section(section);
    UpdateResult::none()
}

/// Handle startup dialog enter edit message
pub fn handle_startup_dialog_enter_edit(state: &mut AppState) -> UpdateResult {
    // Task 10b: Prevent entering edit mode on disabled fields
    if state.startup_dialog_state.flavor_editable() {
        state.startup_dialog_state.enter_edit();
    }
    UpdateResult::none()
}

/// Handle startup dialog exit edit message
pub fn handle_startup_dialog_exit_edit(state: &mut AppState) -> UpdateResult {
    state.startup_dialog_state.exit_edit();
    UpdateResult::none()
}
