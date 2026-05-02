## Task: DevTools-mode scroll routing (Inspector / Performance / Network)

**Objective**: Implement `crates/fdemon-app/src/handler/mouse/devtools.rs::handle_scroll` so the wheel navigates the active DevTools panel: Inspector tree (line-step), Network request list (line-step + Shift page-step), and explicitly no-op on the Performance frame timeline. When the Network filter input is active the wheel is a no-op (mirrors keyboard handler at `keys.rs:418-425`).

**Depends on**: 01-mouse-handler-restructure

**Estimated Time**: 1.5h

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/handler/mouse/devtools.rs` — Replace stub `handle_scroll` body; add `#[cfg(test)] mod tests`.

**Files Read (Dependencies):**
- `crates/fdemon-app/src/state.rs` — `DevToolsPanel` enum (line 127-137); `state.devtools_view_state.active_panel`.
- `crates/fdemon-app/src/session/network.rs` — `NetworkState::filter_input_active` flag (referenced at `keys.rs:411-415`).
- `crates/fdemon-app/src/message.rs` — `Message::DevToolsInspectorNavigate(InspectorNav)`, `Message::NetworkNavigate(NetworkNav)`, the `InspectorNav::{Up, Down}` and `NetworkNav::{Up, Down, PageUp, PageDown}` variants.
- `crates/fdemon-app/src/input_mouse.rs` — `KeyModSet::is_shift_only` (added in Task 01).
- `crates/fdemon-app/src/handler/keys.rs` — Reference: `handle_key_devtools` lines 401-592, especially the filter-input gate at 410-426, the Network nav block at 484-492, and the Inspector nav block at 537-548.

### Details

```rust
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

pub(super) fn handle_scroll(
    state: &AppState,
    dir: ScrollDir,
    mods: KeyModSet,
) -> Option<Message> {
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

fn handle_network_scroll(
    state: &AppState,
    dir: ScrollDir,
    mods: KeyModSet,
) -> Option<Message> {
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
```

### Acceptance Criteria

1. `Inspector` panel + `ScrollDir::Up`, no modifiers → `Some(Message::DevToolsInspectorNavigate(InspectorNav::Up))`.
2. `Inspector` panel + `ScrollDir::Down`, no modifiers → `Some(Message::DevToolsInspectorNavigate(InspectorNav::Down))`.
3. `Performance` panel + any wheel direction/modifier → `None`.
4. `Network` panel + `Up` no modifiers → `Some(Message::NetworkNavigate(NetworkNav::Up))`.
5. `Network` panel + `Down` no modifiers → `Some(Message::NetworkNavigate(NetworkNav::Down))`.
6. `Network` panel + `Up` + Shift-only → `Some(Message::NetworkNavigate(NetworkNav::PageUp))`.
7. `Network` panel + `Down` + Shift-only → `Some(Message::NetworkNavigate(NetworkNav::PageDown))`.
8. `Network` panel + filter input active + any wheel direction/modifier → `None`.
9. `Network` panel + Ctrl-only or Alt-only → `None` (consistent with Normal mode).
10. `ScrollDir::Left` / `ScrollDir::Right` → `None` for every panel/state.
11. No new `Message` variants introduced.

### Testing

```rust
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

    // Network filter-input gate test requires constructing a SessionHandle with
    // network.filter_input_active=true. Use the same construction pattern that
    // existing handler::devtools::network::tests uses (see e.g. crates/fdemon-app/
    // src/handler/devtools/network.rs tests for the helper).
    //
    // Pseudocode:
    //   let mut s = state_with_panel(DevToolsPanel::Network);
    //   s.session_manager.add(... session with network.filter_input_active = true ...);
    //   assert!(handle_scroll(&s, ScrollDir::Up, KeyModSet::NONE).is_none());
    #[test]
    fn network_filter_active_swallows_scroll() {
        // Implementor: wire a session with filter_input_active=true via the
        // existing test helpers in crates/fdemon-app/src/session/network.rs.
        // The intent is: when filter input is taking keyboard focus, the
        // wheel must not move the request selection underneath it.
    }

    #[test]
    fn ctrl_or_alt_only_is_no_op_in_inspector_and_network() {
        let inspector = state_with_panel(DevToolsPanel::Inspector);
        let network = state_with_panel(DevToolsPanel::Network);
        for s in [&inspector, &network] {
            assert!(
                handle_scroll(s, ScrollDir::Up, KeyModSet::new(false, true, false)).is_none()
            );
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
```

The Network filter-active test is sketched as a TODO because constructing a `SessionHandle` with `network.filter_input_active = true` requires the same builder/test-helper pattern used by existing `crates/fdemon-app/src/handler/devtools/network.rs` tests. Implementor should grep for `filter_input_active = true` in test code and reuse the helper.

### Notes

- **No `Inspector` PageUp/PageDown.** `InspectorNav` (`message.rs:33-38`) only has `Up`, `Down`, `Expand`, `Collapse` — no page step. The plan explicitly says: "DevTools Inspector: `InspectorNav::Up`/`Down`" with no Shift handling. Adding a new `InspectorNav` variant is out of Phase 2 scope; Shift+wheel here falls back to single-step move (small UX win) rather than `None`.
- **Performance panel scroll is intentional no-op.** PLAN.md Phase 2 step 1 says "DevTools Performance: no-op (frame timeline is keyboard-arrow only)". The keyboard handler at `keys.rs:568-579` only binds Left/Right for frame navigation; there is no up/down navigation concept on the Performance bar chart.
- **Filter-input gate must check `selected_session`.** `keys.rs:411-415` reads `selected().map(|h| h.session.network.filter_input_active).unwrap_or(false)`. The mouse handler must follow the same path; if no session is selected, `filter_active` is false and scroll proceeds normally.
- **Drop precedence within filter-input gate.** Even Shift+wheel returns `None` when filter is active — the user is editing text, the wheel must not move the table underneath the cursor.
- **Touchpad horizontal.** Per PLAN.md "Out of scope": `ScrollLeft`/`ScrollRight` are no-ops. Network has no horizontal scroll concept; Inspector tree could in theory scroll horizontally for deeply-nested trees, but no consumer exists today.
