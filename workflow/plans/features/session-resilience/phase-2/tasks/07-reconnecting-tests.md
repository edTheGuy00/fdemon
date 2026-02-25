## Task: Add unit tests for reconnection message flow

**Objective**: Add unit tests verifying that the reconnection event pipeline works end-to-end: the app handler correctly updates `VmConnectionStatus` on `VmServiceReconnecting` messages, and the status resets on `VmServiceConnected`.

**Depends on**: 05-emit-reconnect-events, 06-forward-events-update

### Scope

- `crates/fdemon-app/src/handler/tests.rs`: Add test functions for reconnection message handling
- `crates/fdemon-app/src/handler/devtools/mod.rs`: Verify existing `#[cfg(test)]` block (add tests if inline tests exist for related handlers)

### Details

**Coverage gap:** The `handle_vm_service_reconnecting` handler (devtools/mod.rs:242-258) and `VmServiceConnected` handler (update.rs:1179-1254) have no tests verifying the reconnection status flow. The TUI widgets do have render tests for `VmConnectionStatus::Reconnecting`, but the handler logic that sets the status is untested.

**Existing test patterns to follow:**
- `test_vm_service_connected_sets_vm_connected` (handler/tests.rs) — tests `VmServiceConnected` message
- `test_vm_service_disconnected_clears_state` (handler/tests.rs) — tests `VmServiceDisconnected` message
- `test_session_exited_cleans_up_network_monitoring` (handler/tests.rs) — the Phase 1 pattern

**Existing helpers to reuse:**
- `test_device(id, name) -> Device` — standard test device constructor
- `AppState::new()` — creates a fresh state
- `state.session_manager.create_session(&device)` — creates a session
- `state.session_manager.select(session_id)` — makes a session the active/selected one

### Test 1: `test_vm_service_reconnecting_sets_connection_status`

Verifies that `handle_vm_service_reconnecting` updates `devtools_view_state.connection_status` to `Reconnecting` when the session is active.

```rust
#[test]
fn test_vm_service_reconnecting_sets_connection_status() {
    let mut state = AppState::new();
    let device = test_device("dev-1", "Device 1");
    let session_id = state.session_manager.create_session(&device).unwrap();
    state.session_manager.select(session_id);

    // Verify initial state
    assert_eq!(
        state.devtools_view_state.connection_status,
        VmConnectionStatus::Connected
    );

    // Action
    let result = update(
        &mut state,
        Message::VmServiceReconnecting {
            session_id,
            attempt: 2,
            max_attempts: 10,
        },
    );

    // Assert
    assert_eq!(
        state.devtools_view_state.connection_status,
        VmConnectionStatus::Reconnecting {
            attempt: 2,
            max_attempts: 10,
        }
    );
    assert!(result.action.is_none());
}
```

**Placement:** Group with other VM Service handler tests (near `test_vm_service_connected_sets_vm_connected`).

### Test 2: `test_vm_service_reconnecting_ignores_inactive_session`

Verifies that `handle_vm_service_reconnecting` does NOT update `connection_status` when the reconnecting session is not the currently selected session.

```rust
#[test]
fn test_vm_service_reconnecting_ignores_inactive_session() {
    let mut state = AppState::new();
    let device1 = test_device("dev-1", "Device 1");
    let device2 = test_device("dev-2", "Device 2");
    let session_1 = state.session_manager.create_session(&device1).unwrap();
    let session_2 = state.session_manager.create_session(&device2).unwrap();

    // Select session 2 (session 1 is inactive)
    state.session_manager.select(session_2);

    // Action: reconnecting event for inactive session 1
    update(
        &mut state,
        Message::VmServiceReconnecting {
            session_id: session_1,
            attempt: 3,
            max_attempts: 10,
        },
    );

    // Assert: connection_status should NOT be Reconnecting (it's for inactive session)
    assert_eq!(
        state.devtools_view_state.connection_status,
        VmConnectionStatus::Connected,
        "connection_status should not change for inactive session"
    );
}
```

### Test 3: `test_vm_service_connected_after_reconnecting_resets_status`

Verifies the full reconnection cycle: `Reconnecting` → `Connected`. The `VmServiceConnected` handler should reset `connection_status` back to `Connected`.

```rust
#[test]
fn test_vm_service_connected_after_reconnecting_resets_status() {
    let mut state = AppState::new();
    let device = test_device("dev-1", "Device 1");
    let session_id = state.session_manager.create_session(&device).unwrap();
    state.session_manager.select(session_id);

    // First: simulate reconnecting
    update(
        &mut state,
        Message::VmServiceReconnecting {
            session_id,
            attempt: 1,
            max_attempts: 10,
        },
    );
    assert_eq!(
        state.devtools_view_state.connection_status,
        VmConnectionStatus::Reconnecting {
            attempt: 1,
            max_attempts: 10,
        }
    );

    // Then: simulate successful reconnection
    update(
        &mut state,
        Message::VmServiceConnected { session_id },
    );

    // Assert: status should be back to Connected
    assert_eq!(
        state.devtools_view_state.connection_status,
        VmConnectionStatus::Connected,
    );
}
```

**Note:** The `VmServiceConnected` handler also triggers `StartPerformanceMonitoring` and may trigger `RequestWidgetTree`. The test should check the `result.action` if these side effects matter for the reconnection flow. At minimum, verify `connection_status` is reset.

### Test 4: `test_vm_service_reconnecting_progressive_attempts`

Verifies that successive `VmServiceReconnecting` messages update the attempt counter correctly.

```rust
#[test]
fn test_vm_service_reconnecting_progressive_attempts() {
    let mut state = AppState::new();
    let device = test_device("dev-1", "Device 1");
    let session_id = state.session_manager.create_session(&device).unwrap();
    state.session_manager.select(session_id);

    for attempt in 1..=3 {
        update(
            &mut state,
            Message::VmServiceReconnecting {
                session_id,
                attempt,
                max_attempts: 10,
            },
        );
        assert_eq!(
            state.devtools_view_state.connection_status,
            VmConnectionStatus::Reconnecting {
                attempt,
                max_attempts: 10,
            }
        );
    }
}
```

### Acceptance Criteria

1. `test_vm_service_reconnecting_sets_connection_status` passes — active session updates `connection_status`
2. `test_vm_service_reconnecting_ignores_inactive_session` passes — inactive session is a no-op
3. `test_vm_service_connected_after_reconnecting_resets_status` passes — full reconnection cycle verified
4. `test_vm_service_reconnecting_progressive_attempts` passes — attempt counter updates correctly
5. All existing tests still pass (`cargo test -p fdemon-app`)
6. `cargo clippy -p fdemon-app -- -D warnings` clean
7. `cargo fmt --all --check` clean

### Notes

- These tests exercise the TEA handler layer only (not the daemon emission or actions.rs forwarding). Daemon-to-handler integration would require a mock WebSocket server, which is out of scope for this phase.
- The `update()` function in tests.rs is the standard test entry point that dispatches to `handler::update()`. Use it consistently.
- `VmConnectionStatus` must be imported — check if it's already in scope in the test module. It's defined in `crates/fdemon-app/src/state.rs` and should be importable as `crate::state::VmConnectionStatus` or through the prelude.
- The handler only updates `connection_status` if the session is selected (active). This is tested explicitly in Test 2. If a future change makes status per-session rather than global, these tests will need updating.
- Test 3 tests the `VmServiceConnected` handler in a post-reconnection context. The handler has several side effects (perf monitoring restart, inspector refresh) — the test should at minimum verify `connection_status` is reset, and may optionally verify the action returned.

---

## Completion Summary

**Status:** Not started
