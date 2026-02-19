## Task: Reset DevTools State on Session Switch

**Objective**: Clear `DevToolsViewState` (inspector tree, layout data, overlay flags, loading state, errors) when the user switches between sessions, preventing stale data from one session being displayed for another.

**Depends on**: 01-fix-loading-stuck

**Estimated Time**: 1-2 hours

### Scope

- `crates/fdemon-app/src/state.rs`: Add `DevToolsViewState::reset()` and `LayoutExplorerState::reset()` methods
- `crates/fdemon-app/src/handler/session_lifecycle.rs`: Call `devtools_view_state.reset()` in session switch handlers
- `crates/fdemon-app/src/handler/tests.rs`: Add regression tests

### Details

#### Current State

`devtools_view_state` is a single global field on `AppState` (state.rs:162-177). The session switch handlers (`handle_select_session_by_index`, `handle_next_session`, `handle_previous_session` in session_lifecycle.rs:81-97) only call `session_manager.select_*()` — they never touch `devtools_view_state`. This means:

- Inspector tree from session 1 remains visible when switching to session 2
- Layout data from session 1 appears to belong to session 2
- Overlay toggles (rainbow, debug paint, perf) show stale state
- Loading spinner or error messages from a prior session persist

#### Fix: Add Reset Methods

**LayoutExplorerState::reset()** (state.rs — new method):

```rust
impl LayoutExplorerState {
    pub fn reset(&mut self) {
        self.layout = None;
        self.loading = false;
        self.error = None;
    }
}
```

**DevToolsViewState::reset()** (state.rs — new method):

```rust
impl DevToolsViewState {
    pub fn reset(&mut self) {
        self.inspector.reset();           // already exists at state.rs:108-114
        self.layout_explorer.reset();     // new method above
        self.overlay_repaint_rainbow = false;
        self.overlay_debug_paint = false;
        self.overlay_performance = false;
        self.vm_connection_error = None;  // added in task 02
        // NOTE: active_panel is intentionally preserved — user's panel choice
        // should persist across session switches
    }
}
```

**Session switch handlers** (session_lifecycle.rs):

Add `state.devtools_view_state.reset();` to all three functions:

```rust
pub fn handle_select_session_by_index(state: &mut AppState, index: usize) -> UpdateResult {
    let old_index = state.session_manager.selected_index();
    state.session_manager.select_by_index(index);
    if state.session_manager.selected_index() != old_index {
        state.devtools_view_state.reset();
    }
    UpdateResult::none()
}
```

Note the guard: only reset when the selected index actually changed (prevents unnecessary reset when user presses the same session number).

Apply the same pattern to `handle_next_session` and `handle_previous_session`:

```rust
pub fn handle_next_session(state: &mut AppState) -> UpdateResult {
    let old_id = state.session_manager.selected().map(|h| h.session.id);
    state.session_manager.select_next();
    let new_id = state.session_manager.selected().map(|h| h.session.id);
    if old_id != new_id {
        state.devtools_view_state.reset();
    }
    UpdateResult::none()
}
```

#### Design Decision: Keep `active_panel`

The `active_panel` field is intentionally preserved across session switches. If the user is on the Performance panel and switches sessions, they should stay on the Performance panel — not be kicked back to Inspector. The data shown in the panel should reset, but the panel choice is a UI preference.

### Acceptance Criteria

1. `DevToolsViewState::reset()` clears inspector, layout explorer, overlays, and vm_connection_error
2. `InspectorState::reset()` is already correct (state.rs:108-114) — no changes needed
3. `LayoutExplorerState::reset()` clears layout, loading, and error
4. Session switch (by index, next, previous) resets DevTools state when the session actually changes
5. Session switch to the same session does NOT reset (no-op guard)
6. `active_panel` is preserved across session switches
7. All existing tests pass + new regression tests added

### Testing

```rust
#[test]
fn test_session_switch_resets_devtools_state() {
    let mut state = make_state_with_two_sessions();
    // Populate devtools state for session 1
    state.devtools_view_state.inspector.loading = true;
    state.devtools_view_state.inspector.error = Some("old error".into());
    state.devtools_view_state.overlay_repaint_rainbow = true;

    // Switch to session 2
    handle_select_session_by_index(&mut state, 1);

    // Assert everything reset
    assert!(!state.devtools_view_state.inspector.loading);
    assert!(state.devtools_view_state.inspector.error.is_none());
    assert!(!state.devtools_view_state.overlay_repaint_rainbow);
}

#[test]
fn test_session_switch_preserves_active_panel() {
    let mut state = make_state_with_two_sessions();
    state.devtools_view_state.active_panel = DevToolsPanel::Performance;

    handle_select_session_by_index(&mut state, 1);

    assert_eq!(state.devtools_view_state.active_panel, DevToolsPanel::Performance);
}

#[test]
fn test_session_switch_same_session_does_not_reset() {
    let mut state = make_state_with_two_sessions();
    state.devtools_view_state.inspector.loading = true;

    // Switch to same session (index 0 when already on 0)
    handle_select_session_by_index(&mut state, 0);

    // loading should NOT be cleared
    assert!(state.devtools_view_state.inspector.loading);
}
```

### Notes

- `InspectorState::reset()` already exists (state.rs:108-114) and is tested but never called in production code. This task makes it a production code path.
- If the user is in DevTools mode and switches sessions, the empty inspector/layout will show. This is correct — better to show empty state than stale data from a different session.
- A follow-up improvement could auto-fetch the widget tree for the new session when switching while in DevTools mode, but that's out of scope for this fix.
- This task depends on task 01 because the `vm_connection_error` field (added in task 02) must also be cleared in `reset()`, and the loading guard from task 01 prevents re-triggering the stuck loading bug if auto-fetch were added later.

---

## Completion Summary

**Status:** Not Started
