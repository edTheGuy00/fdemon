//! Mouse press handling when the tag-filter overlay is visible.
//!
//! Routed to from `handler/mouse/mod.rs::handle_press` whenever
//! `state.tag_filter_visible == true`, regardless of the underlying
//! `ui_mode`. Mirrors how the keyboard handler intercepts all keys at
//! `handler/keys.rs:105-126`.
//!
//! Tag-row regions emit `Message::TagFilterClickRow { index }`; footer
//! action labels emit `Message::ShowAllNativeTags` / `Message::HideAllNativeTags`.

use crate::input_mouse::{KeyModSet, MouseButton};
use crate::message::Message;
use crate::state::AppState;

/// Hit-test a left/middle click against the tag-filter overlay region registry.
///
/// Called when `state.tag_filter_visible == true`, regardless of the underlying
/// `ui_mode`. Returns the matched region's resolved [`Message`].
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
    use crate::state::AppState;

    fn state_with_tag_filter() -> AppState {
        let mut s = AppState::new();
        s.tag_filter_visible = true;
        s
    }

    #[test]
    fn no_region_returns_none() {
        let mut s = state_with_tag_filter();
        assert!(handle_press(&mut s, 0, 0, MouseButton::Left, KeyModSet::NONE).is_none());
    }

    #[test]
    fn click_on_tag_row_returns_tag_filter_click_row() {
        let mut s = state_with_tag_filter();
        let mut regions = s.mouse_regions.take();
        regions.builder().click(
            MouseRect::new(0, 0, 20, 1),
            MouseAction::emit(Message::TagFilterClickRow { index: 2 }),
        );
        s.mouse_regions.set(regions);
        let r = handle_press(&mut s, 0, 0, MouseButton::Left, KeyModSet::NONE);
        assert!(matches!(r, Some(Message::TagFilterClickRow { index: 2 })));
    }

    #[test]
    fn click_on_show_all_action_returns_show_all_native_tags() {
        let mut s = state_with_tag_filter();
        let mut regions = s.mouse_regions.take();
        regions.builder().click(
            MouseRect::new(0, 5, 5, 1),
            MouseAction::emit(Message::ShowAllNativeTags),
        );
        s.mouse_regions.set(regions);
        let r = handle_press(&mut s, 0, 5, MouseButton::Left, KeyModSet::NONE);
        assert!(matches!(r, Some(Message::ShowAllNativeTags)));
    }

    #[test]
    fn right_click_is_no_op() {
        let mut s = state_with_tag_filter();
        let mut regions = s.mouse_regions.take();
        regions.builder().click(
            MouseRect::new(0, 0, 20, 1),
            MouseAction::emit(Message::TagFilterClickRow { index: 0 }),
        );
        s.mouse_regions.set(regions);
        assert!(handle_press(&mut s, 0, 0, MouseButton::Right, KeyModSet::NONE).is_none());
    }
}
