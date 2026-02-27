## Task: Add Unit Tests for Phase 3 Health Monitoring

**Objective**: Add comprehensive unit tests covering the process watchdog, VM heartbeat failure handling, exit code capture, and downstream handler behavior with real exit codes.

**Depends on**: 01-process-watchdog, 02-get-version-rpc, 03-vm-heartbeat, 04-wait-for-exit-task

### Scope

- `crates/fdemon-daemon/src/vm_service/protocol.rs`: Tests for `VersionInfo` deserialization
- `crates/fdemon-daemon/src/process.rs`: Tests for exit code capture and `wait_for_exit` behavior
- `crates/fdemon-app/src/handler/tests.rs`: Tests for `handle_session_exited` with non-None exit codes
- `crates/fdemon-app/src/actions.rs` (or adjacent test file): Tests for watchdog/heartbeat constants

### Details

#### 1. VersionInfo Deserialization Tests

Location: `crates/fdemon-daemon/src/vm_service/protocol.rs` (inline `#[cfg(test)]` module)

```rust
#[test]
fn test_version_info_deserialize_with_type_field() {
    // Real VM Service response includes "type": "Version"
    let json = serde_json::json!({
        "type": "Version",
        "major": 4,
        "minor": 16
    });
    let info: VersionInfo = serde_json::from_value(json).unwrap();
    assert_eq!(info.major, 4);
    assert_eq!(info.minor, 16);
}

#[test]
fn test_version_info_deserialize_minimal() {
    let json = serde_json::json!({ "major": 3, "minor": 0 });
    let info: VersionInfo = serde_json::from_value(json).unwrap();
    assert_eq!(info.major, 3);
    assert_eq!(info.minor, 0);
}

#[test]
fn test_version_info_missing_fields_fails() {
    let json = serde_json::json!({ "major": 4 });
    assert!(serde_json::from_value::<VersionInfo>(json).is_err());
}
```

#### 2. Exit Code Handling Tests

Location: `crates/fdemon-app/src/handler/tests.rs`

The existing `handle_session_exited` function has three match arms for `code`:
- `Some(0)` → "Flutter process exited normally"
- `Some(n)` → "Flutter process exited with code N"
- `None` → "Flutter process exited"

Currently only `None` is exercised. Add:

```rust
#[test]
fn test_session_exited_with_code_zero() {
    // Setup: create state with an active session
    // Action: send DaemonEvent::Exited { code: Some(0) }
    // Assert: session log contains "Flutter process exited normally"
    // Assert: session phase == AppPhase::Stopped
}

#[test]
fn test_session_exited_with_nonzero_code() {
    // Setup: create state with an active session
    // Action: send DaemonEvent::Exited { code: Some(1) }
    // Assert: session log contains "Flutter process exited with code 1"
    // Assert: session phase == AppPhase::Stopped
}

#[test]
fn test_session_exited_with_none_code() {
    // This likely already exists — verify it covers the "Flutter process exited" message
}
```

Follow the existing test patterns in `handler/tests.rs` (look at `test_handle_session_exited_*` tests near the session exit test section).

#### 3. Process Watchdog Constant Validation

Location: `crates/fdemon-app/src/actions.rs` (inline test module)

```rust
#[test]
fn test_watchdog_interval_is_reasonable() {
    assert_eq!(PROCESS_WATCHDOG_INTERVAL, Duration::from_secs(5));
}
```

#### 4. Heartbeat Constant Validation

Location: `crates/fdemon-app/src/actions.rs` (inline test module)

```rust
#[test]
fn test_heartbeat_constants_are_reasonable() {
    assert_eq!(HEARTBEAT_INTERVAL, Duration::from_secs(30));
    assert_eq!(HEARTBEAT_TIMEOUT, Duration::from_secs(5));
    assert_eq!(MAX_HEARTBEAT_FAILURES, 3);
    // Detection time = interval * max_failures = 90s
    assert!(HEARTBEAT_INTERVAL.as_secs() * MAX_HEARTBEAT_FAILURES as u64 <= 120,
        "Heartbeat detection time should be at most 2 minutes");
}
```

#### 5. FlutterProcess Exit Code Tests

Location: `crates/fdemon-daemon/src/process.rs` (inline `#[cfg(test)]` module)

These tests depend on the task 04 refactor. The specifics will vary based on the chosen approach (`Arc<Mutex<Child>>` vs oneshot channel), but the test scenarios are:

