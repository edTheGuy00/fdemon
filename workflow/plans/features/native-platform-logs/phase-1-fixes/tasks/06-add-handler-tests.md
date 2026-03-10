## Task: Add TEA Handler Tests for Native Log Messages

**Objective**: Add unit tests for `Message::NativeLog`, `Message::NativeLogCaptureStarted`, `Message::NativeLogCaptureStopped`, and `maybe_start_native_log_capture()` to cover the new TEA message handling paths.

**Depends on**: 02-wire-tool-guard-and-session-safety

**Review Issue:** #11 (Minor)

### Scope

- `crates/fdemon-app/src/handler/tests.rs`: Add new test functions

### Details

There are currently **zero tests** for any of the native log message variants or the `maybe_start_native_log_capture` function. The handler tests file (`tests.rs`) has extensive coverage for other message types (VM service, performance, network) and provides established test patterns to follow.

#### Test Patterns to Follow

The existing test infrastructure provides:

- **`state_with_session(device, phase)`** helper (session.rs tests) — creates `AppState` with a single session in the given phase
- **`test_device("android-device", "Android Device")`** — creates a mock `Device` with `platform: "android"`
- **`flush_all_pending_logs(state)`** — flushes batched logs before asserting log contents
- **`attach_perf_shutdown(state, session_id)`** helper (tests.rs:4513) — creates and attaches a `watch::Sender<bool>` to a session handle. This is the model for a native log shutdown helper.

#### Tests to Add

##### 1. `test_native_log_creates_log_entry_with_native_source`

Process `Message::NativeLog` for an existing session. Verify a `LogEntry` with `LogSource::Native { tag }` is added to the session's log buffer.

```rust
#[test]
fn test_native_log_creates_log_entry_with_native_source() {
    let mut state = /* state with android session in Running phase */;
    let session_id = state.session_manager.active_session_id().unwrap();

    let event = NativeLogEvent {
        tag: "MyNativeTag".to_string(),
        level: LogLevel::Warning,
        message: "native warning message".to_string(),
        timestamp: Some("2024-01-01 00:00:00.000".to_string()),
    };

    let result = update(&mut state, Message::NativeLog { session_id, event });

    flush_all_pending_logs(&mut state);
    let handle = state.session_manager.get(session_id).unwrap();
    let last_log = handle.session.logs.back().unwrap();
    assert!(matches!(last_log.source, LogSource::Native { ref tag } if tag == "MyNativeTag"));
    assert_eq!(last_log.level, LogLevel::Warning);
    assert_eq!(last_log.message, "native warning message");
}
```

##### 2. `test_native_log_for_missing_session_is_no_op`

Process `Message::NativeLog` with a non-existent `session_id`. Should return `UpdateResult::none()` without panicking.

##### 3. `test_native_log_capture_started_stores_handles`

Process `Message::NativeLogCaptureStarted` for an existing session. Verify `handle.native_log_shutdown_tx` and `handle.native_log_task_handle` are set.

```rust
#[test]
fn test_native_log_capture_started_stores_handles() {
    let mut state = /* state with session */;
    let session_id = state.session_manager.active_session_id().unwrap();

    let (shutdown_tx, _rx) = tokio::sync::watch::channel(false);
    let task_handle = Arc::new(Mutex::new(Some(/* mock JoinHandle or use tokio::spawn */)));

    update(&mut state, Message::NativeLogCaptureStarted {
        session_id,
        shutdown_tx: Arc::new(shutdown_tx),
        task_handle,
    });

    let handle = state.session_manager.get(session_id).unwrap();
    assert!(handle.native_log_shutdown_tx.is_some());
    assert!(handle.native_log_task_handle.is_some());
}
```

##### 4. `test_native_log_capture_started_for_closed_session_sends_shutdown`

Process `NativeLogCaptureStarted` with a non-existent `session_id`. Verify `shutdown_tx.send(true)` was called by checking the `watch::Receiver`.

```rust
#[test]
fn test_native_log_capture_started_for_closed_session_sends_shutdown() {
    let mut state = /* state with no sessions (or wrong session_id) */;
    let missing_id = SessionId::new();

    let (shutdown_tx, mut shutdown_rx) = tokio::sync::watch::channel(false);
    let task_handle = Arc::new(Mutex::new(None)); // No actual task

    update(&mut state, Message::NativeLogCaptureStarted {
        session_id: missing_id,
        shutdown_tx: Arc::new(shutdown_tx),
        task_handle,
    });

    // Verify shutdown was signaled
    assert_eq!(*shutdown_rx.borrow(), true);
}
```

