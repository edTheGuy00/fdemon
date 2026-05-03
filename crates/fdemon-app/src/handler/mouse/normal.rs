//! Scroll routing for `UiMode::Normal`.
//!
//! Mirrors the keyboard handler at [`crate::handler::keys::handle_key_normal`]:
//! when the tag-filter overlay is open it intercepts up/down navigation,
//! otherwise vertical scrolling drives the log view directly.

use crate::input_mouse::{KeyModSet, ScrollDir};
use crate::message::Message;
use crate::state::AppState;

pub(super) fn handle_scroll(state: &AppState, dir: ScrollDir, mods: KeyModSet) -> Option<Message> {
    // Tag-filter overlay intercepts wheel up/down (mirrors keys.rs:112-114).
    if state.tag_filter_visible {
        return match dir {
            ScrollDir::Up => Some(Message::TagFilterMoveUp),
            ScrollDir::Down => Some(Message::TagFilterMoveDown),
            ScrollDir::Left | ScrollDir::Right => None,
        };
    }

    // Shift+wheel → page scroll (mirrors keys.rs:269-270).
    if mods.is_shift_only() {
        return match dir {
            ScrollDir::Up => Some(Message::PageUp),
            ScrollDir::Down => Some(Message::PageDown),
            ScrollDir::Left | ScrollDir::Right => None,
        };
    }

    // Plain wheel → line scroll (mirrors keys.rs:265-266).
    // Ctrl+wheel and Alt+wheel (with or without other modifiers) return None
    // because is_shift_only() is false for those combinations.
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

    fn state_with_tag_filter(visible: bool) -> AppState {
        let mut s = AppState::new();
        s.tag_filter_visible = visible;
        s
    }

    #[test]
    fn plain_wheel_up_scrolls_up() {
        let s = state_with_tag_filter(false);
        let msg = handle_scroll(&s, ScrollDir::Up, KeyModSet::NONE);
        assert!(matches!(msg, Some(Message::ScrollUp)));
    }

    #[test]
    fn plain_wheel_down_scrolls_down() {
        let s = state_with_tag_filter(false);
        let msg = handle_scroll(&s, ScrollDir::Down, KeyModSet::NONE);
        assert!(matches!(msg, Some(Message::ScrollDown)));
    }

    #[test]
    fn shift_wheel_up_pages_up() {
        let s = state_with_tag_filter(false);
        let msg = handle_scroll(&s, ScrollDir::Up, KeyModSet::new(true, false, false));
        assert!(matches!(msg, Some(Message::PageUp)));
    }

    #[test]
    fn shift_wheel_down_pages_down() {
        let s = state_with_tag_filter(false);
        let msg = handle_scroll(&s, ScrollDir::Down, KeyModSet::new(true, false, false));
        assert!(matches!(msg, Some(Message::PageDown)));
    }

    #[test]
    fn ctrl_wheel_is_a_no_op() {
        let s = state_with_tag_filter(false);
        let msg = handle_scroll(&s, ScrollDir::Up, KeyModSet::new(false, true, false));
        assert!(msg.is_none());
    }

    #[test]
    fn alt_wheel_is_a_no_op() {
        let s = state_with_tag_filter(false);
        let msg = handle_scroll(&s, ScrollDir::Down, KeyModSet::new(false, false, true));
        assert!(msg.is_none());
    }

    #[test]
    fn ctrl_shift_wheel_is_a_no_op() {
        let s = state_with_tag_filter(false);
        let msg = handle_scroll(&s, ScrollDir::Up, KeyModSet::new(true, true, false));
        assert!(msg.is_none());
    }

    #[test]
    fn tag_filter_visible_routes_to_tag_filter_nav() {
        let s = state_with_tag_filter(true);
        assert!(matches!(
            handle_scroll(&s, ScrollDir::Up, KeyModSet::NONE),
            Some(Message::TagFilterMoveUp)
        ));
        assert!(matches!(
            handle_scroll(&s, ScrollDir::Down, KeyModSet::NONE),
            Some(Message::TagFilterMoveDown)
        ));
    }

    #[test]
    fn tag_filter_visible_ignores_shift_modifier() {
        let s = state_with_tag_filter(true);
        let mods = KeyModSet::new(true, false, false);
        // Tag-filter overlay does not page-scroll; Shift is dropped.
        assert!(matches!(
            handle_scroll(&s, ScrollDir::Up, mods),
            Some(Message::TagFilterMoveUp)
        ));
    }

    #[test]
    fn horizontal_wheel_is_no_op_in_both_states() {
        let off = state_with_tag_filter(false);
        let on = state_with_tag_filter(true);
        for s in [&off, &on] {
            assert!(handle_scroll(s, ScrollDir::Left, KeyModSet::NONE).is_none());
            assert!(handle_scroll(s, ScrollDir::Right, KeyModSet::NONE).is_none());
        }
    }
}
