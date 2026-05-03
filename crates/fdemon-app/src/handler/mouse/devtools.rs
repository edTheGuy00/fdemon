//! Scroll routing for `UiMode::DevTools`.
//!
//! Dispatches by `state.devtools_view_state.active_panel`:
//! - Inspector → tree row navigation (Up/Down only; no page step)
//! - Performance → no-op (frame timeline is keyboard Left/Right only)
//! - Network → request-list navigation (Up/Down; Shift → PageUp/PageDown);
//!   no-op when filter input is active

use crate::input_mouse::{KeyModSet, ScrollDir};
use crate::message::{InspectorNav, Message, NetworkNav};
use crate::state::{AppState, DevToolsPanel};

pub(super) fn handle_scroll(state: &AppState, dir: ScrollDir, mods: KeyModSet) -> Option<Message> {
    match state.devtools_view_state.active_panel {
        DevToolsPanel::Inspector => handle_inspector_scroll(dir, mods),
        DevToolsPanel::Performance => None,
        DevToolsPanel::Network => handle_network_scroll(state, dir, mods),
    }
}

fn handle_inspector_scroll(dir: ScrollDir, mods: KeyModSet) -> Option<Message> {
    // Inspector has no page-step navigation — Shift+wheel falls back to a
    // single-step move rather than no-op (small UX win for shift-held scrolls).
    // Ctrl/Alt with no Shift returns None as in normal mode.
    if !mods.shift && (mods.ctrl || mods.alt) {
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