##### 5. `test_native_log_capture_stopped_clears_handles`

Process `NativeLogCaptureStopped` for an existing session that has handles set. Verify both `native_log_shutdown_tx` and `native_log_task_handle` are `None` afterwards.

##### 6. `test_maybe_start_native_log_capture_returns_action_for_android`

Call `maybe_start_native_log_capture` with an Android session, `adb = true`, native logs enabled. Verify it returns `Some(UpdateAction::StartNativeLogCapture { .. })`.

##### 7. `test_maybe_start_native_log_capture_returns_none_when_tools_unavailable`

Call `maybe_start_native_log_capture` with `adb = false`. Verify it returns `None`.

##### 8. `test_maybe_start_native_log_capture_returns_none_when_disabled`

Call with `settings.native_logs.enabled = false`. Verify `None`.

##### 9. `test_maybe_start_native_log_capture_returns_none_when_already_running`

Call with `handle.native_log_shutdown_tx = Some(...)`. Verify `None` (double-start guard).

##### 10. `test_maybe_start_native_log_capture_returns_none_for_linux`

Call with `platform = "linux"`. Verify `None`.

### Acceptance Criteria

1. At least 10 new test functions covering all native log message paths
2. Tests cover: NativeLog (success + missing session), NativeLogCaptureStarted (success + session gone), NativeLogCaptureStopped, and `maybe_start_native_log_capture` (success, tools unavailable, disabled, already running, unsupported platform)
3. All new tests pass: `cargo test -p fdemon-app -- native_log`
4. No regressions: `cargo test -p fdemon-app --lib` passes
5. `cargo clippy -p fdemon-app -- -D warnings` passes

### Testing

Run: `cargo test -p fdemon-app -- native_log --nocapture` to verify all new tests pass with visible output.

### Notes

- Follow the existing naming convention: `test_<message_variant>_<scenario>` (e.g., `test_native_log_creates_log_entry_with_native_source`).
- `NativeLogEvent` is imported from `fdemon_daemon::NativeLogEvent` (re-exported via `fdemon_daemon::native_logs`).
- The `JoinHandle` in `NativeLogCaptureStarted` is wrapped in `Arc<Mutex<Option<JoinHandle<()>>>>` (the `SharedTaskHandle` pattern). For tests, use `Arc::new(Mutex::new(None))` when no actual task is needed.
- Use `flush_all_pending_logs(&mut state)` before asserting log contents — native logs go through the `LogBatcher` (batched queue) path.
- The `maybe_start_native_log_capture` function is in `handler::session` — import as `super::session::maybe_start_native_log_capture` from the tests module.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/handler/tests.rs` | Added 10 new unit tests for native log message handling plus 2 test helper functions (`android_device`, `linux_device`, `attach_native_log_shutdown`) |

### Notable Decisions/Tradeoffs

1. **`SessionId::new()` does not exist**: `SessionId` is a `u64` type alias, not a struct. Used `u64::MAX` as a guaranteed-missing session ID in the "missing session" tests — no real session counter will reach that value.
2. **`attach_native_log_shutdown` helper**: Followed the exact same pattern as `attach_perf_shutdown` and `attach_network_shutdown` to stay consistent with existing test infrastructure.
3. **Tokio runtime for JoinHandle tests**: Tests that need a real `JoinHandle` use `tokio::runtime::Runtime::new().unwrap().block_on(...)` (same approach as `test_close_session_cleans_up_network_monitoring`).

### Testing Performed

- `cargo test -p fdemon-app -- native_log --nocapture` - Passed (21 tests, 10 new handler tests + 11 pre-existing)
- `cargo test -p fdemon-app --lib` - Passed (1474 tests, 0 failed)
- `cargo clippy -p fdemon-app -- -D warnings` - Passed (no warnings)
- `cargo fmt --all` - Passed

### Risks/Limitations

1. **macOS-only `macos` platform test not added**: `native_logs_available("macos")` is gated behind `#[cfg(target_os = "macos")]` in `ToolAvailability`, making a cross-platform test for macOS log capture non-trivial. The 10 required tests cover all other paths; a macOS-specific test could be added as a follow-up with `#[cfg(target_os = "macos")]`.
