## Task: Replace Redundant `is_some()` + Branch with `or_else`

**Objective**: Replace the verbose `if ide_override.is_some() { ide_override } else { detect_parent_ide() }` pattern with the idiomatic `ide_override.or_else(|| detect_parent_ide())`.

**Depends on**: None

**Severity**: Minor

### Scope

- `crates/fdemon-app/src/actions/mod.rs`: Line ~604-608

### Details

**Current code:**
```rust
let ide = if ide_override.is_some() {
    ide_override
} else {
    crate::config::settings::detect_parent_ide()
};
```

**Fixed code:**
```rust
let ide = ide_override.or_else(crate::config::settings::detect_parent_ide);
```

Both `ide_override` and `detect_parent_ide()` return `Option<ParentIde>`, so `or_else` is a direct replacement.

### Acceptance Criteria

1. The `is_some()` + branch pattern is replaced with `or_else`
2. `cargo clippy --workspace -- -D warnings` — Pass
3. No behavior change

### Testing

- Existing tests cover this code path. No new tests needed.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/actions/mod.rs` | Replaced 5-line `if is_some() { ... } else { ... }` block with single-line `or_else` call at line 604 |

### Notable Decisions/Tradeoffs

1. **Function pointer vs closure**: Used `crate::config::settings::detect_parent_ide` as a bare function pointer (rather than a closure `|| detect_parent_ide()`) since the function takes no arguments and returns `Option<ParentIde>`, matching `or_else`'s expected `FnOnce() -> Option<T>` signature. This is consistent with the task's "Fixed code" example.

### Testing Performed

- `cargo fmt --all` - Passed
- `cargo check --workspace` - Passed (2 pre-existing warnings from other tasks in this phase, in `ide_config/mod.rs` and `ide_config/merge.rs`)
- `cargo test --workspace` - Passed (all unit and doc tests pass)
- `cargo clippy --workspace -- -D warnings` - 2 pre-existing errors in `ide_config/mod.rs:255` and `ide_config/merge.rs:8` from other phase tasks; verified these errors exist with or without this task's change

### Risks/Limitations

1. **Pre-existing clippy failures**: The `cargo clippy --workspace -- -D warnings` acceptance criterion cannot fully pass because two errors in `ide_config/mod.rs` and `ide_config/merge.rs` were introduced by other tasks in this phase (unused re-exports). These are outside the scope of task 07 and were confirmed to be independent of this change.
