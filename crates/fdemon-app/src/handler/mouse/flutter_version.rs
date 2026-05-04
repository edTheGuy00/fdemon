//! Scroll routing for `UiMode::FlutterVersion`.
//!
//! Mirrors `handle_key_flutter_version` (keys.rs:332-355): wheel up/down
//! moves the version list selection; no page-step (no keyboard analogue).

use crate::input_mouse::{KeyModSet, ScrollDir};
use crate::message::Message;
use crate::state::AppState;

pub(super) fn handle_scroll(
    _state: &AppState,
    dir: ScrollDir,
    _mods: KeyModSet,
) -> Option<Message> {
    // Modifiers ignored: FlutterVersion has no page-step analogue in the
    // keyboard handler (keys.rs:332-355 binds only j/k and Up/Down).
    match dir {
        ScrollDir::Up => Some(Message::FlutterVersionUp),
        ScrollDir::Down => Some(Message::FlutterVersionDown),
        ScrollDir::Left | ScrollDir::Right => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input_mouse::KeyModSet;
    use crate::state::AppState;

    #[test]
    fn wheel_up_moves_version_selection_up() {
        let s = AppState::new();
        assert!(matches!(
            handle_scroll(&s, ScrollDir::Up, KeyModSet::NONE),
            Some(Message::FlutterVersionUp)
        ));
    }

    #[test]
    fn wheel_down_moves_version_selection_down() {
        let s = AppState::new();
        assert!(matches!(
            handle_scroll(&s, ScrollDir::Down, KeyModSet::NONE),
            Some(Message::FlutterVersionDown)
        ));
    }

    #[test]
    fn modifiers_do_not_change_behavior() {
        let s = AppState::new();
        for mods in [
            KeyModSet::new(true, false, false),
            KeyModSet::new(false, true, false),
            KeyModSet::new(false, false, true),
            KeyModSet::new(true, true, true),
        ] {
            assert!(matches!(
                handle_scroll(&s, ScrollDir::Up, mods),
                Some(Message::FlutterVersionUp)
            ));
        }
    }

    #[test]
    fn horizontal_wheel_no_op() {
        let s = AppState::new();
        assert!(handle_scroll(&s, ScrollDir::Left, KeyModSet::NONE).is_none());
        assert!(handle_scroll(&s, ScrollDir::Right, KeyModSet::NONE).is_none());
    }
}
