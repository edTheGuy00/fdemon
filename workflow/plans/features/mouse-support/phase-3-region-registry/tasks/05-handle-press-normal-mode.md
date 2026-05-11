## Task: Hit-test Press Events in `handler/mouse/normal.rs`

**Objective**: Wire `MouseInput::Press { Left | Middle, .. }` in `UiMode::Normal` to the registry hit-test. Returns the matched region's `Message` (with the busy gate applied for `HotReload`/`HotRestart`/`StopApp`). Right-click and Release/Drag remain `None`.

**Depends on**: 03

**Estimated Time**: 1 hour

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/handler/mouse/normal.rs`: Add a `handle_press(state, x, y, button, mods) -> Option<Message>` function and wire it into the existing top-level `handle_*` chain. Also extend the existing `handler::mouse::mod.rs` dispatcher entry for `MouseInput::Press`.

**Files Read (Dependencies):**
- `crates/fdemon-app/src/mouse_regions.rs` (Task 01): `MouseRegions::hit_test`, `MouseAction::resolve`.
- `crates/fdemon-app/src/state.rs` (Task 03): `state.mouse_regions: Cell<MouseRegions>`, `state.session_manager.any_session_busy()`.
- `crates/fdemon-app/src/message.rs`: All Message variants for the busy-gate match.

### Details

#### Dispatcher change

Currently `handler/mouse/mod.rs::handle_mouse` matches:

```rust
MouseInput::Press { .. } | MouseInput::Release { .. } | MouseInput::Drag { .. } => None,
```

Change it to dispatch `Press` to the per-mode handler (mirroring the existing `Scroll` dispatch), but keep `Release` and `Drag` as `None`. Phase 3 only handles `Normal` mode for clicks; other modes' press handlers stay as `None` until Phase 4/5.

```rust
pub fn handle_mouse(state: &AppState, input: MouseInput) -> Option<Message> {
    match input {
        MouseInput::Scroll { direction, modifiers, .. } => {
            handle_scroll(state, direction, modifiers)
        }
        MouseInput::Press { x, y, button, modifiers } => {
            handle_press(state, x, y, button, modifiers)
        }
        // Phase 4+ may wire drag-to-select etc. — currently no-op.
        MouseInput::Release { .. } | MouseInput::Drag { .. } => None,
    }
}

fn handle_press(state: &AppState, x: u16, y: u16, button: MouseButton, mods: KeyModSet)
    -> Option<Message>
{
    match state.ui_mode {
        UiMode::Normal => normal::handle_press(state, x, y, button, mods),
        // Phase 5 wires DevTools/Settings/dialog modes; for now, no-op.
        _ => None,
    }
}
```

Add `MouseButton` to the `use` line at the top of `mod.rs`:

```rust
use crate::input_mouse::{KeyModSet, MouseButton, MouseInput, ScrollDir};
```

(`MouseButton` is already in the `use` list of `mod.rs`'s test module — verify and reuse.)

#### `normal::handle_press`

```rust
// crates/fdemon-app/src/handler/mouse/normal.rs

use crate::input_mouse::{KeyModSet, MouseButton, ScrollDir};
use crate::message::Message;
use crate::mouse_regions::{MouseAction, MouseRegions};
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
    let matched = regions.hit_test(x, y, button).map(|entry| {
        let action = match button {
            MouseButton::Left => entry.on_left.as_ref(),
            MouseButton::Middle => entry.on_middle.as_ref(),
            MouseButton::Right => None,
        };
        action.map(|a| a.resolve(x, y))
    });
    // Put the registry back unchanged. Re-rendering will repopulate it.
    state.mouse_regions.set(regions);

    let msg = matched.flatten()?;

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
```

#### Why `&AppState` works

`Cell<MouseRegions>` permits interior mutation through a shared reference (that is the whole point of `Cell`). `state.mouse_regions.take()` requires only `&Cell<MouseRegions>`, and `set()` likewise. So `handle_press` can keep the existing `&AppState` signature — no need to switch to `&mut AppState`. This preserves the call signature parity with `handle_scroll`.

### Acceptance Criteria

1. `handler::mouse::mod.rs` dispatches `MouseInput::Press` to a per-mode `handle_press` (Phase 3 only implements `Normal`; other modes return `None`).
2. `Release` and `Drag` arms remain `None` for every mode.
3. `normal::handle_press`:
   - Returns `None` when `state.tag_filter_visible` is true (any button, any coord).
   - Returns `None` for `MouseButton::Right` (any coord).
   - Returns `None` when `(x, y)` does not fall within any recorded region.
   - Returns `Some(msg)` matching the registry entry, with `MouseAction::resolve(x, y)` applied for `EmitWithCoord` actions.
   - Gates `HotReload`, `HotRestart`, `StopApp` on `any_session_busy()` and returns `None` when busy.
4. The take/put-back pair leaves `state.mouse_regions` populated *after* the call (the registry is restored, not consumed).
5. `cargo test --workspace` passes; new tests below all pass.
6. No clippy warnings.

### Testing

Add tests to `crates/fdemon-app/src/handler/mouse/normal.rs::tests` (the existing module):

```rust
#[test]
fn press_outside_any_region_is_none() {
    let state = AppState::new();
    let result = handle_press(&state, 100, 100, MouseButton::Left, KeyModSet::NONE);
    assert!(result.is_none());
}

