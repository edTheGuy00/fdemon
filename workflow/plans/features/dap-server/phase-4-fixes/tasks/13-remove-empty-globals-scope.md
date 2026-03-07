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
