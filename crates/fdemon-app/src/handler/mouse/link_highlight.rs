//! Scroll routing for `UiMode::LinkHighlight`.
//!
//! Mirrors `handle_key_link_highlight` (keys.rs:361-383): plain wheel scrolls
//! the log view; Shift+wheel does page scroll. Same messages as Normal mode.

use crate::input_mouse::{KeyModSet, ScrollDir};
use crate::message::Message;
use crate::state::AppState;

pub(super) fn handle_scroll(_state: &AppState, dir: ScrollDir, mods: KeyModSet) -> Option<Message> {
    if mods.is_shift_only() {
        return match dir {
            ScrollDir::Up => Some(Message::PageUp),
            ScrollDir::Down => Some(Message::PageDown),
            ScrollDir::Left | ScrollDir::Right => None,
        };
    }
    if mods.ctrl || mods.alt {
        return None;
    }
    match dir {
        ScrollDir::Up => Some(Message::ScrollUp),
        ScrollDir::Down => Some(Message::ScrollDown),
        ScrollDir::Left | ScrollDir::Right => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input_mouse::KeyModSet;
    use crate::state::AppState;

    #[test]
    fn plain_wheel_scrolls() {
        let s = AppState::new();
        assert!(matches!(
            handle_scroll(&s, ScrollDir::Up, KeyModSet::NONE),
            Some(Message::ScrollUp)
        ));
        assert!(matches!(
            handle_scroll(&s, ScrollDir::Down, KeyModSet::NONE),
            Some(Message::ScrollDown)
        ));
    }

    #[test]
    fn shift_wheel_pages() {
        let s = AppState::new();
        let mods = KeyModSet::new(true, false, false);
        assert!(matches!(
            handle_scroll(&s, ScrollDir::Up, mods),
            Some(Message::PageUp)
        ));
        assert!(matches!(
            handle_scroll(&s, ScrollDir::Down, mods),
            Some(Message::PageDown)
        ));
    }

    #[test]
    fn ctrl_or_alt_only_no_op() {
        let s = AppState::new();
        assert!(handle_scroll(&s, ScrollDir::Up, KeyModSet::new(false, true, false)).is_none());
        assert!(handle_scroll(&s, ScrollDir::Down, KeyModSet::new(false, false, true)).is_none());
    }

    #[test]
    fn ctrl_shift_no_op() {
        let s = AppState::new();
        let msg = handle_scroll(&s, ScrollDir::Up, KeyModSet::new(true, true, false));
        assert!(msg.is_none());
    }

    #[test]
    fn horizontal_wheel_no_op() {
        let s = AppState::new();
        assert!(handle_scroll(&s, ScrollDir::Left, KeyModSet::NONE).is_none());
        assert!(handle_scroll(&s, ScrollDir::Right, KeyModSet::NONE).is_none());
    }
}
