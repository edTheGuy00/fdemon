## Task: Add Configurable Timeout for CI Environments

**Objective**: Make the fixed 1-second timeout in `recv_event()` configurable to prevent flaky tests on slow CI environments.

**Depends on**: None (can be done independently)

**Priority**: Minor (CI reliability improvement)

**Source**: [REVIEW.md](../../../REVIEW.md) - Logic Reasoning Review, Warning #2; Risks & Tradeoffs Review, Recommendation #2

### Scope

- `tests/e2e/mock_daemon.rs`: `MockDaemonHandle::recv_event()` method
- `tests/e2e.rs`: `with_timeout()` helper (already exists, may need adjustment)

### Details

The current implementation uses a hardcoded 1-second timeout:

```rust
// Current - fixed timeout
pub async fn recv_event(&mut self) -> Option<DaemonEvent> {
    tokio::time::timeout(Duration::from_secs(1), self.event_rx.recv())
        .await
        .ok()
        .flatten()
}
```

**Problem:** CI environments can be slower than local development machines. A 1-second timeout may cause intermittent failures.

**Fix: Add configurable timeout**

```rust
impl MockDaemonHandle {
    /// Default timeout for event reception
    pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(1);

    /// Receive the next event with default timeout (1 second)
    pub async fn recv_event(&mut self) -> Option<DaemonEvent> {
        self.recv_event_with_timeout(Self::DEFAULT_TIMEOUT).await
    }

    /// Receive the next event with custom timeout
    ///
    /// Use longer timeouts for CI environments or slow operations.
    /// Returns `None` if timeout expires or channel is closed.
    pub async fn recv_event_with_timeout(&mut self, timeout: Duration) -> Option<DaemonEvent> {
        tokio::time::timeout(timeout, self.event_rx.recv())
            .await
            .ok()
            .flatten()
    }

    /// Receive the next event, expecting it to be a specific type
    pub async fn expect_stdout(&mut self) -> Option<String> {
        self.expect_stdout_with_timeout(Self::DEFAULT_TIMEOUT).await
    }

    /// Receive the next stdout event with custom timeout
    pub async fn expect_stdout_with_timeout(&mut self, timeout: Duration) -> Option<String> {
        match self.recv_event_with_timeout(timeout).await? {
            DaemonEvent::Stdout(line) => Some(line),
            _ => None,
        }
    }
}
```

**Optional: Environment-based timeout**

```rust
impl MockDaemonHandle {
    /// Get timeout from environment or use default
    fn get_timeout() -> Duration {
        std::env::var("E2E_TEST_TIMEOUT_MS")
            .ok()
            .and_then(|s| s.parse().ok())
            .map(Duration::from_millis)
            .unwrap_or(Duration::from_secs(1))
    }

    pub async fn recv_event(&mut self) -> Option<DaemonEvent> {
        self.recv_event_with_timeout(Self::get_timeout()).await
    }
}
```

### Usage in Tests

```rust
// Default timeout (1 second)
let event = handle.recv_event().await;

// Longer timeout for slow operations
let event = handle.recv_event_with_timeout(Duration::from_secs(5)).await;

// Environment variable for CI
// E2E_TEST_TIMEOUT_MS=5000 cargo test --test e2e
```

### Acceptance Criteria

1. `recv_event_with_timeout(Duration)` method added
2. `recv_event()` delegates to `recv_event_with_timeout()` with default
3. `expect_stdout_with_timeout(Duration)` method added
4. Optional: Environment variable support for CI
5. All existing tests pass (backward compatible)
6. `cargo clippy --test e2e` passes

### Testing

```bash
cargo test --test e2e
cargo clippy --test e2e

# Test with longer timeout
E2E_TEST_TIMEOUT_MS=5000 cargo test --test e2e
```

### Notes

- This is a backward-compatible change
- Existing tests continue to work unchanged
- CI can set environment variable for longer timeouts
- Consider documenting the environment variable in README or CI config

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `tests/e2e/mock_daemon.rs` | Added configurable timeout support to `MockDaemonHandle` with `DEFAULT_TIMEOUT` constant, `recv_event_with_timeout()`, and `expect_stdout_with_timeout()` methods. Updated module documentation to reflect configurable timeouts. Added two new tests to demonstrate timeout functionality. |

### Notable Decisions/Tradeoffs

1. **Did not implement environment variable support**: The task marked this as "optional", and the primary goal was to provide programmatic timeout control for tests. Environment variable support can be added later if CI reliability issues arise. The current implementation provides the flexibility needed without the complexity of environment variable parsing.

2. **Added demonstration tests**: Added `test_recv_event_with_custom_timeout()` and `test_expect_stdout_with_custom_timeout()` to demonstrate and verify the new timeout functionality, ensuring the feature works as expected.

### Testing Performed

- `cargo test --test e2e` - Passed (58 tests, including 2 new timeout tests)
- `cargo clippy --test e2e` - Passed (no warnings in e2e test code)
- `cargo fmt` - Code properly formatted

### Risks/Limitations

1. **Backward compatibility**: This change is fully backward compatible. All existing tests continue to use the default 1-second timeout and pass without modification.

2. **Environment variable support deferred**: The optional environment variable feature was not implemented. If CI environments need automatic timeout adjustment, this can be added in a follow-up task.
