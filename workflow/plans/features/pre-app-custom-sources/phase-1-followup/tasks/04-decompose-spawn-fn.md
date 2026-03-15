## Task: Decompose `spawn_pre_app_sources` Function

**Objective**: Extract the per-source spawn logic from `spawn_pre_app_sources` (~273 lines) into a private helper function to meet the 50-line function guideline.

**Depends on**: None

**Severity**: Minor

### Scope

- `crates/fdemon-app/src/actions/native_logs.rs`: Extract helper from lines 469-663

### Details

`spawn_pre_app_sources` (lines 429-701) has five logical sections. The per-source loop body (Section C, lines 469-663) is the largest and most cohesive extraction candidate.

#### Suggested Extraction

Extract the per-source loop body into a helper:

```rust
/// Spawns a single pre-app custom source, its forwarding task, and optionally
/// registers its readiness check future into the JoinSet.
fn spawn_one_pre_app_source(
    source_config: &CustomSourceConfig,
    session_id: SessionId,
    project_path: &Path,
    settings: &NativeLogsSettings,
    msg_tx: &mpsc::Sender<Message>,
    join_set: &mut JoinSet<(String, ReadyCheckResult)>,
    sources_with_checks: &mut usize,
)
```

The main function then becomes a compact loop:

```rust
for source_config in pre_app_sources {
    spawn_one_pre_app_source(
        &source_config, session_id, &project_path, &settings,
        &msg_tx, &mut join_set, &mut sources_with_checks,
    );
}
```

Followed by the coordinator await logic (Section D) and gate release (Section E), which are already compact.

### Acceptance Criteria

1. `spawn_pre_app_sources` body is under 50 lines after extraction
2. New helper function is private (no visibility change)
3. All existing tests pass unchanged
4. No behavioral change — pure refactor

### Notes

- The `ready_rx` oneshot channel is created and consumed within the same loop iteration, so it moves cleanly into the helper
- The helper will need to be `async` since it calls `CustomLogCapture::new().spawn_with_readiness()`
- `join_set` and `sources_with_checks` are passed as `&mut` since the helper pushes futures and increments the counter

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/actions/native_logs.rs` | Added `use std::path::Path` and `use crate::config::CustomSourceConfig` imports; extracted per-source loop body into `async fn spawn_one_pre_app_source`; extracted spawned async block into `async fn run_pre_app_sources_coordinator`; `spawn_pre_app_sources` body reduced to 37 lines |

### Notable Decisions/Tradeoffs

1. **Two extractions instead of one**: The task specified extracting the per-source loop body into `spawn_one_pre_app_source`. After that extraction the `spawn_pre_app_sources` body was still ~80 lines because the `tokio::spawn(async move { ... })` block (containing the coordinator logic) remained. A second extraction into `run_pre_app_sources_coordinator` was needed to meet the under-50-line criterion. Both helpers are private (`async fn` without `pub`).

2. **`run_pre_app_sources_coordinator` takes owned values**: The coordinator is spawned as a free `async fn` via `tokio::spawn(run_pre_app_sources_coordinator(...))`, so it takes owned `Vec<CustomSourceConfig>`, `PathBuf`, `NativeLogsSettings` etc. rather than references. This matches the move-semantics of the original `tokio::spawn(async move { ... })` block.

### Testing Performed

- `cargo check -p fdemon-app` - Passed
- `cargo test -p fdemon-app` - Passed (1646 tests)
- `cargo clippy -p fdemon-app -- -D warnings` - Passed (no warnings)
- `cargo fmt -p fdemon-app -- --check` - Passed (after applying fmt)

### Risks/Limitations

1. **No risk**: Pure refactor — no logic moved across async boundaries, no behavior change. The `run_pre_app_sources_coordinator` function is exactly the moved body of the previous `tokio::spawn(async move {...})` block.
