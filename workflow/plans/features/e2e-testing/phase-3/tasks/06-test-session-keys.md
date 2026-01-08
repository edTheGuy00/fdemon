## Task: Test Number Keys Switch Sessions

**Objective**: Create PTY-based tests verifying that number keys (1-9) switch between sessions in multi-session mode.

**Depends on**: 03-test-startup-header, 04-test-device-selector

### Scope

- `tests/e2e/tui_interaction.rs`: Add session switching tests

### Details

Add the following tests to `tests/e2e/tui_interaction.rs`:

```rust
/// Test that number keys switch between sessions
#[tokio::test]
async fn test_number_keys_switch_sessions() {
    let fixture = TestFixture::simple_app();
    let mut session = FdemonSession::spawn(&fixture.path())
        .expect("Failed to spawn fdemon");

    // Wait for initial session to be running
    session.expect_running()
        .expect("First session should be running");

    // Verify we're on session 1 (tab indicator)
    session.expect("[1]")
        .expect("Should show session 1 indicator");

    // Open device selector to add another session
    session.send_key('d').expect("Should send 'd' key");
    session.expect_device_selector().expect("Should show device selector");

    // Select a device to create session 2
    session.send_special(SpecialKey::Enter).expect("Should select device");

    // Wait for second session
    session.expect("[2]")
        .expect("Should show session 2 indicator");

    // Press '1' to switch to session 1
    session.send_key('1').expect("Should send '1' key");
    session.expect("Session 1|\\[1\\].*active")
        .expect("Should switch to session 1");

    // Press '2' to switch back to session 2
    session.send_key('2').expect("Should send '2' key");
    session.expect("Session 2|\\[2\\].*active")
        .expect("Should switch to session 2");

    session.kill().expect("Should kill process");
}

/// Test Tab key cycles through sessions
#[tokio::test]
async fn test_tab_cycles_sessions() {
    let fixture = TestFixture::simple_app();
    let mut session = FdemonSession::spawn(&fixture.path())
        .expect("Failed to spawn fdemon");

    session.expect_running().expect("Should be running");

    // With only one session, Tab should be a no-op
    session.send_special(SpecialKey::Tab).expect("Should send Tab");

    // Should still be on session 1
    session.expect("[1]").expect("Should still show session 1");

    // TODO: Add second session and verify Tab cycles

    session.kill().expect("Should kill process");
}

/// Test that pressing a number for non-existent session is ignored
#[tokio::test]
async fn test_invalid_session_number_ignored() {
    let fixture = TestFixture::simple_app();
    let mut session = FdemonSession::spawn(&fixture.path())
        .expect("Failed to spawn fdemon");

    session.expect_running().expect("Should be running");

    // Only session 1 exists
    session.expect("[1]").expect("Should show session 1");

    // Press '5' - should be ignored (no session 5)
    session.send_key('5').expect("Should send '5' key");

    // Should still be on session 1, no crash or error
    session.expect("[1]").expect("Should still show session 1");

    session.kill().expect("Should kill process");
}

/// Test 'x' key closes current session
#[tokio::test]
async fn test_x_key_closes_session() {
    // This test requires multi-session setup
    // Closing the last session should show device selector or exit
    let fixture = TestFixture::simple_app();
    let mut session = FdemonSession::spawn(&fixture.path())
        .expect("Failed to spawn fdemon");

    session.expect_running().expect("Should be running");

    // Press 'x' to close current session
    session.send_key('x').expect("Should send 'x' key");

    // Should show confirmation or close directly
    // Behavior depends on implementation
    session.expect("close|Close|confirm|Device")
        .expect("Should respond to close command");

    session.kill().expect("Should kill process");
}
```

### Test Behavior Verification

The tests verify:
1. Number keys (1-9) switch to corresponding session
2. Tab cycles through available sessions
3. Invalid session numbers are gracefully ignored
4. 'x' key closes/removes the current session
5. Session indicators in UI update when switching

### Acceptance Criteria

1. Pressing '1' switches to session 1 (if exists)
2. Pressing '2' switches to session 2 (if exists)
3. Tab cycles forward through sessions
4. Invalid session numbers don't cause errors
5. Session tab bar updates to show active session

### Testing

```bash
# Run session switching tests
cargo test --test e2e session -- --nocapture

# Run specific test
cargo test --test e2e test_number_keys_switch_sessions -- --nocapture
```

### Notes

- Multi-session testing requires creating multiple sessions first
- Session creation requires device selection, which needs a device
- Consider using mock devices or Linux desktop for CI
- Session limit is 9 (keys 1-9)

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
