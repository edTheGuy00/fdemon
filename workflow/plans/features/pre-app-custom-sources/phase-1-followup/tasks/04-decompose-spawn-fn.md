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