#[test]
fn press_right_button_is_no_op_even_with_matching_region() {
    use crate::mouse_regions::{MouseAction, MouseRect};
    let state = AppState::new();
    let mut regions = state.mouse_regions.take();
    regions.builder().click(
        MouseRect::new(0, 0, 10, 1),
        MouseAction::Emit(Message::HotReload),
    );
    state.mouse_regions.set(regions);

    let result = handle_press(&state, 0, 0, MouseButton::Right, KeyModSet::NONE);
    assert!(result.is_none(), "right button is reserved for future");
}

#[test]
fn press_left_on_recorded_region_returns_emit_message() {
    use crate::mouse_regions::{MouseAction, MouseRect};
    let state = AppState::new();
    let mut regions = state.mouse_regions.take();
    regions.builder().click(
        MouseRect::new(5, 0, 3, 1),
        MouseAction::Emit(Message::HotReload),
    );
    state.mouse_regions.set(regions);

    let result = handle_press(&state, 6, 0, MouseButton::Left, KeyModSet::NONE);
    assert!(matches!(result, Some(Message::HotReload)));
}

#[test]
fn press_middle_on_left_only_region_is_none() {
    use crate::mouse_regions::{MouseAction, MouseRect};
    let state = AppState::new();
    let mut regions = state.mouse_regions.take();
    regions.builder().click(
        MouseRect::new(0, 0, 10, 1),
        MouseAction::Emit(Message::HotReload),
    );
    state.mouse_regions.set(regions);

    let result = handle_press(&state, 0, 0, MouseButton::Middle, KeyModSet::NONE);
    assert!(result.is_none());
}

#[test]
fn press_middle_on_left_middle_region_returns_middle_message() {
    use crate::mouse_regions::{MouseAction, MouseRect};
    let state = AppState::new();
    let mut regions = state.mouse_regions.take();
    regions.builder().click_left_middle(
        MouseRect::new(0, 0, 10, 1),
        MouseAction::Emit(Message::SelectSessionByIndex(2)),
        MouseAction::Emit(Message::CloseSessionAt(2)),
    );
    state.mouse_regions.set(regions);

    let left = handle_press(&state, 0, 0, MouseButton::Left, KeyModSet::NONE);
    let middle = handle_press(&state, 0, 0, MouseButton::Middle, KeyModSet::NONE);
    assert!(matches!(left, Some(Message::SelectSessionByIndex(2))));
    assert!(matches!(middle, Some(Message::CloseSessionAt(2))));
}

#[test]
fn press_with_emit_with_coord_resolves_against_position() {
    use crate::mouse_regions::{MouseAction, MouseRect};
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
    use crate::mouse_regions::{MouseAction, MouseRect};
    use crate::test_utils::*; // adjust to the actual test_device helper path

    let mut state = AppState::new();
    let id = state
        .session_manager
        .create_session(&test_device("d1", "iPhone"))
        .unwrap();
    // Mark the session as busy by starting a reload (no command sender needed
    // for the test — the `is_busy()` method checks pending state, not the
    // sender).
    state
        .session_manager
        .get_mut(id)
        .unwrap()
        .session
        .start_reload();
    assert!(state.session_manager.any_session_busy(), "precondition");

    let mut regions = state.mouse_regions.take();
    regions.builder().click(
        MouseRect::new(0, 0, 3, 1),
        MouseAction::Emit(Message::HotReload),
    );
    regions.builder().click(
        MouseRect::new(5, 0, 3, 1),
        MouseAction::Emit(Message::RequestQuit),
    );
    state.mouse_regions.set(regions);

    let reload = handle_press(&state, 0, 0, MouseButton::Left, KeyModSet::NONE);
    let quit = handle_press(&state, 5, 0, MouseButton::Left, KeyModSet::NONE);
    assert!(reload.is_none(), "HotReload gated by busy");
    assert!(matches!(quit, Some(Message::RequestQuit)), "RequestQuit not gated");
}

