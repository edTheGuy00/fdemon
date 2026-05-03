//! Scroll routing for `UiMode::Startup` and `UiMode::NewSessionDialog`.
//!
//! Both modes share a single keyboard handler in `handler::keys` and share
//! this mouse handler too. Modal precedence mirrors `handle_key_new_session_dialog`:
//! fuzzy modal → dart-defines modal → main dialog (dispatched by focused pane).

use crate::input_mouse::{KeyModSet, ScrollDir};
use crate::message::Message;
use crate::new_session_dialog::DialogPane;
use crate::state::AppState;

pub(super) fn handle_scroll(state: &AppState, dir: ScrollDir, _mods: KeyModSet) -> Option<Message> {
    let dialog = &state.new_session_dialog_state;

    // Modal precedence — matches keys.rs:799-804.
    if dialog.is_fuzzy_modal_open() {
        return match dir {
            ScrollDir::Up => Some(Message::NewSessionDialogFuzzyUp),
            ScrollDir::Down => Some(Message::NewSessionDialogFuzzyDown),
            _ => None,
        };
    }

    if dialog.is_dart_defines_modal_open() {
        // The dart-defines modal handler at keys.rs:851-855 routes Up/Down
        // unconditionally (regardless of List vs Edit pane) — so do we.
        return match dir {
            ScrollDir::Up => Some(Message::NewSessionDialogDartDefinesUp),
            ScrollDir::Down => Some(Message::NewSessionDialogDartDefinesDown),
            _ => None,
        };
    }

    // Main dialog — dispatch by focused pane.
    match dialog.focused_pane {
        DialogPane::TargetSelector => match dir {
            ScrollDir::Up => Some(Message::NewSessionDialogDeviceUp),
            ScrollDir::Down => Some(Message::NewSessionDialogDeviceDown),
            _ => None,
        },
        DialogPane::LaunchContext => match dir {
            ScrollDir::Up => Some(Message::NewSessionDialogFieldPrev),
            ScrollDir::Down => Some(Message::NewSessionDialogFieldNext),
            _ => None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input_mouse::KeyModSet;
    use crate::new_session_dialog::{
        DartDefine, DartDefinesModalState, DialogPane, FuzzyModalState, FuzzyModalType,
    };
    use crate::state::AppState;

    fn fresh_state() -> AppState {
        AppState::new()
    }

    #[test]
    fn main_dialog_target_selector_scroll_moves_device_selection() {
        let mut s = fresh_state();
        s.new_session_dialog_state.focused_pane = DialogPane::TargetSelector;
        assert!(matches!(
            handle_scroll(&s, ScrollDir::Up, KeyModSet::NONE),
            Some(Message::NewSessionDialogDeviceUp)
        ));
        assert!(matches!(
            handle_scroll(&s, ScrollDir::Down, KeyModSet::NONE),
            Some(Message::NewSessionDialogDeviceDown)
        ));
    }

    #[test]
    fn main_dialog_launch_context_scroll_moves_field_focus() {
        let mut s = fresh_state();
        s.new_session_dialog_state.focused_pane = DialogPane::LaunchContext;
        assert!(matches!(
            handle_scroll(&s, ScrollDir::Up, KeyModSet::NONE),
            Some(Message::NewSessionDialogFieldPrev)
        ));
        assert!(matches!(
            handle_scroll(&s, ScrollDir::Down, KeyModSet::NONE),
            Some(Message::NewSessionDialogFieldNext)
        ));
    }

    #[test]
    fn fuzzy_modal_takes_precedence_over_main_dialog() {
        let mut s = fresh_state();
        // Set focused_pane to TargetSelector — without the modal, this would
        // route to DeviceUp/Down. With fuzzy modal open, must route to FuzzyUp/Down.
        s.new_session_dialog_state.focused_pane = DialogPane::TargetSelector;
        s.new_session_dialog_state.fuzzy_modal =
            Some(FuzzyModalState::new(FuzzyModalType::Config, vec![]));
        assert!(matches!(
            handle_scroll(&s, ScrollDir::Up, KeyModSet::NONE),
            Some(Message::NewSessionDialogFuzzyUp)
        ));
        assert!(matches!(
            handle_scroll(&s, ScrollDir::Down, KeyModSet::NONE),
            Some(Message::NewSessionDialogFuzzyDown)
        ));
    }

    #[test]
    fn dart_defines_modal_takes_precedence_over_main_dialog() {
        let mut s = fresh_state();
        // Set focused_pane to LaunchContext — without the modal, this would
        // route to FieldPrev/Next. With dart-defines modal open, must route to
        // DartDefinesUp/Down.
        s.new_session_dialog_state.focused_pane = DialogPane::LaunchContext;
        s.new_session_dialog_state.dart_defines_modal = Some(DartDefinesModalState::new(vec![]));
        assert!(matches!(
            handle_scroll(&s, ScrollDir::Up, KeyModSet::NONE),
            Some(Message::NewSessionDialogDartDefinesUp)
        ));
        assert!(matches!(
            handle_scroll(&s, ScrollDir::Down, KeyModSet::NONE),
            Some(Message::NewSessionDialogDartDefinesDown)
        ));
    }

    #[test]
    fn dart_defines_modal_routes_in_both_panes() {
        let mut s = fresh_state();

        // List pane (default after open): Up → DartDefinesUp.
        s.new_session_dialog_state.dart_defines_modal =
            Some(DartDefinesModalState::new(vec![DartDefine::new("K", "V")]));
        assert!(matches!(
            handle_scroll(&s, ScrollDir::Up, KeyModSet::NONE),
            Some(Message::NewSessionDialogDartDefinesUp)
        ));

        // Switch to Edit pane: Up → still DartDefinesUp (unlike Settings dart-defines).
        if let Some(ref mut modal) = s.new_session_dialog_state.dart_defines_modal {
            modal.switch_pane();
        }
        assert!(matches!(
            handle_scroll(&s, ScrollDir::Up, KeyModSet::NONE),
            Some(Message::NewSessionDialogDartDefinesUp)
        ));
    }

    #[test]
    fn modifier_keys_do_not_change_behavior() {
        let mut s = fresh_state();
        s.new_session_dialog_state.focused_pane = DialogPane::TargetSelector;
        for mods in [
            KeyModSet::new(true, false, false),
            KeyModSet::new(false, true, false),
            KeyModSet::new(true, true, false),
        ] {
            assert!(matches!(
                handle_scroll(&s, ScrollDir::Up, mods),
                Some(Message::NewSessionDialogDeviceUp)
            ));
        }
    }

    #[test]
    fn horizontal_wheel_no_op_in_every_pane_and_modal() {
        let mut s = fresh_state();
        for pane in [DialogPane::TargetSelector, DialogPane::LaunchContext] {
            s.new_session_dialog_state.focused_pane = pane;
            assert!(handle_scroll(&s, ScrollDir::Left, KeyModSet::NONE).is_none());
            assert!(handle_scroll(&s, ScrollDir::Right, KeyModSet::NONE).is_none());
        }
    }

    #[test]
    fn horizontal_wheel_no_op_in_fuzzy_modal() {
        let mut s = fresh_state();
        s.new_session_dialog_state.fuzzy_modal =
            Some(FuzzyModalState::new(FuzzyModalType::Flavor, vec![]));
        assert!(handle_scroll(&s, ScrollDir::Left, KeyModSet::NONE).is_none());
        assert!(handle_scroll(&s, ScrollDir::Right, KeyModSet::NONE).is_none());
    }

    #[test]
    fn horizontal_wheel_no_op_in_dart_defines_modal() {
        let mut s = fresh_state();
        s.new_session_dialog_state.dart_defines_modal = Some(DartDefinesModalState::new(vec![]));
        assert!(handle_scroll(&s, ScrollDir::Left, KeyModSet::NONE).is_none());
        assert!(handle_scroll(&s, ScrollDir::Right, KeyModSet::NONE).is_none());
    }
}
