## Task: Update breakpoint conditions when changed at same line

**Objective**: When `setBreakpoints` reuses an existing breakpoint at the same line, update its `condition`, `hit_condition`, and `log_message` if they differ from the incoming request.

**Depends on**: 02-split-adapter-mod

**Severity**: Major

### Scope

- `crates/fdemon-dap/src/adapter/handlers.rs` (post-split; currently `adapter/mod.rs:1652-1659`)

### Details

**Current (skips condition updates):**
```rust
for sbp in &desired {
    if let Some(dap_id) = self.breakpoint_state.find_by_source_line(&uri, sbp.line) {
        if let Some(entry) = self.breakpoint_state.lookup_by_dap_id(dap_id) {
            response_breakpoints.push(entry_to_dap_breakpoint(entry, &args.source));
        }
        continue;  // ← skips condition update!
    }
    // ... add new breakpoint with conditions
}
```

**Fixed — compare and update conditions before reuse:**
```rust
for sbp in &desired {
    if let Some(dap_id) = self.breakpoint_state.find_by_source_line(&uri, sbp.line) {
        // Update conditions if they changed.
        let new_condition = BreakpointCondition {
            condition: sbp.condition.clone(),
            hit_condition: sbp.hit_condition.clone(),
            log_message: sbp.log_message.clone(),
        };
        self.breakpoint_state.update_condition(dap_id, new_condition);

        if let Some(entry) = self.breakpoint_state.lookup_by_dap_id(dap_id) {
            response_breakpoints.push(entry_to_dap_breakpoint(entry, &args.source));
        }
        continue;
    }
    // ... add new breakpoint with conditions
}
```

This requires adding an `update_condition` method to `BreakpointState` (in `breakpoints.rs`) if one doesn't already exist.

### Acceptance Criteria

1. Changing a breakpoint's condition at the same line takes effect immediately
2. `BreakpointState` has an `update_condition(dap_id, BreakpointCondition)` method
3. Add test: set breakpoint at line 10 unconditional, then set again at line 10 with `condition: "x > 5"` — verify condition is updated
4. Existing tests pass
5. `cargo test -p fdemon-dap` — Pass

### Testing

```rust
#[tokio::test]
async fn test_set_breakpoints_updates_condition_on_same_line() {
    // 1. setBreakpoints line 10 with no condition
    // 2. setBreakpoints line 10 with condition "x > 5"
    // 3. Verify the breakpoint entry has condition "x > 5"
}
```
