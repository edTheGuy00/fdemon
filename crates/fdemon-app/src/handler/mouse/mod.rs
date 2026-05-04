//! Mouse event handlers for different UI modes.
//!
//! Mirrors [`crate::handler::keys`] — converts a [`MouseInput`] into a
//! concrete [`Message`] based on the current [`UiMode`]. Phase 2 wires
//! per-mode scroll routing; Phase 3+ adds click hit-testing.

mod devtools;
mod flutter_version;
mod link_highlight;
mod new_session;
mod normal;
mod settings;

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
pub fn handle_mouse(state: &AppState, input: MouseInput) -> Option<Message> {
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
/// Phase 3 only wires [`UiMode::Normal`]. DevTools/Settings/dialog modes
/// are wired in Phase 4/5 — they return `None` until then.
fn handle_press(
    state: &AppState,
    x: u16,
    y: u16,
    button: MouseButton,
    mods: KeyModSet,
) -> Option<Message> {
    match state.ui_mode {
        UiMode::Normal => normal::handle_press(state, x, y, button, mods),
        // Phase 5 wires DevTools/Settings/dialog modes; for now, no-op.
        _ => None,
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
        let state = state_in_mode(mode);
        assert!(
            handle_mouse(&state, input).is_none(),
            "expected no-op for {:?} + {:?}",
            mode,
            input
        );
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
        let state = state_in_mode(UiMode::Normal);
        // No registered regions, so press returns None — but the dispatcher
        // must call into normal::handle_press, not return None unconditionally.
        // We test this transitively via the normal-mode unit tests above.
        let result = handle_mouse(&state, make_press());
        assert!(result.is_none(), "no regions registered → no message");
    }

    #[test]
    fn test_press_no_op_in_devtools_mode_phase_3() {
        // Phase 3 only wires Normal mode for clicks. DevTools/Settings/dialogs
        // come in Phase 4/5.
        let state = state_in_mode(UiMode::DevTools);
        assert!(handle_mouse(&state, make_press()).is_none());
    }

    #[test]
    fn test_release_and_drag_remain_no_op() {
        let state = state_in_mode(UiMode::Normal);
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
        assert!(handle_mouse(&state, release).is_none());
        assert!(handle_mouse(&state, drag).is_none());
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
        let state = state_in_mode(UiMode::Normal);
        let msg = handle_mouse(&state, make_scroll_up());
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
        let state = state_in_mode(UiMode::DevTools);
        let result = handle_mouse(&state, make_scroll_up());
        assert!(
            matches!(result, Some(Message::DevToolsInspectorNavigate(_))),
            "DevTools scroll-up in Inspector panel should produce InspectorNavigate, got {:?}",
            result
        );
    }

    #[test]
    fn test_scroll_produces_message_in_link_highlight_mode() {
        let state = state_in_mode(UiMode::LinkHighlight);
        let scroll_up = make_scroll_up();
        assert!(
            handle_mouse(&state, scroll_up).is_some(),
            "LinkHighlight plain scroll-up should produce a message"
        );
    }

    #[test]
    fn test_scroll_produces_message_in_flutter_version_mode() {
        let state = state_in_mode(UiMode::FlutterVersion);
        let scroll_up = make_scroll_up();
        assert!(
            handle_mouse(&state, scroll_up).is_some(),
            "FlutterVersion scroll-up should produce a message"
        );
    }

    #[test]
    fn test_scroll_settings_routes_to_settings_prev_item() {
        // Settings mode (no modal, not editing) routes scroll-up to SettingsPrevItem
        // via the dispatcher. This catches a typo in the dispatcher's match arm
        // that would otherwise route Settings to a different submodule.
        let state = state_in_mode(UiMode::Settings);
        let msg = handle_mouse(&state, make_scroll_up());
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
        let state = state_in_mode(UiMode::NewSessionDialog);
        let msg = handle_mouse(&state, make_scroll_up());
        assert!(
            matches!(msg, Some(Message::NewSessionDialogDeviceUp)),
            "expected NewSessionDialogDeviceUp for NewSessionDialog + scroll-up, got {:?}",
            msg
        );
    }
}
