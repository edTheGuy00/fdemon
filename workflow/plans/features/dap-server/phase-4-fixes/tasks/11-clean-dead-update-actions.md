## Task: Clean up dead UpdateAction arms

**Objective**: Remove or convert the 5 dead `UpdateAction` variants that log "not yet wired (Phase 2)" — Phase 2 is complete and these are superseded by `DebugBackend` trait calls.

**Depends on**: None

**Severity**: Minor

### Scope

- `crates/fdemon-app/src/actions/mod.rs`: lines ~350-416
- `crates/fdemon-app/src/message.rs`: Remove corresponding `UpdateAction` variants if no longer produced

### Details

The 5 dead variants with misleading "Phase 2" comments:

1. `UpdateAction::PauseIsolate { session_id, vm_handle, isolate_id }`
2. `UpdateAction::ResumeIsolate { session_id, vm_handle, isolate_id, step }`
3. `UpdateAction::AddBreakpoint { session_id, vm_handle, isolate_id, script_uri, line, column }`
4. `UpdateAction::RemoveBreakpoint { session_id, vm_handle, isolate_id, breakpoint_id }`
5. `UpdateAction::SetIsolatePauseMode { session_id, vm_handle, isolate_id, mode }`

**Steps:**

1. Search for any code that produces these variants (should find none — they're superseded)
2. If truly unreachable, remove the variants from the `UpdateAction` enum
3. Remove the match arms from `handle_action()`
4. If any code still references them, convert the match arms to `unreachable!("superseded by DebugBackend")`

### Acceptance Criteria

1. No dead `UpdateAction` match arms with "Phase 2" comments
2. Variants removed from enum if no producers exist
3. `cargo check --workspace` — Pass
4. `cargo test --workspace` — Pass

### Notes

- Verify with `grep -r "PauseIsolate\|ResumeIsolate\|AddBreakpoint\|RemoveBreakpoint\|SetIsolatePauseMode" crates/` that no code produces these variants
- If any are still produced somewhere, keep the variant but replace the `warn!` with actual handling or `unreachable!`
