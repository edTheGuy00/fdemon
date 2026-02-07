//! Settings page handlers
//!
//! Handles navigation, editing, and persistence of settings.

use crate::app::confirm_dialog::ConfirmDialogState;
use crate::app::message::Message;
use crate::app::settings_items::get_selected_item;
use crate::app::state::AppState;
use crate::config::{SettingValue, SettingsTab};

use super::{update, UpdateResult};

/// Handle show settings message
pub fn handle_show_settings(state: &mut AppState) -> UpdateResult {
    state.show_settings();
    UpdateResult::none()
}

/// Handle hide settings message
pub fn handle_hide_settings(state: &mut AppState) -> UpdateResult {
    // Check for unsaved changes - show confirmation dialog if dirty
    if state.settings_view_state.dirty {
        state.confirm_dialog_state = Some(ConfirmDialogState::new(
            "Unsaved Changes",
            "You have unsaved changes. What do you want to do?",
            vec![
                ("Save & Close", Message::SettingsSaveAndClose),
                ("Discard Changes", Message::ForceHideSettings),
                ("Cancel", Message::CancelQuit),
            ],
        ));
        state.ui_mode = crate::app::state::UiMode::ConfirmDialog;
    } else {
        state.hide_settings();
    }
    UpdateResult::none()
}

/// Handle settings next tab message
pub fn handle_settings_next_tab(state: &mut AppState) -> UpdateResult {
    state.settings_view_state.next_tab();
    UpdateResult::none()
}

/// Handle settings previous tab message
pub fn handle_settings_prev_tab(state: &mut AppState) -> UpdateResult {
    state.settings_view_state.prev_tab();
    UpdateResult::none()
}

/// Handle settings goto tab message
pub fn handle_settings_goto_tab(state: &mut AppState, idx: usize) -> UpdateResult {
    if let Some(tab) = SettingsTab::from_index(idx) {
        state.settings_view_state.goto_tab(tab);
    }
    UpdateResult::none()
}

/// Handle settings next item message
pub fn handle_settings_next_item(state: &mut AppState) -> UpdateResult {
    let item_count = get_item_count_for_tab(&state.settings, state.settings_view_state.active_tab);
    state.settings_view_state.select_next(item_count);
    UpdateResult::none()
}

/// Handle settings previous item message
pub fn handle_settings_prev_item(state: &mut AppState) -> UpdateResult {
    let item_count = get_item_count_for_tab(&state.settings, state.settings_view_state.active_tab);
    state.settings_view_state.select_previous(item_count);
    UpdateResult::none()
}

/// Handle settings toggle edit message
pub fn handle_settings_toggle_edit(state: &mut AppState) -> UpdateResult {
    // Toggle edit mode
    if state.settings_view_state.editing {
        state.settings_view_state.stop_editing();
    } else {
        // Get the current item and start editing with its value
        if let Some(item) = get_selected_item(
            &state.settings,
            &state.project_path,
            &state.settings_view_state,
        ) {
            // Start editing based on value type
            match &item.value {
                SettingValue::Bool(_) => {
                    // Bool toggles directly without edit mode
                    return update(state, Message::SettingsToggleBool);
                }
                SettingValue::Enum { .. } => {
                    // Enums cycle through options
                    return update(state, Message::SettingsCycleEnumNext);
                }
                SettingValue::Number(n) => {
                    state.settings_view_state.start_editing(&n.to_string());
                }
                SettingValue::Float(f) => {
                    state.settings_view_state.start_editing(&f.to_string());
                }
                SettingValue::String(s) => {
                    state.settings_view_state.start_editing(s);
                }
                SettingValue::List(_) => {
                    // List starts with empty buffer to add new item
                    state.settings_view_state.start_editing("");
                }
            }
        }
    }
    UpdateResult::none()
}

