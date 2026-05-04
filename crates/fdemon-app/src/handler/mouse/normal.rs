//! Mouse event handlers for `UiMode::Normal`.
//!
//! Mirrors the keyboard handler at [`crate::handler::keys::handle_key_normal`]:
//! when the tag-filter overlay is open it intercepts up/down navigation,
//! otherwise vertical scrolling drives the log view directly.
//!
//! Phase 3 adds click hit-testing via [`handle_press`], which queries the
//! per-frame [`MouseRegions`] registry recorded during `render::view()`.

use crate::input_mouse::{KeyModSet, MouseButton, ScrollDir};
use crate::message::Message;
use crate::state::AppState;

/// Hit-test a left/middle click against the registry recorded during the
/// most recent `render::view`. Returns the matched region's resolved
/// [`Message`], gated by the same busy/tag-filter checks as the keyboard
/// handler.
///
/// Modifier keys (`mods`) are not consulted in Phase 3 — modifier+click
/// shortcuts are deferred to a future phase. They are accepted in the
/// signature for symmetry with `handle_scroll`.
pub(super) fn handle_press(
    state: &AppState,
    x: u16,
    y: u16,
    button: MouseButton,
    _mods: KeyModSet,
) -> Option<Message> {
    // Tag-filter overlay: clicks fall through to the underlying log view's
    // registry, which is empty in Phase 3 (the overlay does not register
    // regions until Phase 5). For now, treat clicks while tag-filter is
    // visible as no-ops to avoid surprising the user.
    if state.tag_filter_visible {
        return None;
    }

    // Right-click is reserved for future right-click context menus.
    if matches!(button, MouseButton::Right) {
        return None;
    }

    // ── Hit-test against the registry ────────────────────────────────────
    // EXCEPTION: TEA render-hint write-back via Cell — see docs/CODE_STANDARDS.md Principle 3
    let regions = state.mouse_regions.take();
    let action_opt = regions.hit_test(x, y, button).and_then(|entry| {
        let action = match button {
            MouseButton::Left => entry.on_left.as_ref(),
            MouseButton::Middle => entry.on_middle.as_ref(),
            MouseButton::Right => None,
        };
        action.map(|a| a.resolve(x, y))
    });
    // Put the registry back unchanged. Re-rendering will repopulate it.
    state.mouse_regions.set(regions);

    let msg = action_opt?;

    // ── Busy gate (mirrors handler/keys.rs:167-173) ──────────────────────
    // HotReload/HotRestart/StopApp are gated by any-session-busy in the
    // keyboard handler. Mirror that here so a click during a reload is a
    // silent no-op rather than queuing a second reload.
    let busy = state.session_manager.any_session_busy();
    if busy && matches!(msg, Message::HotReload | Message::HotRestart | Message::StopApp) {
        return None;
    }

    Some(msg)
}

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
    use crate::mouse_regions::{MouseAction, MouseRect, MouseRegions};
    use crate::state::AppState;

    fn state_with_tag_filter(visible: bool) -> AppState {
        let mut s = AppState::new();
        s.tag_filter_visible = visible;
        s
    }

    /// Minimal device for session creation in tests.
    fn test_device(id: &str, name: &str) -> fdemon_daemon::Device {
        fdemon_daemon::Device {
            id: id.to_string(),
            name: name.to_string(),
            platform: "android".to_string(),
            emulator: false,
            category: None,
            platform_type: None,
            ephemeral: false,
            emulator_id: None,
        }
    }

    // ── handle_press tests ────────────────────────────────────────────────

    #[test]
    fn press_outside_any_region_is_none() {
        let state = AppState::new();
        let result = handle_press(&state, 100, 100, MouseButton::Left, KeyModSet::NONE);
        assert!(result.is_none());
    }

    #[test]
    fn press_right_button_is_no_op_even_with_matching_region() {
        let state = AppState::new();
        let mut regions = state.mouse_regions.take();
        regions
            .builder()
            .click(MouseRect::new(0, 0, 10, 1), MouseAction::emit(Message::HotReload));
        state.mouse_regions.set(regions);

        let result = handle_press(&state, 0, 0, MouseButton::Right, KeyModSet::NONE);
        assert!(result.is_none(), "right button is reserved for future");
    }

    #[test]
    fn press_left_on_recorded_region_returns_emit_message() {
        let state = AppState::new();
        let mut regions = state.mouse_regions.take();
        regions
            .builder()
            .click(MouseRect::new(5, 0, 3, 1), MouseAction::emit(Message::HotReload));
        state.mouse_regions.set(regions);

        let result = handle_press(&state, 6, 0, MouseButton::Left, KeyModSet::NONE);
        assert!(matches!(result, Some(Message::HotReload)));
    }

    #[test]
    fn press_middle_on_left_only_region_is_none() {
        let state = AppState::new();
        let mut regions = state.mouse_regions.take();
        regions
            .builder()
            .click(MouseRect::new(0, 0, 10, 1), MouseAction::emit(Message::HotReload));
        state.mouse_regions.set(regions);

        let result = handle_press(&state, 0, 0, MouseButton::Middle, KeyModSet::NONE);
        assert!(result.is_none());
    }

    #[test]
    fn press_middle_on_left_middle_region_returns_middle_message() {
        let state = AppState::new();
        let mut regions = state.mouse_regions.take();
        regions.builder().click_left_middle(
            MouseRect::new(0, 0, 10, 1),
            MouseAction::emit(Message::SelectSessionByIndex(2)),
            MouseAction::emit(Message::CloseSessionAt(2)),
        );
        state.mouse_regions.set(regions);

        let left = handle_press(&state, 0, 0, MouseButton::Left, KeyModSet::NONE);
        let middle = handle_press(&state, 0, 0, MouseButton::Middle, KeyModSet::NONE);
        assert!(matches!(left, Some(Message::SelectSessionByIndex(2))));
        assert!(matches!(middle, Some(Message::CloseSessionAt(2))));
    }

    #[test]
    fn press_with_emit_with_coord_resolves_against_position() {
        let state = AppState::new();
        let mut regions = state.mouse_regions.take();
        regions.builder().click(
            MouseRect::new(0, 5, 100, 10),
            MouseAction::EmitWithCoord(|_x, y| Message::SelectSessionByIndex((y - 5) as usize)),
        );
        state.mouse_regions.set(regions);

        let result = handle_press(&state, 50, 8, MouseButton::Left, KeyModSet::NONE);
        assert!(matches!(result, Some(Message::SelectSessionByIndex(3))));
    }

    #[test]
    fn press_when_busy_blocks_hot_reload_only() {
        let mut state = AppState::new();
        let id = state
            .session_manager
            .create_session(&test_device("d1", "iPhone"))
            .unwrap();
        // Mark the session as busy by starting a reload.
        state
            .session_manager
            .get_mut(id)
            .unwrap()
            .session
            .start_reload();
        assert!(state.session_manager.any_session_busy(), "precondition");

        let mut regions = state.mouse_regions.take();
        regions
            .builder()
            .click(MouseRect::new(0, 0, 3, 1), MouseAction::emit(Message::HotReload));
        regions
            .builder()
            .click(MouseRect::new(5, 0, 3, 1), MouseAction::emit(Message::RequestQuit));
        state.mouse_regions.set(regions);

        let reload = handle_press(&state, 0, 0, MouseButton::Left, KeyModSet::NONE);
        let quit = handle_press(&state, 5, 0, MouseButton::Left, KeyModSet::NONE);
        assert!(reload.is_none(), "HotReload gated by busy");
        assert!(matches!(quit, Some(Message::RequestQuit)), "RequestQuit not gated");
    }

    #[test]
    fn press_take_putback_preserves_registry() {
        let state = AppState::new();
        let mut regions = state.mouse_regions.take();
        regions
            .builder()
            .click(MouseRect::new(0, 0, 10, 1), MouseAction::emit(Message::HotReload));
        state.mouse_regions.set(regions);

        let _ = handle_press(&state, 0, 0, MouseButton::Left, KeyModSet::NONE);

        // The registry should still hold the entry after a hit-test.
        let regions = state.mouse_regions.take();
        assert_eq!(regions.len(), 1, "registry preserved across hit-test");
        state.mouse_regions.set(regions);
    }

    #[test]
    fn press_when_tag_filter_visible_is_no_op() {
        let mut state = AppState::new();
        state.tag_filter_visible = true;
        let mut regions = state.mouse_regions.take();
        regions
            .builder()
            .click(MouseRect::new(0, 0, 10, 1), MouseAction::emit(Message::HotReload));
        state.mouse_regions.set(regions);

        let result = handle_press(&state, 0, 0, MouseButton::Left, KeyModSet::NONE);
        assert!(result.is_none());
    }

    // ── handle_scroll tests ───────────────────────────────────────────────

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
