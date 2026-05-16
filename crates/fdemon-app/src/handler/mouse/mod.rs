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
use crate::state::{AppState, ToastLevel, UiMode};

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
/// Right-click is handled first, uniformly across all modes, by
/// [`handle_right_click`]: if the coordinates land on a log-row region, the
/// handler emits [`Message::CopyLogEntryToClipboard`]; otherwise it pushes a
/// dedup hint toast and returns `None`. This precedes both the tag-filter gate
/// and the mode dispatch, so every mode receives the same right-click behaviour
/// without per-mode changes.
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
    // ── Right-click: copy log line or show hint toast (log-text-selection fix) ──
    // Handled uniformly above the tag-filter and mode dispatch so that every
    // UI mode receives the same right-click behaviour without per-mode changes.
    if button == MouseButton::Right {
        return handle_right_click(state, x, y);
    }

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

/// Hint text shown when right-clicking outside a log row.
///
/// Kept as a named constant so the toast-push site and the dedup check
/// reference the same string literal, and so tests can import it without
/// embedding a magic string.
pub(crate) const RIGHT_CLICK_HINT: &str = "Right-click copies log lines; nothing to copy here.";

/// Handle a right-click uniformly across all UI modes.
///
/// ## Option B hit-test rewrite
///
/// The [`crate::mouse_regions::MouseRegions`] registry does not store an
/// `on_right` action (Option A). Instead, Option B is used: the registry is
/// queried with [`MouseButton::Left`] at the same coordinates and z-ordering.
/// If the resulting action resolves to [`Message::ClickLogRow`], the message
/// is rewritten to [`Message::CopyLogEntryToClipboard`]. Any other hit (or a
/// miss) falls through to the hint toast.
///
/// ## Dedup
///
/// Before pushing the fallback toast, [`AppState::toasts`] is scanned for an
/// existing entry with the same text. If one is already present the toast is
/// not pushed again — rapid right-clicks on empty space do not stack the
/// same message.
fn handle_right_click(state: &mut AppState, x: u16, y: u16) -> Option<Message> {
    // EXCEPTION (TEA): mouse_regions is a render-hint cell. See docs/CODE_STANDARDS.md
    // "Region Registry Pattern" and docs/REVIEW_FOCUS.md approved-exceptions list.
    let regions = state.mouse_regions.take_guard();
    let left_msg = regions
        .hit_test(x, y, MouseButton::Left)
        .and_then(|entry| entry.on_left.as_ref())
        .map(|a| a.resolve(x, y));
    // Put the registry back before any mutable state access below.
    drop(regions);

    if let Some(Message::ClickLogRow { entry_id, .. }) = left_msg {
        return Some(Message::CopyLogEntryToClipboard { entry_id });
    }

    // Fallback: dedup-push hint toast.
    if !state.toasts.iter().any(|t| t.text == RIGHT_CLICK_HINT) {
        state.push_toast(ToastLevel::Info, RIGHT_CLICK_HINT);
    }
    None
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

    // ── Right-click tests (log-text-selection-broken fix, Task 04) ────────────

    fn make_right_press(x: u16, y: u16) -> MouseInput {
        MouseInput::Press {
            x,
            y,
            button: MouseButton::Right,
            modifiers: KeyModSet::NONE,
        }
    }

    /// Right-click over a registered log-row region emits `CopyLogEntryToClipboard`
    /// with the correct `entry_id`.
    #[test]
    fn test_right_click_on_log_row_emits_copy_message() {
        use crate::mouse_regions::{MouseAction, MouseRect};

        let mut state = state_in_mode(UiMode::Normal);
        let entry_id: u64 = 42;

        // Register a log-row region (same as the TUI would during render).
        let mut regions = state.mouse_regions.take();
        regions.builder().click(
            MouseRect::new(0, 5, 80, 1),
            MouseAction::emit(Message::ClickLogRow {
                entry_id,
                frame_index: None,
            }),
        );
        state.mouse_regions.set(regions);

        let result = handle_mouse(&mut state, make_right_press(10, 5));
        assert!(
            matches!(result, Some(Message::CopyLogEntryToClipboard { entry_id: id }) if id == entry_id),
            "right-click on log row should emit CopyLogEntryToClipboard {{ entry_id: {} }}, got {:?}",
            entry_id,
            result
        );
    }

    /// Right-click outside any log region pushes the hint toast and returns `None`.
    #[test]
    fn test_right_click_off_log_row_pushes_toast() {
        let mut state = state_in_mode(UiMode::Normal);
        // No regions registered.
        let result = handle_mouse(&mut state, make_right_press(50, 10));
        assert!(result.is_none(), "no log region → None");
        assert_eq!(
            state.toasts.len(),
            1,
            "one hint toast should be pushed when right-clicking off a log row"
        );
        assert_eq!(state.toasts[0].text, RIGHT_CLICK_HINT);
    }

    /// Right-click fallback toast also fires when in Settings mode (no log rows visible).
    #[test]
    fn test_right_click_in_settings_mode_pushes_toast() {
        let mut state = state_in_mode(UiMode::Settings);
        let result = handle_mouse(&mut state, make_right_press(0, 0));
        assert!(result.is_none(), "Settings mode right-click → None");
        assert_eq!(state.toasts.len(), 1, "hint toast pushed in Settings mode");
        assert_eq!(state.toasts[0].text, RIGHT_CLICK_HINT);
    }

    /// Two consecutive right-clicks off log rows result in exactly one toast (dedup).
    #[test]
    fn test_right_click_dedup() {
        let mut state = state_in_mode(UiMode::Normal);

        // First right-click: toast is pushed.
        let _ = handle_mouse(&mut state, make_right_press(0, 0));
        assert_eq!(state.toasts.len(), 1, "first right-click pushes one toast");

        // Second right-click: dedup prevents a second identical toast.
        let _ = handle_mouse(&mut state, make_right_press(0, 0));
        assert_eq!(
            state.toasts.len(),
            1,
            "second right-click must not duplicate the same toast"
        );
    }
}
