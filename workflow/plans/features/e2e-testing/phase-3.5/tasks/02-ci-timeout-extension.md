## Task: Add CI-Aware Timeout Extension

**Objective**: Extend timeouts when running in CI environments to account for slower containers and resource constraints.

**Depends on**: 01-fix-spawn-default

### Scope

- `tests/e2e/pty_utils.rs`: Add CI-aware timeout constants

### Details

#### 1. Add CI Detection

```rust
/// Check if running in CI environment
fn is_ci() -> bool {
    std::env::var("CI").is_ok() || std::env::var("GITHUB_ACTIONS").is_ok()
}

/// Timeout multiplier for CI environments
/// CI containers have less CPU/memory, need longer waits
const CI_TIMEOUT_MULTIPLIER: u64 = 2;
```

#### 2. Update Timeout Constants

```rust
/// Default timeout for expect operations
/// Extended in CI environments for reliability
pub fn default_timeout() -> Duration {
    let base = 10; // seconds
    let multiplier = if is_ci() { CI_TIMEOUT_MULTIPLIER } else { 1 };
    Duration::from_secs(base * multiplier)
}

/// Time to wait for graceful quit before force-killing
pub fn quit_timeout() -> Duration {
    let base = 2000; // milliseconds
    let multiplier = if is_ci() { CI_TIMEOUT_MULTIPLIER } else { 1 };
    Duration::from_millis(base * multiplier)
}
```

#### 3. Use Dynamic Timeouts

Replace constant references with function calls:

```rust
// Before
self.session.set_expect_timeout(Some(DEFAULT_TIMEOUT));

// After
self.session.set_expect_timeout(Some(default_timeout()));
```

### Acceptance Criteria

1. Timeouts are 2x longer when CI=true or GITHUB_ACTIONS=true
2. Local development uses standard timeouts
3. Tests pass reliably in CI environments
4. Documentation notes CI behavior

### Testing

```bash
# Local (standard timeouts)
cargo test --test e2e

# Simulate CI (extended timeouts)
CI=true cargo test --test e2e

# Verify timeout values
cargo test --test e2e test_timeout_values -- --nocapture
```

### Notes

- Don't over-extend timeouts (2x is usually enough)
- If tests still flaky at 2x, the test design may be the issue
- Monitor test execution time to ensure <60s budget

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `tests/e2e/pty_utils.rs` | Added CI detection and dynamic timeout functions; replaced constant timeout references with function calls; added tests for CI detection and timeout behavior |

### Notable Decisions/Tradeoffs

1. **CI Detection**: Checks both `CI` and `GITHUB_ACTIONS` environment variables for broad CI platform support
2. **Function-based Timeouts**: Converted `DEFAULT_TIMEOUT` constant to `default_timeout()` function for dynamic calculation based on CI environment
3. **Test Isolation**: New tests save and restore environment variables to avoid interfering with other tests

### Testing Performed

- `cargo fmt` - Passed
- `cargo check` - Passed
- `cargo test --test e2e pty_utils::tests` - Passed (11 tests, 6 ignored PTY tests)
- `cargo test --test e2e pty_utils::tests::test_timeout_values` - Passed
- `cargo test --test e2e pty_utils::tests::test_is_ci_detection` - Passed
- `CI=true cargo test --test e2e pty_utils::tests::test_timeout_values` - Passed (verified 2x timeout in CI)
- `cargo clippy -- -D warnings` - Passed

### Risks/Limitations

1. **Environment Variable Pollution**: Tests that manipulate CI environment variables include cleanup code to restore original state, but concurrent test execution could cause interference
2. **CI Platform Coverage**: Only checks for `CI` and `GITHUB_ACTIONS` variables; other CI platforms may use different variables (but these two cover most common cases)
