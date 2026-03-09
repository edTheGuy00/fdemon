## Task: Remove expect() in handle_set_breakpoints

**Objective**: Replace `expect("entry was just inserted")` with graceful error handling to prevent panics in library code.

**Depends on**: 02-split-adapter-mod

**Severity**: Major

### Scope

- `crates/fdemon-dap/src/adapter/handlers.rs` (post-split; currently `adapter/mod.rs:1702`)

### Details

**Current (can panic):**
```rust
let entry = self
    .breakpoint_state
    .lookup_by_dap_id(dap_id)
    .expect("entry was just inserted");
response_breakpoints.push(entry_to_dap_breakpoint(entry, &args.source));
```

**Fixed:**
```rust
let Some(entry) = self.breakpoint_state.lookup_by_dap_id(dap_id) else {
    tracing::error!(
        "BUG: breakpoint entry missing immediately after insert (dap_id={})",
        dap_id
    );
    continue; // Skip this breakpoint in the response
};
response_breakpoints.push(entry_to_dap_breakpoint(entry, &args.source));
```

Per `CODE_STANDARDS.md`: `unwrap()` and `expect()` without justification are red flags that can cause panics in production.

### Acceptance Criteria

1. No `expect()` calls in `handle_set_breakpoints`
2. Graceful error handling with `tracing::error!` for invariant violation
3. Existing breakpoint tests pass
4. `cargo test -p fdemon-dap` — Pass

### Notes

- This is in a loop, so `continue` is appropriate to skip one bad breakpoint without failing the entire request
- The `tracing::error!` with "BUG:" prefix signals an internal invariant violation worth investigating
