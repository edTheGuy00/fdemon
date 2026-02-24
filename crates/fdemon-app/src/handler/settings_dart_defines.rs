//! Settings panel — dart defines modal handlers
//!
//! Handles the key-value editor modal for dart defines in the settings launch
//! config tab.  The modal state lives at
//! `AppState::settings_view_state::dart_defines_modal`, and the index of the
//! config being edited is tracked at
//! `AppState::settings_view_state::editing_config_idx`.

use crate::config::launch::{load_launch_configs, save_launch_configs};
use crate::handler::UpdateResult;
use crate::new_session_dialog::{DartDefine, DartDefinesModalState, DartDefinesPane};
use crate::state::AppState;

/// Open the dart defines modal for the launch config at `config_idx`.
///
/// Loads the current dart defines from disk and initialises the
/// `DartDefinesModalState`.  The config index is stored on
/// `settings_view_state.editing_config_idx` so that the close handler knows
/// which config to update.
pub fn handle_settings_dart_defines_open(state: &mut AppState, config_idx: usize) -> UpdateResult {
    if state.settings_view_state.has_modal_open() {
        return UpdateResult::none();
    }
    let configs = load_launch_configs(&state.project_path);
    if let Some(resolved) = configs.get(config_idx) {
        let mut defines: Vec<DartDefine> = resolved
            .config
            .dart_defines
            .iter()
            .map(|(k, v)| DartDefine::new(k.clone(), v.clone()))
            .collect();
        defines.sort_by(|a, b| a.key.cmp(&b.key));
        state.settings_view_state.dart_defines_modal = Some(DartDefinesModalState::new(defines));
        state.settings_view_state.editing_config_idx = Some(config_idx);
    }
    UpdateResult::none()
}

/// Close the dart defines modal and persist all changes to `.fdemon/launch.toml`.
///
/// Takes the defines from the modal, converts them to a `HashMap<String,
/// String>`, and saves the updated launch configs to disk.  The
/// `editing_config_idx` is cleared regardless of whether the save succeeded.
pub fn handle_settings_dart_defines_close(state: &mut AppState) -> UpdateResult {
    if let Some(modal) = state.settings_view_state.dart_defines_modal.take() {
        if let Some(config_idx) = state.settings_view_state.editing_config_idx.take() {
            let mut configs = load_launch_configs(&state.project_path);
            if let Some(resolved) = configs.get_mut(config_idx) {
                resolved.config.dart_defines = modal
                    .defines
                    .iter()
                    .map(|d| (d.key.clone(), d.value.clone()))
                    .collect();
                let config_vec: Vec<_> = configs.iter().map(|r| r.config.clone()).collect();
                if let Err(e) = save_launch_configs(&state.project_path, &config_vec) {
                    state.settings_view_state.error = Some(format!("Failed to save: {}", e));
                }
            }
        } else {
            tracing::warn!(
                "dart defines modal closed with no editing_config_idx — changes discarded"
            );
        }
    }
    UpdateResult::none()
}

/// Cancel the dart defines modal without persisting changes.
pub fn handle_settings_dart_defines_cancel(state: &mut AppState) -> UpdateResult {
    state.settings_view_state.dart_defines_modal = None;
    state.settings_view_state.editing_config_idx = None;
    UpdateResult::none()
}

/// Switch focus between the list pane and the edit pane.
pub fn handle_settings_dart_defines_switch_pane(state: &mut AppState) -> UpdateResult {
    if let Some(ref mut modal) = state.settings_view_state.dart_defines_modal {
        modal.switch_pane();
    }
    UpdateResult::none()
}

/// Navigate up in the dart defines list (List pane only).
pub fn handle_settings_dart_defines_up(state: &mut AppState) -> UpdateResult {
    if let Some(ref mut modal) = state.settings_view_state.dart_defines_modal {
        if modal.active_pane == DartDefinesPane::List {
            modal.navigate_up();
        }
    }
    UpdateResult::none()
}

/// Navigate down in the dart defines list (List pane only).
pub fn handle_settings_dart_defines_down(state: &mut AppState) -> UpdateResult {
    if let Some(ref mut modal) = state.settings_view_state.dart_defines_modal {
        if modal.active_pane == DartDefinesPane::List {
            modal.navigate_down();
        }
    }
    UpdateResult::none()
}

