## Task: Fix `has_unstarted_post_app` Guard for Shared Sources

**Objective**: Extend the `has_unstarted_post_app` check in `maybe_start_native_log_capture` to account for shared post-app sources stored on `AppState.shared_source_handles`, preventing spurious `StartNativeLogCapture` action dispatches on hot-restart.

**Depends on**: None

**Severity**: MINOR

**Review Reference**: [REVIEW.md](../../../../reviews/features/pre-app-custom-sources-phase-2/REVIEW.md) — "MINOR — `has_unstarted_post_app` guard blind spot for shared post-app sources"

### Scope

- `crates/fdemon-app/src/handler/session.rs`: Fix `running_names` construction in `maybe_start_native_log_capture` (~lines 319-330)
- `crates/fdemon-app/src/handler/tests.rs`: Add test for shared post-app guard

### Context

`maybe_start_native_log_capture` (`session.rs:287–414`) builds a `running_names` set from `handle.custom_source_handles` (per-session only) and checks if all post-app sources in config are running:

```rust
let running_names: std::collections::HashSet<&str> = handle
    .custom_source_handles       // ← per-session handles only
    .iter()
    .map(|h| h.name.as_str())
    .collect();
let has_unstarted_post_app = state
    .settings
    .native_logs
    .custom_sources
    .iter()
    .filter(|s| !s.start_before_app)
    .any(|s| !running_names.contains(s.name.as_str()));
```

Shared sources live in `state.shared_source_handles` (not `handle.custom_source_handles`), so a shared post-app source will **always** appear "unstarted" in this check, even when it's already running globally. This causes:

- Guard Branch A (line 333): `native_log_shutdown_tx.is_some() && !has_unstarted_post_app` — never fires when a shared post-app source is configured.
- Guard Branch B (line 342): same — never fires.

Result: `StartNativeLogCapture` is emitted on every hot-restart for configs with shared post-app sources. The downstream `spawn_custom_sources` correctly skips the shared source (via its own `running_shared_names` guard), so no duplicate process is created — but the action dispatch and the call into `spawn_native_log_capture` are unnecessary overhead.

### Details

Extend the `running_names` set to include shared source names from `state.shared_source_handles`. Only include shared sources that are **not** `start_before_app` (since the guard only filters post-app sources):

```rust
let mut running_names: std::collections::HashSet<&str> = handle
    .custom_source_handles
    .iter()
    .map(|h| h.name.as_str())
    .collect();

// Include shared post-app sources that are already running globally.
// These are stored on AppState, not on the per-session handle.
for shared_handle in &state.shared_source_handles {
    if !shared_handle.start_before_app {
        running_names.insert(shared_handle.name.as_str());
    }
}
```

The rest of the function remains unchanged. The `has_unstarted_post_app` computation at lines 324-330 will now correctly see shared post-app sources as "running" and the guard branches will fire appropriately.

**Alternative approach** (simpler, equivalent): Include ALL shared source names regardless of `start_before_app`, since the `has_unstarted_post_app` filter already applies `.filter(|s| !s.start_before_app)` to the config side. A shared pre-app source name in `running_names` wouldn't affect the result because it would never match a config entry that passes the `!s.start_before_app` filter. However, filtering on the insert side is clearer about intent.

### Acceptance Criteria

1. When all post-app sources (including shared ones) are already running, `maybe_start_native_log_capture` returns `None` on hot-restart (guard Branch A or B fires).
2. When a shared post-app source is configured but NOT yet running (not in `state.shared_source_handles`), the function still returns `Some(StartNativeLogCapture)` to trigger the spawn.
3. New test passes; all existing tests continue to pass.

### Testing

Add a test in `handler/tests.rs` following the pattern of `test_hot_restart_skips_duplicate_custom_sources` (line ~6971). The test should:

```rust
#[test]
fn test_guard_accounts_for_shared_post_app_sources() {
    let mut state = make_test_state();

    // Configure a shared post-app source
    state.settings.native_logs.custom_sources = vec![CustomSourceConfig {
        name: "my-shared-logger".to_string(),
        command: "tail".to_string(),
        args: vec!["-f".to_string(), "/tmp/log".to_string()],
        shared: true,
        start_before_app: false,
        ..Default::default()
    }];

    // Register the shared source as already running on AppState
    state.shared_source_handles.push(SharedSourceHandle {
        name: "my-shared-logger".to_string(),
        shutdown_tx: /* ... */,
        task_handle: None,
        start_before_app: false,
    });

    // Create a session with custom_source_handles empty (shared sources aren't stored here)
    // and native_log_shutdown_tx = Some (platform capture running)
    let session_id = /* ... */;
    // ... set up session handle with native_log_shutdown_tx.is_some() ...

    // Build an AppStart message
    let msg = DaemonMessage::AppStart(AppStartEvent { app_id: "app1".into(), .. });

    // Should return None — shared post-app source is already running
    let result = maybe_start_native_log_capture(&state, session_id, &msg);
    assert!(result.is_none());
}
```

Also add a complementary test that verifies the function **does** return `Some` when the shared post-app source is NOT in `state.shared_source_handles` (i.e., it genuinely needs to be started).

### Notes

- This fix only affects the guard in `maybe_start_native_log_capture`. The downstream `spawn_custom_sources` guards remain unchanged and continue to serve as a second layer of defense.
- The `running_shared_names` snapshot at lines 397-398 of the function (used by `StartNativeLogCapture` action) is a separate concern — it provides dedup information to the spawn side. This task only fixes the guard that decides whether to emit the action at all.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/handler/session.rs` | Changed `running_names` from `let` to `let mut` and added a loop to insert shared post-app source names from `state.shared_source_handles` |
| `crates/fdemon-app/src/handler/tests.rs` | Added `test_guard_accounts_for_shared_post_app_sources` and `test_guard_emits_action_when_shared_post_app_source_not_yet_running` |

### Notable Decisions/Tradeoffs

1. **Test uses Android + `native_log_shutdown_tx`**: The guard Branch A (`native_log_shutdown_tx.is_some() && !has_unstarted_post_app`) requires platform capture to be already running. The test uses an Android session with `adb: true` and `attach_native_log_shutdown` to set up this precondition, matching the task scaffold's note about `native_log_shutdown_tx = Some`.

2. **Complementary test uses Linux**: The second test (verifying `Some` is still returned when the source isn't yet running) uses a Linux device where `has_custom_sources = true` drives `should_start`, avoiding any platform-capture dependency. This keeps it simple and platform-independent.

3. **Filter on `!start_before_app`**: Shared sources are inserted into `running_names` only if `!start_before_app`, consistent with the task specification. This is the clearer-intent approach over inserting all shared source names.

### Testing Performed

- `cargo fmt --all` - Passed
- `cargo check -p fdemon-app` - Passed
- `cargo test -p fdemon-app` - Passed (1697 tests)
- `cargo clippy -p fdemon-app -- -D warnings` - Passed

### Risks/Limitations

1. **Branch B not exercised by new tests**: For pure "shared post-app source only" Linux sessions (no platform capture, no per-session handles), neither guard branch fires even after this fix — because Branch B requires `!handle.custom_source_handles.is_empty()`. The downstream `spawn_custom_sources` dedup guard continues to prevent duplicate processes in that case. The fix correctly addresses Branch A (Android/iOS/macOS) as described in the task.
