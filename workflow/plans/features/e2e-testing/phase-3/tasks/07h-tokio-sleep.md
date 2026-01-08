## Task: Use Tokio Sleep Instead of Blocking Sleep

**Objective**: Replace `std::thread::sleep` with `tokio::time::sleep` in async test contexts to avoid blocking the runtime thread pool.

**Depends on**: 07b-extract-magic-numbers

**Priority**: 🟢 MINOR (Consider Fixing)

### Scope

- `tests/e2e/tui_interaction.rs`: Replace blocking sleep with async sleep

### Background

Tests marked with `#[tokio::test]` run in an async context, but currently use `std::thread::sleep` which blocks the entire thread. This is suboptimal because:

- Blocks the tokio runtime thread, reducing concurrency
- Can cause issues if tokio runtime needs to process other tasks
- Doesn't integrate with async cancellation

However, this is a minor issue because:
- Tests run with `#[serial]` anyway (no concurrent tests)
- Sleep durations are short (100-500ms)
- Tests interact with synchronous PTY operations

### Implementation

Replace blocking sleep with async sleep:

```rust
// BEFORE:
std::thread::sleep(Duration::from_millis(INPUT_PROCESSING_DELAY_MS));

// AFTER:
tokio::time::sleep(Duration::from_millis(INPUT_PROCESSING_DELAY_MS)).await;
```

### Locations to Update

After task 07b extracts constants, update all sleep calls:

1. Input processing delays
2. Initialization delays
3. Termination check loops (in helper from 07c)

### Helper Function Update

Update `wait_for_termination` to be async:

```rust
/// Wait for the fdemon process to terminate, checking periodically.
async fn wait_for_termination(session: &mut FdemonSession) -> bool {
    for _ in 0..TERMINATION_CHECK_RETRIES {
        tokio::time::sleep(Duration::from_millis(TERMINATION_CHECK_INTERVAL_MS)).await;
        if let Ok(false) = session.session_mut().is_alive() {
            return true;
        }
    }
    false
}
```

Then update call sites:

```rust
// BEFORE:
assert!(wait_for_termination(&mut session), "Should exit");

// AFTER:
assert!(wait_for_termination(&mut session).await, "Should exit");
```

### Considerations

**Pros:**
- More idiomatic async Rust
- Better integration with tokio runtime
- Enables future async operations in tests

**Cons:**
- More `.await` calls throughout tests
- Helper functions need `async fn` signature
- Marginal benefit given `#[serial]` test isolation

### Acceptance Criteria

1. All `std::thread::sleep` calls replaced with `tokio::time::sleep(...).await`
2. Helper functions updated to be async where needed
3. Tests still pass with same timing behavior
4. `cargo clippy --test e2e -- -D warnings` - No warnings

### Testing

```bash
# Verify no blocking sleep remains
grep -n "std::thread::sleep" tests/e2e/tui_interaction.rs
# Should return empty

# Run tests
cargo test --test e2e tui_interaction -- --nocapture
```

### Notes

- This is a minor improvement, not blocking
- Can be deferred if higher priority items need attention
- Consider if PTY operations themselves are blocking (may negate benefit)
- Alternative: Keep blocking sleep in synchronous helper, document why

---

## Completion Summary

**Status:** Not Started
