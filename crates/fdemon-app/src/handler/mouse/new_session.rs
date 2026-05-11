//! Mouse event handlers for `UiMode::Startup` and `UiMode::NewSessionDialog`.
//!
//! Both modes share a single keyboard handler in `handler::keys` and share
//! this mouse handler too.
//!
//! **Scroll** modal precedence mirrors `handle_key_new_session_dialog`:
//! fuzzy modal → dart-defines modal → main dialog (dispatched by focused pane).
//!
//! **Press** hit-tests against the per-frame region registry. Modal precedence
//! is handled by `z_index` in the registry: main dialog regions are registered
//! at z=1, fuzzy/dart-defines modal regions at z=2. The registry's `hit_test`
//! returns the highest-z match, so no explicit modal check is needed here.

use crate::input_mouse::{KeyModSet, MouseButton, ScrollDir};
use crate::message::Message;
use crate::new_session_dialog::DialogPane;
use crate::state::AppState;

/// Hit-test a left/middle click in `UiMode::Startup` / `UiMode::NewSessionDialog`
/// against the per-frame region registry. Returns the matched region's resolved
/// [`Message`].
///
/// Modal precedence (fuzzy modal → dart-defines modal → main dialog) is handled
/// transparently by the registry's `z_index`: main dialog regions are at z=1
/// and modal-overlay regions are at z=2 (when those modals are open). The
/// dispatcher does not need to know which modal is open.
///
/// **Right-click reserved.** Right-click returns `None` for future context-menu
/// support, matching the convention established in [`normal::handle_press`].
///
/// [`normal::handle_press`]: super::normal::handle_press
pub(super) fn handle_press(
    state: &mut AppState,
    x: u16,
    y: u16,
    button: MouseButton,
    _mods: KeyModSet,
) -> Option<Message> {
    if button == MouseButton::Right {
        return None;
    }
    // Modal precedence is handled by `z_index`:
    //   - main dialog regions: z = 1
    //   - fuzzy modal / dart-defines modal regions: z = 2
    // The registry's hit_test returns the highest-z match — so a click
    // inside an open fuzzy modal lands on the modal's row, not the
    // device-list row underneath.
    //
    // No edit-mode gate — the dialog has no inline-edit state. Field
    // activation goes through the keyboard-Enter chain (Message::FieldActivate).

    // EXCEPTION: TEA render-hint write-back via Cell — see docs/CODE_STANDARDS.md Principle 3
    let regions = state.mouse_regions.take_guard();
    let action_opt = regions.hit_test(x, y, button).and_then(|entry| {
        let action = match button {
            MouseButton::Left => entry.on_left.as_ref(),
            MouseButton::Middle => entry.on_middle.as_ref(),
            MouseButton::Right => None,
        };
        action.map(|a| a.resolve(x, y))
    });
    drop(regions);
    action_opt
}

pub(super) fn handle_scroll(state: &AppState, dir: ScrollDir, _mods: KeyModSet) -> Option<Message> {
    let dialog = &state.new_session_dialog_state;
    // Modifiers ignored: NewSessionDialog's keyboard handlers (keys.rs:793-896)
    // bind no Shift+anything for navigation, so the mouse mirrors that — every
    // wheel direction is single-step regardless of held modifier.

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

    // ── handle_press tests ───────────────────────────────────────────────

    #[test]
    fn press_no_region_returns_none() {
        let mut s = fresh_state();
        assert!(handle_press(&mut s, 0, 0, MouseButton::Left, KeyModSet::NONE).is_none());
    }

    #[test]
    fn press_registered_region_returns_message() {
        use crate::mouse_regions::{MouseAction, MouseRect};
        let mut s = fresh_state();
        let mut regions = s.mouse_regions.take();
        regions.builder().click(
            MouseRect::new(0, 0, 10, 1),
            MouseAction::emit(Message::NewSessionDialogDeviceUp),
        );
        s.mouse_regions.set(regions);
        let r = handle_press(&mut s, 0, 0, MouseButton::Left, KeyModSet::NONE);
        assert!(matches!(r, Some(Message::NewSessionDialogDeviceUp)));
    }

    #[test]
    fn press_right_click_is_no_op() {
        use crate::mouse_regions::{MouseAction, MouseRect};
        let mut s = fresh_state();
        let mut regions = s.mouse_regions.take();
        regions.builder().click(
            MouseRect::new(0, 0, 10, 1),
            MouseAction::emit(Message::NewSessionDialogDeviceUp),
        );
        s.mouse_regions.set(regions);
        assert!(handle_press(&mut s, 0, 0, MouseButton::Right, KeyModSet::NONE).is_none());
    }
}
