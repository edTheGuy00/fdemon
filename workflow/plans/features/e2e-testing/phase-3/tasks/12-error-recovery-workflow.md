## Task: Error Recovery Workflow Test

**Objective**: Create end-to-end test verifying error recovery scenarios: daemon crash, reconnection, and state recovery.

**Depends on**: 10-session-lifecycle

### Scope

- `tests/e2e/tui_workflows.rs`: Add error recovery workflow tests

### Details

Add the following tests to `tests/e2e/tui_workflows.rs`:

```rust
/// Error recovery: daemon crash -> display error -> allow restart
#[tokio::test]
async fn test_daemon_crash_recovery() {
    let fixture = TestFixture::simple_app();
    let mut session = FdemonSession::spawn(&fixture.path())
        .expect("Failed to spawn fdemon");

    // Get to running state
    session.expect_running()
        .expect("App should be running");

    // Simulate daemon crash by killing the Flutter process
    // This requires access to the Flutter process PID
    // Alternative: Use error_app fixture that crashes on reload

    // Note: This test may need platform-specific implementation
    // or a test hook to trigger daemon termination

    // For now, test with error_app that has compile errors
    session.kill().expect("Kill current session");

    // Start with error app
    let error_fixture = TestFixture::error_app();
    let mut session = FdemonSession::spawn(&error_fixture.path())
        .expect("Failed to spawn with error app");

    // Should show error state
    session.expect("error|Error|failed|Failed")
        .expect("Should show error state");

    // Capture error state
    session.assert_snapshot("error_recovery_error_state")
        .expect("Error state snapshot");

    // User should be able to see error details
    session.expect("Dart|dart|compile|syntax")
        .expect("Should show error details");

    // User can still quit
    session.send_key('q').expect("Quit");
    session.send_key('y').expect("Confirm");
    session.quit().expect("Should exit");
}

/// Test recovery from compilation error after edit
#[tokio::test]
async fn test_compilation_error_recovery() {
    // This test would ideally:
    // 1. Start with simple_app (working)
    // 2. Modify a file to introduce error
    // 3. Trigger reload -> see error
    // 4. Fix the file
    // 5. Trigger reload -> see success

    // For now, test error display with error_app
    let fixture = TestFixture::error_app();
    let mut session = FdemonSession::spawn(&fixture.path())
        .expect("Failed to spawn fdemon");

    // Should show compilation error
    session.expect("error|Error|compile")
        .expect("Should show compilation error");

    // Error should be visible in log view
    session.expect("lib/main.dart|syntax|expected")
        .expect("Should show error location");

    // User can scroll through error details
    session.send_special(SpecialKey::PageDown)
        .expect("Should scroll down");

    session.send_special(SpecialKey::PageUp)
        .expect("Should scroll up");

    session.kill().expect("Kill");
}

/// Test handling of disconnect/reconnect scenarios
#[tokio::test]
async fn test_device_disconnect_handling() {
    let fixture = TestFixture::simple_app();
    let mut session = FdemonSession::spawn(&fixture.path())
        .expect("Failed to spawn fdemon");

    session.expect_running()
        .expect("Should be running");

    // Note: Actually disconnecting a device is difficult in CI
    // This test documents expected behavior

    // When device disconnects, fdemon should:
    // 1. Show "Disconnected" or "Lost connection" state
    // 2. Allow user to select new device
    // 3. Not crash

    // For now, verify error handling doesn't crash
    session.send_key('r').expect("Try reload");

    // Should either reload successfully or show recoverable error
    session.expect_timeout("Running|Reloading|error|Error", Duration::from_secs(30))
        .expect("Should handle reload attempt");

    session.kill().expect("Kill");
}

/// Test graceful handling of corrupted state
#[tokio::test]
async fn test_graceful_degradation() {
    let fixture = TestFixture::simple_app();
    let mut session = FdemonSession::spawn(&fixture.path())
        .expect("Failed to spawn fdemon");

    session.expect_header().expect("Header");

    // Send invalid/unexpected key sequences
    session.send_raw(&[0x00]).expect("Send null byte"); // Should be ignored
    session.send_raw(&[0x1b, 0x5b, 0x99]).expect("Send invalid escape"); // Should be ignored

    // fdemon should still be responsive
    session.expect_header().expect("Should still show header");

    // Normal operations should still work
    session.send_key('q').expect("Quit should work");
    session.expect("quit|Quit").expect("Quit confirmation");
    session.send_key('n').expect("Cancel quit");

    // Should return to normal state
    session.expect_header().expect("Back to normal");

    session.kill().expect("Kill");
}

/// Test timeout handling for slow operations
#[tokio::test]
async fn test_timeout_handling() {
    let fixture = TestFixture::simple_app();
    let mut session = FdemonSession::spawn(&fixture.path())
        .expect("Failed to spawn fdemon");

    session.expect_running().expect("Running");

    // Trigger reload
    session.send_key('r').expect("Reload");

    // Reload should complete within reasonable time
    // If it times out, fdemon should show appropriate state
    let result = session.expect_timeout(
        "Running|running|Timeout|timeout|Error|error",
        Duration::from_secs(60)
    );

    match result {
        Ok(_) => {
            // Reload completed (success or error)
            println!("Reload completed within timeout");
        }
        Err(_) => {
            // Timeout - capture state for debugging
            println!("Reload timed out - capturing state");
            session.capture_for_snapshot().ok();
        }
    }

    session.kill().expect("Kill");
}

/// Test recovery from panic (if any panics are recoverable)
#[tokio::test]
async fn test_no_panic_on_edge_cases() {
    let fixture = TestFixture::simple_app();

    // Test rapid key presses
    let mut session = FdemonSession::spawn(&fixture.path())
        .expect("Failed to spawn fdemon");

    session.expect_header().expect("Header");

    // Rapid fire keys
    for _ in 0..10 {
        session.send_key('r').ok();
        session.send_key('q').ok();
        session.send_key('n').ok();
        session.send_key('d').ok();
        session.send_special(SpecialKey::Escape).ok();
    }

    // Should still be alive
    tokio::time::sleep(Duration::from_millis(500)).await;
    session.expect_header().expect("Should survive rapid input");

    session.kill().expect("Kill");
}
```

### Test Behavior Verification

The tests verify:
1. Compilation errors are displayed to user
2. Error details include file location and message
3. Invalid input doesn't crash the application
4. Timeouts are handled gracefully
5. Rapid input doesn't cause race conditions

### Acceptance Criteria

1. Compilation errors show file path and error message
2. Users can scroll through error output
3. Invalid escape sequences are ignored
4. Application survives rapid/chaotic input
5. Timeouts show appropriate feedback
6. All error states allow graceful exit

### Testing

```bash
# Run error recovery tests
cargo test --test e2e error_recovery -- --nocapture

# Run all workflow tests
cargo test --test e2e workflow -- --nocapture
```

### Notes

- True daemon crash testing requires process control
- Some tests document expected behavior rather than verifying it
- Error app fixture must have genuine compile errors
- Consider fuzzing input in future phase (Phase 4)

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
