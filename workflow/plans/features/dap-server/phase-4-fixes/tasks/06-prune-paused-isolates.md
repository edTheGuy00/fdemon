## Task: Prune paused_isolates on IsolateExit

**Objective**: Remove dead isolate IDs from `paused_isolates` when an isolate exits, preventing stale evaluate context.

**Depends on**: 02-split-adapter-mod

**Severity**: Major

### Scope

- `crates/fdemon-dap/src/adapter/events.rs` (post-split; currently `adapter/mod.rs:928-967`)

### Details

**Current:** The `IsolateExit` handler removes the isolate from `thread_map` and `thread_names` but does NOT remove it from `paused_isolates`:

```rust
DebugEvent::IsolateExit { isolate_id } => {
    if let Some(thread_id) = self.thread_map.remove(&isolate_id) {
        self.thread_names.remove(&thread_id);
        // ... emit thread exited event
    }
    // Clear active breakpoints ...
    // BUT: paused_isolates NOT cleaned!
}
```

Compare with `Resumed` handler which correctly prunes:
```rust
self.paused_isolates.retain(|id| id != &isolate_id);
```

**Fix:** Add the same retain call to the `IsolateExit` handler:

```rust
DebugEvent::IsolateExit { isolate_id } => {
    self.paused_isolates.retain(|id| id != &isolate_id);  // ADD THIS
    if let Some(thread_id) = self.thread_map.remove(&isolate_id) {
        // ...
    }
}
```

**Why this matters:** `most_recent_paused_isolate()` returns the last element of `paused_isolates`. If a dead isolate leaks in, evaluate requests target a non-existent isolate and fail.

### Acceptance Criteria

1. `IsolateExit` handler calls `self.paused_isolates.retain(|id| id != &isolate_id)`
2. Add test: after `IsolateExit`, `most_recent_paused_isolate()` does not return the exited isolate
3. Existing tests pass
4. `cargo test -p fdemon-dap` — Pass

### Testing

```rust
#[tokio::test]
async fn test_isolate_exit_prunes_paused_isolates() {
    // 1. Set up adapter with an isolate
    // 2. Send Paused event → isolate enters paused_isolates
    // 3. Send IsolateExit event
    // 4. Assert most_recent_paused_isolate() does NOT return the exited isolate
}
```
