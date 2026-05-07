//! Click handlers for the NewSessionDialog (Phase 5).
//!
//! These functions are dispatched from `handler/update.rs` when a click
//! produces an absolute-index `Message` (see `Message::NewSessionDialog*At`,
//! `Message::NewSessionDialogFocusField`). They mutate state and emit
//! follow-up messages to chain into the existing relative-navigation flow.

use crate::handler::UpdateResult;
use crate::message::Message;
use crate::new_session_dialog::{DeviceListItem, LaunchContextField};
use crate::state::AppState;

/// Set the selected device on the active tab and emit a follow-up
/// [`Message::NewSessionDialogDeviceSelect`] to confirm.
///
/// `index` is the absolute position into the *currently active* tab's flat list
/// (which includes group headers). The handler clamps `index` to the valid range
/// for that list. Emitting `NewSessionDialogDeviceSelect` chains into the existing
/// keyboard flow (select the device, probe connection, etc.).
pub fn handle_select_device_at(state: &mut AppState, index: usize) -> UpdateResult {
    let target = &mut state.new_session_dialog_state.target_selector;

    // Compute the length of the current tab's flat list (headers + devices).
    let list_len = target.flat_list().len();
    if list_len == 0 {
        return UpdateResult::none();
    }
    let clamped = index.min(list_len - 1);

    // Guard: only select if the clamped index is a device row, not a header.
    // The renderer already prevents header-row regions from being registered, but
    // we defend in depth here so the function is correct in isolation.
    let is_device = matches!(
        target.flat_list().get(clamped),
        Some(DeviceListItem::Device(_))
    );
    if !is_device {
        return UpdateResult::none();
    }

    target.selected_index = clamped;
    UpdateResult::message(Message::NewSessionDialogDeviceSelect)
}

/// Set the focused field in the LaunchContext pane and emit a follow-up
/// [`Message::NewSessionDialogFieldActivate`] for fields that activate on Enter.
///
/// Clicking a field is equivalent to "navigate to field then press Enter":
/// `NewSessionDialogFieldActivate` chains into the existing keyboard path so
/// Config, Flavor, and EntryPoint open their fuzzy pickers, and DartDefines
/// opens the dart-defines modal. Mode silently no-ops `FieldActivate` (it uses
/// Left/Right cycling instead of Enter).
pub fn handle_focus_field(state: &mut AppState, field: LaunchContextField) -> UpdateResult {
    use crate::new_session_dialog::DialogPane;
    state.new_session_dialog_state.focused_pane = DialogPane::LaunchContext;
    state.new_session_dialog_state.launch_context.focused_field = field;
    UpdateResult::message(Message::NewSessionDialogFieldActivate)
}

