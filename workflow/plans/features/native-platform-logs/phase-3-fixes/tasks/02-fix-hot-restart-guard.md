## Task: Fix Hot-Restart Guard for Custom-Sources-Only Sessions

**Objective**: Extend the native log startup guard to prevent duplicate custom source processes on hot-restart.

**Depends on**: None

**Review Issue**: #2 (MAJOR)

### Scope

- `crates/fdemon-app/src/handler/session.rs`: Extend guard in `maybe_start_native_log_capture` (~line 303)

### Details

The guard in `maybe_start_native_log_capture` (session.rs:303-309) only checks `handle.native_log_shutdown_tx.is_some()` to prevent double-start on hot-restart. This works for platform capture (Android/iOS/macOS) because the platform capture task sets `native_log_shutdown_tx` via `NativeLogCaptureStarted`.

But for sessions using **only custom sources** (e.g., Linux/Windows/Web targets, or configurations where platform capture is skipped), `native_log_shutdown_tx` is never set:
- `spawn_native_log_capture` calls `spawn_custom_sources` at line 67 (before any platform checks)
- Then the function returns early at lines 72-91 without sending `NativeLogCaptureStarted`
- `native_log_shutdown_tx` remains `None`
- On hot-restart, the guard evaluates to `false` → `StartNativeLogCapture` is emitted again → duplicate processes

**Current guard:**
```rust
if handle.native_log_shutdown_tx.is_some() {
    tracing::info!(
        "[native-logs-debug] Skipping: already running for session {}",
        session_id
    );
    return None;
}
```

**Fixed guard:**
```rust
if handle.native_log_shutdown_tx.is_some()
    || !handle.custom_source_handles.is_empty()
{
    tracing::debug!(
        "Native log capture already running for session {}",
        session_id
    );
    return None;
}
```

Note: the debug log change also addresses issue #5 (remove `[native-logs-debug]` prefix).

### Acceptance Criteria

1. Hot-restart does not spawn duplicate custom source processes
2. The guard checks both `native_log_shutdown_tx` and `custom_source_handles`
3. Platform-only sessions still work correctly (guard fires on `native_log_shutdown_tx`)
4. New test covers the custom-sources-only hot-restart case

### Testing

Add a handler test that:
1. Sets up a session with only custom sources (no platform capture)
2. Triggers `AppStart` → custom sources spawn
3. Triggers another `AppStart` (hot-restart) → guard fires, no duplicates

```rust
#[test]
fn test_hot_restart_skips_duplicate_custom_sources() {
    // Setup session with custom_source_handles populated, native_log_shutdown_tx = None
    // Call maybe_start_native_log_capture
    // Assert returns None (guard fired)
}
```

### Notes

- This guard is the entry point for all native log lifecycle — changes here affect both platform and custom source paths
- The `shutdown_native_logs` method in `handle.rs` already treats both resource types as co-equal (cleans up both), so this guard extension is consistent with that pattern

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/handler/session.rs` | Extended guard in `maybe_start_native_log_capture` to check `custom_source_handles.is_empty()` in addition to `native_log_shutdown_tx.is_some()`; replaced `tracing::info!` with `tracing::debug!` and removed `[native-logs-debug]` prefix |
| `crates/fdemon-app/src/handler/tests.rs` | Added `attach_custom_source_handle` helper and `test_hot_restart_skips_duplicate_custom_sources` test |

### Notable Decisions/Tradeoffs

1. **Pre-existing clippy lint not fixed**: `native_logs.rs` has a pre-existing `clippy::too_many_arguments` lint on `spawn_native_log_capture`. This was present before this task and is outside the task's scope (`handler/session.rs` only). The files I changed produce no new clippy warnings.

2. **Test uses `CustomSourceConfig` from `crate::config`**: The test sets `state.settings.native_logs.custom_sources` (which uses `CustomSourceConfig`) to ensure `has_custom_sources = true` is the reason the guard fires in production, and directly populates `custom_source_handles` to simulate the state after the first `AppStart` has spawned processes.

3. **Inline `||` operator on single line**: After `cargo fmt`, the guard condition fits on one line (`if handle.native_log_shutdown_tx.is_some() || !handle.custom_source_handles.is_empty()`), which is idiomatic Rust.

### Testing Performed

- `cargo check -p fdemon-app` - Passed
- `cargo test -p fdemon-app --lib -- test_hot_restart_skips_duplicate_custom_sources test_maybe_start_native_log_capture` - Passed (6 tests)
- `cargo test -p fdemon-app --lib` - Passed (1551 tests)
- `cargo fmt --all` - Passed
- `cargo clippy -p fdemon-app -- -D warnings` - Failed on pre-existing `too_many_arguments` lint in `native_logs.rs` (not introduced by this task; no new warnings in modified files)

### Risks/Limitations

1. **Pre-existing clippy failure**: The quality gate technically fails due to `clippy::too_many_arguments` in `actions/native_logs.rs`. This lint existed before this task on the branch and is unrelated to the guard fix.
