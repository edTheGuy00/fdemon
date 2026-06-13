//! Settings page handlers
//!
//! Handles navigation, editing, and persistence of settings.

use crate::config::{SettingItem, SettingValue, SettingsTab};
use crate::confirm_dialog::ConfirmDialogState;
use crate::message::Message;
use crate::settings_items::get_selected_item;
use crate::state::{AppState, SettingsClickStamp};

use super::{update, UpdateAction, UpdateResult, DOUBLE_CLICK_WINDOW_MS};

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
                ("Cancel", Message::SettingsCancelClose),
            ],
        ));
        // Keep the settings panel visible behind the dialog: the prompt is a
        // confirmation *over* the page, not a replacement for it.
        state.confirm_dialog_backdrop = Some(crate::state::UiMode::Settings);
        state.ui_mode = crate::state::UiMode::ConfirmDialog;
    } else {
        state.hide_settings();
    }
    // Clear any pending double-click pairing for hygiene (mirrors last_log_click
    // reset behaviour on session change). The 400 ms window would naturally
    // expire, but explicit clearing is cleaner.
    state.last_settings_click = None;
    UpdateResult::none()
}

/// Handle settings next tab message
pub fn handle_settings_next_tab(state: &mut AppState) -> UpdateResult {
    let extra_count = state.extra_settings_tabs.len();
    state.settings_view_state.next_tab(extra_count);
    // Tab change invalidates any pending double-click pairing: row 5 on the
    // Project tab and row 5 on the User tab must not be treated as a pair.
    state.last_settings_click = None;
    UpdateResult::none()
}

/// Handle settings previous tab message
pub fn handle_settings_prev_tab(state: &mut AppState) -> UpdateResult {
    let extra_count = state.extra_settings_tabs.len();
    state.settings_view_state.prev_tab(extra_count);
    // Tab change invalidates any pending double-click pairing.
    state.last_settings_click = None;
    UpdateResult::none()
}

