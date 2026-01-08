## Task: Full Session Lifecycle Test

**Objective**: Create end-to-end test covering the complete session lifecycle: create -> run -> reload -> stop -> remove.

**Depends on**: 08-snapshot-infrastructure, 09-golden-files

### Scope

- `tests/e2e/tui_workflows.rs`: **NEW** - Complex workflow tests

### Details

Create `tests/e2e/tui_workflows.rs` with the session lifecycle test:

```rust
//! Complex workflow tests for end-to-end user journey verification
//!
//! These tests cover multi-step user workflows that exercise
//! multiple features in sequence.

mod pty_utils;

use pty_utils::{FdemonSession, TestFixture, SpecialKey};
use std::time::Duration;

/// Full session lifecycle: create -> run -> reload -> stop -> remove
#[tokio::test]
async fn test_full_session_lifecycle() {
    let fixture = TestFixture::simple_app();
    let mut session = FdemonSession::spawn(&fixture.path())
        .expect("Failed to spawn fdemon");

    // === Phase 1: Session Creation ===
    println!("Phase 1: Session Creation");

    // Wait for header to appear
    session.expect_header()
        .expect("Should show header");

    // Wait for device selector or auto-start
    // Depending on config, may go straight to running
    let initial_state = session.expect_timeout(
        "Device|Running|Initializing",
        Duration::from_secs(10)
    ).expect("Should reach initial state");

    // If device selector shown, select a device
    if initial_state.get(0).map_or(false, |m| m.contains("Device")) {
        session.send_special(SpecialKey::Enter)
            .expect("Should select device");
    }

    // === Phase 2: App Running ===
    println!("Phase 2: App Running");

    session.expect_running()
        .expect("App should reach running state");

    // Verify UI shows running indicator
    session.expect("Running|running")
        .expect("Should show running status");

    // Optionally capture snapshot
    session.assert_snapshot("lifecycle_running")
        .expect("Running state snapshot");

    // === Phase 3: Hot Reload ===
    println!("Phase 3: Hot Reload");

    // Trigger hot reload
    session.send_key('r')
        .expect("Should send reload command");

    // Verify reload state
    session.expect_reloading()
        .expect("Should show reloading state");

    // Wait for reload to complete
    session.expect_running()
        .expect("Should return to running after reload");

    // Verify reload count incremented (if shown in UI)
    // session.expect("Reloads: 1|reload.*1").ok();

    // === Phase 4: Hot Restart ===
    println!("Phase 4: Hot Restart");

    // Trigger hot restart
    session.send_key('R')
        .expect("Should send restart command");

    // Verify restart state
    session.expect("Restart|restart|Starting")
        .expect("Should show restart state");

    // Wait for restart to complete
    session.expect_running()
        .expect("Should return to running after restart");

    // === Phase 5: Stop App ===
    println!("Phase 5: Stop App");

    // Stop the running app (if 's' is the stop key)
    // Note: Check actual keybinding in implementation
    session.send_key('s')
        .expect("Should send stop command");

    // Verify app stopped
    session.expect("Stopped|stopped|Stop|Idle")
        .expect("Should show stopped state");

    // === Phase 6: Session Removal ===
    println!("Phase 6: Session Removal");

    // Close/remove the session
    session.send_key('x')
        .expect("Should send close command");

    // Should show device selector (no sessions) or exit prompt
    session.expect("Device|No sessions|quit|exit")
        .expect("Should handle session removal");

    // === Phase 7: Clean Exit ===
    println!("Phase 7: Clean Exit");

    session.send_key('q')
        .expect("Should send quit");

    // Handle quit confirmation if shown
    session.send_key('y').ok(); // May not need confirmation if no sessions

    let status = session.quit()
        .expect("Should exit cleanly");

    assert!(status.success(), "Should exit with success status");

    println!("Full session lifecycle test completed successfully!");
}

/// Verify session state transitions are valid
#[tokio::test]
async fn test_session_state_machine() {
    let fixture = TestFixture::simple_app();
    let mut session = FdemonSession::spawn(&fixture.path())
        .expect("Failed to spawn fdemon");

    // Track state transitions
    let mut states: Vec<String> = Vec::new();

    // Initial state
    session.expect_header().expect("Header");
    states.push("initialized".to_string());

    // Running state
    session.expect_running().expect("Running");
    states.push("running".to_string());

    // Reload -> Running
    session.send_key('r').expect("Reload");
    session.expect_reloading().expect("Reloading");
    states.push("reloading".to_string());

    session.expect_running().expect("Running again");
    states.push("running".to_string());

    // Verify valid state transitions
    let valid_transitions = [
        ("initialized", "running"),
        ("running", "reloading"),
        ("reloading", "running"),
    ];

    for window in states.windows(2) {
        let (from, to) = (&window[0], &window[1]);
        let is_valid = valid_transitions.iter()
            .any(|(f, t)| f == from && t == to);
        assert!(is_valid, "Invalid transition: {} -> {}", from, to);
    }

    session.kill().expect("Kill");
}
```

### Test Behavior Verification

The lifecycle test verifies:
1. **Creation**: fdemon starts and shows UI
2. **Running**: App reaches running state
3. **Reload**: Hot reload works and returns to running
4. **Restart**: Hot restart works and returns to running
5. **Stop**: App can be stopped
6. **Remove**: Session can be closed
7. **Exit**: Clean application exit

### Acceptance Criteria

1. Test covers all 7 lifecycle phases
2. Each phase transition is verified
3. State machine transitions are valid
4. Test completes in <60 seconds
5. Clean exit with success status code

### Testing

```bash
# Run lifecycle test
cargo test --test e2e test_full_session_lifecycle -- --nocapture

# Run with verbose output
RUST_LOG=debug cargo test --test e2e lifecycle -- --nocapture
```

### Notes

- This is a long-running test; consider `#[ignore]` for quick test runs
- Timeout values may need adjustment for CI
- Actual keybindings may differ from examples; verify against implementation
- Consider breaking into smaller tests if flaky

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
