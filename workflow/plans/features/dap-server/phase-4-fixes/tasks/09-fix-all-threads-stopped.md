## Task: Fix allThreadsStopped for multi-isolate

**Objective**: Stop hardcoding `allThreadsStopped: true` in stopped events; compute it dynamically based on actual isolate pause state.

**Depends on**: 02-split-adapter-mod

**Severity**: Minor

### Scope

- `crates/fdemon-dap/src/adapter/events.rs` (post-split; currently `adapter/mod.rs:1108`)

### Details

**Current (always true):**
```rust
let body = serde_json::json!({
    "reason": reason_str,
    "threadId": thread_id,
    "allThreadsStopped": true,
});
```

**Fixed — compute dynamically:**
```rust
// All threads stopped only if every known thread is in paused_isolates.
let all_stopped = self.thread_map.len() <= 1
    || self.thread_map.keys().all(|iso_id| self.paused_isolates.contains(iso_id));

let body = serde_json::json!({
    "reason": reason_str,
    "threadId": thread_id,
    "allThreadsStopped": all_stopped,
});
```

**Alternative simpler fix:** Just set to `false` always — this is technically more correct for multi-isolate Flutter apps since pausing one isolate doesn't pause others. IDEs handle `false` correctly by only showing the stopped thread as paused.

### Acceptance Criteria

1. `allThreadsStopped` is no longer hardcoded `true`
2. Either computed dynamically or set to `false`
3. Existing tests updated to expect the new value
4. `cargo test -p fdemon-dap` — Pass

### Notes

- Most Flutter apps have a single isolate, so this is rarely triggered in practice
- Setting to `false` is the simpler and safer approach
- The DAP spec says `allThreadsStopped` indicates whether *all* threads are stopped, not just the reported one