/// Confirm selection or activate the focused button.
///
/// In the List pane: loads the selected item into the edit form.
/// In the Edit pane:
///   - Key/Value fields: advance to next field.
///   - Save button: save the edit (returns focus to Key field if save fails).
///   - Delete button: delete the selected define.
pub fn handle_settings_dart_defines_confirm(state: &mut AppState) -> UpdateResult {
    use crate::new_session_dialog::DartDefinesEditField;

    if let Some(ref mut modal) = state.settings_view_state.dart_defines_modal {
        match modal.active_pane {
            DartDefinesPane::List => {
                modal.load_selected_into_edit();
            }
            DartDefinesPane::Edit => match modal.edit_field {
                DartDefinesEditField::Key | DartDefinesEditField::Value => {
                    modal.next_field();
                }
                DartDefinesEditField::Save => {
                    if !modal.save_edit() {
                        modal.edit_field = DartDefinesEditField::Key;
                    }
                }
                DartDefinesEditField::Delete => {
                    modal.delete_selected();
                }
            },
        }
    }
    UpdateResult::none()
}

/// Move to the next field in the edit form (Tab).
pub fn handle_settings_dart_defines_next_field(state: &mut AppState) -> UpdateResult {
    if let Some(ref mut modal) = state.settings_view_state.dart_defines_modal {
        if modal.active_pane == DartDefinesPane::Edit {
            modal.next_field();
        }
    }
    UpdateResult::none()
}

/// Input a character into the currently focused text field (Edit pane only).
pub fn handle_settings_dart_defines_input(state: &mut AppState, c: char) -> UpdateResult {
    if let Some(ref mut modal) = state.settings_view_state.dart_defines_modal {
        if modal.active_pane == DartDefinesPane::Edit {
            modal.input_char(c);
        }
    }
    UpdateResult::none()
}

/// Backspace in the currently focused text field (Edit pane only).
pub fn handle_settings_dart_defines_backspace(state: &mut AppState) -> UpdateResult {
    if let Some(ref mut modal) = state.settings_view_state.dart_defines_modal {
        if modal.active_pane == DartDefinesPane::Edit {
            modal.backspace();
        }
    }
    UpdateResult::none()
}

/// Save the current edit form entry to the defines list.
pub fn handle_settings_dart_defines_save(state: &mut AppState) -> UpdateResult {
    if let Some(ref mut modal) = state.settings_view_state.dart_defines_modal {
        modal.save_edit();
    }
    UpdateResult::none()
}

