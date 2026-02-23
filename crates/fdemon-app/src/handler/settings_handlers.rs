//! Settings page handlers
//!
//! Handles navigation, editing, and persistence of settings.

use crate::config::{SettingValue, SettingsTab};
use crate::confirm_dialog::ConfirmDialogState;
use crate::message::Message;
use crate::settings_items::get_selected_item;
use crate::state::AppState;

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
        state.ui_mode = crate::state::UiMode::ConfirmDialog;
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
    let item_count = get_item_count_for_tab(state);
    state.settings_view_state.select_next(item_count);
    UpdateResult::none()
}

/// Handle settings previous item message
pub fn handle_settings_prev_item(state: &mut AppState) -> UpdateResult {
    let item_count = get_item_count_for_tab(state);
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
            // Dispatch LaunchConfigCreate when the add-new sentinel is selected
            if item.id == "launch.__add_new__" {
                return update(state, Message::LaunchConfigCreate);
            }

            // dart_defines items open the dedicated modal overlay instead of
            // inline edit mode.  Extract config_idx from the item ID which has
            // the format "launch.{idx}.dart_defines".
            if item.id.ends_with(".dart_defines") {
                let parts: Vec<&str> = item.id.split('.').collect();
                if let Some(idx_str) = parts.get(1) {
                    if let Ok(config_idx) = idx_str.parse::<usize>() {
                        return update(state, Message::SettingsDartDefinesOpen { config_idx });
                    }
                }
                return UpdateResult::none();
            }

            // extra_args items open the fuzzy modal overlay instead of inline
            // edit mode.  Extract config_idx from the item ID which has the
            // format "launch.{idx}.extra_args".
            if item.id.ends_with(".extra_args") {
                let parts: Vec<&str> = item.id.split('.').collect();
                if let Some(idx_str) = parts.get(1) {
                    if let Ok(config_idx) = idx_str.parse::<usize>() {
                        return update(state, Message::SettingsExtraArgsOpen { config_idx });
                    }
                }
                return UpdateResult::none();
            }

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

/// Get the number of items in the currently active settings tab.
///
/// Counts are derived by calling the same item builder functions used for
/// rendering, guaranteeing that navigation and display always agree.
fn get_item_count_for_tab(state: &AppState) -> usize {
    use crate::config::{launch::load_launch_configs, load_vscode_configs};
    use crate::settings_items::{
        launch_config_items, project_settings_items, user_prefs_items, vscode_config_items,
    };

    match state.settings_view_state.active_tab {
        SettingsTab::Project => project_settings_items(&state.settings).len(),
        SettingsTab::UserPrefs => {
            user_prefs_items(&state.settings_view_state.user_prefs, &state.settings).len()
        }
        SettingsTab::LaunchConfig => {
            let configs = load_launch_configs(&state.project_path);
            let item_count: usize = configs
                .iter()
                .enumerate()
                .map(|(idx, resolved)| launch_config_items(&resolved.config, idx).len())
                .sum();
            if item_count > 0 {
                item_count + 1 // +1 for "Add New Configuration" button
            } else {
                0
            }
        }
        SettingsTab::VSCodeConfig => {
            let configs = load_vscode_configs(&state.project_path);
            configs
                .iter()
                .enumerate()
                .map(|(idx, resolved)| vscode_config_items(&resolved.config, idx).len())
                .sum()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::SettingsTab;
    use crate::state::AppState;

    /// Helper: create AppState with a given active settings tab
    fn state_with_tab(tab: SettingsTab) -> AppState {
        let mut state = AppState::new();
        state.settings_view_state.active_tab = tab;
        state
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Regression tests: count must always match the item builder output
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_project_tab_count_matches_actual_items() {
        let state = state_with_tab(SettingsTab::Project);
        let count = get_item_count_for_tab(&state);
        let items = crate::settings_items::project_settings_items(&state.settings);
        assert_eq!(
            count,
            items.len(),
            "Project tab count drifted from actual items"
        );
    }

    #[test]
    fn test_user_prefs_tab_count_matches_actual_items() {
        let state = state_with_tab(SettingsTab::UserPrefs);
        let count = get_item_count_for_tab(&state);
        let items = crate::settings_items::user_prefs_items(
            &state.settings_view_state.user_prefs,
            &state.settings,
        );
        assert_eq!(
            count,
            items.len(),
            "UserPrefs tab count drifted from actual items"
        );
    }

    /// With no project path set (PathBuf::new()), no launch config file exists,
    /// so the count must be 0, not the old hardcoded estimate.
    #[test]
    fn test_launch_config_tab_count_is_zero_when_no_configs_exist() {
        let state = state_with_tab(SettingsTab::LaunchConfig);
        let count = get_item_count_for_tab(&state);
        assert_eq!(
            count, 0,
            "LaunchConfig tab should return 0 when no configs are loaded"
        );
    }

    /// With no project path set (PathBuf::new()), no VSCode config file exists,
    /// so the count must be 0, not the old hardcoded estimate.
    #[test]
    fn test_vscode_config_tab_count_is_zero_when_no_configs_exist() {
        let state = state_with_tab(SettingsTab::VSCodeConfig);
        let count = get_item_count_for_tab(&state);
        assert_eq!(
            count, 0,
            "VSCodeConfig tab should return 0 when no configs are loaded"
        );
    }

    /// Verify that the project tab no longer returns the stale hardcoded value
    /// of 17 when the actual item count has grown.
    #[test]
    fn test_project_tab_count_is_not_stale_hardcoded_17() {
        let state = state_with_tab(SettingsTab::Project);
        let count = get_item_count_for_tab(&state);
        assert_ne!(
            count, 17,
            "Project tab count must not be the stale hardcoded value of 17"
        );
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Bug fix tests: "Add New Configuration" button navigation
    // ─────────────────────────────────────────────────────────────────────────

    /// When configs exist, the item count must include +1 for the add-new button.
    #[test]
    fn test_launch_config_item_count_includes_add_new_button() {
        use crate::config::launch::init_launch_file;
        use tempfile::tempdir;

        let temp = tempdir().unwrap();
        init_launch_file(temp.path()).unwrap();

        let mut state = state_with_tab(SettingsTab::LaunchConfig);
        state.project_path = temp.path().to_path_buf();

        let count = get_item_count_for_tab(&state);
        // 7 items per config + 1 for "Add New Configuration" button
        assert_eq!(count, 8, "1 default config (7 items) + 1 add-new button");
    }

    /// When there are no configs, the count must be 0 (no add-new button in nav range).
    #[test]
    fn test_launch_config_item_count_zero_when_no_configs() {
        let state = state_with_tab(SettingsTab::LaunchConfig);
        // No project path means no launch.toml; count must be 0
        assert_eq!(get_item_count_for_tab(&state), 0);
    }

    /// get_selected_item returns the add-new sentinel when selected_index == item count.
    #[test]
    fn test_get_selected_item_returns_add_new_sentinel() {
        use crate::config::launch::init_launch_file;
        use crate::settings_items::get_selected_item;
        use tempfile::tempdir;

        let temp = tempdir().unwrap();
        init_launch_file(temp.path()).unwrap();

        let mut state = state_with_tab(SettingsTab::LaunchConfig);
        state.project_path = temp.path().to_path_buf();

        // Select the add-new slot (index 7 = 7 items for 1 config)
        state.settings_view_state.selected_index = 7;

        let item = get_selected_item(
            &state.settings,
            &state.project_path,
            &state.settings_view_state,
        );
        assert!(item.is_some(), "should return sentinel at add-new index");
        assert_eq!(item.unwrap().id, "launch.__add_new__");
    }

    /// Pressing Enter on the add-new row dispatches LaunchConfigCreate.
    #[test]
    fn test_toggle_edit_on_add_new_dispatches_launch_config_create() {
        use crate::config::launch::init_launch_file;
        use tempfile::tempdir;

        let temp = tempdir().unwrap();
        init_launch_file(temp.path()).unwrap();

        let mut state = state_with_tab(SettingsTab::LaunchConfig);
        state.project_path = temp.path().to_path_buf();

        // Count of existing configs before invoking toggle
        let configs_before = crate::config::launch::load_launch_configs(temp.path()).len();

        // Navigate to the add-new slot
        state.settings_view_state.selected_index = 7;

        // Trigger toggle-edit on the add-new row
        handle_settings_toggle_edit(&mut state);

        // A new config should have been written to disk
        let configs_after = crate::config::launch::load_launch_configs(temp.path()).len();
        assert_eq!(
            configs_after,
            configs_before + 1,
            "LaunchConfigCreate should have created one new config"
        );
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Integration tests: Add New Configuration end-to-end (Phase 2, Task 06)
    // ─────────────────────────────────────────────────────────────────────────

    /// Full end-to-end test: navigate to the add-new button via item count, verify
    /// `get_selected_item` returns the sentinel, and confirm that toggling edit
    /// creates a new config on disk.
    #[test]
    fn test_add_new_config_end_to_end() {
        use crate::config::launch::{init_launch_file, load_launch_configs};
        use crate::settings_items::get_selected_item;
        use tempfile::tempdir;

        let temp = tempdir().unwrap();
        init_launch_file(temp.path()).unwrap();

        let mut state = state_with_tab(SettingsTab::LaunchConfig);
        state.project_path = temp.path().to_path_buf();

        // Determine navigation range
        let item_count = get_item_count_for_tab(&state);
        assert!(item_count > 0, "should have items after init_launch_file");

        // Navigate to the last slot (add-new button)
        state.settings_view_state.selected_index = item_count - 1;

        // Verify the sentinel is returned by get_selected_item
        let selected = get_selected_item(
            &state.settings,
            &state.project_path,
            &state.settings_view_state,
        );
        assert!(selected.is_some(), "sentinel item should be returned");
        assert_eq!(
            selected.unwrap().id,
            "launch.__add_new__",
            "last item must be the add-new sentinel"
        );

        // Count configs before creation
        let configs_before = load_launch_configs(temp.path()).len();

        // Toggle edit triggers LaunchConfigCreate → new config written to disk
        handle_settings_toggle_edit(&mut state);

        let configs_after = load_launch_configs(temp.path()).len();
        assert_eq!(
            configs_after,
            configs_before + 1,
            "toggling edit on the sentinel should create exactly one new config"
        );
    }

    /// Verify that pressing Enter on the add-new sentinel with multiple existing
    /// configs still creates exactly one new config.
    #[test]
    fn test_add_new_config_with_multiple_existing_configs() {
        use crate::config::launch::{init_launch_file, load_launch_configs};
        use tempfile::tempdir;

        let temp = tempdir().unwrap();
        // Create two configs
        init_launch_file(temp.path()).unwrap();

        let mut state = state_with_tab(SettingsTab::LaunchConfig);
        state.project_path = temp.path().to_path_buf();

        // Add a second config by simulating add-new twice
        let item_count = get_item_count_for_tab(&state);
        state.settings_view_state.selected_index = item_count - 1;
        handle_settings_toggle_edit(&mut state);

        let configs_after_first = load_launch_configs(temp.path()).len();
        assert_eq!(
            configs_after_first, 2,
            "should have 2 configs after first add"
        );

        // Navigate to add-new again and create a third
        let item_count2 = get_item_count_for_tab(&state);
        state.settings_view_state.selected_index = item_count2 - 1;
        handle_settings_toggle_edit(&mut state);

        let configs_after_second = load_launch_configs(temp.path()).len();
        assert_eq!(
            configs_after_second, 3,
            "should have 3 configs after second add"
        );
    }

    /// When item_count is 0 (no configs), the add-new sentinel is not navigable.
    #[test]
    fn test_no_sentinel_when_no_configs() {
        use crate::settings_items::get_selected_item;

        let state = state_with_tab(SettingsTab::LaunchConfig);
        // No project_path means no launch.toml: count = 0

        let item_count = get_item_count_for_tab(&state);
        assert_eq!(item_count, 0, "count must be 0 without a project path");

        // selected_index=0 with count=0 should not return the sentinel
        let selected = get_selected_item(
            &state.settings,
            &state.project_path,
            &state.settings_view_state,
        );
        // Either None or not the add-new sentinel
        if let Some(item) = selected {
            assert_ne!(
                item.id, "launch.__add_new__",
                "sentinel must not appear when there are no configs"
            );
        }
    }
}
