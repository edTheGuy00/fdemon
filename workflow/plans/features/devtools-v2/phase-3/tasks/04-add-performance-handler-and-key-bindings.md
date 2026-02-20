## Task: Add Performance Handler Sub-Module and Key Bindings

**Objective**: Create a dedicated `handler/devtools/performance.rs` sub-module for performance message handling, add Left/Right/Esc key bindings for frame selection in the Performance panel, and handle the new `SelectPerformanceFrame`, `VmServiceMemorySample`, and `VmServiceAllocationProfileReceived` messages.

**Depends on**: Task 02 (extend-performance-state-and-messages)

### Scope

- `crates/fdemon-app/src/handler/devtools/performance.rs`: **NEW** — performance-specific handlers
- `crates/fdemon-app/src/handler/devtools/mod.rs`: Add `mod performance;`, delegate performance messages
- `crates/fdemon-app/src/handler/keys.rs`: Add Left/Right/Esc key handling for Performance panel
- `crates/fdemon-app/src/handler/update.rs`: Route new message variants to performance handlers

### Details

#### Create `handler/devtools/performance.rs`

This file handles all performance-panel-specific messages. Currently, performance messages are handled inline in `update.rs` (around lines 1313–1373). Those existing handlers stay in `update.rs` for now (they handle VM service data arrival which is not panel-specific). The new handlers are for UI interaction and the new message types.

```rust
//! Performance panel handlers.
//!
//! Handles frame selection, allocation profile updates, and rich memory samples
//! for the Performance panel's bar chart and time-series views.

use crate::message::Message;
use crate::handler::{UpdateResult, UpdateAction};
use crate::state::AppState;
use fdemon_core::performance::{AllocationProfile, MemorySample};

/// Handle frame selection from Left/Right key navigation.
pub(crate) fn handle_select_performance_frame(
    state: &mut AppState,
    index: Option<usize>,
) -> UpdateResult {
    if let Some(handle) = state.session_manager.current_session_mut() {
        handle.session.performance.selected_frame = index;
    }
    UpdateResult::none()
}

/// Handle Left arrow key — select previous frame.
pub(crate) fn handle_performance_navigate_left(state: &mut AppState) -> UpdateResult {
    if let Some(handle) = state.session_manager.current_session_mut() {
        handle.session.performance.select_prev_frame();
    }
    UpdateResult::none()
}

/// Handle Right arrow key — select next frame.
pub(crate) fn handle_performance_navigate_right(state: &mut AppState) -> UpdateResult {
    if let Some(handle) = state.session_manager.current_session_mut() {
        handle.session.performance.select_next_frame();
    }
    UpdateResult::none()
}

/// Handle Esc — deselect frame (only when a frame is selected, otherwise exit DevTools).
pub(crate) fn handle_performance_deselect_frame(state: &mut AppState) -> UpdateResult {
    if let Some(handle) = state.session_manager.current_session_mut() {
        handle.session.performance.deselect_frame();
    }
    UpdateResult::none()
}

/// Handle rich memory sample received from VM service.
pub(crate) fn handle_memory_sample_received(
    state: &mut AppState,
    session_id: SessionId,
    sample: MemorySample,
) -> UpdateResult {
    if let Some(handle) = state.session_manager.get_mut(session_id) {
        handle.session.performance.memory_samples.push(sample);
    }
    UpdateResult::none()
}

/// Handle allocation profile snapshot received from VM service.
pub(crate) fn handle_allocation_profile_received(
    state: &mut AppState,
    session_id: SessionId,
    profile: AllocationProfile,
) -> UpdateResult {
    if let Some(handle) = state.session_manager.get_mut(session_id) {
        handle.session.performance.allocation_profile = Some(profile);
    }
    UpdateResult::none()
}
```

#### Update `handler/devtools/mod.rs`

Add the module declaration and delegate from the panel switch handler:

```rust
mod performance;

// In handle_switch_panel or handle_devtools_message, delegate:
DevToolsPanel::Performance => {
    // No auto-fetch on panel switch (unchanged)
}
```

#### Update `handler/keys.rs`

In `handle_key_devtools`, add Performance panel key handling. Currently the function has `in_inspector` guard; add a similar `in_performance` guard:

