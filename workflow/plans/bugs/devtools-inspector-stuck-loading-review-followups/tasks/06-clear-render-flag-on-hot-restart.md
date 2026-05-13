## Task: Reset `has_ever_rendered_tree` on Hot Restart

**Objective**: Clear the sticky `has_ever_rendered_tree` flag in `Message::SessionRestartCompleted` so the first `r` after hot restart uses `FetchTrigger::Initial` (full readiness poll) instead of `Refresh` (poll skipped). After hot restart, the framework is re-initializing — the "framework is warm" invariant the flag encodes is temporarily invalid.

**Depends on**: None

**Estimated Time**: 0.5-1 hour

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/handler/update.rs` — clear `inspector.has_ever_rendered_tree` in `Message::SessionRestartCompleted` arm (around line 222-238)
- `crates/fdemon-app/src/state.rs` — update the docstring on `has_ever_rendered_tree` (around lines 250-261) to explicitly list hot restart as a reset point

**Files Read (Dependencies):**
- None

### Details

**Current code (`handler/update.rs:222-237`):**
```rust
Message::SessionRestartCompleted { session_id } => {
    if let Some(handle) = state.session_manager.get_mut(session_id) {
        handle.session.complete_reload();
        // ...
        if let Some(ref vm_handle) = handle.vm_request_handle {
            vm_handle.invalidate_isolate_cache();
        }
    }
    UpdateResult::none()
}
```

The handler invalidates the isolate cache but leaves the inspector state untouched, including `has_ever_rendered_tree`.

**Target code (additions only):**
```rust
Message::SessionRestartCompleted { session_id } => {
    if let Some(handle) = state.session_manager.get_mut(session_id) {
        handle.session.complete_reload();
        // ...
        if let Some(ref vm_handle) = handle.vm_request_handle {
            vm_handle.invalidate_isolate_cache();
        }
    }
    // Hot restart creates a new isolate with a fresh framework state.
    // Reset the sticky render flag so the next fetch polls readiness.
    if state.devtools_view_state.inspector.has_ever_rendered_tree {
        state.devtools_view_state.inspector.has_ever_rendered_tree = false;
    }
    UpdateResult::none()
}
```

**Docstring update (`state.rs:250-261`):** the current docstring says "Only cleared when the entire session is destroyed." Update to:

```rust
/// Sticky flag that becomes `true` after the first successful widget tree
/// render in the current Flutter isolate.
///
/// **Does not reset on [`Self::reset`], fetch debounce clears, or
/// individual fetch failures.** Cleared on:
/// - session destruction (drop)
/// - hot restart (`Message::SessionRestartCompleted`)
///
/// Hot restart creates a new isolate and re-initializes the framework, so
/// the "framework is warm" invariant the flag encodes is temporarily invalid;
/// the next fetch should use the full readiness poll budget.
pub has_ever_rendered_tree: bool,
```

### Acceptance Criteria

1. After `Message::SessionRestartCompleted` is dispatched, `state.devtools_view_state.inspector.has_ever_rendered_tree` is `false`.
2. Other inspector state (e.g., `loading`, `last_fetch_time`, `error`) is not modified by this change.
3. The next `Message::RequestWidgetTree` dispatched after the restart emits `UpdateAction::FetchWidgetTree { trigger: FetchTrigger::Initial, .. }` — verified by a unit test.
4. The flag docstring lists hot restart as an intentional reset point.
5. All existing tests pass.

### Testing

Add a test in `crates/fdemon-app/src/handler/tests.rs`:

```rust
#[test]
fn test_session_restart_clears_has_ever_rendered_tree() {
    let mut state = build_state_with_inspector_rendered_once();
    assert!(state.devtools_view_state.inspector.has_ever_rendered_tree);

    let result = update(&mut state, Message::SessionRestartCompleted { session_id });
    
    assert!(!state.devtools_view_state.inspector.has_ever_rendered_tree);
    // Sanity: other inspector fields preserved
    assert!(state.devtools_view_state.inspector.root.is_none() || ...); 
}

#[test]
fn test_post_restart_request_uses_initial_trigger() {
    let mut state = build_state_with_inspector_rendered_once();
    let _ = update(&mut state, Message::SessionRestartCompleted { session_id });
    
    // Clear debounce to allow the fetch
    state.devtools_view_state.inspector.clear_fetch_debounce();
    
    let result = update(&mut state, Message::RequestWidgetTree { session_id });
    
    match result.action {
        Some(UpdateAction::FetchWidgetTree { trigger, .. }) => {
            assert_eq!(trigger, FetchTrigger::Initial);
        }
        _ => panic!("expected FetchWidgetTree action with Initial trigger"),
    }
}
```

### Notes

- This may produce a single error-toast cycle on the first post-restart `r` press if the framework finishes warming before the poll budget expires, but that's the design — better than the current "skip the poll, hit a not-ready framework" race.
- The existing `try_fetch_widget_tree` transient-error fallback (`getRootWidgetSummaryTree`) further mitigates the cold path.
- Consider whether `inspector.reset()` should also reset the flag — answer: NO. `reset()` is called on session switch (not hot restart) where the user explicitly navigates away and the cached state is still valid for the previous session. Keep the existing `reset()` semantics.

---

## Completion Summary

**Status:** Done
**Branch:** fix/devtools-improvements

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/handler/update.rs` | Added reset of `has_ever_rendered_tree` in `Message::SessionRestartCompleted` arm |
| `crates/fdemon-app/src/state.rs` | Updated docstring on `has_ever_rendered_tree` field and getter to list hot restart as an explicit reset point |
| `crates/fdemon-app/src/handler/tests.rs` | Added 3 new tests: `session_restart_clears_has_ever_rendered_tree`, `post_restart_request_uses_initial_trigger`, `hot_reload_does_not_clear_has_ever_rendered_tree` |

### Notable Decisions/Tradeoffs

1. **Direct field access in update.rs**: The implementation accesses `state.devtools_view_state.inspector.has_ever_rendered_tree` directly (field is `pub`) rather than adding a dedicated method. This keeps the reset one-liner consistent with how other state is cleared inline in `update.rs`.
2. **Conditional reset**: Used `if state.devtools_view_state.inspector.has_ever_rendered_tree { ... = false; }` rather than an unconditional assignment, matching the pattern in the task spec exactly.
3. **Third test added**: Added `hot_reload_does_not_clear_has_ever_rendered_tree` beyond what the task spec required, to explicitly guard the contract that hot reload (a different message) does NOT touch the flag.

### Testing Performed

- `cargo fmt --all -- --check` - Passed (after running `cargo fmt --all` to fix pre-existing style issues in other files)
- `cargo check --workspace --all-targets` - Passed
- `cargo test --workspace` - Passed (2196 + all other crate tests, 0 failures)
- `cargo clippy --workspace --all-targets -- -D warnings` - Passed

### Risks/Limitations

1. **None identified**: The change is additive, touching only the hot-restart handler and docstrings. All acceptance criteria met.