/// Handle settings save message
pub fn handle_settings_save(state: &mut AppState) -> UpdateResult {
    use crate::config::{launch::save_launch_configs, save_settings, save_user_preferences};

    let result = match state.settings_view_state.active_tab {
        SettingsTab::Project => {
            // Save project settings (config.toml)
            save_settings(&state.project_path, &state.settings)
        }
        SettingsTab::UserPrefs => {
            // Save user preferences (settings.local.toml)
            save_user_preferences(&state.project_path, &state.settings_view_state.user_prefs)
        }
        SettingsTab::LaunchConfig => {
            // Save launch configs (launch.toml)
            use crate::config::launch::load_launch_configs;
            let configs = load_launch_configs(&state.project_path);
            let config_vec: Vec<_> = configs.iter().map(|r| r.config.clone()).collect();
            save_launch_configs(&state.project_path, &config_vec)
        }
        SettingsTab::VSCodeConfig => {
            // Read-only - nothing to save
            Ok(())
        }
    };

    match result {
        Ok(()) => {
            state.settings_view_state.clear_dirty();
            state.settings_view_state.error = None;
            tracing::info!("Settings saved successfully");
        }
        Err(e) => {
            let error_msg = format!("Save failed: {}", e);
            tracing::error!("{}", error_msg);
            state.settings_view_state.error = Some(error_msg);
        }
    }

    UpdateResult::none()
}

/// Handle settings reset item message
pub fn handle_settings_reset_item(_state: &mut AppState) -> UpdateResult {
    // Reset setting to default - actual logic will be implemented with widget
    UpdateResult::none()
}

/// Handle settings toggle bool message
pub fn handle_settings_toggle_bool(state: &mut AppState) -> UpdateResult {
    if let Some(item) = get_selected_item(
        &state.settings,
        &state.project_path,
        &state.settings_view_state,
    ) {
        // Only toggle if it's a boolean value
        if let SettingValue::Bool(val) = &item.value {
            // Create new item with flipped value
            let new_value = SettingValue::Bool(!val);
            let mut toggled_item = item.clone();
            toggled_item.value = new_value;

            // Apply based on active tab
            match state.settings_view_state.active_tab {
                SettingsTab::Project => {
                    super::settings::apply_project_setting(&mut state.settings, &toggled_item);
                    state.settings_view_state.mark_dirty();
                }
                SettingsTab::UserPrefs => {
                    super::settings::apply_user_preference(
                        &mut state.settings_view_state.user_prefs,
                        &toggled_item,
                    );
                    state.settings_view_state.mark_dirty();
                }
                SettingsTab::LaunchConfig => {
                    // For launch configs, we need to load, modify, and save
                    // Extract config index from item ID (format: "launch.{idx}.field")
                    let parts: Vec<&str> = toggled_item.id.split('.').collect();
                    if parts.len() >= 3 && parts[0] == "launch" {
                        if let Ok(config_idx) = parts[1].parse::<usize>() {
                            use crate::config::launch::{load_launch_configs, save_launch_configs};
                            let mut configs = load_launch_configs(&state.project_path);
                            if let Some(resolved) = configs.get_mut(config_idx) {
                                super::settings::apply_launch_config_change(
                                    &mut resolved.config,
                                    &toggled_item,
                                );
                                // Save the modified configs back to disk
                                let config_vec: Vec<_> =
                                    configs.iter().map(|r| r.config.clone()).collect();
                                if let Err(e) =
                                    save_launch_configs(&state.project_path, &config_vec)
                                {
                                    tracing::error!("Failed to save launch configs: {}", e);
                                } else {
                                    state.settings_view_state.mark_dirty();
                                }
                            }
                        }
                    }
                }
                SettingsTab::VSCodeConfig => {
                    // Read-only tab - ignore toggle
                }
            }
        }
    }
    UpdateResult::none()
}

/// Handle settings cycle enum next message
pub fn handle_settings_cycle_enum_next(state: &mut AppState) -> UpdateResult {
    // Cycle enum to next value
    state.settings_view_state.mark_dirty();
    UpdateResult::none()
}

/// Handle settings cycle enum previous message
pub fn handle_settings_cycle_enum_prev(state: &mut AppState) -> UpdateResult {
    // Cycle enum to previous value
    state.settings_view_state.mark_dirty();
    UpdateResult::none()
}

