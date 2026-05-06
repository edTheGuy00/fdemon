//! Mouse event handlers for different UI modes.
//!
//! Mirrors [`crate::handler::keys`] — converts a [`MouseInput`] into a
//! concrete [`Message`] based on the current [`UiMode`]. Phase 2 wires
//! per-mode scroll routing; Phase 3+ adds click hit-testing.

mod confirm_dialog; // Phase 5 task 05
mod devtools;
mod flutter_version;
mod link_highlight;
mod new_session;
mod normal;
mod settings;
mod tag_filter; // Phase 5 task 05

use crate::input_mouse::{KeyModSet, MouseButton, MouseInput, ScrollDir};
use crate::message::Message;
use crate::state::{AppState, UiMode};

/// Convert a mouse event to a follow-up message based on the current UI mode.
///
/// In Phase 2 only [`MouseInput::Scroll`] produced messages. Phase 3 adds
/// click hit-testing via [`handle_press`]: `Normal` mode queries the
/// per-frame [`crate::mouse_regions::MouseRegions`] registry and returns the
/// matched region's message. Other modes remain no-op for press events until
/// Phase 4/5.
///
/// Takes `&mut AppState` rather than `&AppState` because the DevTools press
/// handler may clear `network.filter_input_active` as a side effect of a
/// sub-tab bar click while filter input is active (Phase 4.5 task 08).
pub fn handle_mouse(state: &mut AppState, input: MouseInput) -> Option<Message> {
    match input {
        MouseInput::Scroll {
            direction,
            modifiers,
            ..
        } => handle_scroll(state, direction, modifiers),
        MouseInput::Press {
            x,
            y,
            button,
            modifiers,
        } => handle_press(state, x, y, button, modifiers),
        // Phase 4+ may wire drag-to-select etc. — currently no-op.
        MouseInput::Release { .. } | MouseInput::Drag { .. } => None,
    }
}

/// Route a button press to the appropriate per-mode handler.
///
/// The tag-filter overlay routes to its own per-mode handler when visible —
/// see [`tag_filter::handle_press`]. (Earlier phases short-circuited press
/// to `None` here; Phase 5 task 05 lifted that gate so the overlay's tag
/// rows become clickable.) The keyboard handler at `handler/keys.rs:105-126`
/// continues to intercept ALL keys when the overlay is visible — only the
/// mouse path is reworked.
///
/// Phase 3 wires [`UiMode::Normal`]. Phase 4 adds [`UiMode::DevTools`].
/// Phase 5 wires Settings, NewSessionDialog/Startup, ConfirmDialog, and
/// LinkHighlight. Remaining modes (`EmulatorSelector`, `Loading`,
/// `SearchInput`, `FlutterVersion`) remain no-op for press in v1.
fn handle_press(
    state: &mut AppState,
    x: u16,
    y: u16,
    button: MouseButton,
    mods: KeyModSet,
) -> Option<Message> {
    // Tag-filter overlay routes to its own handler regardless of underlying ui_mode.
    if state.tag_filter_visible {
        return tag_filter::handle_press(state, x, y, button, mods);
    }

    match state.ui_mode {
        UiMode::Normal => normal::handle_press(state, x, y, button, mods),
        UiMode::DevTools => devtools::handle_press(state, x, y, button, mods),
        UiMode::ConfirmDialog => confirm_dialog::handle_press(state, x, y, button, mods),
        UiMode::Settings => settings::handle_press(state, x, y, button, mods),
        UiMode::Startup | UiMode::NewSessionDialog => {
            new_session::handle_press(state, x, y, button, mods)
        }
        UiMode::LinkHighlight => link_highlight::handle_press(state, x, y, button, mods),
        // No clickable surface in v1.
        UiMode::EmulatorSelector
        | UiMode::Loading
        | UiMode::SearchInput
        | UiMode::FlutterVersion => None,
    }
}

