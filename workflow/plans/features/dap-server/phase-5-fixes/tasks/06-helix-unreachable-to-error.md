## Task: Replace `unreachable!()` with `Error::config()` in Helix Merge

**Objective**: Replace the `unreachable!()` macro in `merge_helix_languages()` with a proper `Error::config(...)` return, following the project's no-panics-in-library-code standard.

**Depends on**: None

**Severity**: Minor

### Scope

- `crates/fdemon-app/src/ide_config/helix.rs`: Replace `unreachable!()` at line ~207

### Details

**Current code** (line ~207):
```rust
root.insert("language".to_string(), toml::Value::Array(vec![]));
// Safety: we just inserted it.
match root.get_mut("language") {
    Some(toml::Value::Array(arr)) => arr,
    _ => unreachable!("just inserted an Array"),
}
```

**Fixed code:**
```rust
root.insert("language".to_string(), toml::Value::Array(vec![]));
match root.get_mut("language") {
    Some(toml::Value::Array(arr)) => arr,
    _ => return Err(Error::config(
        "failed to retrieve inserted language array from TOML table".to_string(),
    )),
}
```

### Acceptance Criteria

1. No `unreachable!()` in `helix.rs`
2. The error path returns a typed `Error::config(...)` instead of panicking
3. All existing helix tests pass

### Testing

- Existing tests cover the happy path. The error path is theoretically unreachable but now fails gracefully if triggered.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/ide_config/helix.rs` | Replaced `unreachable!("just inserted an Array")` with `return Err(Error::config("failed to retrieve inserted language array from TOML table".to_string()))`. Also removed the now-incorrect `// Safety: we just inserted it.` comment. |

### Notable Decisions/Tradeoffs

1. **Exact message text**: Used the message from the task spec verbatim: `"failed to retrieve inserted language array from TOML table"`. This is consistent with the other `Error::config(...)` calls in the same function.
2. **Comment removal**: Removed the `// Safety: we just inserted it.` comment since it no longer describes an unreachable state but a fallible path, making the comment misleading.
3. **Pre-existing clippy issues**: `cargo clippy --workspace -- -D warnings` fails due to unused re-exports in `ide_config/mod.rs` introduced by another concurrent task's `pub(crate) mod merge` change. Verified with git stash that my change alone (without other in-flight tasks) passes clippy cleanly.

### Testing Performed

- `cargo test -p fdemon-app -- ide_config::helix` — PASS (21 tests)
- `cargo check --workspace` — PASS
- `cargo test --workspace` — PASS (all tests)
- `cargo clippy --workspace -- -D warnings` (my change in isolation) — PASS

### Risks/Limitations

1. **Theoretically unreachable path**: The error path is triggered only if `toml::Map::get_mut` fails immediately after `insert` for the same key — a condition that cannot occur in practice. The change is purely defensive and adds no observable behaviour change.
