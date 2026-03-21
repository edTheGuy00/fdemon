## Task: Events.rs Error Handling and State Cleanup

**Objective**: Log silently-ignored `resume` errors at warn level (M8), and add `exception_refs.clear()` to `on_resume()` for state consistency (L1).

**Depends on**: None

**Estimated Time**: 0.5–1 hour

### Scope

**Files Modified (Write):**
- `crates/fdemon-dap/src/adapter/events.rs`: Two fixes

**Files Read (Dependencies):**
- None

### Details

#### Fix 1: M8 — Log resume errors at warn level (lines 139, 160, 207)

Three call sites silently discard `resume` errors:

**Line 139** (hit condition not met):
```rust
// Before:
let _ = self.backend.resume(&isolate_id, None, None).await;

// After:
if let Err(e) = self.backend.resume(&isolate_id, None, None).await {
    tracing::warn!(
        "Failed to auto-resume isolate {} (hit condition not met): {}",
        isolate_id, e
    );
}
```

**Line 160** (expression condition false):
```rust
if let Err(e) = self.backend.resume(&isolate_id, None, None).await {
    tracing::warn!(
        "Failed to auto-resume isolate {} (condition false): {}",
        isolate_id, e
    );
}
```

**Line 207** (logpoint auto-resume):
```rust
if let Err(e) = self.backend.resume(&isolate_id, None, None).await {
    tracing::warn!(
        "Failed to auto-resume isolate {} (logpoint): {}",
        isolate_id, e
    );
}
```

**Why warn, not error**: These are "best-effort" resumes during conditional breakpoint / logpoint handling. A failed resume means the isolate stays paused (which the user will notice), but it's not a fatal adapter error. `warn!` level ensures it appears in logs for debugging without triggering error-level alerts.

#### Fix 2: L1 — Clear `exception_refs` in `on_resume()` (line 582)

Currently `on_resume()` clears `var_store`, `frame_store`, `evaluate_name_map`, and `first_async_marker_index` but not `exception_refs`. The exception refs are only removed per-thread in the `Resumed` event handler (line 249). This creates a brief inconsistency window.

```rust
pub fn on_resume(&mut self) {
    self.var_store.reset();
    self.frame_store.reset();
    self.evaluate_name_map.clear();
    self.exception_refs.clear();  // ADD THIS
    self.first_async_marker_index = None;
}
```

**Note**: The `DebugEvent::Resumed` handler at line 249 does `self.exception_refs.remove(&thread_id)` (per-thread removal). With this change, `on_resume()` also clears all exception refs. This is safe because `on_resume()` is called when the debuggee resumes, at which point all exception refs are stale regardless of thread ID.

### Acceptance Criteria

1. All three `let _ = self.backend.resume(...)` patterns replaced with `if let Err(e)` + `tracing::warn!`
2. `on_resume()` clears `exception_refs` alongside the other per-stop state
3. No `let _ = self.backend.resume(` remaining in `events.rs`
4. Existing tests pass: `cargo test -p fdemon-dap`
5. `cargo clippy -p fdemon-dap` clean

### Testing

```rust
#[tokio::test]
async fn test_on_resume_clears_exception_refs() {
    let mut adapter = create_test_adapter();
    adapter.exception_refs.insert(1, some_exception_ref());
    adapter.on_resume();
    assert!(adapter.exception_refs.is_empty());
}
```

For the resume error logging, the existing mock backend tests can be extended to return errors from `resume()` and verify the adapter doesn't panic and logs appropriately (though verifying log output directly is optional — the key assertion is that the adapter continues functioning).

### Notes

- These are small, low-risk changes. The error logging change is purely additive (no behavior change for the success path). The `exception_refs.clear()` aligns with the existing cleanup pattern.