/// Handle settings goto tab message
pub fn handle_settings_goto_tab(state: &mut AppState, idx: usize) -> UpdateResult {
    if let Some(tab) = SettingsTab::from_index_with(idx, state.extra_settings_tabs.len()) {
        state.settings_view_state.goto_tab(tab);
        // Tab change invalidates any pending double-click pairing.
        state.last_settings_click = None;
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
            &state.extra_settings_tabs,
        ) {
            use crate::settings_items::{FIELD_DART_DEFINES, FIELD_EXTRA_ARGS, SENTINEL_ADD_NEW};

            // Dispatch LaunchConfigCreate when the add-new sentinel is selected
            if item.id == SENTINEL_ADD_NEW {
                return update(state, Message::LaunchConfigCreate);
            }

            // dart_defines items open the dedicated modal overlay instead of
            // inline edit mode.  Extract config_idx from the item ID which has
            // the format "launch.{idx}.dart_defines".
            if item.id.ends_with(&format!(".{}", FIELD_DART_DEFINES)) {
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
            if item.id.ends_with(&format!(".{}", FIELD_EXTRA_ARGS)) {
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

/// Save the active settings tab and mark the form clean on success.
///
/// This is the single authoritative save path shared by [`handle_settings_save`]
/// and [`handle_settings_save_and_close`]. Both public handlers delegate here so
/// the `match active_tab` block exists in exactly one place.
///
/// Returns `Ok(Some(action))` when the caller must dispatch an [`UpdateAction`]
/// (currently only for [`SettingsTab::Project`] which uses the async
/// [`UpdateAction::PersistSettings`] path).  Returns `Ok(None)` when the save
/// was completed synchronously.  On failure an error is returned and the caller
/// decides whether to close the settings panel.
fn save_active_tab(state: &mut AppState) -> fdemon_core::error::Result<Option<UpdateAction>> {
    use crate::config::save_user_preferences;

    match state.settings_view_state.active_tab {
        SettingsTab::Project => {
            // Persist project settings (config.toml) asynchronously so that
            // file I/O never blocks the TEA event loop.  The caller must
            // dispatch the returned action.
            let action = UpdateAction::PersistSettings {
                settings: Box::new(state.settings.clone()),
                project_path: state.project_path.clone(),
            };
            state.settings_view_state.clear_dirty();
            state.settings_view_state.error = None;
            return Ok(Some(action));
        }
        SettingsTab::UserPrefs => {
            // Save user preferences (settings.local.toml)
            save_user_preferences(&state.project_path, &state.settings_view_state.user_prefs)?;
        }
        SettingsTab::LaunchConfig => {
            // Launch-config edits persist immediately on commit/toggle/cycle via
            // `apply_committed_item`, so there is nothing to flush here. (The old
            // load-then-resave was a no-op round-trip.)
        }
        SettingsTab::VSCodeConfig => {
            // Read-only tab — nothing to save.
        }
        SettingsTab::Extra(i) => {
            // Host-injected tab: delegate persistence to the provider. Errors
            // are surfaced as config errors so the panel shows them inline.
            if let Some(provider) = state.extra_settings_tabs.get(i) {
                provider
                    .save(&state.project_path)
                    .map_err(fdemon_core::error::Error::config)?;
            }
        }
    }

    state.settings_view_state.clear_dirty();
    state.settings_view_state.error = None;
    Ok(None)
}

/// Handle settings save message
pub fn handle_settings_save(state: &mut AppState) -> UpdateResult {
    match save_active_tab(state) {
        Ok(Some(action)) => {
            tracing::info!("Settings save dispatched asynchronously");
            UpdateResult::action(action)
        }
        Ok(None) => {
            tracing::info!("Settings saved successfully");
            UpdateResult::none()
        }
        Err(e) => {
            let error_msg = format!("Save failed: {}", e);
            tracing::error!("{}", error_msg);
            state.settings_view_state.error = Some(error_msg);
            UpdateResult::none()
        }
    }
}

/// Handle settings reset item message
pub fn handle_settings_reset_item(_state: &mut AppState) -> UpdateResult {
    // Reset setting to default - actual logic will be implemented with widget
    UpdateResult::none()
}

/// Apply a committed/toggled setting item to the in-memory model for the active
/// tab, marking the form dirty on success.
///
/// This is the single authoritative apply path shared by the bool-toggle,
/// enum-cycle, and string/number/list commit handlers. Routing per tab:
/// - `Project`: mutate `state.settings` in place (persisted later via the async
///   `PersistSettings` action on save).
/// - `UserPrefs`: mutate the in-memory `user_prefs` (persisted on save).
/// - `LaunchConfig`: load configs, apply to the addressed config by index
///   parsed from the `launch.{idx}.field` id, and save IMMEDIATELY to disk
///   (launch edits have no separate save step). `dirty` is marked only when the
///   write succeeds.
/// - `VSCodeConfig`: read-only, no-op.
fn apply_committed_item(state: &mut AppState, item: &SettingItem) {
    match state.settings_view_state.active_tab {
        SettingsTab::Project => {
            super::settings::apply_project_setting(&mut state.settings, item);
            state.settings_view_state.mark_dirty();
        }
        SettingsTab::UserPrefs => {
            super::settings::apply_user_preference(&mut state.settings_view_state.user_prefs, item);
            state.settings_view_state.mark_dirty();
        }
        SettingsTab::LaunchConfig => {
            // For launch configs, we load, modify, and save immediately.
            // Extract config index from item ID (format: "launch.{idx}.field").
            let parts: Vec<&str> = item.id.split('.').collect();
            if parts.len() >= 3 && parts[0] == "launch" {
                if let Ok(config_idx) = parts[1].parse::<usize>() {
                    use crate::config::launch::{load_launch_configs, save_launch_configs};
                    let mut configs = load_launch_configs(&state.project_path);
                    if let Some(resolved) = configs.get_mut(config_idx) {
                        super::settings::apply_launch_config_change(&mut resolved.config, item);
                        // Save the modified configs back to disk.
                        let config_vec: Vec<_> = configs.iter().map(|r| r.config.clone()).collect();
                        if let Err(e) = save_launch_configs(&state.project_path, &config_vec) {
                            tracing::error!("Failed to save launch configs: {}", e);
                        } else {
                            state.settings_view_state.mark_dirty();
                        }
                    }
                }
            }
        }
        SettingsTab::VSCodeConfig => {
            // Read-only tab — ignore.
        }
        SettingsTab::Extra(i) => {
            // Host-injected tab: route the committed item to the provider's
            // in-memory model. Persistence happens on save via the provider.
            if let Some(provider) = state.extra_settings_tabs.get_mut(i) {
                provider.apply(item);
                state.settings_view_state.mark_dirty();
            }
        }
    }
}

/// Handle settings toggle bool message
pub fn handle_settings_toggle_bool(state: &mut AppState) -> UpdateResult {
    if let Some(item) = get_selected_item(
        &state.settings,
        &state.project_path,
        &state.settings_view_state,
        &state.extra_settings_tabs,
    ) {
        // Only toggle if it's a boolean value
        if let SettingValue::Bool(val) = &item.value {
            // Create new item with flipped value
            let mut toggled_item = item.clone();
            toggled_item.value = SettingValue::Bool(!val);
            apply_committed_item(state, &toggled_item);
        }
    }
    UpdateResult::none()
}

/// Cycle the selected enum setting by `delta` (+1 = next option, -1 = previous,
/// wrapping). No-op when the selected item is not an `Enum`.
fn cycle_selected_enum(state: &mut AppState, delta: i64) -> UpdateResult {
    if let Some(item) = get_selected_item(
        &state.settings,
        &state.project_path,
        &state.settings_view_state,
        &state.extra_settings_tabs,
    ) {
        if let SettingValue::Enum { value, options } = &item.value {
            if options.is_empty() {
                return UpdateResult::none();
            }
            let current = options.iter().position(|o| o == value).unwrap_or(0);
            let len = options.len() as i64;
            let next = (((current as i64 + delta) % len + len) % len) as usize;

            let mut new_item = item.clone();
            new_item.value = SettingValue::Enum {
                value: options[next].clone(),
                options: options.clone(),
            };
            apply_committed_item(state, &new_item);
        }
    }
    UpdateResult::none()
}

/// Handle settings cycle enum next message
pub fn handle_settings_cycle_enum_next(state: &mut AppState) -> UpdateResult {
    cycle_selected_enum(state, 1)
}

/// Handle settings cycle enum previous message
pub fn handle_settings_cycle_enum_prev(state: &mut AppState) -> UpdateResult {
    cycle_selected_enum(state, -1)
}

/// Handle settings increment message
pub fn handle_settings_increment(_state: &mut AppState, _delta: i64) -> UpdateResult {
    // stub — no-op until field-by-field increment logic is implemented.
    // Marking dirty without changing a value would mislead the user into seeing
    // the unsaved-changes confirmation dialog for an increment that changed nothing.
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
///
/// Parses the edit buffer according to the selected item's value type, applies
/// the parsed value to the in-memory model via [`apply_committed_item`], then
/// marks the form dirty and exits edit mode. On a parse failure (Number/Float)
/// the error is surfaced via `settings_view_state.error` and the editor stays
/// open so the user can correct the input — no change is applied, no dirty flag
/// is set.
pub fn handle_settings_commit_edit(state: &mut AppState) -> UpdateResult {
    if !state.settings_view_state.editing {
        return UpdateResult::none();
    }

    let buffer = state.settings_view_state.edit_buffer.clone();

    let item = match get_selected_item(
        &state.settings,
        &state.project_path,
        &state.settings_view_state,
        &state.extra_settings_tabs,
    ) {
        Some(item) => item,
        None => {
            // Nothing to commit to — just exit edit mode.
            state.settings_view_state.stop_editing();
            return UpdateResult::none();
        }
    };

    // Parse the buffer into a new value matching the item's current variant.
    let parsed = match &item.value {
        SettingValue::String(_) => SettingValue::String(buffer.clone()),
        SettingValue::Number(_) => match buffer.trim().parse::<i64>() {
            Ok(n) => SettingValue::Number(n),
            Err(_) => {
                state.settings_view_state.error =
                    Some(format!("Invalid number: '{}'", buffer.trim()));
                return UpdateResult::none();
            }
        },
        SettingValue::Float(_) => match buffer.trim().parse::<f64>() {
            Ok(f) => SettingValue::Float(f),
            Err(_) => {
                state.settings_view_state.error =
                    Some(format!("Invalid number: '{}'", buffer.trim()));
                return UpdateResult::none();
            }
        },
        SettingValue::List(existing) => {
            // Append the typed entry to the existing list (skip empty input).
            let mut list = existing.clone();
            let trimmed = buffer.trim();
            if !trimmed.is_empty() {
                list.push(trimmed.to_string());
            }
            SettingValue::List(list)
        }
        // Bool/Enum are never edited inline (toggled/cycled instead).
        SettingValue::Bool(_) | SettingValue::Enum { .. } => {
            state.settings_view_state.stop_editing();
            return UpdateResult::none();
        }
    };

    let mut committed = item.clone();
    committed.value = parsed;
    apply_committed_item(state, &committed);

    state.settings_view_state.error = None;
    state.settings_view_state.stop_editing();
    UpdateResult::none()
}

/// Handle settings cancel edit message
pub fn handle_settings_cancel_edit(state: &mut AppState) -> UpdateResult {
    // Cancel the current edit
    state.settings_view_state.stop_editing();
    UpdateResult::none()
}

/// Handle settings remove list item message
///
/// Removes the last entry from the selected `List` setting and applies the
/// shortened list via [`apply_committed_item`]. No-op when the selected item is
/// not a non-empty `List`.
pub fn handle_settings_remove_list_item(state: &mut AppState) -> UpdateResult {
    if let Some(item) = get_selected_item(
        &state.settings,
        &state.project_path,
        &state.settings_view_state,
        &state.extra_settings_tabs,
    ) {
        if let SettingValue::List(existing) = &item.value {
            if existing.is_empty() {
                return UpdateResult::none();
            }
            let mut list = existing.clone();
            list.pop();
            let mut new_item = item.clone();
            new_item.value = SettingValue::List(list);
            apply_committed_item(state, &new_item);
        }
    }
    UpdateResult::none()
}

/// Handle settings save and close message
pub fn handle_settings_save_and_close(state: &mut AppState) -> UpdateResult {
    // Delegate to the shared save helper then close on success.
    match save_active_tab(state) {
        Ok(Some(action)) => {
            // For async saves (Project tab), close optimistically — the write
            // happens in the background.  If it fails, a `tracing::warn!` is
            // emitted by the action dispatch arm; surfacing it to the UI is a
            // Phase-2-or-later concern.
            dismiss_unsaved_dialog(state);
            state.hide_settings();
            tracing::info!("Settings save dispatched asynchronously, settings panel closed");
            UpdateResult::action(action)
        }
        Ok(None) => {
            dismiss_unsaved_dialog(state);
            state.hide_settings();
            tracing::info!("Settings saved and closed");
            UpdateResult::none()
        }
        Err(e) => {
            let error_msg = format!("Save failed: {}", e);
            tracing::error!("{}", error_msg);
            state.settings_view_state.error = Some(error_msg);
            // Don't close on error — return to the settings panel so the error
            // is visible (the dialog had replaced the foreground).
            dismiss_unsaved_dialog(state);
            state.ui_mode = crate::state::UiMode::Settings;
            UpdateResult::none()
        }
    }
}

/// Handle force hide settings message
pub fn handle_force_hide_settings(state: &mut AppState) -> UpdateResult {
    // Force close without saving (discard changes)
    state.settings_view_state.clear_dirty();
    dismiss_unsaved_dialog(state);
    state.hide_settings();
    // Clear the stamp for hygiene — mirrors last_log_click reset on session change.
    state.last_settings_click = None;
    UpdateResult::none()
}

/// Handle the "Cancel" choice on the unsaved-changes dialog: dismiss the prompt
/// and return to the settings panel with edits intact.
pub fn handle_settings_cancel_close(state: &mut AppState) -> UpdateResult {
    dismiss_unsaved_dialog(state);
    state.ui_mode = crate::state::UiMode::Settings;
    UpdateResult::none()
}

/// Clear the confirmation-dialog state and its settings backdrop. Shared by the
/// three exits of the unsaved-changes prompt (save / discard / cancel).
fn dismiss_unsaved_dialog(state: &mut AppState) {
    state.confirm_dialog_state = None;
    state.confirm_dialog_backdrop = None;
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
                item_count + crate::settings_items::ADD_NEW_BUTTON_COUNT
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
        SettingsTab::Extra(i) => state
            .extra_settings_tabs
            .get(i)
            .map(|p| p.items().len())
            .unwrap_or(0),
    }
}

/// Handle a single click on a settings panel row.
///
/// Sets `settings_view_state.selected_index = index` so the row appears
/// selected. If the same row was clicked within
/// [`DOUBLE_CLICK_WINDOW_MS`] ms, emits a follow-up [`Message::SettingsToggleEdit`]
/// via [`UpdateResult::message`] to enter edit mode (mirroring the Phase 4
/// log-view double-click pattern).
///
/// Single click never enters edit mode. Settings panel UX requires two clicks
/// to start editing.
///
/// # Edge cases
/// - If `index` is out of range for the active tab's item list, the
///   `selected_index` is clamped to the last valid item (or 0 if the tab is
///   empty). The widget renderer only registers regions for visible rows, so an
///   out-of-range index from a click is unlikely; we clamp defensively.
/// - If `editing == true` (a previous click already entered edit mode), the
///   click is ignored — keyboard `Esc` must close the editor first. This
///   mirrors `handle_settings_next_item` / `handle_settings_prev_item` which
///   also no-op while editing.
pub fn handle_settings_click_row(state: &mut AppState, index: usize) -> UpdateResult {
    // Don't move selection while editing — user must close the editor first.
    if state.settings_view_state.editing {
        return UpdateResult::none();
    }

    let item_count = get_item_count_for_tab(state);
    let clamped = if item_count == 0 {
        0
    } else {
        index.min(item_count - 1)
    };

    // Read the previous click stamp (Copy, so no `take` needed).
    let prev = state.last_settings_click;
    let now = std::time::Instant::now();

    // Update selection.
    state.settings_view_state.selected_index = clamped;

    // Double-click detection: same row, within window.
    let is_double_click = match prev {
        Some(stamp) if stamp.index == clamped => {
            let elapsed_ms = now.saturating_duration_since(stamp.at).as_millis();
            elapsed_ms <= u128::from(DOUBLE_CLICK_WINDOW_MS)
        }
        _ => false,
    };

    if is_double_click {
        // Consume the stamp so a third click within the window doesn't re-fire.
        state.last_settings_click = None;
        // Emit the toggle-edit follow-up.
        UpdateResult::message(Message::SettingsToggleEdit)
    } else {
        // Record this click for potential future double-click pairing.
        state.last_settings_click = Some(SettingsClickStamp {
            index: clamped,
            at: now,
        });
        UpdateResult::none()
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

    #[test]
    fn test_dirty_esc_prompts_before_dismissing_settings() {
        use crate::state::UiMode;

        let mut state = state_with_tab(SettingsTab::Project);
        state.ui_mode = UiMode::Settings;
        state.settings_view_state.dirty = true;

        handle_hide_settings(&mut state);

        // The dialog is raised and the settings panel stays as the backdrop —
        // it is NOT dismissed until the user chooses an action.
        assert_eq!(state.ui_mode, UiMode::ConfirmDialog);
        assert!(state.confirm_dialog_state.is_some());
        assert_eq!(state.confirm_dialog_backdrop, Some(UiMode::Settings));
    }

    #[test]
    fn test_cancel_close_returns_to_settings_with_edits_intact() {
        use crate::state::UiMode;

        let mut state = state_with_tab(SettingsTab::Project);
        state.settings_view_state.dirty = true;
        handle_hide_settings(&mut state); // raise the unsaved-changes prompt

        handle_settings_cancel_close(&mut state);

        // Back in the settings panel; dialog + backdrop cleared; edits preserved.
        assert_eq!(state.ui_mode, UiMode::Settings);
        assert!(state.confirm_dialog_state.is_none());
        assert_eq!(state.confirm_dialog_backdrop, None);
        assert!(state.settings_view_state.dirty);
    }

    #[test]
    fn test_discard_clears_dialog_and_closes_settings() {
        use crate::state::UiMode;

        let mut state = state_with_tab(SettingsTab::Project);
        state.settings_view_state.dirty = true;
        handle_hide_settings(&mut state);

        handle_force_hide_settings(&mut state);

        assert_eq!(state.ui_mode, UiMode::Normal);
        assert!(state.confirm_dialog_state.is_none());
        assert_eq!(state.confirm_dialog_backdrop, None);
        assert!(!state.settings_view_state.dirty);
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
            &state.extra_settings_tabs,
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
            &state.extra_settings_tabs,
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
            &state.extra_settings_tabs,
        );
        // Either None or not the add-new sentinel
        if let Some(item) = selected {
            assert_ne!(
                item.id, "launch.__add_new__",
                "sentinel must not appear when there are no configs"
            );
        }
    }

    // ─────────────────────────────────────────────────────────────────────────
    // handle_settings_click_row — double-click detection tests (Phase 5 Task 03)
    // ─────────────────────────────────────────────────────────────────────────

    fn fresh_state() -> AppState {
        let mut s = AppState::new();
        s.show_settings();
        s
    }

    #[test]
    fn test_handle_settings_click_row_single_click_sets_index_no_follow_up() {
        let mut s = fresh_state();
        let result = handle_settings_click_row(&mut s, 3);
        assert_eq!(s.settings_view_state.selected_index, 3);
        assert!(result.message.is_none());
        assert!(s.last_settings_click.is_some());
    }

    #[test]
    fn test_handle_settings_click_row_double_click_same_row_emits_toggle_edit() {
        let mut s = fresh_state();
        let _ = handle_settings_click_row(&mut s, 2);
        let result = handle_settings_click_row(&mut s, 2);
        assert!(
            matches!(result.message, Some(Message::SettingsToggleEdit)),
            "expected SettingsToggleEdit, got {:?}",
            result.message
        );
        // Stamp consumed.
        assert!(s.last_settings_click.is_none());
    }

    #[test]
    fn test_handle_settings_click_row_second_click_different_row_no_toggle() {
        let mut s = fresh_state();
        let _ = handle_settings_click_row(&mut s, 2);
        let result = handle_settings_click_row(&mut s, 5);
        assert!(result.message.is_none());
        assert_eq!(s.settings_view_state.selected_index, 5);
    }

    #[test]
    fn test_handle_settings_click_row_second_click_after_window_no_toggle() {
        use crate::state::SettingsClickStamp;
        use std::time::{Duration, Instant};

        let mut s = fresh_state();
        // Manually set a stale stamp (older than 400 ms).
        s.last_settings_click = Some(SettingsClickStamp {
            index: 2,
            at: Instant::now() - Duration::from_millis(500),
        });
        let result = handle_settings_click_row(&mut s, 2);
        assert!(result.message.is_none());
    }

    #[test]
    fn test_handle_settings_click_row_while_editing_is_no_op() {
        let mut s = fresh_state();
        s.settings_view_state.selected_index = 1;
        s.settings_view_state.editing = true;
        let snapshot_before = s.settings_view_state.selected_index;
        let result = handle_settings_click_row(&mut s, 7);
        assert!(result.message.is_none());
        assert_eq!(s.settings_view_state.selected_index, snapshot_before);
    }

    #[test]
    fn test_handle_settings_goto_tab_clears_click_stamp() {
        let mut s = fresh_state();
        let _ = handle_settings_click_row(&mut s, 3);
        assert!(s.last_settings_click.is_some());
        let _ = handle_settings_goto_tab(&mut s, 1);
        assert!(s.last_settings_click.is_none());
    }

    #[test]
    fn test_handle_settings_next_tab_clears_click_stamp() {
        let mut s = fresh_state();
        let _ = handle_settings_click_row(&mut s, 0);
        assert!(s.last_settings_click.is_some());
        let _ = handle_settings_next_tab(&mut s);
        assert!(s.last_settings_click.is_none());
    }

    #[test]
    fn test_handle_settings_prev_tab_clears_click_stamp() {
        let mut s = fresh_state();
        let _ = handle_settings_click_row(&mut s, 0);
        assert!(s.last_settings_click.is_some());
        let _ = handle_settings_prev_tab(&mut s);
        assert!(s.last_settings_click.is_none());
    }

    #[test]
    fn test_handle_settings_click_row_third_click_in_window_no_retrigger() {
        let mut s = fresh_state();
        let _ = handle_settings_click_row(&mut s, 2);
        let r2 = handle_settings_click_row(&mut s, 2);
        assert!(
            matches!(r2.message, Some(Message::SettingsToggleEdit)),
            "second click should emit SettingsToggleEdit"
        );
        // Third click within the same window should NOT re-fire toggle.
        let r3 = handle_settings_click_row(&mut s, 2);
        assert!(r3.message.is_none(), "third click must not re-toggle");
    }

    #[test]
    fn test_handle_settings_click_row_out_of_range_index_clamps_to_last() {
        let mut s = fresh_state();
        let count = get_item_count_for_tab(&s);
        // Only meaningful when there are items to clamp to.
        if count > 0 {
            let too_far = count + 100;
            let _ = handle_settings_click_row(&mut s, too_far);
            assert_eq!(
                s.settings_view_state.selected_index,
                count.saturating_sub(1)
            );
        }
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Commit / cycle round-trip tests (S1): an edit must reach state.settings
    // ─────────────────────────────────────────────────────────────────────────

    /// Helper: position the Project tab on the item with the given id.
    fn select_project_item(state: &mut AppState, id: &str) {
        let items = crate::settings_items::project_settings_items(&state.settings);
        let idx = items
            .iter()
            .position(|i| i.id == id)
            .unwrap_or_else(|| panic!("project item {id} not found"));
        state.settings_view_state.selected_index = idx;
    }

    #[test]
    fn test_commit_number_round_trips_to_settings() {
        let mut s = state_with_tab(SettingsTab::Project);
        s.show_settings();
        s.settings_view_state.active_tab = SettingsTab::Project;
        select_project_item(&mut s, "watcher.debounce_ms");

        s.settings_view_state.start_editing("1234");
        handle_settings_commit_edit(&mut s);

        assert_eq!(s.settings.watcher.debounce_ms, 1234);
        assert!(s.settings_view_state.dirty, "commit must mark dirty");
        assert!(!s.settings_view_state.editing, "commit must exit edit mode");
        assert!(s.settings_view_state.error.is_none());
    }

    #[test]
    fn test_commit_invalid_number_sets_error_and_keeps_editing() {
        let mut s = state_with_tab(SettingsTab::Project);
        s.settings_view_state.active_tab = SettingsTab::Project;
        select_project_item(&mut s, "watcher.debounce_ms");
        let original = s.settings.watcher.debounce_ms;

        s.settings_view_state.start_editing("not-a-number");
        handle_settings_commit_edit(&mut s);

        assert_eq!(
            s.settings.watcher.debounce_ms, original,
            "invalid number must not change the value"
        );
        assert!(
            s.settings_view_state.error.is_some(),
            "invalid number must surface an error"
        );
        assert!(
            s.settings_view_state.editing,
            "editor must stay open on parse failure"
        );
        assert!(
            !s.settings_view_state.dirty,
            "parse failure must not mark dirty"
        );
    }

    #[test]
    fn test_commit_string_round_trips_to_settings() {
        let mut s = state_with_tab(SettingsTab::Project);
        s.settings_view_state.active_tab = SettingsTab::Project;
        select_project_item(&mut s, "devtools.browser");

        s.settings_view_state.start_editing("firefox");
        handle_settings_commit_edit(&mut s);

        assert_eq!(s.settings.devtools.browser, "firefox");
        assert!(s.settings_view_state.dirty);
    }

    #[test]
    fn test_cycle_enum_round_trips_to_settings() {
        let mut s = state_with_tab(SettingsTab::Project);
        s.settings_view_state.active_tab = SettingsTab::Project;
        select_project_item(&mut s, "ui.theme");

        // ui.theme options: ["default", "dark", "light"]; default value "default".
        assert_eq!(s.settings.ui.theme, "default");
        handle_settings_cycle_enum_next(&mut s);
        assert_eq!(s.settings.ui.theme, "dark", "next must advance to 'dark'");
        assert!(s.settings_view_state.dirty);

        handle_settings_cycle_enum_prev(&mut s);
        assert_eq!(s.settings.ui.theme, "default", "prev must wrap back");
    }

    #[test]
    fn test_cycle_enum_wraps_around() {
        let mut s = state_with_tab(SettingsTab::Project);
        s.settings_view_state.active_tab = SettingsTab::Project;
        select_project_item(&mut s, "ui.theme");

        // From the first option, prev wraps to the last ("light").
        handle_settings_cycle_enum_prev(&mut s);
        assert_eq!(s.settings.ui.theme, "light");
    }

    /// New devtools apply arm coverage via the commit path (enum field).
    #[test]
    fn test_cycle_enum_devtools_default_panel_round_trips() {
        let mut s = state_with_tab(SettingsTab::Project);
        s.settings_view_state.active_tab = SettingsTab::Project;
        select_project_item(&mut s, "devtools.default_panel");

        assert_eq!(s.settings.devtools.default_panel, "inspector");
        handle_settings_cycle_enum_next(&mut s);
        assert_eq!(s.settings.devtools.default_panel, "performance");
    }

    #[test]
    fn test_toggle_bool_devtools_logging_round_trips() {
        let mut s = state_with_tab(SettingsTab::Project);
        s.settings_view_state.active_tab = SettingsTab::Project;
        select_project_item(&mut s, "devtools.logging.hybrid_enabled");

        let before = s.settings.devtools.logging.hybrid_enabled;
        handle_settings_toggle_bool(&mut s);
        assert_eq!(
            s.settings.devtools.logging.hybrid_enabled, !before,
            "toggling a devtools logging bool must flip it"
        );
        assert!(s.settings_view_state.dirty);
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Extra (host-injected) settings-tab seam
    // ─────────────────────────────────────────────────────────────────────────

    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    /// A minimal in-test [`SettingsTabProvider`] that exposes two items and
    /// records how many times `apply`/`save` were invoked.
    #[derive(Debug)]
    struct FakeProvider {
        toggle: bool,
        applied: Arc<AtomicUsize>,
        saved: Arc<AtomicUsize>,
    }

    impl FakeProvider {
        fn new() -> Self {
            Self {
                toggle: false,
                applied: Arc::new(AtomicUsize::new(0)),
                saved: Arc::new(AtomicUsize::new(0)),
            }
        }
    }

    impl crate::settings_tab_provider::SettingsTabProvider for FakeProvider {
        fn title(&self) -> &str {
            "Fake"
        }

        fn items(&self) -> Vec<SettingItem> {
            vec![
                SettingItem::new("fake.toggle", "Toggle")
                    .value(SettingValue::Bool(self.toggle))
                    .section("Fake".to_string()),
                SettingItem::new("fake.note", "Note")
                    .value(SettingValue::String("hi".to_string()))
                    .section("Fake".to_string()),
            ]
        }

        fn apply(&mut self, item: &SettingItem) {
            self.applied.fetch_add(1, Ordering::SeqCst);
            if item.id == "fake.toggle" {
                if let SettingValue::Bool(b) = item.value {
                    self.toggle = b;
                }
            }
        }

        fn save(&self, _project_path: &std::path::Path) -> Result<(), String> {
            self.saved.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    fn state_with_one_extra_tab() -> (AppState, Arc<AtomicUsize>, Arc<AtomicUsize>) {
        let mut state = AppState::new();
        let provider = FakeProvider::new();
        let applied = provider.applied.clone();
        let saved = provider.saved.clone();
        state.extra_settings_tabs.push(Box::new(provider));
        state.settings_view_state.active_tab = SettingsTab::Extra(0);
        (state, applied, saved)
    }

    #[test]
    fn test_extra_tab_item_count() {
        let (state, _, _) = state_with_one_extra_tab();
        assert_eq!(get_item_count_for_tab(&state), 2);
    }

    #[test]
    fn test_extra_tab_next_prev_navigation() {
        let mut state = AppState::new();
        state
            .extra_settings_tabs
            .push(Box::new(FakeProvider::new()));
        // VSCode -> Extra(0) -> wrap to Project.
        state.settings_view_state.active_tab = SettingsTab::VSCodeConfig;
        handle_settings_next_tab(&mut state);
        assert_eq!(state.settings_view_state.active_tab, SettingsTab::Extra(0));
        handle_settings_next_tab(&mut state);
        assert_eq!(state.settings_view_state.active_tab, SettingsTab::Project);
    }

    #[test]
    fn test_extra_tab_goto_tab() {
        let mut state = AppState::new();
        state
            .extra_settings_tabs
            .push(Box::new(FakeProvider::new()));
        // Index 4 is the first extra tab.
        handle_settings_goto_tab(&mut state, 4);
        assert_eq!(state.settings_view_state.active_tab, SettingsTab::Extra(0));
        // Index 5 is out of range (only one extra tab) — no change.
        handle_settings_goto_tab(&mut state, 5);
        assert_eq!(state.settings_view_state.active_tab, SettingsTab::Extra(0));
    }

    #[test]
    fn test_extra_tab_toggle_bool_routes_to_provider() {
        let (mut state, applied, _) = state_with_one_extra_tab();
        // Select the bool item (index 0) and toggle it.
        state.settings_view_state.selected_index = 0;
        handle_settings_toggle_bool(&mut state);
        assert_eq!(applied.load(Ordering::SeqCst), 1, "apply must be called");
        // The provider's model flipped, so its rebuilt items reflect the change.
        let item = get_selected_item(
            &state.settings,
            &state.project_path,
            &state.settings_view_state,
            &state.extra_settings_tabs,
        )
        .unwrap();
        assert_eq!(item.value, SettingValue::Bool(true));
        assert!(state.settings_view_state.dirty);
    }

    #[test]
    fn test_extra_tab_save_routes_to_provider() {
        let (mut state, _, saved) = state_with_one_extra_tab();
        let result = save_active_tab(&mut state);
        assert!(result.is_ok());
        assert_eq!(saved.load(Ordering::SeqCst), 1, "save must be called");
    }
}