```rust
pub(crate) fn handle_key_devtools(state: &mut AppState, key: KeyEvent) -> Option<Message> {
    let in_inspector = state.devtools_view_state.active_panel == DevToolsPanel::Inspector;
    let in_performance = state.devtools_view_state.active_panel == DevToolsPanel::Performance;

    match key.code {
        // ... existing matches ...

        // Performance panel: frame navigation
        KeyCode::Left if in_performance => {
            Some(Message::SelectPerformanceFrame { index: None }) // handled via navigate_left
            // OR: directly call handler — see implementation note below
        }
        KeyCode::Right if in_performance => {
            Some(Message::SelectPerformanceFrame { index: None }) // handled via navigate_right
        }

        // Esc: in Performance with frame selected → deselect; otherwise exit DevTools
        KeyCode::Esc => {
            if in_performance {
                if let Some(handle) = state.session_manager.current_session() {
                    if handle.session.performance.selected_frame.is_some() {
                        return Some(Message::SelectPerformanceFrame { index: None });
                    }
                }
            }
            Some(Message::ExitDevToolsMode)
        }

        // ... rest unchanged ...
    }
}
```

**Implementation note**: The key handler can either:
1. Return a `Message` and let `update.rs` dispatch to the handler, or
2. Directly mutate state (simpler for pure UI navigation with no side effects)

Follow the existing project pattern. If inspector key handlers return Messages, do the same. If they mutate state directly, do the same. Check how `handle_inspector_navigate` works and mirror that pattern.

#### Route new messages in `update.rs`

Add match arms for the new message variants in the main `update()` function:

```rust
Message::SelectPerformanceFrame { index } => {
    performance::handle_select_performance_frame(state, index)
}
Message::VmServiceMemorySample { session_id, sample } => {
    performance::handle_memory_sample_received(state, session_id, sample)
}
Message::VmServiceAllocationProfileReceived { session_id, profile } => {
    performance::handle_allocation_profile_received(state, session_id, profile)
}
```

#### Update footer hints

In `fdemon-tui/src/widgets/devtools/mod.rs`, update the Performance panel footer:

```rust
DevToolsPanel::Performance => {
    "[Esc] Logs  [i] Inspector  [b] Browser  [←/→] Frames  [Ctrl+p] PerfOverlay"
}
```

### Acceptance Criteria

1. `handler/devtools/performance.rs` file exists with all handler functions
2. Left arrow key selects previous frame when Performance panel is active
3. Right arrow key selects next frame when Performance panel is active
4. Esc deselects frame if one is selected; exits DevTools mode if no frame selected
5. `VmServiceMemorySample` message pushes sample to `memory_samples` ring buffer
6. `VmServiceAllocationProfileReceived` message stores profile in `allocation_profile`
7. `SelectPerformanceFrame` message updates `selected_frame`
8. Footer hints updated to show `[←/→] Frames`
9. All existing handler tests pass
10. `cargo check -p fdemon-app` passes
11. `cargo test -p fdemon-app` passes

### Testing

Add tests in `handler/devtools/performance.rs` (or `handler/tests.rs` following project convention):

