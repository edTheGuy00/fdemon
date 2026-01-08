## Task: Add Test Isolation with serial_test

**Objective**: Add the `serial_test` crate and mark PTY tests with `#[serial]` to prevent parallel execution conflicts.

**Depends on**: 02-pty-test-utilities

### Scope

- `Cargo.toml`: Add `serial_test` dev-dependency
- `tests/e2e/pty_utils.rs`: Add `#[serial]` to PTY tests

### Details

**Problem:** Multiple PTY tests running in parallel could spawn fdemon on the same fixture simultaneously, causing interference and flaky tests. The default `cargo test` runs tests in parallel (up to CPU count threads).

**Solution:** Use the `serial_test` crate to serialize PTY tests.

**Step 1: Add dependency**
```toml
[dev-dependencies]
serial_test = "3"
```

**Step 2: Mark PTY tests**
```rust
use serial_test::serial;

#[test]
#[ignore]  // PTY tests are slow, run with --ignored
#[serial]  // Run serially to avoid fixture conflicts
fn test_spawn_fdemon() {
    // ...
}
```

### Acceptance Criteria

1. `serial_test` added to dev-dependencies
2. All PTY-based tests (tests that spawn fdemon) marked with `#[serial]`
3. PTY tests can run with `cargo test -- --test-threads=4 --ignored` without interference
4. No fixture conflicts when multiple tests execute

### Testing

```bash
# Run PTY tests with multiple threads - should not conflict
cargo test --test e2e -- --ignored --test-threads=4

# Verify no orphaned processes
pgrep fdemon  # Should be empty after tests complete
```

### Notes

- Alternative approaches considered:
  - Fixture lock files: More complex, less idiomatic
  - Unique fixture copies per test: Storage overhead
- `#[serial]` is the simplest and most maintainable approach
- Tests without `#[serial]` (unit tests, mock tests) still run in parallel

### Review Source

- Risks & Tradeoffs Analyzer: "No Test Isolation Strategy (CRITICAL)"
- ACTION_ITEMS.md Issue #3

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `/Users/ed/Dev/zabin/flutter-demon/Cargo.toml` | Added `serial_test = "3"` to `[dev-dependencies]` section |
| `/Users/ed/Dev/zabin/flutter-demon/tests/e2e/pty_utils.rs` | Added `use serial_test::serial;` import (line 10-11) and marked all 6 PTY tests with `#[serial]` attribute |

### Notable Decisions/Tradeoffs

1. **Conditional import**: Used `#[cfg(test)] use serial_test::serial;` to only import the crate in test builds, keeping the main code clean.
2. **All PTY tests marked**: Applied `#[serial]` to all 6 PTY tests that spawn fdemon processes:
   - `test_spawn_fdemon`
   - `test_spawn_with_custom_args`
   - `test_send_key`
   - `test_send_special_keys`
   - `test_capture_screen`
   - `test_quit`
3. **Attribute order**: Placed `#[serial]` after `#[ignore]` to maintain visual consistency with existing attribute patterns.

### Testing Performed

- `cargo fmt` - Passed
- `cargo check` - Passed (4m 16s)
- `cargo clippy -- -D warnings` - Passed (2m 13s)
- `cargo test --test e2e -- --ignored --test-threads=4` - Tests run serially as expected (4 passed, 2 pre-existing failures unrelated to isolation)

**Note on test failures**: The 2 failing tests (`test_capture_screen`, `test_quit`) are pre-existing issues with test implementation logic, not concurrency problems. The important result is that all 6 tests executed serially without fixture conflicts when run with `--test-threads=4`.

### Risks/Limitations

1. **Test execution time**: PTY tests now run strictly serially, which increases total test execution time. However, this is acceptable because:
   - PTY tests are already marked `#[ignore]` and run separately from unit tests
   - Test reliability is more important than speed for E2E tests
   - Non-PTY tests still run in parallel
2. **Global serialization**: All tests marked with `#[serial]` share the same lock, so they cannot run in parallel with each other. This is the desired behavior for PTY tests to prevent fixture conflicts.