/// Handle settings increment message
pub fn handle_settings_increment(state: &mut AppState, _delta: i64) -> UpdateResult {
    // Increment/decrement number value
    // For direct increment without edit mode
    // Actual implementation will be in Task 11 (persistence)
    if !state.settings_view_state.editing {
        state.settings_view_state.mark_dirty();
    }
    UpdateResult::none()
}

/// Handle settings char input message
pub fn handle_settings_char_input(state: &mut AppState, ch: char) -> UpdateResult {
    // Add character to edit buffer
    if state.settings_view_state.editing {
        state.settings_view_state.edit_buffer.push(ch);
    }
    UpdateResult::none()
}

/// Handle settings backspace message
pub fn handle_settings_backspace(state: &mut AppState) -> UpdateResult {
    // Remove last character from edit buffer
    if state.settings_view_state.editing {
        state.settings_view_state.edit_buffer.pop();
    }
    UpdateResult::none()
}

/// Handle settings clear buffer message
pub fn handle_settings_clear_buffer(state: &mut AppState) -> UpdateResult {
    // Clear entire edit buffer
    if state.settings_view_state.editing {
        state.settings_view_state.edit_buffer.clear();
    }
    UpdateResult::none()
}

/// Handle settings commit edit message
pub fn handle_settings_commit_edit(state: &mut AppState) -> UpdateResult {
    // Commit the current edit
    // Actual value update needs to happen here
    if state.settings_view_state.editing {
        state.settings_view_state.mark_dirty();
        state.settings_view_state.stop_editing();
    }
    UpdateResult::none()
}

/// Handle settings cancel edit message
pub fn handle_settings_cancel_edit(state: &mut AppState) -> UpdateResult {
    // Cancel the current edit
    state.settings_view_state.stop_editing();
    UpdateResult::none()
}

/// Handle settings remove list item message
pub fn handle_settings_remove_list_item(state: &mut AppState) -> UpdateResult {
    // Remove last item from list
    state.settings_view_state.mark_dirty();
    UpdateResult::none()
}

/// Handle settings save and close message
pub fn handle_settings_save_and_close(state: &mut AppState) -> UpdateResult {
    // Save then close
    use crate::config::{launch::save_launch_configs, save_settings, save_user_preferences};

    let result = match state.settings_view_state.active_tab {
        SettingsTab::Project => save_settings(&state.project_path, &state.settings),
        SettingsTab::UserPrefs => {
            save_user_preferences(&state.project_path, &state.settings_view_state.user_prefs)
        }
        SettingsTab::LaunchConfig => {
            use crate::config::launch::load_launch_configs;
            let configs = load_launch_configs(&state.project_path);
            let config_vec: Vec<_> = configs.iter().map(|r| r.config.clone()).collect();
            save_launch_configs(&state.project_path, &config_vec)
        }
        SettingsTab::VSCodeConfig => Ok(()),
    };

    match result {
        Ok(()) => {
            state.settings_view_state.clear_dirty();
            state.settings_view_state.error = None;
            state.hide_settings();
            tracing::info!("Settings saved and closed");
        }
        Err(e) => {
            let error_msg = format!("Save failed: {}", e);
            tracing::error!("{}", error_msg);
            state.settings_view_state.error = Some(error_msg);
            // Don't close on error - stay in settings to show error
        }
    }

    UpdateResult::none()
}

/// Handle force hide settings message
pub fn handle_force_hide_settings(state: &mut AppState) -> UpdateResult {
    // Force close without saving (discard changes)
    state.settings_view_state.clear_dirty();
    state.hide_settings();
    UpdateResult::none()
}

/// Get the number of items in a settings tab
fn get_item_count_for_tab(_settings: &crate::config::Settings, tab: SettingsTab) -> usize {
    match tab {
        SettingsTab::Project => {
            // behavior (2) + watcher (4) + ui (6) + devtools (2) + editor (2) = 16
            16
        }
        SettingsTab::UserPrefs => {
            // editor (2) + theme (1) + last_device (1) + last_config (1) = 5
            5
        }
        SettingsTab::LaunchConfig => {
            // Dynamic based on loaded configs
            // For now, estimate
            10
        }
        SettingsTab::VSCodeConfig => {
            // Dynamic based on loaded configs
            5
        }
    }
}
