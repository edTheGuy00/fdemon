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
