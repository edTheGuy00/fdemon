## Task: Integration Tests for Shared Custom Sources

**Objective**: Add comprehensive tests verifying the full shared source lifecycle: single-spawn, log broadcast, session-close survival, and engine-shutdown cleanup.

**Depends on**: 04-tea-handlers, 05-spawn-shared-pre-app, 06-spawn-shared-post-app, 07-pre-app-gate-skip, 08-engine-shutdown

### Scope

- `crates/fdemon-app/src/handler/tests.rs`: Add test module for shared source behavior

### Details

Tests should cover the following scenarios using the existing test harness pattern (create `AppState`, send `Message` variants, assert state changes):

#### Test Cases

1. **`test_shared_source_started_stores_on_app_state`**
   - Send `SharedSourceStarted` → assert `state.shared_source_handles` has one entry
   - Verify the handle is NOT on any `SessionHandle.custom_source_handles`

2. **`test_shared_source_log_broadcasts_to_all_sessions`**
   - Create two sessions in `session_manager`
   - Send `SharedSourceLog` with a log event
   - Assert both sessions received the log entry

3. **`test_shared_source_log_no_sessions_is_noop`**
   - Send `SharedSourceLog` with no sessions in manager
   - Assert no panic, `UpdateResult::none()`

4. **`test_shared_source_survives_session_close`**
   - Send `SharedSourceStarted` to register handle
   - Close a session (remove from manager, call `shutdown_native_logs`)
   - Assert `state.shared_source_handles` still has the entry

5. **`test_shared_source_stopped_removes_and_warns_all`**
   - Create two sessions, send `SharedSourceStarted`
   - Send `SharedSourceStopped`
   - Assert handle removed from `state.shared_source_handles`
   - Assert both sessions got a warning log

6. **`test_launch_with_shared_pre_app_already_running_skips_gate`**
   - Pre-populate `state.shared_source_handles` with a running shared source
   - Trigger launch → assert `UpdateAction::SpawnSession` (not `SpawnPreAppSources`)

7. **`test_launch_with_shared_pre_app_not_running_gates`**
   - Configure shared pre-app source, empty `shared_source_handles`
   - Trigger launch → assert `UpdateAction::SpawnPreAppSources`

8. **`test_non_shared_source_still_per_session`**
   - Configure a non-shared custom source
   - Send `CustomSourceStarted` → assert it's on `SessionHandle.custom_source_handles`
   - Assert `state.shared_source_handles` is empty

9. **`test_shutdown_shared_sources_drains_all`**
   - Add two shared source handles to `state.shared_source_handles`
   - Call `state.shutdown_shared_sources()`
   - Assert vec is empty

10. **`test_shared_source_tag_appears_in_tag_filter`**
    - Send `SharedSourceLog` to a session
    - Assert `handle.native_tag_state` has observed the shared source tag

### Acceptance Criteria

1. All 10 test cases pass
2. Tests are self-contained (no real process spawning — use mock channels)
3. No regressions in existing test suite
4. `cargo test -p fdemon-app` passes cleanly

### Notes

- Follow the existing test patterns in `handler/tests.rs` for creating `AppState`, `SessionManager`, and `Device` fixtures
- Use `tokio::sync::watch::channel(false)` for mock shutdown senders
- Use `Arc<Mutex<Option<JoinHandle>>>` with a no-op spawned task for mock task handles
