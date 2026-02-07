## Task: Remove Unused Statics and Test-Only Code

**Objective**: Remove `PACKAGE_PATH_REGEX` static (completely unused) and move `has_flutter_dependency()` to `#[cfg(test)]` (only used in tests).

**Depends on**: None

**Severity**: MAJOR (clippy `dead_code` warnings)

**Source**: Code Quality Inspector (ACTION_ITEMS.md Major #3, Minor #3)

### Scope

- `crates/fdemon-core/src/stack_trace.rs:37-44`: Delete `PACKAGE_PATH_REGEX`
- `crates/fdemon-core/src/discovery.rs:73-80`: Move `has_flutter_dependency()` to `#[cfg(test)]`

### Details

**1. `PACKAGE_PATH_REGEX` (stack_trace.rs:37-44)**

```rust
#[allow(dead_code)]
/// Regex to match Dart package paths in stack traces (e.g., `package:foo/bar.dart`)
pub(crate) static PACKAGE_PATH_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"package:([^/]+)/(.+\.dart)").unwrap());
```

This static is **completely unused**. The function `is_package_path()` (line 404) uses plain string matching (`starts_with`, `contains`) instead. No code anywhere in the workspace references `PACKAGE_PATH_REGEX`. The `#[allow(dead_code)]` was added to suppress the warning rather than fix it.

**Action:** Delete lines 37-44 entirely (the doc comment + the static). Check if `Lazy` and `Regex` imports can also be removed (they may be used by other statics in the same file).

**2. `has_flutter_dependency()` (discovery.rs:73-80)**

```rust
#[allow(dead_code)]
pub(crate) fn has_flutter_dependency(path: &Path) -> bool {
    // reads pubspec.yaml and checks for flutter dependency
}
```

Only called from two test functions in the same file (lines 693 and 704). The production code uses `check_has_flutter_dependency()` (line 365) which takes already-parsed content. `has_flutter_dependency()` is a convenience wrapper that reads from disk -- exactly what tests need.

**Action:** Move the function into the `#[cfg(test)] mod tests` block, remove the `#[allow(dead_code)]` attribute.

### Acceptance Criteria

1. `PACKAGE_PATH_REGEX` does not exist in the codebase
2. `has_flutter_dependency()` only exists inside a `#[cfg(test)]` block
3. No `#[allow(dead_code)]` remains on either item
4. `cargo check -p fdemon-core` passes with no warnings
5. `cargo test -p fdemon-core --lib` passes

### Testing

```bash
# Verify statics removed
rg 'PACKAGE_PATH_REGEX' crates/
rg 'allow\(dead_code\)' crates/fdemon-core/src/stack_trace.rs
rg 'allow\(dead_code\)' crates/fdemon-core/src/discovery.rs

# Verify compilation and tests
cargo check -p fdemon-core
cargo test -p fdemon-core --lib
```

### Notes

- Check whether removing `PACKAGE_PATH_REGEX` allows removing `once_cell::sync::Lazy` or `regex::Regex` imports -- other statics in the file may still use them
- `has_flutter_dependency` has existing tests that exercise it -- they must continue to pass

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-core/src/stack_trace.rs` | Removed `PACKAGE_PATH_REGEX` static (lines 37-44) including doc comment and `#[allow(dead_code)]` attribute |
| `crates/fdemon-core/src/discovery.rs` | Moved `has_flutter_dependency()` function from line 73 to inside `#[cfg(test)] mod tests` block (line 655), removed `#[allow(dead_code)]` attribute |

### Notable Decisions/Tradeoffs

1. **Kept imports**: The `regex::Regex` and `std::sync::LazyLock` imports were kept in `stack_trace.rs` because other statics still use them (DART_VM_FRAME_REGEX, DART_VM_FRAME_NO_COL_REGEX, FRIENDLY_FRAME_REGEX, ASYNC_GAP_REGEX)
2. **Function placement**: Placed `has_flutter_dependency()` after the helper functions in the tests module (after `create_flutter_package()`) for logical grouping with other test utilities
3. **No visibility change**: The function remains private to the tests module (no `pub` modifier) since it's only used by two test functions in the same file

### Testing Performed

- `cargo check -p fdemon-core` - Passed with no warnings
- `cargo test -p fdemon-core --lib` - Passed (243 tests)
- `cargo clippy -p fdemon-core -- -D warnings` - Passed with no warnings
- `rg 'PACKAGE_PATH_REGEX' crates/` - No matches found
- `rg 'allow\(dead_code\)' crates/fdemon-core/src/stack_trace.rs` - No matches found
- `rg 'allow\(dead_code\)' crates/fdemon-core/src/discovery.rs` - No matches found

### Risks/Limitations

None. All acceptance criteria met:
1. PACKAGE_PATH_REGEX completely removed from codebase
2. has_flutter_dependency() only exists inside #[cfg(test)] block
3. No #[allow(dead_code)] attributes remain on either item
4. All compilation checks and tests pass
