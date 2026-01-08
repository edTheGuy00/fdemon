## Task: Fix quit() Race Condition

**Objective**: Replace the hardcoded 500ms sleep in `quit()` with a polling loop that verifies process termination.

**Depends on**: 02-pty-test-utilities

### Scope

- `tests/e2e/pty_utils.rs`: Refactor `quit()` method

### Details

**Current problematic code:**
```rust
pub fn quit(&mut self) -> PtyResult<()> {
    self.send_key('q')?;
    std::thread::sleep(Duration::from_millis(500));  // Fixed delay
    let alive = self.session.is_alive()?;
    if alive {
        self.kill()?;
        std::thread::sleep(Duration::from_millis(100));
    }
    Ok(())  // Returns success even if process might still be alive!
}
```

**Problems:**
1. Fixed 500ms delay doesn't account for slow CI environments
2. No verification that process actually terminated after `kill()`
3. Returns `Ok(())` even if process might still be running

**Required Implementation:**
```rust
/// Time to wait for graceful quit before force-killing
const QUIT_TIMEOUT_MS: u64 = 2000;
/// Interval between process state checks
const QUIT_POLL_INTERVAL_MS: u64 = 100;

pub fn quit(&mut self) -> PtyResult<()> {
    self.send_key('q')?;

    // Wait for graceful shutdown with polling
    let iterations = QUIT_TIMEOUT_MS / QUIT_POLL_INTERVAL_MS;
    for _ in 0..iterations {
        std::thread::sleep(Duration::from_millis(QUIT_POLL_INTERVAL_MS));
        if !self.session.is_alive()? {
            return Ok(());
        }
    }

    // Still alive after timeout, force kill
    self.kill()?;

    // Verify termination
    for _ in 0..10 {
        std::thread::sleep(Duration::from_millis(QUIT_POLL_INTERVAL_MS));
        if !self.session.is_alive()? {
            return Ok(());
        }
    }

    Err("Process did not terminate after kill".into())
}
```

### Acceptance Criteria

1. `quit()` uses polling loop instead of fixed sleep
2. `quit()` returns error if process doesn't terminate
3. No hardcoded magic numbers (use named constants)
4. Process state verified before returning success
5. Total timeout is configurable for CI (via constants)

### Testing

```rust
#[test]
#[ignore]
fn test_quit_verifies_termination() {
    let fixture = TestFixture::simple_app();
    let mut session = FdemonSession::spawn(&fixture.path()).unwrap();
    session.expect_header().unwrap();

    // quit should succeed and process should be dead
    session.quit().unwrap();
    assert!(!session.is_alive().unwrap_or(true));
}
```

### Notes

- Consider making timeouts configurable via environment variable `FDEMON_TEST_TIMEOUT_MS` for slow CI
- The polling approach is more reliable than fixed delays

### Review Source

- Logic Reasoning Checker: "Race Condition in quit() Method"
- ACTION_ITEMS.md Issue #2

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `/Users/ed/Dev/zabin/flutter-demon/tests/e2e/pty_utils.rs` | Added polling-based quit implementation with named constants |

### Notable Decisions/Tradeoffs

1. **Polling Interval**: Used 100ms polling interval (QUIT_POLL_INTERVAL_MS) as a balance between responsiveness and CPU usage
2. **Graceful Timeout**: Set 2000ms timeout (QUIT_TIMEOUT_MS) for graceful shutdown before force-kill, which should be sufficient for most cases
3. **Post-Kill Verification**: Added additional 1s verification loop (10 iterations x 100ms) after force-kill to ensure process is actually dead
4. **Error Propagation**: Method now returns explicit error if process doesn't terminate, making test failures more obvious

### Testing Performed

- `cargo fmt` - Passed
- `cargo check` - Passed
- `cargo test --test e2e -- --ignored` - Compiled successfully (warnings are expected for unused helper methods)
- `cargo clippy -- -D warnings` - Passed

### Risks/Limitations

1. **CI Environment**: 2000ms timeout may still be insufficient in extremely slow CI environments. Future enhancement could read timeout from environment variable `FDEMON_TEST_TIMEOUT_MS` as suggested in task notes
2. **Process State Race**: Very minor race condition exists between `is_alive()` check and return, but this is inherent to process management and the window is minimal (< 1ms)
