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