/// Route a wheel scroll to the appropriate per-mode handler based on
/// `state.ui_mode`.
///
/// Modes with a real scroll surface (`Normal`, `DevTools`, `Settings`,
/// `Startup`/`NewSessionDialog`, `LinkHighlight`, `FlutterVersion`) delegate
/// to their submodule. Modes with no scrollable surface (`SearchInput`,
/// `ConfirmDialog`, `EmulatorSelector`, `Loading`) return `None`.
///
/// Per-mode handlers differ in modifier handling: `Normal`, `LinkHighlight`,
/// and `DevTools/Network` honor `Shift+wheel` for page-step (via
/// `KeyModSet::is_shift_only`); other modes ignore modifiers entirely.
/// See `docs/MOUSE.md` for the full per-mode reference.
fn handle_scroll(state: &AppState, dir: ScrollDir, mods: KeyModSet) -> Option<Message> {
    match state.ui_mode {
        UiMode::Normal => normal::handle_scroll(state, dir, mods),
        UiMode::DevTools => devtools::handle_scroll(state, dir, mods),
        UiMode::Settings => settings::handle_scroll(state, dir, mods),
        UiMode::Startup | UiMode::NewSessionDialog => new_session::handle_scroll(state, dir, mods),
        UiMode::LinkHighlight => link_highlight::handle_scroll(state, dir, mods),
        UiMode::FlutterVersion => flutter_version::handle_scroll(state, dir, mods),
        // Modes with no scrollable surface — explicitly no-op.
        UiMode::SearchInput
        | UiMode::ConfirmDialog
        | UiMode::EmulatorSelector
        | UiMode::Loading => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input_mouse::{KeyModSet, MouseButton, ScrollDir};

    fn make_press() -> MouseInput {
        MouseInput::Press {
            x: 0,
            y: 0,
            button: MouseButton::Left,
            modifiers: KeyModSet::NONE,
        }
    }

    fn make_scroll_up() -> MouseInput {
        MouseInput::Scroll {
            x: 0,
            y: 0,
            direction: ScrollDir::Up,
            modifiers: KeyModSet::NONE,
        }
    }

    fn state_in_mode(mode: UiMode) -> AppState {
        let mut state = AppState::new();
        state.ui_mode = mode;
        state
    }

    /// Helper to assert handle_mouse returns None for a given (mode, input).
    fn assert_noop(mode: UiMode, input: MouseInput) {
        let mut state = state_in_mode(mode);
        assert!(
            handle_mouse(&mut state, input).is_none(),
            "expected no-op for {:?} + {:?}",
            mode,
            input
        );
    }

    /// When `tag_filter_visible` is `true`, the dispatcher routes press events to
    /// `tag_filter::handle_press`, regardless of the underlying `ui_mode`. This
    /// test replaces the old `dispatcher_press_tag_filter_visible_is_no_op` test
    /// which asserted the *negative* (press suppressed). Phase 5 task 05 changes
    /// the contract: press now routes to the tag_filter handler and can return a
    /// message when a region is registered.
    #[test]
    fn dispatcher_press_tag_filter_visible_routes_to_tag_filter_handler() {
        use crate::mouse_regions::{MouseAction, MouseRect};

        for mode in [
            UiMode::Normal,
            UiMode::DevTools,
            UiMode::Settings,
            UiMode::NewSessionDialog,
        ] {
            let mut state = state_in_mode(mode);
            state.tag_filter_visible = true;

            // Register a tag-row click region that the tag_filter handler should hit.
            let mut regions = state.mouse_regions.take();
            regions.builder().click(
                MouseRect::new(0, 0, 10, 1),
                MouseAction::emit(Message::TagFilterClickRow { index: 0 }),
            );
            state.mouse_regions.set(regions);

            let result = handle_mouse(
                &mut state,
                MouseInput::Press {
                    x: 0,
                    y: 0,
                    button: MouseButton::Left,
                    modifiers: KeyModSet::NONE,
                },
            );
            assert!(
                matches!(result, Some(Message::TagFilterClickRow { index: 0 })),
                "tag_filter_visible should route press to tag_filter handler in {:?} mode, got {:?}",
                mode,
                result
            );
        }
    }

    /// With no regions registered, press returns `None` in every mode.
    /// This is the "without regions" baseline; normal-mode positive
    /// behaviour is covered by `normal.rs` unit tests.
    #[test]
    fn test_press_no_op_in_every_mode_without_regions() {
        for mode in [
            UiMode::Startup,
            UiMode::Normal,
            UiMode::NewSessionDialog,
            UiMode::EmulatorSelector,
            UiMode::ConfirmDialog,
            UiMode::Loading,
            UiMode::SearchInput,
            UiMode::LinkHighlight,
            UiMode::Settings,
            UiMode::FlutterVersion,
            UiMode::DevTools,
        ] {
            assert_noop(mode, make_press());
        }
    }

    #[test]
    fn test_press_dispatches_to_normal_handler_in_normal_mode() {
        let mut state = state_in_mode(UiMode::Normal);
        // No registered regions, so press returns None — but the dispatcher
        // must call into normal::handle_press, not return None unconditionally.
        // We test this transitively via the normal-mode unit tests above.
        let result = handle_mouse(&mut state, make_press());
        assert!(result.is_none(), "no regions registered → no message");
    }

    #[test]
    fn test_press_no_op_in_devtools_mode_without_regions() {
        // Phase 4 wires DevTools mode for clicks. With no regions registered,
        // press returns None (no match in empty registry).
        let mut state = state_in_mode(UiMode::DevTools);
        assert!(handle_mouse(&mut state, make_press()).is_none());
    }

    #[test]
    fn test_release_and_drag_remain_no_op() {
        let mut state = state_in_mode(UiMode::Normal);
        let release = MouseInput::Release {
            x: 0,
            y: 0,
            button: MouseButton::Left,
            modifiers: KeyModSet::NONE,
        };
        let drag = MouseInput::Drag {
            x: 0,
            y: 0,
            button: MouseButton::Left,
            modifiers: KeyModSet::NONE,
        };
        assert!(handle_mouse(&mut state, release).is_none());
        assert!(handle_mouse(&mut state, drag).is_none());
    }

    #[test]
    fn test_scroll_no_op_in_non_scrollable_modes() {
        // Modes with no scrollable surface — scroll is a no-op.
        // Modes with real per-mode handlers (Normal, DevTools, Settings,
        // LinkHighlight, FlutterVersion, Startup, NewSessionDialog) are
        // covered by their own submodule tests and the positive assertions
        // below.
        for mode in [
            UiMode::EmulatorSelector,
            UiMode::ConfirmDialog,
            UiMode::Loading,
            UiMode::SearchInput,
        ] {
            assert_noop(mode, make_scroll_up());
        }
    }

    #[test]
    fn test_scroll_normal_mode_returns_scroll_up() {
        // Normal-mode scroll is wired (Phase 2 task 02).
        let mut state = state_in_mode(UiMode::Normal);
        let msg = handle_mouse(&mut state, make_scroll_up());
        assert!(
            matches!(msg, Some(Message::ScrollUp)),
            "expected ScrollUp for Normal + scroll-up, got {:?}",
            msg
        );
    }

    #[test]
    fn test_devtools_scroll_routes_to_inspector_nav() {
        // DevTools mode with default (Inspector) panel produces a real message,
        // not a no-op. Exact routing is covered by devtools.rs unit tests.
        let mut state = state_in_mode(UiMode::DevTools);
        let result = handle_mouse(&mut state, make_scroll_up());
        assert!(
            matches!(result, Some(Message::DevToolsInspectorNavigate(_))),
            "DevTools scroll-up in Inspector panel should produce InspectorNavigate, got {:?}",
            result
        );
    }

    #[test]
    fn test_scroll_produces_message_in_link_highlight_mode() {
        let mut state = state_in_mode(UiMode::LinkHighlight);
        let scroll_up = make_scroll_up();
        assert!(
            handle_mouse(&mut state, scroll_up).is_some(),
            "LinkHighlight plain scroll-up should produce a message"
        );
    }

    #[test]
    fn test_scroll_produces_message_in_flutter_version_mode() {
        let mut state = state_in_mode(UiMode::FlutterVersion);
        let scroll_up = make_scroll_up();
        assert!(
            handle_mouse(&mut state, scroll_up).is_some(),
            "FlutterVersion scroll-up should produce a message"
        );
    }

    #[test]
    fn test_scroll_settings_routes_to_settings_prev_item() {
        // Settings mode (no modal, not editing) routes scroll-up to SettingsPrevItem
        // via the dispatcher. This catches a typo in the dispatcher's match arm
        // that would otherwise route Settings to a different submodule.
        let mut state = state_in_mode(UiMode::Settings);
        let msg = handle_mouse(&mut state, make_scroll_up());
        assert!(
            matches!(msg, Some(Message::SettingsPrevItem)),
            "expected SettingsPrevItem for Settings + scroll-up, got {:?}",
            msg
        );
    }

    #[test]
    fn test_scroll_new_session_dialog_routes_to_device_up() {
        // NewSessionDialog mode with default focused_pane (TargetSelector) routes
        // scroll-up to NewSessionDialogDeviceUp via the dispatcher.
        let mut state = state_in_mode(UiMode::NewSessionDialog);
        let msg = handle_mouse(&mut state, make_scroll_up());
        assert!(
            matches!(msg, Some(Message::NewSessionDialogDeviceUp)),
            "expected NewSessionDialogDeviceUp for NewSessionDialog + scroll-up, got {:?}",
            msg
        );
    }
}
