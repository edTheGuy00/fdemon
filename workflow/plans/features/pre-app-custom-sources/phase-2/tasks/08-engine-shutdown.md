## Task: Add Shared Source Cleanup to Engine Shutdown

**Objective**: Ensure shared custom sources are gracefully shut down when fdemon quits, alongside the existing per-session cleanup.

**Depends on**: 02-shared-source-handle, 04-tea-handlers

### Scope

- `crates/fdemon-app/src/engine.rs`: Add shared source shutdown in `Engine::shutdown()`

### Details

#### 1. Call `shutdown_shared_sources()` in Engine Shutdown

In `Engine::shutdown()`, add shared source cleanup alongside the existing per-session cleanup (which was added in Phase 1 followup):

```rust
pub async fn shutdown(&mut self) {
    // ... existing: notify plugins, emit shutdown, stop DAP, stop watcher ...

    // Gracefully shut down native logs and custom sources for all sessions.
    for handle in self.state.session_manager.iter_mut() {
        handle.shutdown_native_logs();
    }

    // Shut down shared custom sources (project-level, not per-session).
    self.state.shutdown_shared_sources();

    // Signal all background tasks to stop
    let _ = self.shutdown_tx.send(true);

    // ... existing: drain session tasks with timeout ...
}
```

The order matters: per-session sources first, then shared sources (a shared source might be serving multiple sessions), then the global shutdown signal.

### Acceptance Criteria

1. `shutdown_shared_sources()` is called in `Engine::shutdown()` before `shutdown_tx.send(true)`
2. Shared source shutdown signals are sent (graceful stop)
3. Shared source task handles are aborted (fallback)
4. `shared_source_handles` is drained after shutdown
5. All existing tests pass

### Testing

- This is a 2-line integration point — testing is covered by the integration tests in task 09
- Verify by inspection that the call is placed after per-session shutdown but before `shutdown_tx.send(true)`

### Notes

- The ordering (per-session → shared → global signal) ensures shared sources receive their individual shutdown signal before the blanket `shutdown_tx` fires
- `shutdown_shared_sources()` is synchronous (fire-and-forget signal + abort) — no `.await` needed

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/engine.rs` | Added `self.state.shutdown_shared_sources()` call in `Engine::shutdown()` after per-session native log shutdown and before `shutdown_tx.send(true)`, with explanatory comment |

### Notable Decisions/Tradeoffs

1. **Comment clarity**: Added a multi-line comment explaining the ordering constraint (per-session → shared → global signal) to match the rationale documented in the task. This mirrors the existing comment style already present in the shutdown method.

### Testing Performed

- `cargo check -p fdemon-app` - Passed
- `cargo test -p fdemon-app` - Passed (1685 tests, 0 failed, 4 ignored)

### Risks/Limitations

1. **None**: This is a single synchronous call insertion at a well-defined point in the shutdown sequence. The method `shutdown_shared_sources()` was already implemented and tested in task 02, so this task is purely wiring it into the engine lifecycle.
