//! Mouse press handling for `UiMode::ConfirmDialog`.
//!
//! Yes/No buttons (and any other action buttons stored on
//! `state.confirm_dialog_state.actions`) become clickable. The button's
//! action message is stored on the state; the dispatcher just resolves
//! whatever the registry returns.

use crate::input_mouse::{KeyModSet, MouseButton};
use crate::message::Message;
use crate::state::AppState;

/// Hit-test a left/middle click in `UiMode::ConfirmDialog` against the
/// per-frame region registry. Returns the matched region's resolved [`Message`].
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input_mouse::KeyModSet;
    use crate::mouse_regions::{MouseAction, MouseRect};
    use crate::state::{AppState, UiMode};

    fn state_in_confirm_dialog() -> AppState {
        let mut s = AppState::new();
        s.ui_mode = UiMode::ConfirmDialog;
        s
    }

    #[test]
    fn no_region_returns_none() {
        let mut s = state_in_confirm_dialog();
        assert!(handle_press(&mut s, 0, 0, MouseButton::Left, KeyModSet::NONE).is_none());
    }

    #[test]
    fn click_on_yes_button_returns_confirm_quit() {
        let mut s = state_in_confirm_dialog();
        let mut regions = s.mouse_regions.take();
        regions.builder().click_at_z(
            MouseRect::new(0, 0, 5, 1),
            MouseAction::emit(Message::ConfirmQuit),
            1,
        );
        s.mouse_regions.set(regions);
        let r = handle_press(&mut s, 0, 0, MouseButton::Left, KeyModSet::NONE);
        assert!(matches!(r, Some(Message::ConfirmQuit)));
    }

    #[test]
    fn right_click_is_no_op() {
        let mut s = state_in_confirm_dialog();
        let mut regions = s.mouse_regions.take();
        regions.builder().click(
            MouseRect::new(0, 0, 5, 1),
            MouseAction::emit(Message::ConfirmQuit),
        );
        s.mouse_regions.set(regions);
        assert!(handle_press(&mut s, 0, 0, MouseButton::Right, KeyModSet::NONE).is_none());
    }

    #[test]
    fn middle_click_no_region_returns_none() {
        let mut s = state_in_confirm_dialog();
        assert!(handle_press(&mut s, 0, 0, MouseButton::Middle, KeyModSet::NONE).is_none());
    }
}
