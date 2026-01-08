## Task: Test Device Selector Keyboard Navigation

**Objective**: Create PTY-based tests verifying that the device selector modal responds correctly to keyboard navigation (arrow keys, Enter, Escape).

**Depends on**: 02-pty-test-utilities

### Scope

- `tests/e2e/tui_interaction.rs`: Add device selector tests

### Details

Add the following tests to `tests/e2e/tui_interaction.rs`:

```rust
/// Test that device selector appears and can be navigated with arrow keys
#[tokio::test]
async fn test_device_selector_keyboard_navigation() {
    let fixture = TestFixture::simple_app();
    let mut session = FdemonSession::spawn_with_args(
        &fixture.path(),
        &["--no-auto-start"]  // Force device selector to appear
    ).expect("Failed to spawn fdemon");

    // Wait for device selector to appear
    session.expect_device_selector()
        .expect("Device selector should appear");

    // Verify we can see device list (mock devices or "No devices")
    session.expect("device|Device|emulator|Emulator|No devices")
        .expect("Should show device list or no devices message");

    // Navigate down with arrow key
    session.send_special(SpecialKey::ArrowDown)
        .expect("Should send arrow down");

    // Navigate up with arrow key
    session.send_special(SpecialKey::ArrowUp)
        .expect("Should send arrow up");

    // Escape should close the selector
    session.send_special(SpecialKey::Escape)
        .expect("Should send escape");

    // Clean exit
    session.kill().expect("Should kill process");
}

/// Test that Enter selects a device in the device selector
#[tokio::test]
async fn test_device_selector_enter_selects() {
    let fixture = TestFixture::simple_app();
    let mut session = FdemonSession::spawn_with_args(
        &fixture.path(),
        &["--no-auto-start"]
    ).expect("Failed to spawn fdemon");

    session.expect_device_selector()
        .expect("Device selector should appear");

    // Press Enter to select current device
    session.send_special(SpecialKey::Enter)
        .expect("Should send enter");

    // Should either start running or show error (no device connected)
    session.expect_timeout("Running|Starting|Error|No device", Duration::from_secs(5))
        .expect("Should respond to device selection");

    session.kill().expect("Should kill process");
}

/// Test that 'd' key opens device selector from running state
#[tokio::test]
async fn test_d_key_opens_device_selector() {
    let fixture = TestFixture::simple_app();
    let mut session = FdemonSession::spawn(&fixture.path())
        .expect("Failed to spawn fdemon");

    // Wait for initial state
    session.expect_header().expect("Should show header");

    // Press 'd' to open device selector
    session.send_key('d').expect("Should send 'd' key");

    // Device selector should appear
    session.expect_device_selector()
        .expect("Device selector should open on 'd' key");

    session.kill().expect("Should kill process");
}
```

### Test Behavior Verification

The tests verify:
1. Device selector modal appears on startup (with `--no-auto-start`)
2. Arrow keys navigate the device list
3. Enter key selects the highlighted device
4. Escape key closes the selector
5. 'd' key opens the device selector from normal view

### Acceptance Criteria

1. Arrow key navigation works in device selector
2. Enter selects a device (or shows error if none available)
3. Escape closes the device selector
4. 'd' key opens device selector from main view
5. Tests handle "no devices" case gracefully

### Testing

```bash
# Run device selector tests
cargo test --test e2e device_selector -- --nocapture

# Run with verbose output
RUST_LOG=debug cargo test --test e2e device_selector -- --nocapture
```

### Notes

- Device selector behavior depends on whether devices are available
- In Docker CI, Linux desktop device should be available (via Xvfb)
- Tests should be resilient to "No devices found" scenario
- Consider mocking device discovery for deterministic tests

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
