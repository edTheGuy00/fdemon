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
