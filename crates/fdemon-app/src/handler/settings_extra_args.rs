//! Settings panel — extra args fuzzy modal handlers
//!
//! Handles the fuzzy-search modal for adding/removing `extra_args` entries in
//! the settings launch-config tab.  The modal state lives at
//! `AppState::settings_view_state::extra_args_modal`, and the index of the
//! config being edited is tracked at
//! `AppState::settings_view_state::editing_config_idx` (shared with the dart
//! defines modal — only one modal is ever open at a time).

use crate::config::launch::{load_launch_configs, save_launch_configs};
use crate::handler::UpdateResult;
use crate::new_session_dialog::fuzzy::fuzzy_filter;
use crate::new_session_dialog::{FuzzyModalState, FuzzyModalType};
use crate::state::AppState;

/// Preset Flutter CLI flags shown in the extra args fuzzy picker when
/// the launch config has no existing extra args. Users can always type
/// custom flags via the modal's custom input support.
const PRESET_EXTRA_ARGS: &[&str] = &[
    "--verbose",
    "--trace-startup",
    "--trace-skia",
    "--enable-software-rendering",
    "--dart-entrypoint-args",
];

/// Open the extra args fuzzy modal for the launch config at `config_idx`.
///
/// Loads the current `extra_args` from disk and initialises a
/// [`FuzzyModalState`] with them as the item list.  When the list is empty the
/// preset suggestions are shown so the user has something to pick from.
/// The config index is stored on `settings_view_state.editing_config_idx` so
/// that the confirm/close handlers know which config to update.
pub fn handle_settings_extra_args_open(state: &mut AppState, config_idx: usize) -> UpdateResult {
    if state.settings_view_state.has_modal_open() {
        return UpdateResult::none();
    }
    let configs = load_launch_configs(&state.project_path);
    if let Some(resolved) = configs.get(config_idx) {
        let items: Vec<String> = if resolved.config.extra_args.is_empty() {
            PRESET_EXTRA_ARGS.iter().map(|s| s.to_string()).collect()
        } else {
            resolved.config.extra_args.clone()
        };
        state.settings_view_state.extra_args_modal =
            Some(FuzzyModalState::new(FuzzyModalType::ExtraArgs, items));
        state.settings_view_state.editing_config_idx = Some(config_idx);
    }
    UpdateResult::none()
}

/// Close the extra args modal without persisting any changes.
///
/// Clears both `extra_args_modal` and `editing_config_idx`.
pub fn handle_settings_extra_args_close(state: &mut AppState) -> UpdateResult {
    state.settings_view_state.extra_args_modal = None;
    state.settings_view_state.editing_config_idx = None;
    UpdateResult::none()
}

/// Handle a character typed into the modal's search field.
///
/// Appends the character to the query and re-runs the fuzzy filter.
pub fn handle_settings_extra_args_input(state: &mut AppState, c: char) -> UpdateResult {
    if let Some(ref mut modal) = state.settings_view_state.extra_args_modal {
        modal.input_char(c);
        let filtered = fuzzy_filter(&modal.query, &modal.items);
        modal.update_filter(filtered);
    }
    UpdateResult::none()
}

/// Remove the last character from the modal's search field.
///
/// Re-runs the fuzzy filter after the query changes.
pub fn handle_settings_extra_args_backspace(state: &mut AppState) -> UpdateResult {
    if let Some(ref mut modal) = state.settings_view_state.extra_args_modal {
        modal.backspace();
        let filtered = fuzzy_filter(&modal.query, &modal.items);
        modal.update_filter(filtered);
    }
    UpdateResult::none()
}

/// Clear the entire query in the modal's search field.
///
/// Re-runs the fuzzy filter (which will show all items again).
pub fn handle_settings_extra_args_clear(state: &mut AppState) -> UpdateResult {
    if let Some(ref mut modal) = state.settings_view_state.extra_args_modal {
        modal.clear_query();
        let filtered = fuzzy_filter(&modal.query, &modal.items);
        modal.update_filter(filtered);
    }
    UpdateResult::none()
}

/// Move the selection cursor up in the filtered results list.
pub fn handle_settings_extra_args_up(state: &mut AppState) -> UpdateResult {
    if let Some(ref mut modal) = state.settings_view_state.extra_args_modal {
        modal.navigate_up();
    }
    UpdateResult::none()
}

/// Move the selection cursor down in the filtered results list.
pub fn handle_settings_extra_args_down(state: &mut AppState) -> UpdateResult {
    if let Some(ref mut modal) = state.settings_view_state.extra_args_modal {
        modal.navigate_down();
    }
    UpdateResult::none()
}

