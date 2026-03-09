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

**Status:** Not Started