```rust
#[test]
fn test_left_arrow_selects_prev_frame() {
    let mut state = make_state_with_perf_session();
    push_frames(&mut state, 5);
    state.current_session_mut().performance.selected_frame = Some(3);
    let result = update(&mut state, key_event(KeyCode::Left));
    assert_eq!(state.current_session().performance.selected_frame, Some(2));
}

#[test]
fn test_right_arrow_selects_next_frame() {
    let mut state = make_state_with_perf_session();
    push_frames(&mut state, 5);
    state.current_session_mut().performance.selected_frame = Some(2);
    let result = update(&mut state, key_event(KeyCode::Right));
    assert_eq!(state.current_session().performance.selected_frame, Some(3));
}

#[test]
fn test_esc_with_frame_selected_deselects() {
    let mut state = make_state_with_perf_session();
    push_frames(&mut state, 5);
    state.current_session_mut().performance.selected_frame = Some(2);
    let result = update(&mut state, key_event(KeyCode::Esc));
    assert_eq!(state.current_session().performance.selected_frame, None);
    // Still in DevTools mode
    assert_eq!(state.ui_mode, UiMode::DevTools);
}

#[test]
fn test_esc_without_frame_selected_exits_devtools() {
    let mut state = make_state_with_perf_session();
    state.current_session_mut().performance.selected_frame = None;
    let result = update(&mut state, key_event(KeyCode::Esc));
    assert_ne!(state.ui_mode, UiMode::DevTools);
}

#[test]
fn test_memory_sample_received_pushes_to_buffer() {
    let mut state = make_state_with_session();
    let session_id = state.current_session_id();
    let sample = test_memory_sample();
    let result = update(&mut state, Message::VmServiceMemorySample { session_id, sample });
    assert_eq!(state.current_session().performance.memory_samples.len(), 1);
}

#[test]
fn test_allocation_profile_received_stores_profile() {
    let mut state = make_state_with_session();
    let session_id = state.current_session_id();
    let profile = AllocationProfile {
        members: vec![],
        timestamp: chrono::Local::now(),
    };
    let result = update(&mut state, Message::VmServiceAllocationProfileReceived {
        session_id,
        profile,
    });
    assert!(state.current_session().performance.allocation_profile.is_some());
}

#[test]
fn test_left_right_noop_when_not_in_performance_panel() {
    let mut state = make_state_with_session();
    state.devtools_view_state.active_panel = DevToolsPanel::Inspector;
    // Left/Right should not affect performance state
}
```

### Notes

- **Esc key priority**: The Esc key now has two behaviors in DevTools mode. When the Performance panel is active AND a frame is selected, Esc deselects the frame. Otherwise, Esc exits DevTools mode. This matches the pattern used in other TUI tools where Esc "unwinds" one layer of selection.
- **Footer hint update is in fdemon-tui**: Although this task is primarily fdemon-app, the footer hint text lives in `fdemon-tui/src/widgets/devtools/mod.rs`. Update it as part of this task since it directly relates to the key bindings.
- **Left/Right don't conflict**: The Left/Right arrow keys are currently not handled in the Performance panel (they fall through to `_ => None` in `handle_key_devtools`). The Inspector panel uses Left/Right for tree navigation, guarded by `if in_inspector`. No conflict.
- **Performance handler file size**: This file starts small (< 100 lines). It will grow as Phase 4 (Network) pattern suggests, but should remain well under 500 lines for Phase 3's scope.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/handler/devtools/performance.rs` | **NEW** — 3 handler functions (`handle_select_performance_frame`, `handle_memory_sample_received`, `handle_allocation_profile_received`) + 15 unit tests |
| `crates/fdemon-app/src/handler/devtools/mod.rs` | Added `pub(crate) mod performance;` declaration |
| `crates/fdemon-app/src/handler/keys.rs` | Added `in_performance` guard, Left/Right arrow → `SelectPerformanceFrame` with computed index, Esc deselects frame before exiting DevTools |
| `crates/fdemon-app/src/handler/update.rs` | Routed `SelectPerformanceFrame`, `VmServiceMemorySample`, `VmServiceAllocationProfileReceived` to performance handlers (replaced stub match arms from Task 02) |
| `crates/fdemon-tui/src/widgets/devtools/mod.rs` | Updated Performance panel footer hints to include `[←/→] Frames` |

### Notable Decisions/Tradeoffs

1. **Key handler emits Message (not direct mutation)**: Left/Right compute the target index inline in `keys.rs` then emit `SelectPerformanceFrame { index }`, following the same pattern as Inspector tree navigation. This keeps state mutation in the handler layer.
2. **Esc two-phase behavior**: Esc in Performance panel first deselects frame (if selected), then exits DevTools mode on second press — matching the "unwind one layer" UX pattern.

### Testing Performed

- `cargo check -p fdemon-app` — Passed
- `cargo test -p fdemon-app` — Passed (945 tests, 15 new)
- `cargo clippy -p fdemon-app -- -D warnings` — Passed

### Risks/Limitations

1. **No phase breakdown display yet**: The handler sets `selected_frame` but phase breakdown rendering depends on Task 07 (panel rewire).
