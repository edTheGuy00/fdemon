## Task: Fix `NativeLogCaptureStopped` Tag State Reset

**Objective**: Prevent `NativeLogCaptureStopped` from resetting tag visibility state while custom sources are still running.

**Depends on**: 02-fix-hot-restart-guard (overlapping guard/lifecycle code area)

**Review Issue**: #3 (MAJOR)

### Scope

- `crates/fdemon-app/src/handler/update.rs`: Guard the `native_tag_state` reset in `NativeLogCaptureStopped` handler (~line 2015-2023)

### Details

The `NativeLogCaptureStopped` handler (update.rs:2015-2023) unconditionally resets `native_tag_state` to `NativeTagState::default()` when the platform capture process exits. This destroys:

- All tags discovered from custom sources that are still streaming
- User's per-tag hide/show choices made in the tag filter overlay (the `T`-key UI)

`NativeLogCaptureStopped` is sent only when the **platform capture** process exits (`adb logcat` crashes, `log stream` ends, etc.). Custom sources have independent lifecycles and may still be running.

Other reset points are correct:
- `handle_session_exited` (session.rs:163) — resets tag state AND calls `shutdown_native_logs()` which stops custom sources too
- `handle_session_message_state` on `AppStop` (session.rs:231) — same, both are reset together

**Current code:**
```rust
Message::NativeLogCaptureStopped { session_id } => {
    if let Some(handle) = state.session_manager.get_mut(session_id) {
        handle.native_log_shutdown_tx = None;
        handle.native_log_task_handle = None;
        handle.native_tag_state = crate::session::NativeTagState::default();
    }
    UpdateResult::none()
}
```

**Fixed code:**
```rust
Message::NativeLogCaptureStopped { session_id } => {
    if let Some(handle) = state.session_manager.get_mut(session_id) {
        handle.native_log_shutdown_tx = None;
        handle.native_log_task_handle = None;
        // Only reset tag state if no custom sources are still emitting events.
        if handle.custom_source_handles.is_empty() {
            handle.native_tag_state = crate::session::NativeTagState::default();
        }
    }
    UpdateResult::none()
}
```

### Acceptance Criteria

1. When `adb logcat` exits while custom sources are running, tag filter selections are preserved
2. When `adb logcat` exits and no custom sources are running, tag state resets as before
3. Session exit still resets tag state completely (existing behavior preserved)
4. New test covers the conditional reset

### Testing

```rust
#[test]
fn test_native_log_capture_stopped_preserves_tags_when_custom_sources_running() {
    // Setup session with both platform capture and custom sources
    // Add some hidden tags to native_tag_state
    // Send NativeLogCaptureStopped
    // Assert native_tag_state.hidden_tags is NOT reset (custom sources still running)
}

#[test]
fn test_native_log_capture_stopped_resets_tags_when_no_custom_sources() {
    // Setup session with platform capture only
    // Add some hidden tags to native_tag_state
    // Send NativeLogCaptureStopped
    // Assert native_tag_state IS reset (no custom sources running)
}
```

### Notes

- There is a symmetric concern for `CustomSourceStopped` — when the last custom source stops but platform capture is still running, should tag state be preserved? Current behavior preserves it (no reset in `CustomSourceStopped`), which is correct.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/handler/update.rs` | Wrapped `native_tag_state` reset in `NativeLogCaptureStopped` handler with `if handle.custom_source_handles.is_empty()` guard |
| `crates/fdemon-app/src/handler/tests.rs` | Added two new tests: `test_native_log_capture_stopped_preserves_tags_when_custom_sources_running` and `test_native_log_capture_stopped_resets_tags_when_no_custom_sources` |

### Notable Decisions/Tradeoffs

1. **Comment placement**: Added an explanatory comment above the guard condition to make the intent clear to future readers — the asymmetry between `NativeLogCaptureStopped` (platform only) and session exit (resets both) warrants explicit documentation.
2. **Test coverage of hidden state**: The preserve-test toggles a tag to hidden before sending the stop message, directly verifying both `tag_count` and `hidden_count` survive the message. This exercises the user's actual concern (filter choices being wiped).

### Testing Performed

- `cargo fmt --all` - Passed
- `cargo check -p fdemon-app` - Passed
- `cargo test -p fdemon-app --lib` - Passed (1553 tests, 0 failed; all 4 `NativeLogCaptureStopped` tests pass)
- `cargo clippy -p fdemon-app -- -D warnings` - Passed

### Risks/Limitations

1. **None identified**: The guard is a minimal one-liner using the already-populated `custom_source_handles` vec. Session-exit reset paths in `session.rs` are unaffected and continue to reset unconditionally.
