## Task: Add unit tests for network cleanup in exit and AppStop paths

**Objective**: Add unit tests verifying that `handle_session_exited` and the `AppStop` handler both properly clean up `network_task_handle` and `network_shutdown_tx`, closing the test coverage gap.

**Depends on**: 01-network-cleanup-exit, 02-network-cleanup-appstop

### Scope

- `crates/fdemon-app/src/handler/tests.rs`: Add two new test functions

### Details

**Coverage gap:** The existing test suite has perf cleanup tests for all paths but network cleanup tests only for `CloseCurrentSession`. After tasks 01 and 02 add the cleanup code, this task adds the tests.

**Test coverage before Phase 1:**

| Trigger | Perf tested | Network tested |
|---------|:-----------:|:--------------:|
| `CloseCurrentSession` | ✅ (line 4225) | ✅ (line 4249) |
| `handle_session_exited` | ✅ (line 4305) | ❌ **Add here** |
| `AppStop` | ✅ (line 4335) | ❌ **Add here** |

**Existing helpers to reuse:**

1. `attach_network_shutdown(state, session_id) -> watch::Receiver<bool>` (line 5145) — creates a `watch::channel(false)`, wraps sender in `Arc`, stores it on `handle.network_shutdown_tx`, returns the receiver for assertion. Note: this helper only sets `network_shutdown_tx`, not `network_task_handle`.

2. `test_device(id, name) -> Device` — standard test device constructor.

### Test 1: `test_session_exited_cleans_up_network_monitoring`

Mirror the structure of `test_session_exited_signals_perf_shutdown` (line 4305):

```rust
#[test]
fn test_session_exited_cleans_up_network_monitoring() {
    let device = test_device("dev-1", "Device 1");
    let mut state = AppState::new();
    let session_id = state.session_manager.create_session(&device).unwrap();

    let mut network_rx = attach_network_shutdown(&mut state, session_id);

    // Action
    super::session::handle_session_exited(&mut state, session_id, Some(0));

    // Assert: shutdown signal was sent
    assert!(
        *network_rx.borrow_and_update(),
        "network_shutdown_tx should be signaled on handle_session_exited"
    );

    // Assert: field was cleared
    let handle = state.session_manager.get(session_id).unwrap();
    assert!(
        handle.network_shutdown_tx.is_none(),
        "network_shutdown_tx should be cleared after process exit"
    );
    assert!(
        handle.network_task_handle.is_none(),
        "network_task_handle should be None after process exit"
    );
}
```

**Placement:** Immediately after `test_session_exited_signals_perf_shutdown` (after line ~4332) to keep related tests grouped.

### Test 2: `test_app_stop_cleans_up_network_monitoring`

Mirror the structure of `test_app_stop_signals_perf_shutdown` (line 4335):

```rust
#[test]
fn test_app_stop_cleans_up_network_monitoring() {
    use fdemon_core::{AppStart, AppStop, DaemonMessage};

    let mut state = AppState::new();
    let device = test_device("dev-1", "Device 1");
    let session_id = state.session_manager.create_session(&device).unwrap();

    // Mark session as started with a known app_id
    let start_msg = DaemonMessage::AppStart(AppStart {
        app_id: "test-app".to_string(),
        device_id: "dev-1".to_string(),
        directory: "/tmp/app".to_string(),
        launch_mode: None,
        supports_restart: true,
    });
    super::session::handle_session_message_state(&mut state, session_id, &start_msg);

    let mut network_rx = attach_network_shutdown(&mut state, session_id);

    // Action
    let stop_msg = DaemonMessage::AppStop(AppStop {
        app_id: "test-app".to_string(),
        error: None,
    });
    super::session::handle_session_message_state(&mut state, session_id, &stop_msg);

    // Assert: shutdown signal was sent
    assert!(
        *network_rx.borrow_and_update(),
        "network_shutdown_tx should be signaled on AppStop"
    );

    // Assert: field was cleared
    let handle = state.session_manager.get(session_id).unwrap();
    assert!(
        handle.network_shutdown_tx.is_none(),
        "network_shutdown_tx should be cleared after AppStop"
    );
    assert!(
        handle.network_task_handle.is_none(),
        "network_task_handle should be None after AppStop"
    );
}
```

**Placement:** Immediately after `test_app_stop_signals_perf_shutdown` (after line ~4379) to keep related tests grouped.

### Acceptance Criteria

1. `test_session_exited_cleans_up_network_monitoring` passes — verifies `network_shutdown_tx` is signaled and cleared
2. `test_app_stop_cleans_up_network_monitoring` passes — verifies `network_shutdown_tx` is signaled and cleared
3. Both tests verify `network_task_handle` is `None` after cleanup
4. All existing tests still pass (`cargo test -p fdemon-app`)
5. `cargo clippy -p fdemon-app -- -D warnings` clean

### Notes

- These tests are synchronous (no `rt.block_on` needed) because they only check the `watch::channel` signal and `Option` state, not a real `JoinHandle`. The existing `CloseCurrentSession` network test at line 4249 uses `rt.block_on` because it creates a real `JoinHandle` — but that's testing the abort path specifically. For these tests, confirming the shutdown signal is sufficient since the production code also does `.take().abort()` before the signal.
- The `attach_network_shutdown` helper at line 5145 only sets `network_shutdown_tx`, not `network_task_handle`. Since `SessionHandle::new()` initializes `network_task_handle` to `None`, and the cleanup code uses `.take()` (which is a no-op on `None`), the assertion `handle.network_task_handle.is_none()` will pass naturally. This is correct — it confirms the code path doesn't panic on `None` handles.
- If desired, a more thorough async test with a real `JoinHandle` (like the `CloseCurrentSession` test) could be added, but it's not strictly necessary — the abort logic is identical across all cleanup paths and is already tested by the existing `CloseCurrentSession` test.

---

## Completion Summary

**Status:** Not started
