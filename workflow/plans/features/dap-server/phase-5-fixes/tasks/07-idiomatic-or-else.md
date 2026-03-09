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

**Status:** Not Started
