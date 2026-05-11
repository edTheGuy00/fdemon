## Task: Mouse Press Dispatcher for `UiMode::DevTools`

**Objective**: Add `pub(super) fn handle_press(state, x, y, button, mods) -> Option<Message>` in `handler/mouse/devtools.rs`. Wire it into `handler/mouse/mod.rs::handle_press` so left and middle clicks in `UiMode::DevTools` consult the registry. The implementation mirrors `handler/mouse/normal.rs::handle_press` (RAII guard, hit-test, button match, action resolve) plus a Network-filter-input gate that drops clicks while the user is typing a filter.

**Depends on**: None (Wave 2 — independent of Tasks 01–04 because it only references existing types and the registry API).

**Estimated Time**: 0.75 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/handler/mouse/devtools.rs`: Add `handle_press` function alongside the existing `handle_scroll`. Add ≥ 4 unit tests.
- `crates/fdemon-app/src/handler/mouse/mod.rs`: Replace the `_ => None` arm of `handle_press` so `UiMode::DevTools` dispatches to `devtools::handle_press`. Other modes remain `None` until Phase 5.

**Files Read (Dependencies):**
- `crates/fdemon-app/src/handler/mouse/normal.rs::handle_press` (template for the take-guard + hit-test pattern, including right-click reservation)
- `crates/fdemon-app/src/mouse_regions.rs::MouseRegionsCell::take_guard` and `MouseRegions::hit_test`
- `crates/fdemon-app/src/state.rs::DevToolsViewState::active_panel`
- `crates/fdemon-app/src/handler/mouse/devtools.rs::handle_network_scroll` (template for the Network filter-active gate)

### Details

#### `mouse/mod.rs` change

Replace:

```rust
match state.ui_mode {
    UiMode::Normal => normal::handle_press(state, x, y, button, mods),
    // Phase 5 wires DevTools/Settings/dialog modes; for now, no-op.
    _ => None,
}
```

with:

```rust
match state.ui_mode {
    UiMode::Normal => normal::handle_press(state, x, y, button, mods),
    UiMode::DevTools => devtools::handle_press(state, x, y, button, mods),
    // Phase 5 wires Settings/dialog modes; for now, no-op.
    _ => None,
}
```

Update the doc comment on `handle_press` to mention `UiMode::DevTools` is now wired.

#### `handler/mouse/devtools.rs` `handle_press`

```rust
/// Hit-test a left/middle click in `UiMode::DevTools` against the per-frame
/// region registry. Returns the matched region's resolved [`Message`].
///
/// **Filter-input gate.** When the Network panel's filter input is active
/// (the user is typing a filter pattern), all clicks are silently
/// dropped — mirroring [`handle_network_scroll`]'s behaviour.
///
/// **Right-click reserved.** As in [`normal::handle_press`], right-click
/// returns `None` for future context-menu support.
pub(super) fn handle_press(
    state: &AppState,
    x: u16,
    y: u16,
    button: MouseButton,
    _mods: KeyModSet,
) -> Option<Message> {
    // Right-click reserved.
    if matches!(button, MouseButton::Right) {
        return None;
    }

    // Filter-input gate (Network panel only).
    if state.devtools_view_state.active_panel == DevToolsPanel::Network {
        let filter_active = state
            .session_manager
            .selected()
            .map(|h| h.session.network.filter_input_active)
            .unwrap_or(false);
        if filter_active {
            return None;
        }
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

    action_opt
}
```

Note: `DevToolsPanel` is imported via the existing `use crate::state::{AppState, DevToolsPanel};` at the top of `devtools.rs`.

### Acceptance Criteria

1. Pressing left button at coordinates that match a registered region in DevTools mode returns the region's emitted `Message`.
2. Right click in DevTools mode returns `None` regardless of registry contents.
3. Middle click hits a registered `click_left_middle` region's middle action.
4. Left click while `network.filter_input_active = true` and `active_panel == Network` returns `None` even if a region matches at those coordinates.
5. Filter-active gate does NOT apply when `active_panel == Inspector` or `Performance` — those panels do not have a filter input.
6. The `tag_filter_visible` gate at `mouse/mod.rs::handle_press` continues to short-circuit DevTools mode clicks (no behaviour change to that branch).
7. `mouse/mod.rs::dispatcher_press_tag_filter_visible_is_no_op` test continues to pass — it iterates over every `UiMode` including `DevTools`.
8. New tests in `mouse/devtools.rs` cover the four scenarios above. `cargo test --workspace`, `cargo fmt`, `cargo clippy -- -D warnings` pass.

### Testing

```rust
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
        let state = state_in_devtools_panel(DevToolsPanel::Inspector);
        let mut regions = state.mouse_regions.take();
        regions.builder().click(
            MouseRect::new(0, 0, 10, 1),
            MouseAction::emit(Message::SwitchDevToolsPanel(DevToolsPanel::Performance)),
        );
        state.mouse_regions.set(regions);

        let result = handle_press(&state, 0, 0, MouseButton::Left, KeyModSet::NONE);
        assert!(matches!(
            result,
            Some(Message::SwitchDevToolsPanel(DevToolsPanel::Performance))
        ));
    }

    #[test]
    fn right_click_is_noop() {
        let state = state_in_devtools_panel(DevToolsPanel::Inspector);
        let mut regions = state.mouse_regions.take();
        regions.builder().click(
            MouseRect::new(0, 0, 10, 1),
            MouseAction::emit(Message::SwitchDevToolsPanel(DevToolsPanel::Performance)),
        );
        state.mouse_regions.set(regions);

        let result = handle_press(&state, 0, 0, MouseButton::Right, KeyModSet::NONE);
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

        let result = handle_press(&state, 0, 0, MouseButton::Left, KeyModSet::NONE);
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

        let result = handle_press(&state, 0, 0, MouseButton::Left, KeyModSet::NONE);
        assert!(matches!(
            result,
            Some(Message::SwitchDevToolsPanel(DevToolsPanel::Performance))
        ));
    }

    #[test]
    fn click_outside_any_region_is_none() {
        let state = state_in_devtools_panel(DevToolsPanel::Inspector);
        let result = handle_press(&state, 100, 100, MouseButton::Left, KeyModSet::NONE);
        assert!(result.is_none());
    }
}
```

### Notes

- **Why no busy gate.** The Normal-mode handler gates `HotReload` / `HotRestart` / `StopApp` on `any_session_busy`. None of the DevTools click messages (`SwitchDevToolsPanel`, `SelectPerformanceFrame`, `NetworkSelectRequest`, `NetworkSwitchDetailTab`, `DevToolsInspectorSelectRow`, `DevToolsInspectorToggleNode`) trigger long-running Flutter operations. They are pure UI navigation. Skipping the busy gate keeps the panel responsive even mid-reload.
- **Modifier keys ignored.** `_mods` is accepted for symmetry with `normal::handle_press` but not consulted. Modifier+click in DevTools mode is reserved for a future enhancement.
- **Why the filter-active gate is in the dispatcher, not the registry.** The registry is layout-agnostic — it doesn't know which mode is active. Gating at hit-test time (after consulting the registry) is fine because filter-active is a per-frame property; the user can't be typing a filter and clicking simultaneously, so missing one click is acceptable.
- **Why no `tag_filter_visible` gate inside `devtools::handle_press`.** That gate already lives at the dispatcher level (`mouse/mod.rs::handle_press` line 63), set up in Phase 3.5 Task 08. DevTools mode inherits it for free.
- **`drop(regions)` is explicit.** The guard's Drop puts the registry back; explicit drop makes the lifetime visible to readers and enforces no-overlap with the optional follow-up logic. Mirrors `normal::handle_press`.
- **No `state.devtools_view_state.active_panel` match-on-DevToolsPanel logic.** The dispatcher does not need to know which panel is active to hit-test — the registry was populated by whichever panel rendered most recently, and only that panel's regions are present. Future phases may add per-panel gates here, but v1 keeps it simple.

---

## Completion Summary

**Status:** Done
**Branch:** feat/mouse-support

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/handler/mouse/devtools.rs` | Added `handle_press` function (RAII guard, filter-input gate, hit-test pattern); added `MouseButton` import; updated module-level doc comment; added `press_tests` module with 5 unit tests |
| `crates/fdemon-app/src/handler/mouse/mod.rs` | Added `UiMode::DevTools => devtools::handle_press(...)` arm in `handle_press`; updated doc comment to mention Phase 4 adds DevTools; renamed `test_press_no_op_in_devtools_mode_phase_3` to `test_press_no_op_in_devtools_mode_without_regions` |

### Notable Decisions/Tradeoffs

1. **Test renamed instead of deleted**: The existing `test_press_no_op_in_devtools_mode_phase_3` test was preserved with a renamed identifier (`test_press_no_op_in_devtools_mode_without_regions`) since the behaviour (no regions → None) is still correct; only the Phase-3-only framing was stale.
2. **Explicit `drop(regions)`**: Mirrors `normal::handle_press` for readability — makes the RAII guard lifetime visible to readers.
3. **5 tests added** (task required >= 4): left-click resolves, right-click no-op, network filter-active suppresses, inspector not gated by network filter, click outside any region returns None.

### Testing Performed

- `cargo check -p fdemon-app` - Passed
- `cargo test -p fdemon-app --lib -- handler::mouse` - Passed (73 tests, all passing)
- `cargo fmt --all` - Passed (no changes needed)
- `cargo clippy -p fdemon-app -- -D warnings` - Passed
- `cargo test --workspace --lib` - Passed (913 tests, 0 failed)

### Risks/Limitations

1. **No busy gate in DevTools**: Per task spec, DevTools click messages are pure UI navigation (no long-running Flutter operations), so the busy gate is intentionally omitted. This is documented in the task Notes section.