/// Delete the currently selected dart define from the list.
pub fn handle_settings_dart_defines_delete(state: &mut AppState) -> UpdateResult {
    if let Some(ref mut modal) = state.settings_view_state.dart_defines_modal {
        modal.delete_selected();
    }
    UpdateResult::none()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::launch::init_launch_file;
    use crate::state::AppState;
    use tempfile::tempdir;

    fn state_with_launch_config() -> (AppState, tempfile::TempDir) {
        let temp = tempdir().unwrap();
        init_launch_file(temp.path()).unwrap();
        let mut state = AppState::new();
        state.project_path = temp.path().to_path_buf();
        (state, temp)
    }

    #[test]
    fn test_open_modal_sets_dart_defines_modal_and_config_idx() {
        let (mut state, _temp) = state_with_launch_config();
        assert!(state.settings_view_state.dart_defines_modal.is_none());
        assert!(state.settings_view_state.editing_config_idx.is_none());

        handle_settings_dart_defines_open(&mut state, 0);

        assert!(state.settings_view_state.dart_defines_modal.is_some());
        assert_eq!(state.settings_view_state.editing_config_idx, Some(0));
    }

    #[test]
    fn test_open_modal_out_of_range_leaves_modal_none() {
        let (mut state, _temp) = state_with_launch_config();

        handle_settings_dart_defines_open(&mut state, 99);

        assert!(state.settings_view_state.dart_defines_modal.is_none());
        assert!(state.settings_view_state.editing_config_idx.is_none());
    }

    #[test]
    fn test_close_modal_persists_defines_to_disk() {
        let (mut state, temp) = state_with_launch_config();

        // Open modal
        handle_settings_dart_defines_open(&mut state, 0);

        // Add a dart define via the modal state
        if let Some(ref mut modal) = state.settings_view_state.dart_defines_modal {
            modal.defines.push(DartDefine::new("MY_KEY", "my_value"));
        }

        // Close persists
        handle_settings_dart_defines_close(&mut state);

        // Modal and idx are cleared
        assert!(state.settings_view_state.dart_defines_modal.is_none());
        assert!(state.settings_view_state.editing_config_idx.is_none());

        // Load from disk and verify
        let configs = load_launch_configs(temp.path());
        assert!(!configs.is_empty());
        assert_eq!(
            configs[0].config.dart_defines.get("MY_KEY"),
            Some(&"my_value".to_string())
        );
    }

    #[test]
    fn test_close_modal_with_no_modal_is_noop() {
        let (mut state, _temp) = state_with_launch_config();
        // No modal open — should not panic
        handle_settings_dart_defines_close(&mut state);
    }

    #[test]
    fn test_switch_pane_toggles_active_pane() {
        let (mut state, _temp) = state_with_launch_config();
        handle_settings_dart_defines_open(&mut state, 0);

        let initial_pane = state
            .settings_view_state
            .dart_defines_modal
            .as_ref()
            .unwrap()
            .active_pane
            .clone();

        handle_settings_dart_defines_switch_pane(&mut state);

        let new_pane = state
            .settings_view_state
            .dart_defines_modal
            .as_ref()
            .unwrap()
            .active_pane
            .clone();

        assert_ne!(initial_pane, new_pane);
    }

    #[test]
    fn test_navigate_up_and_down() {
        use crate::new_session_dialog::DartDefine;
        let (mut state, _temp) = state_with_launch_config();
        handle_settings_dart_defines_open(&mut state, 0);

        // Add two defines so navigation is meaningful
        if let Some(ref mut modal) = state.settings_view_state.dart_defines_modal {
            modal.defines = vec![DartDefine::new("A", "1"), DartDefine::new("B", "2")];
            modal.selected_index = 0;
        }

        handle_settings_dart_defines_down(&mut state);
        assert_eq!(
            state
                .settings_view_state
                .dart_defines_modal
                .as_ref()
                .unwrap()
                .selected_index,
            1
        );

        handle_settings_dart_defines_up(&mut state);
        assert_eq!(
            state
                .settings_view_state
                .dart_defines_modal
                .as_ref()
                .unwrap()
                .selected_index,
            0
        );
    }

    #[test]
    fn test_input_and_backspace_in_edit_pane() {
        use crate::new_session_dialog::DartDefinesPane;
        let (mut state, _temp) = state_with_launch_config();
        handle_settings_dart_defines_open(&mut state, 0);

        // Switch to edit pane
        if let Some(ref mut modal) = state.settings_view_state.dart_defines_modal {
            modal.active_pane = DartDefinesPane::Edit;
        }

        handle_settings_dart_defines_input(&mut state, 'H');
        handle_settings_dart_defines_input(&mut state, 'i');

        assert_eq!(
            state
                .settings_view_state
                .dart_defines_modal
                .as_ref()
                .unwrap()
                .editing_key,
            "Hi"
        );

        handle_settings_dart_defines_backspace(&mut state);

        assert_eq!(
            state
                .settings_view_state
                .dart_defines_modal
                .as_ref()
                .unwrap()
                .editing_key,
            "H"
        );
    }

    #[test]
    fn test_save_and_delete() {
        use crate::new_session_dialog::DartDefinesPane;
        let (mut state, _temp) = state_with_launch_config();
        handle_settings_dart_defines_open(&mut state, 0);

        // Prepare edit pane with a new entry
        if let Some(ref mut modal) = state.settings_view_state.dart_defines_modal {
            modal.active_pane = DartDefinesPane::Edit;
            modal.editing_key = "FOO".to_string();
            modal.editing_value = "bar".to_string();
            modal.is_new = true;
        }

        handle_settings_dart_defines_save(&mut state);

        let count = state
            .settings_view_state
            .dart_defines_modal
            .as_ref()
            .unwrap()
            .defines
            .len();
        assert_eq!(count, 1);
        assert_eq!(
            state
                .settings_view_state
                .dart_defines_modal
                .as_ref()
                .unwrap()
                .defines[0]
                .key,
            "FOO"
        );

        handle_settings_dart_defines_delete(&mut state);

        let count_after = state
            .settings_view_state
            .dart_defines_modal
            .as_ref()
            .unwrap()
            .defines
            .len();
        assert_eq!(count_after, 0);
    }

    /// Verify the full round-trip: open → edit → close → re-open confirms
    /// the persisted value is visible.
    #[test]
    fn test_open_reflects_previously_saved_defines() {
        let (mut state, temp) = state_with_launch_config();

        // Open, add a define, close (persists)
        handle_settings_dart_defines_open(&mut state, 0);
        if let Some(ref mut modal) = state.settings_view_state.dart_defines_modal {
            modal.defines.push(DartDefine::new("PERSIST", "yes"));
        }
        handle_settings_dart_defines_close(&mut state);

        // Re-open should reflect saved define
        state.project_path = temp.path().to_path_buf();
        handle_settings_dart_defines_open(&mut state, 0);
        let modal = state
            .settings_view_state
            .dart_defines_modal
            .as_ref()
            .unwrap();
        assert!(
            modal.defines.iter().any(|d| d.key == "PERSIST"),
            "Expected PERSIST define to be loaded from disk"
        );
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Phase-2 review regression anchors (Task 06)
    // ─────────────────────────────────────────────────────────────────────────

    /// Cancel discards in-flight edits — no data reaches disk.
    ///
    /// Regression anchor for Critical #1: `SettingsDartDefinesCancel` must
    /// discard all working-copy changes and leave the on-disk launch config
    /// untouched.
    #[test]
    fn test_dart_defines_cancel_does_not_persist() {
        let (mut state, temp) = state_with_launch_config();

        // Open the modal and append a define to the working copy.
        handle_settings_dart_defines_open(&mut state, 0);
        if let Some(ref mut modal) = state.settings_view_state.dart_defines_modal {
            modal
                .defines
                .push(DartDefine::new("SHOULD_NOT_SAVE", "true"));
        }

        // Cancel — must NOT persist changes.
        handle_settings_dart_defines_cancel(&mut state);

        // Modal and editing index are cleared.
        assert!(state.settings_view_state.dart_defines_modal.is_none());
        assert!(state.settings_view_state.editing_config_idx.is_none());

        // Disk state is unchanged — the added define must NOT be present.
        let configs = load_launch_configs(temp.path());
        assert!(!configs.is_empty());
        assert!(
            configs[0]
                .config
                .dart_defines
                .get("SHOULD_NOT_SAVE")
                .is_none(),
            "Cancel must not persist in-flight changes to disk"
        );
    }

    /// Defines are sorted alphabetically when the modal is opened.
    ///
    /// Regression anchor for Minor #9: `handle_settings_dart_defines_open`
    /// must sort the loaded defines by key before populating the modal so the
    /// list is always in a deterministic order.
    #[test]
    fn test_dart_defines_sorted_alphabetically_on_open() {
        let temp = tempdir().unwrap();

        // Write a launch config that intentionally has out-of-order dart defines.
        let mut dart_defines = std::collections::HashMap::new();
        dart_defines.insert("zebra".to_string(), "1".to_string());
        dart_defines.insert("apple".to_string(), "2".to_string());
        dart_defines.insert("mango".to_string(), "3".to_string());

        let configs = vec![crate::config::LaunchConfig {
            name: "Sorted Test".to_string(),
            device: "auto".to_string(),
            dart_defines,
            ..Default::default()
        }];
        crate::config::launch::save_launch_configs(temp.path(), &configs).unwrap();

        let mut state = crate::state::AppState::new();
        state.project_path = temp.path().to_path_buf();

        handle_settings_dart_defines_open(&mut state, 0);

        let modal = state
            .settings_view_state
            .dart_defines_modal
            .as_ref()
            .expect("modal should be open");

        let keys: Vec<&str> = modal.defines.iter().map(|d| d.key.as_str()).collect();
        assert_eq!(
            keys,
            vec!["apple", "mango", "zebra"],
            "Dart defines must be sorted alphabetically by key on open"
        );
    }

    /// Re-opening the dart defines modal when it is already open is a no-op.
    ///
    /// Regression anchor for Critical #2: the `has_modal_open()` guard must
    /// prevent a second `handle_settings_dart_defines_open` call from
    /// overwriting the `editing_config_idx` of the first open.
    #[test]
    fn test_dart_defines_open_blocked_when_already_open() {
        let (mut state, _temp) = state_with_launch_config();

        // Open the dart defines modal for config 0.
        handle_settings_dart_defines_open(&mut state, 0);
        assert!(state.settings_view_state.dart_defines_modal.is_some());
        assert_eq!(state.settings_view_state.editing_config_idx, Some(0));

        // Attempt to re-open for a different index — must be blocked.
        handle_settings_dart_defines_open(&mut state, 1);

        assert_eq!(
            state.settings_view_state.editing_config_idx,
            Some(0),
            "editing_config_idx must not be overwritten by a second open"
        );
    }

    /// Routing: Enter on a dart_defines item dispatches SettingsDartDefinesOpen.
    #[test]
    fn test_enter_on_dart_defines_item_opens_modal() {
        use crate::config::launch::init_launch_file;
        use crate::config::SettingsTab;
        use crate::handler::settings_handlers::handle_settings_toggle_edit;

        let temp = tempdir().unwrap();
        init_launch_file(temp.path()).unwrap();

        let mut state = AppState::new();
        state.project_path = temp.path().to_path_buf();
        state.settings_view_state.active_tab = SettingsTab::LaunchConfig;

        // Find the dart_defines item index
        use crate::config::launch::load_launch_configs;
        use crate::settings_items::launch_config_items;
        let configs = load_launch_configs(temp.path());
        assert!(!configs.is_empty(), "need at least one config");
        let items = launch_config_items(&configs[0].config, 0);
        let dart_defines_idx = items
            .iter()
            .position(|item| item.id.ends_with(".dart_defines"))
            .expect("dart_defines item must exist");

        state.settings_view_state.selected_index = dart_defines_idx;

        handle_settings_toggle_edit(&mut state);

        assert!(
            state.settings_view_state.dart_defines_modal.is_some(),
            "dart defines modal should be open after pressing Enter on dart_defines item"
        );
        assert_eq!(state.settings_view_state.editing_config_idx, Some(0));
    }
}
