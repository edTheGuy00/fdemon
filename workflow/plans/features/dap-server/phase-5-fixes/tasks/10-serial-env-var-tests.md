## Task: Add `#[serial]` to Env Var Mutation Tests

**Objective**: Add `serial_test` as a dev-dependency and annotate env-var-mutating tests with `#[serial]` to prevent race conditions in parallel test execution.

**Depends on**: None

**Severity**: Minor

### Scope

- `crates/fdemon-app/Cargo.toml`: Add `serial_test` dev-dependency
- `crates/fdemon-app/src/config/settings.rs`: Add `#[serial]` to the two affected tests

### Details

**Current tests** (`settings.rs` lines ~1835-1858):
```rust
#[test]
fn test_emacs_detection_via_inside_emacs() {
    let was_set = std::env::var("INSIDE_EMACS").is_ok();
    if !was_set {
        // SAFETY: setting env vars in single-threaded context for this test.
        unsafe { std::env::set_var("INSIDE_EMACS", "1") };
        let result = detect_parent_ide();
        unsafe { std::env::remove_var("INSIDE_EMACS") };
        ...
    }
}
```

The `// SAFETY:` comment is incorrect — Rust's test harness runs tests in parallel. `set_var`/`remove_var` are `unsafe` in Rust 2024 precisely because they mutate shared process state.

**Fix:**

1. Add to `crates/fdemon-app/Cargo.toml`:
   ```toml
   [dev-dependencies]
   serial_test = "3"
   ```
   (Or add to workspace `Cargo.toml` if using workspace dependencies.)

2. In `settings.rs` test module, add:
   ```rust
   use serial_test::serial;
   ```

3. Annotate both tests:
   ```rust
   #[test]
   #[serial]
   fn test_emacs_detection_via_inside_emacs() { ... }

   #[test]
   #[serial]
   fn test_helix_detection_via_helix_runtime() { ... }
   ```

4. Update the `// SAFETY:` comments to reference `#[serial]`:
   ```rust
   // SAFETY: `#[serial]` ensures no other test runs concurrently.
   ```

### Acceptance Criteria

1. Both env-var tests are annotated with `#[serial]`
2. `// SAFETY:` comments reference `#[serial]` as the serialization mechanism
3. `cargo test -p fdemon-app` — Pass
4. The guard pattern (`if !was_set`) can optionally be kept as extra safety

### Testing

- `cargo test -p fdemon-app -- --test-threads=1` and `--test-threads=4` both pass.

### Notes

- Check if `serial_test` is already used elsewhere in the workspace. If so, use the workspace version.
- The `if !was_set` guard is a secondary safety net — keep it unless you're confident `#[serial]` is sufficient.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/Cargo.toml` | Added `serial_test.workspace = true` to `[dev-dependencies]` |
| `crates/fdemon-app/src/config/settings.rs` | Added `use serial_test::serial;` import; added `#[serial]` to both env-var tests; updated `// SAFETY:` comments to reference `#[serial]` |

### Notable Decisions/Tradeoffs

1. **Workspace dependency**: Used `serial_test.workspace = true` instead of `serial_test = "3"` directly, as `serial_test = "3"` was already declared in the workspace `Cargo.toml` dev-dependencies. This keeps version management centralised.
2. **Guard pattern retained**: The `if !was_set` guard was kept as a secondary safety net, as the task noted it is acceptable and provides defence-in-depth against CI environments that set `INSIDE_EMACS` or `HELIX_RUNTIME`.

### Testing Performed

- `cargo fmt --all` - Passed
- `cargo check --workspace` - Passed (pre-existing unused-import warnings from concurrent task changes, not related to this task)
- `cargo test --workspace` - Passed (all 0 failed)
- `cargo test -p fdemon-app -- test_emacs_detection_via_inside_emacs test_helix_detection_via_helix_runtime --test-threads=1` - Passed (2/2)
- `cargo test -p fdemon-app -- test_emacs_detection_via_inside_emacs test_helix_detection_via_helix_runtime --test-threads=4` - Passed (2/2)
- `cargo clippy --workspace -- -D warnings` - Pre-existing failures in `ide_config/mod.rs` from concurrent task changes (unused imports from `pub(crate) use` rewrite); not introduced by this task

### Risks/Limitations

1. **Clippy pre-existing failures**: `cargo clippy --workspace -- -D warnings` fails due to unused-import warnings in `crates/fdemon-app/src/ide_config/mod.rs` caused by another concurrent task changing `pub mod merge` to `pub(crate) mod merge`. These are not introduced by this task's changes.
