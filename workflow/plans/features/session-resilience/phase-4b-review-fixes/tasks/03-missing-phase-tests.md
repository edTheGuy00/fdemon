## Task: Add missing `Quitting` and `Reloading` phase tests

**Objective**: Add test coverage for `Quitting` and `Reloading` phases at the `SessionManager` and `handle_launch` integration levels. Currently these phases are only tested at the `Session::is_active()` unit level.

**Depends on**: 01-remove-find-by-device-id (task 01 removes the `find_by_device_id` cross-check assertion in the existing tests)

### Scope

- `crates/fdemon-app/src/session_manager.rs`: Add 2 tests for `find_active_by_device_id`
- `crates/fdemon-app/src/handler/new_session/launch_context.rs`: Add 2 tests for `handle_launch`

### Details

#### 1. SessionManager tests (in `session_manager.rs` test module)

Add after the existing `test_find_active_by_device_id_*` tests:

```rust
#[test]
fn test_find_active_by_device_id_skips_quitting_session() {
    let mut manager = SessionManager::new();
    let id = manager
        .create_session(&test_device("dev1", "Device 1"))
        .unwrap();
    manager.get_mut(id).unwrap().session.phase = AppPhase::Quitting;
    assert!(manager.find_active_by_device_id("dev1").is_none());
}

#[test]
fn test_find_active_by_device_id_finds_reloading_session() {
    let mut manager = SessionManager::new();
    let id = manager
        .create_session(&test_device("dev1", "Device 1"))
        .unwrap();
    manager.get_mut(id).unwrap().session.phase = AppPhase::Reloading;
    assert_eq!(manager.find_active_by_device_id("dev1"), Some(id));
}
```

#### 2. `handle_launch` integration tests (in `launch_context.rs` test module)

Add after the existing device-reuse tests (around line 1397):

```rust
#[test]
fn test_handle_launch_allows_device_reuse_when_session_quitting() {
    // Same pattern as test_handle_launch_allows_device_reuse_when_session_stopped
    // but set phase to AppPhase::Quitting instead of Stopped
    // Should allow device reuse (Quitting is not active)
}

#[test]
fn test_handle_launch_blocks_device_with_reloading_session() {
    // Same pattern as test_handle_launch_blocks_device_with_running_session
    // but set phase to AppPhase::Reloading instead of Running
    // Should block device reuse (Reloading is active)
}
```

Follow the exact test patterns from the existing tests at lines 1284–1397 in `launch_context.rs`. The key difference is only the `AppPhase` variant set on the pre-existing session.

### Acceptance Criteria

1. `test_find_active_by_device_id_skips_quitting_session` passes
2. `test_find_active_by_device_id_finds_reloading_session` passes
3. `test_handle_launch_allows_device_reuse_when_session_quitting` passes
4. `test_handle_launch_blocks_device_with_reloading_session` passes
5. All existing tests still pass
6. `cargo clippy --workspace -- -D warnings` clean

### Testing

Run `cargo test -p fdemon-app` to verify all new and existing tests pass.

### Notes

- `AppPhase::Quitting` is currently unreachable on individual sessions (only set on `AppState.phase` globally). These tests are defensive coverage for correctness — if `Quitting` ever becomes reachable on sessions, the behavior is already verified.
- `AppPhase::Reloading` is an active phase and SHOULD block device reuse — a reload in progress means the Flutter process is still alive.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/session_manager.rs` | Added 2 tests: `test_find_active_by_device_id_skips_quitting_session` and `test_find_active_by_device_id_finds_reloading_session` after the existing `test_find_active_by_device_id_returns_none_for_unknown_device` test |
| `crates/fdemon-app/src/handler/new_session/launch_context.rs` | Added 2 tests: `test_handle_launch_allows_device_reuse_when_session_quitting` and `test_handle_launch_blocks_device_with_reloading_session` after the existing `test_handle_launch_blocks_device_with_initializing_session` test |

### Notable Decisions/Tradeoffs

1. **Exact pattern match**: Both sets of tests follow the identical structure as their nearest existing counterparts — only the `AppPhase` variant differs. This keeps the test suite consistent and the intent immediately readable.
2. **No new imports needed in session_manager.rs**: `AppPhase` was already in scope via `use fdemon_core::{AppPhase, LogSource}` in the test module.

### Testing Performed

- `cargo test -p fdemon-app -- find_active_by_device_id_skips_quitting find_active_by_device_id_finds_reloading handle_launch_allows_device_reuse_when_session_quitting handle_launch_blocks_device_with_reloading` - Passed (4 tests)
- `cargo test -p fdemon-app` - Passed (1160 tests)
- `cargo clippy --workspace -- -D warnings` - Passed (clean)

### Risks/Limitations

1. **Quitting phase reachability**: `AppPhase::Quitting` is currently unreachable on individual sessions. These tests are defensive/forward-looking coverage. If the phase is ever wired to per-session state, the behavior is already verified correct.
