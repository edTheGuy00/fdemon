## Task: Add PermanentlyDisconnected Propagation Test

**Objective**: Add a handler-level test for the full failure path: `VmServiceReconnecting` followed by `VmServiceDisconnected` (triggered when `PermanentlyDisconnected` breaks the forwarding loop). This covers the terminal failure scenario not exercised by the existing 4 tests.

**Depends on**: None

**Review Reference**: Phase-2 Review Issue #5

### Scope

- `crates/fdemon-app/src/handler/tests.rs`: Add new test

### Details

The four existing reconnection tests cover:
- `test_vm_service_reconnecting_sets_connection_status` (line 3320)
- `test_vm_service_reconnecting_ignores_inactive_session` (line 3354)
- `test_vm_service_connected_after_reconnecting_resets_status` (line 3383)
- `test_vm_service_reconnecting_progressive_attempts` (line 3417)

Missing: the path where reconnection **fails permanently** — the user sees "Reconnecting (N/10)..." and then the connection is lost entirely.

In `forward_vm_events` (`actions.rs:1020-1042`), `PermanentlyDisconnected` breaks the loop, which falls through to send `Message::VmServiceDisconnected { session_id }`.

**Test: `test_vm_service_disconnected_after_reconnecting_clears_status`**

```rust
#[test]
fn test_vm_service_disconnected_after_reconnecting_clears_status() {
    // Setup: create state with active session, VM connected
    // Step 1: Send VmServiceReconnecting — verify connection_status is Reconnecting
    // Step 2: Send VmServiceDisconnected — verify connection_status resets to Disconnected
    // Also verify: vm_connected == false, vm_request_handle cleared
}
```

### Acceptance Criteria

1. New test `test_vm_service_disconnected_after_reconnecting_clears_status` passes
2. Test verifies `connection_status` transitions: `Connected` -> `Reconnecting` -> `Disconnected`
3. Test verifies per-session state is cleaned up (`vm_connected`, `vm_request_handle`)
4. `cargo test -p fdemon-app` passes

### Notes

- Follow the pattern of `test_vm_service_connected_after_reconnecting_resets_status` (line 3383) which tests the happy path
- This test exercises the sad path where reconnection exhausts all attempts
