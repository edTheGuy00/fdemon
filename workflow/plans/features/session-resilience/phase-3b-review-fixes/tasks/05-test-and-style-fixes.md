## Task: Test Cleanup and Style Fixes

**Objective**: Bundle of minor test hygiene and style improvements identified in the phase-3 review: remove duplicate test, rename tests to follow convention, add platform guards, and extract magic constant.

**Depends on**: Tasks 01, 02, 03, 04 (run after all implementation tasks are stable)

**Review Reference**: Phase-3 Review Issues #7, #8, #9, #10 and Review REVIEW.md Issue #5 (magic number)

### Scope

- `crates/fdemon-app/src/handler/tests.rs`: Remove duplicate test, rename tests
- `crates/fdemon-daemon/src/process.rs`: Add `#[cfg(unix)]` guards, extract `9999` constant

### Details

#### Fix 1: Remove duplicate test `test_session_exited_updates_session_phase`

**File:** `crates/fdemon-app/src/handler/tests.rs:655-672`

This test is a strict subset of the newer `test_session_exited_with_code_zero` (lines 698-730). Both send `DaemonEvent::Exited { code: Some(0) }` and assert `AppPhase::Stopped`. The newer test also asserts the log message content.

**Action:** Delete lines 655-672 (`test_session_exited_updates_session_phase`).

#### Fix 2: Rename new tests to follow naming convention

**File:** `crates/fdemon-app/src/handler/tests.rs`

Per CODE_STANDARDS.md, tests should follow `test_<function>_<scenario>_<expected_result>`. The phase-3 tests don't include the expected result:

| Current Name | Suggested Name |
|---|---|
| `test_session_exited_with_code_zero` | `test_handle_session_exited_code_zero_logs_normal_exit` |
| `test_session_exited_with_no_code` | `test_handle_session_exited_no_code_logs_unknown_exit` |
| `test_session_exited_with_nonzero_code` | `test_handle_session_exited_nonzero_code_logs_error` |
| `test_session_disconnect_cleans_up_vm_state` | `test_handle_session_exited_clears_vm_connected_and_shutdown_tx` |

Also rename the new test from task 03:
| `test_handle_session_exited_duplicate_exit_is_idempotent` | (already follows the convention) |

**Note:** Only rename tests introduced in phase 3/3b. Do not rename pre-existing tests in this task.

#### Fix 3: Add `#[cfg(unix)]` to platform-dependent test helpers

**File:** `crates/fdemon-daemon/src/process.rs`

The `spawn_test_process` helper (line 444) and `test_shutdown_kills_long_running_process` (line 597) use `sh -c` and `sleep`, which are POSIX-only. On Windows, these tests would panic at `.expect()`.

**Action:** Add `#[cfg(unix)]` to:
1. The `spawn_test_process` helper function (line 444)
2. All tests that call `spawn_test_process`:
   - `test_wait_for_exit_emits_exited_event_with_code` (line ~494)
   - `test_wait_for_exit_emits_none_for_signal_kill` (line ~520)
   - `test_has_exited_returns_true_after_process_exits` (line ~548)
   - `test_has_exited_returns_false_while_running` (line ~570)
   - `test_shutdown_kills_long_running_process` (line ~590)

Check the exact test names and line numbers before applying — they may have shifted.

#### Fix 4: Extract magic number `9999` as a named constant

**File:** `crates/fdemon-daemon/src/process.rs:333`

The shutdown command uses a hardcoded JSON-RPC request ID:
```rust
let shutdown_cmd = r#"{"method":"daemon.shutdown","id":9999}"#;
```

**Action:** Extract to a named constant at module level:
```rust
/// JSON-RPC request ID used for the `daemon.shutdown` command.
/// Chosen to be well above the `RequestTracker`'s sequential range (starting at 1)
/// to avoid collisions with in-flight requests.
const SHUTDOWN_REQUEST_ID: u32 = 9999;
```

Then use `format!` to construct the command:
```rust
let shutdown_cmd = format!(
    r#"{{"method":"daemon.shutdown","id":{}}}"#,
    SHUTDOWN_REQUEST_ID
);
```

**Note:** This is pre-existing code, not introduced by phase 3. It was flagged because it was touched during the `process.rs` refactor.

### Acceptance Criteria

1. `test_session_exited_updates_session_phase` is removed (no duplicate)
2. Phase-3 test names follow the `test_<function>_<scenario>_<expected_result>` convention
3. `spawn_test_process` and all dependent tests have `#[cfg(unix)]`
4. `SHUTDOWN_REQUEST_ID` constant replaces the hardcoded `9999`
5. No behaviour changes — all fixes are purely cosmetic/hygiene
6. `cargo check --workspace` passes
7. `cargo clippy --workspace -- -D warnings` clean
8. `cargo test --workspace` passes
9. `cargo fmt --all` passes

### Notes

- This task should be the last to execute since it touches test files that may be modified by tasks 01-04
- The test renames only apply to tests introduced in phase 3/3b — do not rename existing tests that predate this work
- The `#[cfg(unix)]` guards mean these tests will be skipped on Windows CI (if/when added). This is acceptable since the process spawning code itself is Unix-focused
- The `SHUTDOWN_REQUEST_ID` format! approach introduces a minor runtime cost (string formatting vs static string literal). This is negligible for a shutdown path that executes at most once per session

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/handler/tests.rs` | Removed duplicate `test_session_exited_updates_session_phase`; renamed 4 phase-3 tests to follow `test_<function>_<scenario>_<expected_result>` convention |
| `crates/fdemon-daemon/src/process.rs` | Added `SHUTDOWN_REQUEST_ID` constant replacing hardcoded `9999`; added `#[cfg(unix)]` to `spawn_test_process` helper and 5 dependent tests |

### Notable Decisions/Tradeoffs

1. **VmServiceDisconnected rename**: The task referenced `test_session_disconnect_cleans_up_vm_state` which doesn't exist. The actual phase-3 test was `test_vm_service_disconnected_cleans_up_devtools_tasks`. Renamed to `test_handle_vm_service_disconnected_clears_vm_connected_and_shutdown_tx` (not `test_handle_session_exited_*` as suggested) because the test exercises `VmServiceDisconnected`, not session exit — using `session_exited` in the name would be factually wrong.
2. **`test_session_exited_with_error_code` rename**: The task table listed `test_session_exited_with_nonzero_code` (doesn't exist); the actual pre-existing test was `test_session_exited_with_error_code`. This is a phase-3 test so it was correctly renamed to `test_handle_session_exited_nonzero_code_logs_error`.

### Testing Performed

- `cargo fmt --all` - Passed
- `cargo check --workspace` - Passed
- `cargo test --workspace` - Passed (all tests pass, no regressions)
- `cargo clippy --workspace -- -D warnings` - Passed (no warnings)

### Risks/Limitations

1. **None**: All changes are purely cosmetic/hygiene — test renames, platform guards, and constant extraction. No behaviour changes.