```rust
#[tokio::test]
async fn test_wait_for_exit_captures_zero_exit_code() {
    // Spawn `true` (exits with 0)
    // Verify DaemonEvent::Exited { code: Some(0) } arrives on the channel
}

#[tokio::test]
async fn test_wait_for_exit_captures_nonzero_exit_code() {
    // Spawn `false` (exits with 1)
    // Verify DaemonEvent::Exited { code: Some(1) } arrives on the channel
}

#[tokio::test]
async fn test_stdout_reader_does_not_emit_exited() {
    // Spawn a process that exits
    // Verify stdout_reader does NOT send DaemonEvent::Exited
    // Only wait_for_exit sends it
}
```

Note: These tests spawn real subprocesses (`true`, `false`, `echo`). Use short-lived commands to keep tests fast.

#### 6. VmServiceDisconnected on Heartbeat Failure (Handler)

Location: `crates/fdemon-app/src/handler/tests.rs`

The `VmServiceDisconnected` handler already has test coverage. Verify that the disconnect → cleanup flow works the same regardless of whether disconnect was triggered by heartbeat failure or WebSocket close:

```rust
#[test]
fn test_vm_service_disconnected_cleans_up_devtools_tasks() {
    // This likely already exists. Verify it covers:
    // - perf_task_handle cleared
    // - perf_shutdown_tx cleared
    // - network_task_handle cleared
    // - network_shutdown_tx cleared
    // - session.vm_connected == false
}
```

### Acceptance Criteria

1. `VersionInfo` deserialization tests pass (valid, minimal, and missing-field cases)
2. `handle_session_exited` tested with `Some(0)`, `Some(1)`, and `None` exit codes
3. Watchdog and heartbeat constants have validation tests
4. If task 04 is implemented: `wait_for_exit` task tests verify exit code capture
5. All new tests pass: `cargo test --workspace`
6. `cargo clippy --workspace -- -D warnings` clean

### Testing

This IS the testing task. Run:

```bash
cargo test --workspace
cargo clippy --workspace -- -D warnings
```

### Notes

- Tests should be added incrementally as tasks 01-04 land, not all at once
- The async process tests may be flaky if `true`/`false` commands are not available — use `sh -c "exit 0"` / `sh -c "exit 1"` as more portable alternatives
- Follow existing test naming convention: `test_<function>_<scenario>_<expected_outcome>`
- The heartbeat and watchdog are difficult to test in isolation because they're embedded in async loops. Focus tests on: (a) the constants, (b) the downstream handlers that process events, (c) the data types

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/actions.rs` | Added `#[cfg(test)] mod tests` with `test_watchdog_interval_is_reasonable` and `test_heartbeat_constants_are_reasonable` |
| `crates/fdemon-app/src/handler/tests.rs` | Added `test_session_exited_with_code_zero`, `test_session_exited_with_none_code`, and `test_vm_service_disconnected_cleans_up_devtools_tasks` |

### Notable Decisions/Tradeoffs

1. **Skipped already-covered tests**: `VersionInfo` deserialization tests in `protocol.rs` already exist with names `test_version_info_deserialize`, `test_version_info_deserialize_minimal`, and `test_version_info_deserialize_missing_fields_fails` — they cover the same scenarios as requested. No duplication added.

2. **Skipped already-covered exit code tests**: `test_session_exited_with_error_code` (Some(1)) and `test_session_exited_updates_session_phase` (Some(0)) already exist. Only the "exited normally" message assertion for Some(0) and the None code test were missing — both were added.

3. **VmServiceDisconnected comprehensive cleanup test**: The task spec requested a test checking all four cleanup fields (`perf_task_handle`, `perf_shutdown_tx`, `network_task_handle`, `network_shutdown_tx`). Existing tests only checked subsets. The new test `test_vm_service_disconnected_cleans_up_devtools_tasks` covers all four fields plus `vm_connected`.

4. **process.rs tests**: All 5 required process exit tests (`test_exit_code_captured_on_normal_exit`, `test_exit_code_captured_on_error_exit`, `test_stdout_reader_does_not_emit_exited_event`, etc.) were already implemented by task 04. No additions needed.

### Testing Performed

- `cargo check --workspace` - Passed
- `cargo test -p fdemon-app` - Passed (1,141 tests, 5 new tests included)
- `cargo test --workspace` - Passed (all crates)
- `cargo clippy --workspace -- -D warnings` - Passed (no warnings)
- `cargo fmt --all` - Passed (formatter applied)

### Risks/Limitations

1. **No async process tests added**: The process watchdog and heartbeat tasks run in async loops embedded in `spawn_session`; they are not unit-testable in isolation. The constant validation tests provide the primary safety net for these parameters.
