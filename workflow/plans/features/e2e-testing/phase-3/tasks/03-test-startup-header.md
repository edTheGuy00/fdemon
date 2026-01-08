## Task: Test Startup Shows Header

**Objective**: Create PTY-based test verifying that fdemon displays the header with project name on startup.

**Depends on**: 02-pty-test-utilities

### Scope

- `tests/e2e/tui_interaction.rs`: **NEW** - TUI interaction test file

### Details

Create `tests/e2e/tui_interaction.rs` starting with the startup header test:

```rust
//! PTY-based TUI interaction tests
//!
//! Tests keyboard input handling and TUI rendering using
//! pseudo-terminal interaction via expectrl.

mod pty_utils;

use pty_utils::{FdemonSession, TestFixture, SpecialKey};

/// Test that fdemon shows the header bar with project name on startup
#[tokio::test]
async fn test_startup_shows_header() {
    let fixture = TestFixture::simple_app();
    let mut session = FdemonSession::spawn(&fixture.path())
        .expect("Failed to spawn fdemon");

    // Wait for header to appear
    session.expect_header()
        .expect("Header should appear on startup");

    // Verify project name is shown
    session.expect("simple_app")
        .expect("Project name should be in header");

    // Clean exit
    session.send_key('q').expect("Should send quit key");
    session.expect("quit").expect("Should show quit confirmation");
    session.send_key('y').expect("Should confirm quit");
    session.quit().expect("Should exit cleanly");
}

/// Test that fdemon shows version in header (if enabled)
#[tokio::test]
async fn test_startup_shows_phase() {
    let fixture = TestFixture::simple_app();
    let mut session = FdemonSession::spawn(&fixture.path())
        .expect("Failed to spawn fdemon");

    // Should show initial phase (e.g., "Initializing" or "Device Selection")
    session.expect_timeout("Initializing|Device", Duration::from_secs(5))
        .expect("Should show initial phase");

    session.kill().expect("Should kill process");
}
```

### Test Behavior Verification

The test verifies:
1. fdemon starts without crashing
2. Header bar is rendered with project name
3. Initial phase indicator is shown
4. Process can be cleanly terminated

### Acceptance Criteria

1. Test passes when fdemon starts successfully
2. Test fails if header doesn't appear within timeout
3. Test fails if project name is not displayed
4. Test properly cleans up fdemon process

### Testing

```bash
# Run this specific test
cargo test --test e2e test_startup_shows_header -- --nocapture

# Run all startup tests
cargo test --test e2e startup -- --nocapture
```

### Notes

- This is the first TUI interaction test; keep it simple as a foundation
- Use generous timeouts (5-10s) initially, optimize later
- If headless mode outputs JSON instead of TUI, test the JSON events instead
- Consider environment variable `FDEMON_TEST_MODE=pty` to switch behaviors

---

## Completion Summary

**Status:** Not Started

**Files Modified:**
- (none yet)

**Implementation Details:**

(to be filled after implementation)

**Testing Performed:**
- `cargo fmt` - Pending
- `cargo clippy` - Pending
- `cargo test` - Pending

**Notable Decisions:**
- (none yet)

**Risks/Limitations:**
- (none yet)
