## Task: Add Tests for Reconnect Handler Fixes

**Objective**: Add unit tests verifying the three reconnect handler fixes: (1) `VmServiceReconnected` preserves performance state, (2) performance polling task is cleaned up on reconnection, and (3) background session VM events don't pollute foreground connection_status.

**Depends on**: 01-reconnected-message-variant, 02-cleanup-perf-on-reconnect, 03-guard-connection-status

**Review Reference**: Phase-2 Review Issues #2, #3, #6

### Scope

- `crates/fdemon-app/src/handler/tests.rs`: Add new test functions

### Details

#### Test 1: VmServiceReconnected preserves performance state

```rust
#[test]
fn test_vm_service_reconnected_preserves_performance_state() {
    // Setup: create state with active session, VM connected
    // Populate performance state with some data (memory_samples, frame_history, etc.)
    // Send Message::VmServiceReconnected { session_id }
    // Verify: performance state data is NOT wiped
    // Verify: vm_connected == true
    // Verify: connection_status == Connected
    // Verify: log contains "reconnected" (not "connected")
}
```

#### Test 2: VmServiceReconnected returns StartPerformanceMonitoring

```rust
#[test]
fn test_vm_service_reconnected_restarts_monitoring() {
    // Setup: create state with active session
    // Send Message::VmServiceReconnected { session_id }
    // Verify: UpdateAction is StartPerformanceMonitoring
}
```

#### Test 3: VmServiceConnected still resets performance (initial connection)

```rust
#[test]
fn test_vm_service_connected_still_resets_performance() {
    // Regression test: ensure VmServiceConnected still clears performance
    // (the original behavior for initial connection / hot-restart is preserved)
    // Setup: populate performance state with data
    // Send Message::VmServiceConnected { session_id }
    // Verify: performance state IS reset
}
```

#### Test 4: Performance task cleaned up before restart on reconnect

```rust
#[test]
fn test_vm_service_reconnected_cleans_up_perf_task() {
    // Setup: create state with active session
    // Set perf_task_handle and perf_shutdown_tx to Some(...)
    // Send Message::VmServiceReconnected { session_id }
    // Verify: perf_task_handle is None (was taken/aborted)
    // Verify: perf_shutdown_tx is None (was taken/signaled)
}
```

#### Test 5: Background session connect doesn't affect foreground status

```rust
#[test]
fn test_vm_service_connected_background_session_no_status_change() {
    // Setup: two sessions, session A active, session B background
    // Set connection_status to Reconnecting (simulating session A reconnecting)
    // Send Message::VmServiceConnected { session_id: B }
    // Verify: connection_status is STILL Reconnecting (not overwritten to Connected)
}
```

#### Test 6: Background session disconnect doesn't affect foreground status

```rust
#[test]
fn test_vm_service_disconnected_background_session_no_status_change() {
    // Setup: two sessions, session A active (Connected), session B background
    // Send Message::VmServiceDisconnected { session_id: B }
    // Verify: connection_status is STILL Connected (not overwritten to Disconnected)
}
```

#### Test 7: Background session connection failure doesn't show error

```rust
#[test]
fn test_vm_service_connection_failed_background_session_no_error() {
    // Setup: two sessions, session A active, session B background
    // Send Message::VmServiceConnectionFailed { session_id: B, error: "timeout" }
    // Verify: vm_connection_error is still None (not polluted by background session)
}
```

### Acceptance Criteria

1. All 7 tests pass
2. Tests cover the three fix areas: state preservation, task cleanup, multi-session guarding
3. Tests follow existing patterns in `handler/tests.rs` (state setup helpers, update() calls)
4. `cargo test -p fdemon-app` passes with no regressions
5. Multi-session test setup matches existing multi-session test patterns in the file

### Notes

- Look at existing reconnection tests starting at line 3320 for setup patterns
- For multi-session tests, check if there are existing helpers for creating two-session state — if not, create a minimal setup
- The perf task cleanup test (test 4) may need to create mock `JoinHandle` and `watch::Sender` — check how existing tests handle this
