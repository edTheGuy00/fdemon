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

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/handler/tests.rs` | Added 3 new gap-filling tests at end of shared source section |

### Notable Decisions/Tradeoffs

1. **Gap analysis before adding**: Audited all 10 task test cases against existing tests in `handler/tests.rs`, `launch_context.rs`, and `state.rs`. Found that 7 of 10 were already implemented under different (but equivalent) names by previous tasks (04, 07, 08). Only 3 genuine gaps remained in `handler/tests.rs`.

2. **Tests added (gaps)**:
   - `test_shared_source_started_stores_on_app_state` — verifies `state.shared_source_handles` gains one entry AND that no `SessionHandle.custom_source_handles` entry is created
   - `test_shared_source_survives_session_close` — removes session via `session_manager.remove_session()` + `shutdown_native_logs()` and asserts the shared handle persists on `AppState`
   - `test_non_shared_source_still_per_session` — sends `CustomSourceStarted` (non-shared) and asserts handle lands on `SessionHandle.custom_source_handles` while `state.shared_source_handles` stays empty

3. **No duplication**: Tests 2, 3, 5, 10 were covered by existing handler tests with equivalent assertions. Tests 6, 7 were covered in `launch_context.rs` tests. Test 9 was covered in `state.rs` tests. Avoided adding duplicates per task instructions.

### Testing Performed

- `cargo check -p fdemon-app` - Passed
- `cargo test -p fdemon-app -- test_shared_source_started_stores_on_app_state test_shared_source_survives_session_close test_non_shared_source_still_per_session` - Passed (3 tests)
- `cargo test -p fdemon-app` - Passed (1694 unit tests, 0 failures)
- `cargo clippy -p fdemon-app -- -D warnings` - Passed (no warnings)

### Risks/Limitations

1. **Test count discrepancy**: The task says "All 10 test cases pass" but 7 already existed under different names from prior tasks. The 3 gaps added here fill the missing coverage. The acceptance criteria for behavior coverage is fully met.