/// Confirm the current selection or custom query.
///
/// The selected value (or the raw query text when `allows_custom` is true and
/// no items match) is appended to the config's `extra_args` list if it is not
/// already present.  The updated configs are persisted to
/// `.fdemon/launch.toml`.  The modal is closed after a successful confirm.
///
/// When `selected_value()` returns `None` (empty filter with no typed query),
/// the function returns early so the modal stays open — the user retains their
/// context without losing the modal unexpectedly.
pub fn handle_settings_extra_args_confirm(state: &mut AppState) -> UpdateResult {
    // Extract selected value in a temporary scope to satisfy the borrow checker:
    // `modal` borrows `state` immutably, so we must drop that borrow before we
    // can mutate `state` below (saving configs, closing the modal, etc.).
    let selected = {
        let modal = match state.settings_view_state.extra_args_modal.as_ref() {
            Some(m) => m,
            None => return UpdateResult::none(),
        };
        match modal.selected_value() {
            Some(v) => v,
            // No selection and no custom query — keep the modal open so the
            // user doesn't silently lose their context.
            None => return UpdateResult::none(),
        }
    };

    if let Some(config_idx) = state.settings_view_state.editing_config_idx {
        let mut configs = load_launch_configs(&state.project_path);
        if let Some(resolved) = configs.get_mut(config_idx) {
            // Add the arg if not already present
            if !resolved.config.extra_args.contains(&selected) {
                resolved.config.extra_args.push(selected);
            }
            let config_vec: Vec<_> = configs.iter().map(|r| r.config.clone()).collect();
            if let Err(e) = save_launch_configs(&state.project_path, &config_vec) {
                state.settings_view_state.error = Some(format!("Failed to save: {}", e));
            }
        }
    }

    // Only close the modal after a successful selection was processed
    state.settings_view_state.extra_args_modal = None;
    state.settings_view_state.editing_config_idx = None;
    UpdateResult::none()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::launch::{init_launch_file, load_launch_configs};
    use crate::config::SettingsTab;
    use crate::handler::settings_handlers::handle_settings_toggle_edit;
    use crate::settings_items::launch_config_items;
    use crate::state::AppState;
    use tempfile::tempdir;

    fn state_with_launch_config() -> (AppState, tempfile::TempDir) {
        let temp = tempdir().unwrap();
        init_launch_file(temp.path()).unwrap();
        let mut state = AppState::new();
        state.project_path = temp.path().to_path_buf();
        (state, temp)
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Open / close
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_open_sets_extra_args_modal_and_config_idx() {
        let (mut state, _temp) = state_with_launch_config();
        assert!(state.settings_view_state.extra_args_modal.is_none());
        assert!(state.settings_view_state.editing_config_idx.is_none());

        handle_settings_extra_args_open(&mut state, 0);

        assert!(state.settings_view_state.extra_args_modal.is_some());
        assert_eq!(state.settings_view_state.editing_config_idx, Some(0));
    }

    #[test]
    fn test_open_out_of_range_leaves_modal_none() {
        let (mut state, _temp) = state_with_launch_config();

        handle_settings_extra_args_open(&mut state, 99);

        assert!(state.settings_view_state.extra_args_modal.is_none());
        assert!(state.settings_view_state.editing_config_idx.is_none());
    }

    #[test]
    fn test_open_with_empty_extra_args_shows_presets() {
        let (mut state, _temp) = state_with_launch_config();

        handle_settings_extra_args_open(&mut state, 0);

        let modal = state.settings_view_state.extra_args_modal.as_ref().unwrap();
        assert!(
            !modal.items.is_empty(),
            "Modal items should contain preset args when config.extra_args is empty"
        );
        // Verify at least one preset is present
        assert!(
            modal.items.iter().any(|i| i == "--verbose"),
            "Expected '--verbose' preset to be in items"
        );
    }

    #[test]
    fn test_close_clears_modal_and_idx_without_saving() {
        let (mut state, temp) = state_with_launch_config();

        handle_settings_extra_args_open(&mut state, 0);
        assert!(state.settings_view_state.extra_args_modal.is_some());

        handle_settings_extra_args_close(&mut state);

        assert!(state.settings_view_state.extra_args_modal.is_none());
        assert!(state.settings_view_state.editing_config_idx.is_none());

        // Nothing should have been written to disk
        let configs = load_launch_configs(temp.path());
        assert!(
            configs[0].config.extra_args.is_empty(),
            "Close should not persist any changes"
        );
    }

    #[test]
    fn test_close_with_no_modal_is_noop() {
        let (mut state, _temp) = state_with_launch_config();
        // No modal open — should not panic
        handle_settings_extra_args_close(&mut state);
        assert!(state.settings_view_state.extra_args_modal.is_none());
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Input / backspace / clear
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_input_appends_to_query_and_filters() {
        let (mut state, _temp) = state_with_launch_config();
        handle_settings_extra_args_open(&mut state, 0);

        handle_settings_extra_args_input(&mut state, '-');
        handle_settings_extra_args_input(&mut state, 'v');

        let modal = state.settings_view_state.extra_args_modal.as_ref().unwrap();
        assert_eq!(modal.query, "-v");
        // Filtered list should only include items matching "-v"
        for &idx in &modal.filtered_indices {
            assert!(
                modal.items[idx].to_lowercase().contains("-v"),
                "Filtered items should contain '-v'"
            );
        }
    }

    #[test]
    fn test_backspace_removes_last_char() {
        let (mut state, _temp) = state_with_launch_config();
        handle_settings_extra_args_open(&mut state, 0);

        handle_settings_extra_args_input(&mut state, '-');
        handle_settings_extra_args_input(&mut state, 'v');
        handle_settings_extra_args_backspace(&mut state);

        let modal = state.settings_view_state.extra_args_modal.as_ref().unwrap();
        assert_eq!(modal.query, "-");
    }

    #[test]
    fn test_clear_empties_query() {
        let (mut state, _temp) = state_with_launch_config();
        handle_settings_extra_args_open(&mut state, 0);

        handle_settings_extra_args_input(&mut state, '-');
        handle_settings_extra_args_input(&mut state, 'v');
        handle_settings_extra_args_clear(&mut state);

        let modal = state.settings_view_state.extra_args_modal.as_ref().unwrap();
        assert!(modal.query.is_empty());
        // All items should be visible after clear
        assert_eq!(modal.filtered_indices.len(), modal.items.len());
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Navigation
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_navigate_down_and_up() {
        let (mut state, _temp) = state_with_launch_config();
        handle_settings_extra_args_open(&mut state, 0);

        let initial_idx = state
            .settings_view_state
            .extra_args_modal
            .as_ref()
            .unwrap()
            .selected_index;

        handle_settings_extra_args_down(&mut state);
        let after_down = state
            .settings_view_state
            .extra_args_modal
            .as_ref()
            .unwrap()
            .selected_index;

        // Only changes if there are at least 2 items
        let item_count = state
            .settings_view_state
            .extra_args_modal
            .as_ref()
            .unwrap()
            .items
            .len();
        if item_count > 1 {
            assert_ne!(initial_idx, after_down, "Down should move selection");
        }

        handle_settings_extra_args_up(&mut state);
        let after_up = state
            .settings_view_state
            .extra_args_modal
            .as_ref()
            .unwrap()
            .selected_index;
        assert_eq!(
            after_up, initial_idx,
            "Up after Down should return to start"
        );
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Confirm
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_confirm_adds_selected_arg_to_config() {
        let (mut state, temp) = state_with_launch_config();
        handle_settings_extra_args_open(&mut state, 0);

        // Navigate to the first preset ("--verbose")
        let modal = state.settings_view_state.extra_args_modal.as_ref().unwrap();
        let first_item = modal.items[modal.filtered_indices[0]].clone();

        handle_settings_extra_args_confirm(&mut state);

        // Modal should be closed
        assert!(state.settings_view_state.extra_args_modal.is_none());
        assert!(state.settings_view_state.editing_config_idx.is_none());

        // Arg should be persisted on disk
        let configs = load_launch_configs(temp.path());
        assert!(
            configs[0].config.extra_args.contains(&first_item),
            "Confirmed arg should be persisted to disk"
        );
    }

    #[test]
    fn test_confirm_with_custom_arg_via_query() {
        let (mut state, temp) = state_with_launch_config();
        handle_settings_extra_args_open(&mut state, 0);

        // Type a custom arg that doesn't match any preset
        let custom_arg = "--my-custom-flag";
        for c in custom_arg.chars() {
            handle_settings_extra_args_input(&mut state, c);
        }

        // filtered_indices should be empty (no preset matches "--my-custom-flag")
        // FuzzyModalState::selected_value() falls back to query when allows_custom && no matches
        handle_settings_extra_args_confirm(&mut state);

        assert!(state.settings_view_state.extra_args_modal.is_none());

        let configs = load_launch_configs(temp.path());
        assert!(
            configs[0]
                .config
                .extra_args
                .contains(&custom_arg.to_string()),
            "Custom arg typed via query should be persisted"
        );
    }

    #[test]
    fn test_confirm_does_not_duplicate_existing_arg() {
        let (mut state, temp) = state_with_launch_config();
        handle_settings_extra_args_open(&mut state, 0);

        let modal = state.settings_view_state.extra_args_modal.as_ref().unwrap();
        let first_item = modal.items[modal.filtered_indices[0]].clone();

        // Confirm twice
        handle_settings_extra_args_confirm(&mut state);
        handle_settings_extra_args_open(&mut state, 0);
        // Select the same item again (it's now in the real list, not presets)
        handle_settings_extra_args_confirm(&mut state);

        let configs = load_launch_configs(temp.path());
        let count = configs[0]
            .config
            .extra_args
            .iter()
            .filter(|a| *a == &first_item)
            .count();
        assert_eq!(count, 1, "Arg should not be duplicated on second confirm");
    }

    #[test]
    fn test_confirm_with_no_modal_is_noop() {
        let (mut state, _temp) = state_with_launch_config();
        // No modal open — should not panic
        handle_settings_extra_args_confirm(&mut state);
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Phase-2 review regression anchors (Task 06)
    // ─────────────────────────────────────────────────────────────────────────

    /// Opening the dart defines modal is a no-op when the extra args modal is
    /// already open.
    ///
    /// Regression anchor for Critical #2: the `has_modal_open()` guard in
    /// `handle_settings_dart_defines_open` must prevent a second modal from
    /// being opened while the extra args modal is active.
    #[test]
    fn test_dart_defines_open_noop_when_extra_args_modal_active() {
        let (mut state, _temp) = state_with_launch_config();

        // Open the extra args modal first.
        handle_settings_extra_args_open(&mut state, 0);
        assert!(state.settings_view_state.extra_args_modal.is_some());
        assert_eq!(state.settings_view_state.editing_config_idx, Some(0));

        // Attempting to open the dart defines modal must be a no-op.
        crate::handler::settings_dart_defines::handle_settings_dart_defines_open(&mut state, 0);

        assert!(
            state.settings_view_state.dart_defines_modal.is_none(),
            "dart_defines_modal must remain None while extra_args_modal is open"
        );
    }

    /// Opening the extra args modal is a no-op when the dart defines modal is
    /// already open.
    ///
    /// Regression anchor for Critical #2: the `has_modal_open()` guard in
    /// `handle_settings_extra_args_open` must prevent a second modal from
    /// being opened while the dart defines modal is active.
    #[test]
    fn test_extra_args_open_noop_when_dart_defines_modal_active() {
        let (mut state, _temp) = state_with_launch_config();

        // Open the dart defines modal first.
        crate::handler::settings_dart_defines::handle_settings_dart_defines_open(&mut state, 0);
        assert!(state.settings_view_state.dart_defines_modal.is_some());
        assert_eq!(state.settings_view_state.editing_config_idx, Some(0));

        // Attempting to open the extra args modal must be a no-op.
        handle_settings_extra_args_open(&mut state, 0);

        assert!(
            state.settings_view_state.extra_args_modal.is_none(),
            "extra_args_modal must remain None while dart_defines_modal is open"
        );
    }

    /// Confirming with no selection (empty filter, no query) keeps the modal open.
    ///
    /// Regression anchor for Major #6: when `selected_value()` returns `None`
    /// the handler must return early without closing the modal so the user
    /// does not silently lose their context.
    #[test]
    fn test_extra_args_confirm_with_no_selection_keeps_modal_open() {
        let (mut state, _temp) = state_with_launch_config();
        handle_settings_extra_args_open(&mut state, 0);

        // Force the filtered list to be empty and the query to be empty so
        // that `selected_value()` returns `None`.
        if let Some(ref mut modal) = state.settings_view_state.extra_args_modal {
            modal.filtered_indices.clear();
            modal.query.clear();
        }

        // Confirm with nothing selected — modal must stay open.
        handle_settings_extra_args_confirm(&mut state);

        assert!(
            state.settings_view_state.extra_args_modal.is_some(),
            "Modal must remain open when confirm is called with no selection"
        );
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Routing: Enter on extra_args item opens modal
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_enter_on_extra_args_item_opens_modal() {
        let temp = tempdir().unwrap();
        init_launch_file(temp.path()).unwrap();

        let mut state = AppState::new();
        state.project_path = temp.path().to_path_buf();
        state.settings_view_state.active_tab = SettingsTab::LaunchConfig;

        // Find the extra_args item index
        let configs = load_launch_configs(temp.path());
        assert!(!configs.is_empty(), "need at least one config");
        let items = launch_config_items(&configs[0].config, 0);
        let extra_args_idx = items
            .iter()
            .position(|item| item.id.ends_with(".extra_args"))
            .expect("extra_args item must exist");

        state.settings_view_state.selected_index = extra_args_idx;

        handle_settings_toggle_edit(&mut state);

        assert!(
            state.settings_view_state.extra_args_modal.is_some(),
            "extra args modal should be open after pressing Enter on extra_args item"
        );
        assert_eq!(state.settings_view_state.editing_config_idx, Some(0));
    }
}
