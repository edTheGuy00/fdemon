## Task: Multi-Session Workflow Test

**Objective**: Create end-to-end test verifying multi-session scenarios including parallel reloads and session ordering.

**Depends on**: 10-session-lifecycle

### Scope

- `tests/e2e/tui_workflows.rs`: Add multi-session workflow tests

### Details

Add the following tests to `tests/e2e/tui_workflows.rs`:

```rust
/// Multi-session workflow: create two sessions and switch between them
#[tokio::test]
async fn test_multi_session_workflow() {
    let fixture = TestFixture::simple_app();
    let mut session = FdemonSession::spawn(&fixture.path())
        .expect("Failed to spawn fdemon");

    // === Create First Session ===
    println!("Creating first session...");

    session.expect_running()
        .expect("First session should be running");

    // Verify session 1 indicator
    session.expect("[1]")
        .expect("Should show session 1 tab");

    // === Create Second Session ===
    println!("Creating second session...");

    // Open device selector
    session.send_key('d')
        .expect("Should open device selector");

    session.expect_device_selector()
        .expect("Should show device selector");

    // Select device to create new session
    session.send_special(SpecialKey::Enter)
        .expect("Should select device");

    // Wait for second session
    session.expect("[2]")
        .expect("Should show session 2 tab");

    // === Verify Session Switching ===
    println!("Testing session switching...");

    // Should be on session 2 now
    session.expect("Session 2|\\[2\\].*active")
        .expect("Session 2 should be active");

    // Switch to session 1
    session.send_key('1')
        .expect("Should switch to session 1");

    session.expect("Session 1|\\[1\\].*active")
        .expect("Session 1 should be active");

    // Switch back to session 2
    session.send_key('2')
        .expect("Should switch to session 2");

    session.expect("Session 2|\\[2\\].*active")
        .expect("Session 2 should be active");

    // === Test Tab Cycling ===
    println!("Testing Tab cycling...");

    session.send_special(SpecialKey::Tab)
        .expect("Should cycle to next session");

    session.expect("\\[1\\].*active")
        .expect("Should be on session 1 after Tab");

    session.send_special(SpecialKey::Tab)
        .expect("Should cycle again");

    session.expect("\\[2\\].*active")
        .expect("Should be on session 2 after Tab");

    // === Snapshot Multi-Session UI ===
    session.assert_snapshot("multi_session_tabs")
        .expect("Multi-session snapshot");

    // === Clean Up ===
    session.kill().expect("Should kill process");
}

/// Test parallel hot reload across all sessions
#[tokio::test]
async fn test_parallel_reload_all_sessions() {
    let fixture = TestFixture::simple_app();
    let mut session = FdemonSession::spawn(&fixture.path())
        .expect("Failed to spawn fdemon");

    // Create two sessions
    session.expect_running().expect("First session running");

    session.send_key('d').expect("Open device selector");
    session.expect_device_selector().expect("Device selector");
    session.send_special(SpecialKey::Enter).expect("Select device");
    session.expect("[2]").expect("Second session created");

    // Both sessions should be running
    session.expect_running().expect("Sessions running");

    // Trigger reload all ('a' + 'r' or specific keybinding)
    // Note: Check actual keybinding for "reload all"
    session.send_key('r')  // May reload only current session
        .expect("Should reload");

    // Current session should show reloading
    session.expect_reloading().expect("Should be reloading");

    // Wait for reload to complete
    session.expect_running().expect("Should return to running");

    // Switch to other session and verify it's still running
    session.send_key('1').expect("Switch to session 1");
    session.expect_running().expect("Session 1 should be running");

    session.kill().expect("Kill");
}

/// Test session ordering remains consistent
#[tokio::test]
async fn test_session_ordering() {
    let fixture = TestFixture::simple_app();
    let mut session = FdemonSession::spawn(&fixture.path())
        .expect("Failed to spawn fdemon");

    // Create sessions 1, 2, 3
    session.expect_running().expect("Session 1");

    // Create session 2
    session.send_key('d').expect("d");
    session.expect_device_selector().expect("Selector");
    session.send_special(SpecialKey::Enter).expect("Enter");
    session.expect("[2]").expect("Session 2");

    // Create session 3
    session.send_key('d').expect("d");
    session.expect_device_selector().expect("Selector");
    session.send_special(SpecialKey::Enter).expect("Enter");
    session.expect("[3]").expect("Session 3");

    // Verify order: 1, 2, 3
    session.send_key('1').expect("Switch to 1");
    session.send_special(SpecialKey::Tab).expect("Tab");
    session.expect("\\[2\\]").expect("Should be on 2");

    session.send_special(SpecialKey::Tab).expect("Tab");
    session.expect("\\[3\\]").expect("Should be on 3");

    session.send_special(SpecialKey::Tab).expect("Tab");
    session.expect("\\[1\\]").expect("Should wrap to 1");

    // Close session 2 and verify order updates
    session.send_key('2').expect("Switch to 2");
    session.send_key('x').expect("Close session");

    // Sessions should now be 1, 3 (or renumbered to 1, 2)
    // Behavior depends on implementation
    session.expect("\\[1\\]|\\[3\\]")
        .expect("Should have remaining sessions");

    session.kill().expect("Kill");
}

/// Test closing all sessions shows appropriate UI
#[tokio::test]
async fn test_close_all_sessions() {
    let fixture = TestFixture::simple_app();
    let mut session = FdemonSession::spawn(&fixture.path())
        .expect("Failed to spawn fdemon");

    session.expect_running().expect("Session running");

    // Close the only session
    session.send_key('x').expect("Close session");

    // Should show device selector or empty state
    session.expect("Device|No sessions|Select")
        .expect("Should handle no sessions");

    // Should still be able to create new session or quit
    session.send_key('q').expect("Quit");

    session.quit().expect("Should exit");
}
```

### Test Behavior Verification

The tests verify:
1. Multiple sessions can be created
2. Session switching works with number keys and Tab
3. Session tab bar updates correctly
4. Reload works in multi-session mode
5. Session ordering is maintained
6. Closing sessions updates the UI properly

### Acceptance Criteria

1. Can create up to 9 sessions
2. Number keys (1-9) switch to correct session
3. Tab cycles through sessions in order
4. Session tab bar reflects current state
5. Closing a session updates numbering/ordering
6. Empty session state is handled gracefully

### Testing

```bash
# Run multi-session tests
cargo test --test e2e multi_session -- --nocapture

# Run specific test
cargo test --test e2e test_parallel_reload_all_sessions -- --nocapture
```

### Notes

- Multi-session tests require multiple device connections (or mock)
- In Docker CI, only one Linux desktop device may be available
- Consider mocking device list for deterministic multi-session tests
- Session limit is 9; test boundary conditions

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
