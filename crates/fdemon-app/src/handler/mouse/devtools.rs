//! Mouse event handlers for `UiMode::DevTools`.
//!
//! **Scroll** dispatches by `state.devtools_view_state.active_panel`:
//! - Inspector → tree row navigation (Up/Down with no modifiers; any modifier
//!   returns None because there is no page-step analogue for the inspector tree)
//! - Performance → no-op (frame timeline is keyboard Left/Right only)
//! - Network → request-list navigation (Up/Down; Shift → PageUp/PageDown);
//!   no-op when filter input is active
//!
//! **Press** hit-tests against the per-frame region registry (populated during
//! `render::view()`). The Network filter-input gate drops clicks while the user
//! is typing a filter pattern.

use crate::input_mouse::{KeyModSet, MouseButton, ScrollDir};
use crate::message::{InspectorNav, Message, NetworkNav};
use crate::state::{AppState, DevToolsPanel};

/// Hit-test a left/middle click in `UiMode::DevTools` against the per-frame
/// region registry. Returns the matched region's resolved [`Message`].
///
/// **Filter-input gate.** When the Network panel's filter input is active
/// (the user is typing a filter pattern), non-tab clicks are silently
/// dropped — mirroring [`handle_network_scroll`]'s behaviour. However,
/// clicks on the DevTools sub-tab bar (`[i]/[p]/[n]`) that resolve to
/// [`Message::SwitchDevToolsPanel`] are **not** suppressed; they escape the
/// filter, switch the panel, and also clear `filter_input_active` so the
/// user is never trapped in filter mode by mouse-only interaction.
///
/// **Right-click reserved.** As in [`normal::handle_press`], right-click
/// returns `None` for future context-menu support.
///
/// [`normal::handle_press`]: super::normal::handle_press
/// [`handle_network_scroll`]: handle_network_scroll
pub(super) fn handle_press(
    state: &mut AppState,
    x: u16,
    y: u16,
    button: MouseButton,
    _mods: KeyModSet,
) -> Option<Message> {
    // Right-click reserved.
    if button == MouseButton::Right {
        return None;
    }

    // ── Hit-test against the registry ────────────────────────────────────
    // EXCEPTION: TEA render-hint write-back via Cell — see docs/CODE_STANDARDS.md Principle 3
    // Guard puts the registry back on Drop, including on early-return paths.
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

    let message = action_opt?;

    // Filter-input gate (Network panel only).
    //
    // When the user is typing a filter pattern, suppress all clicks except
    // those that switch to a different DevTools panel. A sub-tab bar click
    // (`[i]/[p]/[n]`) while filter input is active escapes the filter: we
    // clear `filter_input_active` here so the caller's `SwitchDevToolsPanel`
    // handler does not need to know about the click context.
    if state.devtools_view_state.active_panel == DevToolsPanel::Network {
        let filter_active = state
            .session_manager
            .selected()
            .map(|h| h.session.network.filter_input_active)
            .unwrap_or(false);
        if filter_active {
            if matches!(message, Message::SwitchDevToolsPanel(_)) {
                // Sub-tab click escapes the filter. Clear filter input mode as
                // part of the click action so the user is not trapped.
                if let Some(handle) = state.session_manager.selected_mut() {
                    handle.session.network.filter_input_active = false;
                }
            } else {
                return None; // Suppress non-tab clicks while filter is active.
            }
        }
    }

    Some(message)
}

pub(super) fn handle_scroll(state: &AppState, dir: ScrollDir, mods: KeyModSet) -> Option<Message> {
    match state.devtools_view_state.active_panel {
        DevToolsPanel::Inspector => handle_inspector_scroll(dir, mods),
        DevToolsPanel::Performance => None,
        DevToolsPanel::Network => handle_network_scroll(state, dir, mods),
    }
}

fn handle_inspector_scroll(dir: ScrollDir, mods: KeyModSet) -> Option<Message> {
    // Inspector has no page-step navigation — there is no `InspectorNav::PageUp`
    // analogue. Any modifier combination (including Shift, Ctrl, Alt) returns
    // None for parity with normal.rs / link_highlight.rs / handle_network_scroll.
    if mods.shift || mods.ctrl || mods.alt {
        return None;
    }
    match dir {
        ScrollDir::Up => Some(Message::DevToolsInspectorNavigate(InspectorNav::Up)),
        ScrollDir::Down => Some(Message::DevToolsInspectorNavigate(InspectorNav::Down)),
        ScrollDir::Left | ScrollDir::Right => None,
    }
}

