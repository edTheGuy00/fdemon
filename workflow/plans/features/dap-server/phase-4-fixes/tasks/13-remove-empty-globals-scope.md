## Task: Remove empty Globals scope from scopes response

**Objective**: Stop advertising a "Globals" scope that always returns an empty variable list, which confuses users.

**Depends on**: 02-split-adapter-mod

**Severity**: Minor

### Scope

- `crates/fdemon-dap/src/adapter/variables.rs` (post-split; currently `adapter/mod.rs:2360-2364`)
- `crates/fdemon-dap/src/adapter/handlers.rs`: Remove Globals from `handle_scopes` response

### Details

**Current:**
```rust
// In handle_scopes — Globals scope is advertised:
// scope { name: "Globals", variablesReference: N, ... }

// In get_scope_variables — always returns empty:
ScopeKind::Globals => {
    // Globals are expensive — return empty for now.
    Ok(Vec::new())
}
```

**Fix:**
1. Remove the Globals scope from the `handle_scopes` response
2. Remove the `ScopeKind::Globals` arm from `get_scope_variables` (or leave it returning empty as a dead arm)
3. If `ScopeKind::Globals` is no longer reachable, consider removing the variant entirely

### Acceptance Criteria

1. Scopes response does not include a "Globals" scope
2. Existing scope/variable tests updated
3. `cargo test -p fdemon-dap` — Pass

### Notes

- Globals scope can be re-added when proper library-level variable enumeration is implemented
- This improves the IDE UX — users won't see an expandable "Globals" node that's always empty

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-dap/src/adapter/variables.rs` | Removed Globals scope allocation and `DapScope` item from `handle_scopes`; updated doc comments for `handle_scopes` and `get_scope_variables`; kept `ScopeKind::Globals` arm in `get_scope_variables` for exhaustiveness |
| `crates/fdemon-dap/src/adapter/mod.rs` | Updated 4 tests: renamed `test_scopes_returns_locals_and_globals` to `test_scopes_returns_only_locals` with updated assertions (1 scope instead of 2); removed globals assertion from `test_scopes_locals_scope_has_correct_var_ref_kind`; updated stale comment in `test_variables_globals_scope_returns_empty_list`; updated comment in `test_variables_count_capped_at_max` |

### Notable Decisions/Tradeoffs

1. **Kept `ScopeKind::Globals` enum variant**: The variant is still used in `mod.rs` tests (`test_variables_globals_scope_returns_empty_list`, `test_variables_count_capped_at_max`, `test_variable_store_*` in stack.rs). Removing it would require updating many unrelated store-level tests. The task says "consider removing" — retained for now to avoid disrupting those tests, consistent with the note that it can be re-added later with proper implementation.

2. **Kept `ScopeKind::Globals` arm in `get_scope_variables`**: Required for exhaustive match. Updated comment to clearly state it is not advertised in the scopes response.

3. **`handle_scopes` is in `variables.rs`, not `handlers.rs`**: The task mentioned `handlers.rs` but `handle_scopes` lives in `variables.rs` post-split (Task 02). Changed the correct file.

### Testing Performed

- `cargo fmt --all` - Passed (no formatting changes needed)
- `cargo check --workspace` - Passed (fdemon-app errors are pre-existing, unrelated to this task)
- `cargo test -p fdemon-dap` - Passed (581 tests, 0 failed)
- `cargo clippy -p fdemon-dap -- -D warnings` - Passed (0 warnings)

### Risks/Limitations

1. **`ScopeKind::Globals` is dead through the normal DAP flow**: The variant exists but is never reached via `handle_scopes`. It can be cleaned up when the store-level tests are updated or when a proper globals implementation is added.
