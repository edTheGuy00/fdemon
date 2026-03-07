## Task: Log resume failures in conditional breakpoint / logpoint paths

**Objective**: Replace three silent `let _ = self.backend.resume(...)` calls with proper error logging to prevent invisible debugging failures.

**Depends on**: 02-split-adapter-mod

**Severity**: Major

### Scope

- `crates/fdemon-dap/src/adapter/events.rs` (post-split; currently `adapter/mod.rs:1026, 1047, 1094`)

### Details

Three locations silently discard resume errors:

**1. Hit-condition not met (line ~1026):**
```rust
// Current
let _ = self.backend.resume(&isolate_id, None).await;

// Fixed
if let Err(e) = self.backend.resume(&isolate_id, None).await {
    tracing::warn!(
        "Auto-resume failed after hit-condition skip (isolate={}): {}",
        isolate_id, e
    );
}
```

**2. Expression condition evaluated to falsy (line ~1047):**
```rust
// Current
let _ = self.backend.resume(&isolate_id, None).await;

// Fixed
if let Err(e) = self.backend.resume(&isolate_id, None).await {
    tracing::warn!(
        "Auto-resume failed after false condition (isolate={}): {}",
        isolate_id, e
    );
}
```

**3. Logpoint auto-resume (line ~1094):**
```rust
// Current
let _ = self.backend.resume(&isolate_id, None).await;

// Fixed
if let Err(e) = self.backend.resume(&isolate_id, None).await {
    tracing::warn!(
        "Auto-resume failed after logpoint (isolate={}): {}",
        isolate_id, e
    );
}
```

**Why this matters:** If resume fails, the isolate stays paused forever from the VM's perspective, but the adapter has already returned without emitting a `stopped` event. The IDE is left in an inconsistent state — the play/pause button won't reflect reality.

### Acceptance Criteria

1. All three `let _ = self.backend.resume(...)` replaced with `if let Err(e)` + `warn!` logging
2. Each log message identifies the context (hit-condition, false-condition, logpoint)
3. Existing tests pass
4. `cargo test -p fdemon-dap` — Pass

### Notes

- A future enhancement could emit a `stopped` event as fallback when resume fails, but logging is sufficient for now
- Per `CODE_STANDARDS.md`, `let _ = do_something()` is an anti-pattern for ignoring errors