fn handle_network_scroll(state: &AppState, dir: ScrollDir, mods: KeyModSet) -> Option<Message> {
    // Filter input mode swallows scroll, mirroring keys.rs:417-425 which
    // routes only Esc/Enter/Backspace/Char into the filter buffer.
    let filter_active = state
        .session_manager
        .selected()
        .map(|h| h.session.network.filter_input_active)
        .unwrap_or(false);
    if filter_active {
        return None;
    }

    if mods.is_shift_only() {
        return match dir {
            ScrollDir::Up => Some(Message::NetworkNavigate(NetworkNav::PageUp)),
            ScrollDir::Down => Some(Message::NetworkNavigate(NetworkNav::PageDown)),
            ScrollDir::Left | ScrollDir::Right => None,
        };
    }

    if mods.ctrl || mods.alt {
        return None;
    }

    match dir {
        ScrollDir::Up => Some(Message::NetworkNavigate(NetworkNav::Up)),
        ScrollDir::Down => Some(Message::NetworkNavigate(NetworkNav::Down)),
        ScrollDir::Left | ScrollDir::Right => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input_mouse::KeyModSet;
    use crate::state::{AppState, DevToolsPanel};

    fn state_with_panel(panel: DevToolsPanel) -> AppState {
        let mut s = AppState::new();
        s.devtools_view_state.active_panel = panel;
        s
    }

    #[test]
    fn inspector_wheel_up_navigates_inspector_up() {
        let s = state_with_panel(DevToolsPanel::Inspector);
        let msg = handle_scroll(&s, ScrollDir::Up, KeyModSet::NONE);
        assert!(matches!(
            msg,
            Some(Message::DevToolsInspectorNavigate(InspectorNav::Up))
        ));
    }

    #[test]
    fn inspector_wheel_down_navigates_inspector_down() {
        let s = state_with_panel(DevToolsPanel::Inspector);
        let msg = handle_scroll(&s, ScrollDir::Down, KeyModSet::NONE);
        assert!(matches!(
            msg,
            Some(Message::DevToolsInspectorNavigate(InspectorNav::Down))
        ));
    }

    #[test]
    fn performance_wheel_is_always_none() {
        let s = state_with_panel(DevToolsPanel::Performance);
        for dir in [ScrollDir::Up, ScrollDir::Down] {
            for mods in [
                KeyModSet::NONE,
                KeyModSet::new(true, false, false),
                KeyModSet::new(false, true, false),
            ] {
                assert!(handle_scroll(&s, dir, mods).is_none());
            }
        }
    }

    #[test]
    fn network_wheel_navigates_request_list() {
        let s = state_with_panel(DevToolsPanel::Network);
        assert!(matches!(
            handle_scroll(&s, ScrollDir::Up, KeyModSet::NONE),
            Some(Message::NetworkNavigate(NetworkNav::Up))
        ));
        assert!(matches!(
            handle_scroll(&s, ScrollDir::Down, KeyModSet::NONE),
            Some(Message::NetworkNavigate(NetworkNav::Down))
        ));
    }

    #[test]
    fn network_shift_wheel_pages() {
        let s = state_with_panel(DevToolsPanel::Network);
        let mods = KeyModSet::new(true, false, false);
        assert!(matches!(
            handle_scroll(&s, ScrollDir::Up, mods),
            Some(Message::NetworkNavigate(NetworkNav::PageUp))
        ));
        assert!(matches!(
            handle_scroll(&s, ScrollDir::Down, mods),
            Some(Message::NetworkNavigate(NetworkNav::PageDown))
        ));
    }

    #[test]
    fn network_filter_active_swallows_scroll() {
        use fdemon_daemon::Device;

        fn test_device() -> Device {
            Device {
                id: "test-device".to_string(),
                name: "Test Device".to_string(),
                platform: "android".to_string(),
                emulator: false,
                category: None,
                platform_type: None,
                ephemeral: false,
                emulator_id: None,
            }
        }

        let mut s = state_with_panel(DevToolsPanel::Network);
        let device = test_device();
        let _session_id = s.session_manager.create_session(&device).unwrap();
        s.session_manager
            .selected_mut()
            .unwrap()
            .session
            .network
            .filter_input_active = true;

        // Every direction/modifier must be a no-op while filter input is active.
        for dir in [
            ScrollDir::Up,
            ScrollDir::Down,
            ScrollDir::Left,
            ScrollDir::Right,
        ] {
            for mods in [
                KeyModSet::NONE,
                KeyModSet::new(true, false, false),
                KeyModSet::new(false, true, false),
                KeyModSet::new(false, false, true),
            ] {
                assert!(
                    handle_scroll(&s, dir, mods).is_none(),
                    "expected no-op for {:?} {:?} when filter input active",
                    mods,
                    dir
                );
            }
        }
    }

    #[test]
    fn ctrl_or_alt_only_is_no_op_in_inspector_and_network() {
        let inspector = state_with_panel(DevToolsPanel::Inspector);
        let network = state_with_panel(DevToolsPanel::Network);
        for s in [&inspector, &network] {
            assert!(handle_scroll(s, ScrollDir::Up, KeyModSet::new(false, true, false)).is_none());
            assert!(
                handle_scroll(s, ScrollDir::Down, KeyModSet::new(false, false, true)).is_none()
            );
        }
    }

    #[test]
    fn inspector_any_modifier_combination_returns_none() {
        let s = state_with_panel(DevToolsPanel::Inspector);
        let combos = [
            KeyModSet::new(true, false, false), // Shift
            KeyModSet::new(false, true, false), // Ctrl
            KeyModSet::new(false, false, true), // Alt
            KeyModSet::new(true, true, false),  // Shift+Ctrl
            KeyModSet::new(true, false, true),  // Shift+Alt
            KeyModSet::new(false, true, true),  // Ctrl+Alt
            KeyModSet::new(true, true, true),   // Shift+Ctrl+Alt
        ];
        for mods in combos {
            for dir in [ScrollDir::Up, ScrollDir::Down] {
                assert!(
                    handle_scroll(&s, dir, mods).is_none(),
                    "expected None for Inspector + {:?} + {:?}",
                    dir,
                    mods
                );
            }
        }
    }

    #[test]
    fn horizontal_wheel_no_op_in_every_panel() {
        for panel in [
            DevToolsPanel::Inspector,
            DevToolsPanel::Performance,
            DevToolsPanel::Network,
        ] {
            let s = state_with_panel(panel);
            assert!(handle_scroll(&s, ScrollDir::Left, KeyModSet::NONE).is_none());
            assert!(handle_scroll(&s, ScrollDir::Right, KeyModSet::NONE).is_none());
        }
    }
}

#[cfg(test)]
mod press_tests {
    use super::*;
    use crate::input_mouse::{KeyModSet, MouseButton};
    use crate::message::Message;
    use crate::mouse_regions::{MouseAction, MouseRect};

    fn state_in_devtools_panel(panel: DevToolsPanel) -> AppState {
        let mut s = AppState::new();
        s.ui_mode = crate::state::UiMode::DevTools;
        s.devtools_view_state.active_panel = panel;
        s
    }

    #[test]
    fn left_click_on_recorded_region_returns_emit_message() {
        let mut state = state_in_devtools_panel(DevToolsPanel::Inspector);
        let mut regions = state.mouse_regions.take();
        regions.builder().click(
            MouseRect::new(0, 0, 10, 1),
            MouseAction::emit(Message::SwitchDevToolsPanel(DevToolsPanel::Performance)),
        );
        state.mouse_regions.set(regions);

        let result = handle_press(&mut state, 0, 0, MouseButton::Left, KeyModSet::NONE);
        assert!(matches!(
            result,
            Some(Message::SwitchDevToolsPanel(DevToolsPanel::Performance))
        ));
    }

    #[test]
    fn right_click_is_noop() {
        let mut state = state_in_devtools_panel(DevToolsPanel::Inspector);
        let mut regions = state.mouse_regions.take();
        regions.builder().click(
            MouseRect::new(0, 0, 10, 1),
            MouseAction::emit(Message::SwitchDevToolsPanel(DevToolsPanel::Performance)),
        );
        state.mouse_regions.set(regions);

        let result = handle_press(&mut state, 0, 0, MouseButton::Right, KeyModSet::NONE);
        assert!(result.is_none());
    }

    #[test]
    fn click_in_network_panel_with_filter_active_is_noop() {
        use fdemon_daemon::Device;

        let mut state = state_in_devtools_panel(DevToolsPanel::Network);
        let device = Device {
            id: "d".into(),
            name: "Dev".into(),
            platform: "android".into(),
            emulator: false,
            category: None,
            platform_type: None,
            ephemeral: false,
            emulator_id: None,
        };
        state.session_manager.create_session(&device).unwrap();
        state
            .session_manager
            .selected_mut()
            .unwrap()
            .session
            .network
            .filter_input_active = true;

        let mut regions = state.mouse_regions.take();
        regions.builder().click(
            MouseRect::new(0, 0, 10, 1),
            MouseAction::emit(Message::ToggleNetworkRecording),
        );
        state.mouse_regions.set(regions);

        let result = handle_press(&mut state, 0, 0, MouseButton::Left, KeyModSet::NONE);
        assert!(result.is_none(), "filter-active suppresses clicks");
    }

    #[test]
    fn click_in_inspector_with_network_filter_active_is_not_gated() {
        // Filter-active applies only to Network panel; clicks in
        // Inspector/Performance must still resolve.
        use fdemon_daemon::Device;

        let mut state = state_in_devtools_panel(DevToolsPanel::Inspector);
        let device = Device {
            id: "d".into(),
            name: "Dev".into(),
            platform: "android".into(),
            emulator: false,
            category: None,
            platform_type: None,
            ephemeral: false,
            emulator_id: None,
        };
        state.session_manager.create_session(&device).unwrap();
        state
            .session_manager
            .selected_mut()
            .unwrap()
            .session
            .network
            .filter_input_active = true; // unrelated to current panel

        let mut regions = state.mouse_regions.take();
        regions.builder().click(
            MouseRect::new(0, 0, 10, 1),
            MouseAction::emit(Message::SwitchDevToolsPanel(DevToolsPanel::Performance)),
        );
        state.mouse_regions.set(regions);

        let result = handle_press(&mut state, 0, 0, MouseButton::Left, KeyModSet::NONE);
        assert!(matches!(
            result,
            Some(Message::SwitchDevToolsPanel(DevToolsPanel::Performance))
        ));
    }

    #[test]
    fn click_outside_any_region_is_none() {
        let mut state = state_in_devtools_panel(DevToolsPanel::Inspector);
        let result = handle_press(&mut state, 100, 100, MouseButton::Left, KeyModSet::NONE);
        assert!(result.is_none());
    }

    #[test]
    fn middle_click_on_recorded_region_returns_middle_action() {
        use crate::message::InspectorNav;

        let mut state = state_in_devtools_panel(DevToolsPanel::Inspector);

        // Register a region with distinct left and middle actions.
        let mut regions = state.mouse_regions.take();
        regions.builder().click_left_middle(
            MouseRect::new(10, 5, 5, 1),
            MouseAction::emit(Message::DevToolsInspectorNavigate(InspectorNav::Down)),
            MouseAction::emit(Message::DevToolsInspectorNavigate(InspectorNav::Up)),
        );
        state.mouse_regions.set(regions);

        // Middle click must return the on_middle action (Up), not the on_left (Down).
        let result = handle_press(&mut state, 12, 5, MouseButton::Middle, KeyModSet::NONE);
        assert!(
            matches!(
                result,
                Some(Message::DevToolsInspectorNavigate(InspectorNav::Up))
            ),
            "middle click on a click_left_middle region must resolve on_middle, got {:?}",
            result
        );
    }

    #[test]
    fn network_filter_active_sub_tab_click_switches_panel_and_clears_filter() {
        use fdemon_daemon::Device;

        let mut state = state_in_devtools_panel(DevToolsPanel::Network);
        let device = Device {
            id: "d".into(),
            name: "Dev".into(),
            platform: "android".into(),
            emulator: false,
            category: None,
            platform_type: None,
            ephemeral: false,
            emulator_id: None,
        };
        state.session_manager.create_session(&device).unwrap();
        state
            .session_manager
            .selected_mut()
            .unwrap()
            .session
            .network
            .filter_input_active = true;

        // Register a SwitchDevToolsPanel region (simulates the sub-tab bar).
        let mut regions = state.mouse_regions.take();
        regions.builder().click(
            MouseRect::new(0, 0, 14, 1),
            MouseAction::emit(Message::SwitchDevToolsPanel(DevToolsPanel::Inspector)),
        );
        state.mouse_regions.set(regions);

        let result = handle_press(&mut state, 7, 0, MouseButton::Left, KeyModSet::NONE);
        assert!(
            matches!(
                result,
                Some(Message::SwitchDevToolsPanel(DevToolsPanel::Inspector))
            ),
            "sub-tab click must NOT be suppressed by filter gate, got {:?}",
            result
        );
        assert!(
            !state
                .session_manager
                .selected()
                .unwrap()
                .session
                .network
                .filter_input_active,
            "sub-tab click while filter active must clear filter_input_active"
        );
    }
}
