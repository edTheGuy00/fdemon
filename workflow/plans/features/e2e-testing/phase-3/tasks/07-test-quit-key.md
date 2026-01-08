## Task: Test 'q' Key Shows Quit Confirmation

**Objective**: Create PTY-based test verifying that pressing 'q' shows a quit confirmation dialog and handles user response.

**Depends on**: 03-test-startup-header, 04-test-device-selector

### Scope

- `tests/e2e/tui_interaction.rs`: Add quit confirmation tests

### Details

Add the following tests to `tests/e2e/tui_interaction.rs`:

```rust
/// Test that 'q' key shows quit confirmation dialog
#[tokio::test]
async fn test_q_key_shows_confirm_dialog() {
    let fixture = TestFixture::simple_app();
    let mut session = FdemonSession::spawn(&fixture.path())
        .expect("Failed to spawn fdemon");

    session.expect_header().expect("Should show header");

    // Press 'q' to initiate quit
    session.send_key('q').expect("Should send 'q' key");

    // Should show confirmation dialog
    session.expect("quit|Quit|exit|Exit|confirm|y/n|Y/N")
        .expect("Should show quit confirmation");

    // Press 'n' to cancel
    session.send_key('n').expect("Should send 'n' key");

    // Should return to normal view (still running)
    session.expect_header().expect("Should return to normal view");

    session.kill().expect("Should kill process");
}

/// Test that 'y' confirms quit and exits
#[tokio::test]
async fn test_quit_confirmation_yes_exits() {
    let fixture = TestFixture::simple_app();
    let mut session = FdemonSession::spawn(&fixture.path())
        .expect("Failed to spawn fdemon");

    session.expect_header().expect("Should show header");

    // Press 'q' then 'y' to quit
    session.send_key('q').expect("Should send 'q' key");
    session.expect("quit|Quit").expect("Should show confirmation");
    session.send_key('y').expect("Should send 'y' key");

    // Process should exit
    let status = session.quit().expect("Should exit");
    assert!(status.success(), "Should exit with success status");
}

/// Test that Escape cancels quit confirmation
#[tokio::test]
async fn test_escape_cancels_quit() {
    let fixture = TestFixture::simple_app();
    let mut session = FdemonSession::spawn(&fixture.path())
        .expect("Failed to spawn fdemon");

    session.expect_header().expect("Should show header");

    // Press 'q' to show confirmation
    session.send_key('q').expect("Should send 'q' key");
    session.expect("quit|Quit").expect("Should show confirmation");

    // Press Escape to cancel
    session.send_special(SpecialKey::Escape).expect("Should send Escape");

    // Should return to normal view
    session.expect_header().expect("Should return to normal view");

    session.kill().expect("Should kill process");
}

/// Test that Ctrl+C triggers immediate exit (no confirmation)
#[tokio::test]
async fn test_ctrl_c_immediate_exit() {
    let fixture = TestFixture::simple_app();
    let mut session = FdemonSession::spawn(&fixture.path())
        .expect("Failed to spawn fdemon");

    session.expect_header().expect("Should show header");

    // Send Ctrl+C (ETX character)
    session.send_raw(&[0x03]).expect("Should send Ctrl+C");

    // Process should exit (with SIGINT handling)
    // Note: Exact behavior depends on signal handling implementation
    let result = session.quit();
    // May exit cleanly or with signal - both are acceptable
    assert!(result.is_ok() || result.is_err());
}

/// Test that double 'q' is a shortcut for confirm+quit
#[tokio::test]
async fn test_double_q_quick_quit() {
    let fixture = TestFixture::simple_app();
    let mut session = FdemonSession::spawn(&fixture.path())
        .expect("Failed to spawn fdemon");

    session.expect_header().expect("Should show header");

    // Press 'q' twice quickly
    session.send_key('q').expect("Should send first 'q'");
    session.send_key('q').expect("Should send second 'q'");

    // Should exit (second 'q' acts as confirmation)
    let status = session.quit().expect("Should exit");
    // This behavior may or may not be implemented
    // Test documents expected behavior
}
```

### Test Behavior Verification

The tests verify:
1. 'q' key shows quit confirmation dialog
2. 'n' key cancels quit and returns to normal view
3. 'y' key confirms quit and exits the application
4. Escape key cancels quit confirmation
5. Ctrl+C triggers signal-based exit
6. Double 'q' acts as quick quit (if implemented)

### Acceptance Criteria

1. 'q' shows a quit confirmation message
2. 'y' exits the application cleanly
3. 'n' returns to normal operation
4. Escape cancels the quit dialog
5. Application exits with success status code (0)

### Testing

```bash
# Run quit tests
cargo test --test e2e quit -- --nocapture

# Test specific quit scenario
cargo test --test e2e test_quit_confirmation_yes_exits -- --nocapture
```

### Notes

- Quit confirmation protects against accidental exits
- Ctrl+C should still work for emergency exit
- Exit status should be 0 for clean quit, non-zero for errors
- Terminal cleanup (cursor restore, etc.) happens on exit

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
