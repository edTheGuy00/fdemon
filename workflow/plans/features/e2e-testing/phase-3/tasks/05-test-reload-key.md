## Task: Test 'r' Key Triggers Hot Reload

**Objective**: Create PTY-based test verifying that pressing 'r' triggers a hot reload when an app is running.

**Depends on**: 03-test-startup-header, 04-test-device-selector

### Scope

- `tests/e2e/tui_interaction.rs`: Add hot reload keyboard test

### Details

Add the following test to `tests/e2e/tui_interaction.rs`:

```rust
/// Test that 'r' key triggers hot reload when app is running
#[tokio::test]
async fn test_r_key_triggers_reload() {
    let fixture = TestFixture::simple_app();
    let mut session = FdemonSession::spawn(&fixture.path())
        .expect("Failed to spawn fdemon");

    // Wait for app to be running
    session.expect_running()
        .expect("App should reach running state");

    // Press 'r' to trigger hot reload
    session.send_key('r').expect("Should send 'r' key");

    // Should show reloading indicator
    session.expect_reloading()
        .expect("Should show reloading state");

    // Should return to running state
    session.expect_running()
        .expect("Should return to running after reload");

    // Clean exit
    session.send_key('q').expect("Should send quit");
    session.send_key('y').expect("Should confirm quit");
    session.quit().expect("Should exit cleanly");
}

/// Test that 'R' (shift+r) triggers hot restart
#[tokio::test]
async fn test_shift_r_triggers_restart() {
    let fixture = TestFixture::simple_app();
    let mut session = FdemonSession::spawn(&fixture.path())
        .expect("Failed to spawn fdemon");

    session.expect_running()
        .expect("App should reach running state");

    // Press 'R' (uppercase) for hot restart
    session.send_key('R').expect("Should send 'R' key");

    // Should show restarting indicator (different from reload)
    session.expect("Restart|restart")
        .expect("Should show restart indicator");

    // Should return to running
    session.expect_running()
        .expect("Should return to running after restart");

    session.kill().expect("Should kill process");
}

/// Test that 'r' does nothing when no app is running
#[tokio::test]
async fn test_r_key_no_op_when_not_running() {
    let fixture = TestFixture::simple_app();
    let mut session = FdemonSession::spawn_with_args(
        &fixture.path(),
        &["--no-auto-start"]
    ).expect("Failed to spawn fdemon");

    // Wait for device selector (not running state)
    session.expect_device_selector()
        .expect("Should show device selector");

    // Press 'r' - should have no effect
    session.send_key('r').expect("Should send 'r' key");

    // Should still be in device selector (no crash, no state change)
    session.expect_device_selector()
        .expect("Should still show device selector");

    session.kill().expect("Should kill process");
}
```

### Test Behavior Verification

The tests verify:
1. 'r' key triggers hot reload when app is running
2. UI shows "Reloading" state during reload
3. App returns to "Running" state after reload completes
4. 'R' (shift) triggers hot restart (full restart)
5. 'r' key is ignored when no app is running (no crash)

### Acceptance Criteria

1. 'r' successfully triggers hot reload
2. Reload state is visually indicated
3. App returns to running state after reload
4. 'R' triggers restart (different from reload)
5. Key presses are ignored in invalid states (graceful handling)

### Testing

```bash
# Run reload key tests
cargo test --test e2e reload -- --nocapture

# Run with timing info
time cargo test --test e2e test_r_key_triggers_reload -- --nocapture
```

### Notes

- This test requires a running Flutter app, which needs a device
- In Docker CI, use Linux desktop device via Xvfb
- Reload timing varies; use generous timeouts (10-30s)
- Consider testing reload failure scenarios (compile error in fixture)

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `tests/e2e/tui_interaction.rs` | Added three hot reload key tests after device selector tests section |

### Notable Decisions/Tradeoffs

1. **Test Organization**: Added tests in a new "Hot Reload Tests" section after the existing "Device Selector Tests" section to maintain logical grouping
2. **Test Pattern Matching**: Used existing `expect_running()` and `expect_reloading()` helper methods from `pty_utils.rs` for consistent state verification
3. **Test Coverage**: Implemented all three tests as specified:
   - `test_r_key_triggers_reload` - verifies 'r' key triggers hot reload when running
   - `test_shift_r_triggers_restart` - verifies 'R' key triggers hot restart
   - `test_r_key_no_op_when_not_running` - verifies 'r' key is safely ignored when not running
4. **Graceful Exit Handling**: Used different exit strategies for different tests - graceful quit with confirmation for reload test, force kill for restart test (as it's a more disruptive operation)
5. **No Auto-Start Flag**: Used `spawn_with_args` with `--no-auto-start` flag in the no-op test to ensure the app doesn't automatically start, allowing us to verify behavior when not running

### Testing Performed

- `cargo fmt` - Passed (code formatted automatically)
- `cargo check` - Passed (no compilation errors)
- `cargo test --test e2e reload --no-run` - Passed (test compilation successful)
- `cargo clippy --test e2e -- -D warnings` - Passed (no warnings)

### Risks/Limitations

1. **Device Availability**: These tests require a running Flutter app which needs a device. In CI environments without devices, the tests may timeout at `expect_running()`. The tests are marked with `#[serial]` to prevent conflicts.
2. **Timing Sensitivity**: The tests use default timeouts (10 seconds) which should be sufficient for most systems, but slow systems or under heavy load may experience timeouts.
3. **State Transitions**: The restart test expects a "Restart|restart" pattern which may vary depending on the exact UI text. This pattern should be flexible enough to handle variations.
4. **Test Isolation**: All tests are marked with `#[serial]` from the serial_test crate to ensure they run sequentially and don't interfere with each other's PTY sessions.