/// Set the selected match in the fuzzy modal and emit a follow-up
/// [`Message::NewSessionDialogFuzzyConfirm`].
///
/// `index` is the absolute position into `fuzzy_modal.filtered_indices`.
/// If the fuzzy modal is closed or has no matches, this is a no-op.
pub fn handle_fuzzy_select_at(state: &mut AppState, index: usize) -> UpdateResult {
    let Some(modal) = state.new_session_dialog_state.fuzzy_modal.as_mut() else {
        return UpdateResult::none();
    };
    if modal.filtered_indices.is_empty() {
        return UpdateResult::none();
    }
    let clamped = index.min(modal.filtered_indices.len() - 1);
    modal.selected_index = clamped;
    UpdateResult::message(Message::NewSessionDialogFuzzyConfirm)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::new_session_dialog::{
        DialogPane, FuzzyModalState, FuzzyModalType, LaunchContextField, TargetTab,
    };
    use crate::state::AppState;
    use fdemon_daemon::Device;

    fn test_device(id: &str) -> Device {
        Device {
            id: id.to_string(),
            name: id.to_string(),
            platform: "ios".to_string(),
            emulator: false,
            category: None,
            platform_type: None,
            ephemeral: false,
            emulator_id: None,
        }
    }

    #[test]
    fn handle_select_device_at_sets_index_and_emits_select() {
        let mut state = AppState::new();
        state.new_session_dialog_state.target_selector.active_tab = TargetTab::Connected;
        state
            .new_session_dialog_state
            .target_selector
            .set_connected_devices(vec![test_device("a"), test_device("b"), test_device("c")]);

        // The flat list includes a header row at index 0 followed by device rows.
        // Clicking on flat-list index 2 corresponds to the second device.
        let result = handle_select_device_at(&mut state, 2);
        assert_eq!(
            state
                .new_session_dialog_state
                .target_selector
                .selected_index,
            2
        );
        assert!(matches!(
            result.message,
            Some(Message::NewSessionDialogDeviceSelect)
        ));
    }

    #[test]
    fn handle_select_device_at_empty_list_is_noop() {
        let mut state = AppState::new();
        state.new_session_dialog_state.target_selector.active_tab = TargetTab::Connected;
        state
            .new_session_dialog_state
            .target_selector
            .set_connected_devices(vec![]);

        let result = handle_select_device_at(&mut state, 0);
        assert!(result.message.is_none());
    }

    #[test]
    fn handle_select_device_at_clamps_to_last_item() {
        let mut state = AppState::new();
        state.new_session_dialog_state.target_selector.active_tab = TargetTab::Connected;
        state
            .new_session_dialog_state
            .target_selector
            .set_connected_devices(vec![test_device("a"), test_device("b")]);

        // Pass a large index — should be clamped to last flat-list position.
        // With 2 connected devices the flat list is [Header, Device, Device],
        // so the last index (flat_len - 1 = 2) is a Device row and DeviceSelect fires.
        let flat_len = state
            .new_session_dialog_state
            .target_selector
            .flat_list()
            .len();
        let result = handle_select_device_at(&mut state, 999);
        assert_eq!(
            state
                .new_session_dialog_state
                .target_selector
                .selected_index,
            flat_len - 1
        );
        assert!(matches!(
            result.message,
            Some(Message::NewSessionDialogDeviceSelect)
        ));
    }

    #[test]
    fn handle_select_device_at_with_header_index_is_noop() {
        // With 1 connected device the flat list is [Header, Device].
        // Index 0 is a header row. Clicking it must not update selection
        // and must not emit NewSessionDialogDeviceSelect.
        let mut state = AppState::new();
        state.new_session_dialog_state.target_selector.active_tab = TargetTab::Connected;
        state
            .new_session_dialog_state
            .target_selector
            .set_connected_devices(vec![test_device("a")]);

        // Place the cursor on the device (index 1) first.
        state
            .new_session_dialog_state
            .target_selector
            .selected_index = 1;
        let initial_index = state
            .new_session_dialog_state
            .target_selector
            .selected_index;

        // Click on index 0, which is the header row.
        let result = handle_select_device_at(&mut state, 0);

        // Selection must be unchanged and no DeviceSelect emitted.
        assert_eq!(
            state
                .new_session_dialog_state
                .target_selector
                .selected_index,
            initial_index
        );
        assert!(result.message.is_none());
    }

    #[test]
    fn handle_focus_field_sets_focused_pane_and_field_and_emits_activate() {
        let mut state = AppState::new();
        let result = handle_focus_field(&mut state, LaunchContextField::Mode);
        assert_eq!(
            state.new_session_dialog_state.focused_pane,
            DialogPane::LaunchContext
        );
        assert_eq!(
            state.new_session_dialog_state.launch_context.focused_field,
            LaunchContextField::Mode
        );
        assert!(matches!(
            result.message,
            Some(Message::NewSessionDialogFieldActivate)
        ));
    }

    #[test]
    fn handle_focus_field_sets_all_field_variants() {
        for field in [
            LaunchContextField::Config,
            LaunchContextField::Mode,
            LaunchContextField::Flavor,
            LaunchContextField::EntryPoint,
            LaunchContextField::DartDefines,
            LaunchContextField::Launch,
        ] {
            let mut state = AppState::new();
            let result = handle_focus_field(&mut state, field);
            assert_eq!(
                state.new_session_dialog_state.launch_context.focused_field, field,
                "field {:?} not set",
                field
            );
            assert!(matches!(
                result.message,
                Some(Message::NewSessionDialogFieldActivate)
            ));
        }
    }

    #[test]
    fn handle_fuzzy_select_at_sets_index_and_emits_confirm() {
        let mut state = AppState::new();
        state.new_session_dialog_state.fuzzy_modal = Some(FuzzyModalState::new(
            FuzzyModalType::Flavor,
            vec!["dev".into(), "prod".into()],
        ));

        let result = handle_fuzzy_select_at(&mut state, 1);
        let modal = state.new_session_dialog_state.fuzzy_modal.as_ref().unwrap();
        assert_eq!(modal.selected_index, 1);
        assert!(matches!(
            result.message,
            Some(Message::NewSessionDialogFuzzyConfirm)
        ));
    }

    #[test]
    fn handle_fuzzy_select_at_clamps_to_last() {
        let mut state = AppState::new();
        state.new_session_dialog_state.fuzzy_modal = Some(FuzzyModalState::new(
            FuzzyModalType::Flavor,
            vec!["dev".into(), "prod".into()],
        ));

        let result = handle_fuzzy_select_at(&mut state, 999);
        let modal = state.new_session_dialog_state.fuzzy_modal.as_ref().unwrap();
        // filtered_indices has 2 items (both match empty query), last index = 1
        assert_eq!(modal.selected_index, 1);
        assert!(matches!(
            result.message,
            Some(Message::NewSessionDialogFuzzyConfirm)
        ));
    }

    #[test]
    fn handle_fuzzy_select_at_no_modal_is_noop() {
        let mut state = AppState::new();
        // fuzzy_modal is None
        let result = handle_fuzzy_select_at(&mut state, 0);
        assert!(result.message.is_none());
    }

    #[test]
    fn handle_fuzzy_select_at_empty_matches_is_noop() {
        let mut state = AppState::new();
        let mut modal = FuzzyModalState::new(FuzzyModalType::Flavor, vec!["dev".into()]);
        // Force empty filtered_indices
        modal.filtered_indices.clear();
        state.new_session_dialog_state.fuzzy_modal = Some(modal);

        let result = handle_fuzzy_select_at(&mut state, 0);
        assert!(result.message.is_none());
    }
}
