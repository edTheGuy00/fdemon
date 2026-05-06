//! Mouse event handlers for `UiMode::LinkHighlight`.
//!
//! **Scroll** mirrors `handle_key_link_highlight` (keys.rs:361-383): plain wheel
//! scrolls the log view; Shift+wheel does page scroll. Same messages as Normal mode.
//!
//! **Press** hit-tests against the per-frame region registry. Link badge regions
//! emit `Message::SelectLink(c)` carrying the link character.

use crate::input_mouse::{KeyModSet, MouseButton, ScrollDir};
use crate::message::Message;
use crate::state::AppState;

/// Hit-test a left/middle click in `UiMode::LinkHighlight` against the per-frame
/// region registry. Returns the matched region's resolved [`Message`].
///
/// Link badge regions emit `Message::SelectLink(c)` carrying the link character.
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

    // ── handle_press tests ───────────────────────────────────────────────

    #[test]
    fn press_no_region_returns_none() {
        let mut s = AppState::new();
        assert!(handle_press(&mut s, 0, 0, MouseButton::Left, KeyModSet::NONE).is_none());
    }

    #[test]
    fn press_link_badge_returns_select_link() {
        use crate::mouse_regions::{MouseAction, MouseRect};
        let mut s = AppState::new();
        let mut regions = s.mouse_regions.take();
        regions.builder().click(
            MouseRect::new(5, 3, 3, 1),
            MouseAction::emit(Message::SelectLink('a')),
        );
        s.mouse_regions.set(regions);
        let r = handle_press(&mut s, 5, 3, MouseButton::Left, KeyModSet::NONE);
        assert!(matches!(r, Some(Message::SelectLink('a'))));
    }

    #[test]
    fn press_right_click_is_no_op() {
        use crate::mouse_regions::{MouseAction, MouseRect};
        let mut s = AppState::new();
        let mut regions = s.mouse_regions.take();
        regions.builder().click(
            MouseRect::new(0, 0, 5, 1),
            MouseAction::emit(Message::SelectLink('b')),
        );
        s.mouse_regions.set(regions);
        assert!(handle_press(&mut s, 0, 0, MouseButton::Right, KeyModSet::NONE).is_none());
    }
}
