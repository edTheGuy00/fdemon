//! Mouse event handlers for different UI modes.
//!
//! Mirrors [`crate::handler::keys`] — converts a [`MouseInput`] into a
//! concrete [`Message`] based on the current [`UiMode`]. Phase 1 of the
//! mouse-support feature implements this as a no-op shell so events flow
//! into the engine without behavior changes; later phases populate per-mode
//! dispatch (scroll wheel, region hit-testing, dialog clicks).

use crate::input_mouse::MouseInput;
use crate::message::Message;
use crate::state::{AppState, UiMode};

/// Convert a mouse event to a follow-up message based on the current UI mode.
///
/// Returns `None` in Phase 1 — every variant is intentionally unhandled.
/// Phase 2 introduces scroll-wheel routing, Phase 3+ adds click hit-testing.
pub fn handle_mouse(state: &AppState, _input: MouseInput) -> Option<Message> {
    match state.ui_mode {
        UiMode::Startup
        | UiMode::Normal
        | UiMode::NewSessionDialog
        | UiMode::EmulatorSelector
        | UiMode::ConfirmDialog
        | UiMode::Loading
        | UiMode::SearchInput
        | UiMode::LinkHighlight
        | UiMode::Settings
        | UiMode::FlutterVersion
        | UiMode::DevTools => None,
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

    #[test]
    fn test_press_no_op_in_every_mode() {
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
    fn test_scroll_no_op_in_every_mode() {
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
            assert_noop(mode, make_scroll_up());
        }
    }
}