#[test]
fn press_take_putback_preserves_registry() {
    use crate::mouse_regions::{MouseAction, MouseRect};
    let state = AppState::new();
    let mut regions = state.mouse_regions.take();
    regions.builder().click(
        MouseRect::new(0, 0, 10, 1),
        MouseAction::Emit(Message::HotReload),
    );
    state.mouse_regions.set(regions);

    let _ = handle_press(&state, 0, 0, MouseButton::Left, KeyModSet::NONE);

    // The registry should still hold the entry after a hit-test.
    let regions = state.mouse_regions.take();
    assert_eq!(regions.len(), 1, "registry preserved across hit-test");
    state.mouse_regions.set(regions);
}

#[test]
fn press_when_tag_filter_visible_is_no_op() {
    use crate::mouse_regions::{MouseAction, MouseRect};
    let mut state = AppState::new();
    state.tag_filter_visible = true;
    let mut regions = state.mouse_regions.take();
    regions.builder().click(
        MouseRect::new(0, 0, 10, 1),
        MouseAction::Emit(Message::HotReload),
    );
    state.mouse_regions.set(regions);

    let result = handle_press(&state, 0, 0, MouseButton::Left, KeyModSet::NONE);
    assert!(result.is_none());
}
```

Add corresponding tests to `handler/mouse/mod.rs::tests` to verify dispatch:

```rust
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
        x: 0, y: 0,
        button: MouseButton::Left,
        modifiers: KeyModSet::NONE,
    };
    let drag = MouseInput::Drag {
        x: 0, y: 0,
        button: MouseButton::Left,
        modifiers: KeyModSet::NONE,
    };
    assert!(handle_mouse(&state, release).is_none());
    assert!(handle_mouse(&state, drag).is_none());
}
```

Update the existing `test_press_no_op_in_every_mode` test in `mod.rs::tests`: it currently asserts that `Press` is a no-op in every mode. After this task lands, it must be replaced with mode-specific assertions (Normal can match if regions are registered; other Phase-3 modes are still no-op). Adjust the test to register no regions and assert all-modes-no-op as a "without regions" baseline.

### Notes

- The take/put-back dance happens twice per click (once in `view()` from Task 04, once here). Both go through the same `Cell` API and both restore the registry. There is no ABA hazard because the TEA loop is single-threaded.
- Why not gate `ClearLogs` clicks like the keyboard handler does? `ClearLogs` is keyboard-only by design (PLAN.md "Edge Cases & Risks" — `clear_logs` collision). No widget will ever register a region whose action is `Message::ClearLogs`. The busy gate here only matches what the registry can produce.
- The `_mods: KeyModSet` parameter is intentionally unused. Future phases may wire Shift+click → "open in new session" etc. Keep the parameter so the signature is forward-compatible.
- `use crate::test_utils::*` in tests: confirm the actual path. If the test helpers are in a different location (e.g., `crate::handler::tests`), copy the construct-busy-session pattern from `handler/tests.rs` rather than dragging in test_utils.

---

## Completion Summary

**Status:** Done
**Branch:** feat/mouse-support

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/handler/mouse/normal.rs` | Added `handle_press` function with tag-filter guard, right-click guard, hit-test, and busy gate; added 8 new tests in `mod tests`; moved `MouseAction`/`MouseRect`/`MouseRegions` imports into `#[cfg(test)]` scope |
| `crates/fdemon-app/src/handler/mouse/mod.rs` | Added `MouseButton` to top-level `use`; split `Press` arm out of the `None` catch-all into a `handle_press` dispatcher function; replaced `test_press_no_op_in_every_mode` with `test_press_no_op_in_every_mode_without_regions`; added 3 new dispatcher tests |

### Notable Decisions/Tradeoffs

1. **Import placement**: `MouseAction`/`MouseRect`/`MouseRegions` are only referenced by name in the test module, so they were placed inside `#[cfg(test)]` to avoid clippy unused-import warnings in production builds.
2. **take/put-back pattern**: The registry is taken, hit-tested, then restored before the busy gate — this ensures the registry is always restored even if the busy gate returns `None`.
3. **Local `test_device` helper**: No project-level `test_utils` module exists; the pattern from `handler/tests.rs` was copied locally into `normal.rs::tests`.

### Testing Performed

- `cargo test -p fdemon-app --lib handler::mouse` — 68 passed, 0 failed
- `cargo test --workspace --lib` — 4,880 passed (2027 + 372 + 740 + 842 + 899), 0 failed
- `cargo clippy -p fdemon-app` — 0 warnings, 0 errors

### Risks/Limitations

1. **Phase 3 scope**: DevTools/Settings/dialog modes return `None` for press events by design; they will be wired in Phase 4/5.
