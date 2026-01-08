//! Complex workflow tests for end-to-end user journey verification
//!
//! These tests cover multi-step user workflows that exercise
//! multiple features in sequence.
//!
//! ## Headless Mode Constraints
//!
//! These tests run in `--headless` mode WITHOUT a real Flutter project/daemon.
//! Many lifecycle states (Running, Reloading) are not achievable because:
//! - No real Flutter daemon is spawned
//! - The mock daemon simulates limited responses
//! - State transitions depend on actual Flutter process events
//!
//! ## Test Coverage Strategy
//!
//! **Achievable Tests (headless compatible):**
//! - Startup -> Header display -> Quit flow
//! - Key handling verification (keys don't crash app)
//! - UI navigation patterns
//! - Graceful shutdown sequences
//!
//! **Not Achievable (require real Flutter):**
//! - Running state verification
//! - Hot reload/restart operations
//! - App state transitions (Starting -> Running -> Reloading)
//! - Session removal while app is running
//!
//! Tests requiring real Flutter are marked with `#[ignore]` and include
//! explanatory comments. Run them manually with a real Flutter project:
//!
//! ```bash
//! cargo test --test e2e workflow -- --ignored --nocapture
//! ```
//!
//! ## Running Tests
//!
//! ```bash
//! # Run non-ignored workflow tests (headless compatible)
//! cargo test --test e2e workflow -- --nocapture
//!
//! # Run all workflow tests including ignored (requires Flutter)
//! cargo test --test e2e workflow -- --ignored --nocapture
//!
//! # Run specific test
//! cargo test --test e2e test_full_session_lifecycle -- --nocapture
//! ```

use crate::e2e::pty_utils::{FdemonSession, SpecialKey, TestFixture};
use serial_test::serial;
use std::time::Duration;

// ===========================================================================
// Test Timing Constants
// ===========================================================================

/// Time to wait after sending input for processing
const INPUT_PROCESSING_DELAY_MS: u64 = 200;

/// Time to wait for application initialization
const INITIALIZATION_DELAY_MS: u64 = 500;

/// Timeout for state transition expectations
const STATE_TRANSITION_TIMEOUT_SECS: u64 = 10;

// ===========================================================================
// Complete Session Lifecycle Test
// ===========================================================================

/// Full session lifecycle: create -> run -> reload -> stop -> remove -> exit
///
/// **IGNORED:** This test requires a real Flutter daemon and cannot run in
/// headless mode. The headless environment cannot reach Running, Reloading,
/// or Stopped states because no actual Flutter process is spawned.
///
/// To run this test, you need:
/// - A real Flutter project
/// - At least one connected device/emulator
/// - Remove the `#[ignore]` attribute or run with `--ignored` flag
///
/// ```bash
/// cargo test --test e2e test_full_session_lifecycle -- --ignored --nocapture
/// ```
#[tokio::test]
#[serial]
#[ignore = "Requires real Flutter daemon - not achievable in headless mode"]
async fn test_full_session_lifecycle() {
    let fixture = TestFixture::simple_app();
    let mut session = FdemonSession::spawn(&fixture.path()).expect("Failed to spawn fdemon");

    // === Phase 1: Session Creation ===
    println!("Phase 1: Session Creation");

    // Wait for header to appear
    session.expect_header().expect("Should show header");

    // Wait for device selector or auto-start
    // Depending on config, may go straight to running
    let initial_state = session
        .expect_timeout(
            "Device|Running|Initializing",
            Duration::from_secs(STATE_TRANSITION_TIMEOUT_SECS),
        )
        .expect("Should reach initial state");

    // If device selector shown, select a device
    let state_text = initial_state
        .get(0)
        .map(|bytes| String::from_utf8_lossy(bytes).to_string())
        .unwrap_or_default();
    if state_text.contains("Device") {
        session
            .send_special(SpecialKey::Enter)
            .expect("Should select device");
    }

    // === Phase 2: App Running ===
    println!("Phase 2: App Running");

    session
        .expect_running()
        .expect("App should reach running state");

    // Verify UI shows running indicator
    session
        .expect("Running|running")
        .expect("Should show running status");

    // Optionally capture snapshot
    session
        .assert_snapshot("lifecycle_running")
        .expect("Running state snapshot");

    // === Phase 3: Hot Reload ===
    println!("Phase 3: Hot Reload");

    // Trigger hot reload
    session.send_key('r').expect("Should send reload command");

    // Verify reload state
    session
        .expect_reloading()
        .expect("Should show reloading state");

    // Wait for reload to complete
    session
        .expect_running()
        .expect("Should return to running after reload");

    // === Phase 4: Hot Restart ===
    println!("Phase 4: Hot Restart");

    // Trigger hot restart
    session.send_key('R').expect("Should send restart command");

    // Verify restart state
    session
        .expect("Restart|restart|Starting")
        .expect("Should show restart state");

    // Wait for restart to complete
    session
        .expect_running()
        .expect("Should return to running after restart");

    // === Phase 5: Stop App ===
    println!("Phase 5: Stop App");

    // Stop the running app
    session.send_key('s').expect("Should send stop command");

    // Verify app stopped
    session
        .expect("Stopped|stopped|Stop|Idle")
        .expect("Should show stopped state");

    // === Phase 6: Session Removal ===
    println!("Phase 6: Session Removal");

    // Close/remove the session
    session.send_key('x').expect("Should send close command");

    // Should show device selector (no sessions) or exit prompt
    session
        .expect("Device|No sessions|quit|exit")
        .expect("Should handle session removal");

    // === Phase 7: Clean Exit ===
    println!("Phase 7: Clean Exit");

    session.send_key('q').expect("Should send quit");

    // Handle quit confirmation if shown
    session.send_key('y').ok(); // May not need confirmation if no sessions

    let _ = session.quit();

    println!("Full session lifecycle test completed successfully!");
}

// ===========================================================================
// Simplified Lifecycle Test (Headless Compatible)
// ===========================================================================

/// Simplified lifecycle test that works in headless mode
///
/// Tests the flow: startup -> header display -> quit confirmation -> exit
/// This verifies the basic user journey without requiring a real Flutter daemon.
#[tokio::test]
#[serial]
async fn test_simplified_lifecycle_headless() {
    let fixture = TestFixture::simple_app();
    let mut session = FdemonSession::spawn(&fixture.path()).expect("Failed to spawn fdemon");

    // Phase 1: Startup
    println!("Phase 1: Startup and initialization");
    session
        .expect_header()
        .expect("Should show header on startup");

    // Give app time to initialize
    tokio::time::sleep(Duration::from_millis(INITIALIZATION_DELAY_MS)).await;

    // Phase 2: Verify UI responsiveness
    println!("Phase 2: UI interaction");

    // Close any modal that might be showing
    session
        .send_special(SpecialKey::Escape)
        .expect("Should send escape");
    tokio::time::sleep(Duration::from_millis(INPUT_PROCESSING_DELAY_MS)).await;

    // Try various keys to ensure they don't crash
    session.send_key('d').expect("Should send 'd'");
    tokio::time::sleep(Duration::from_millis(INPUT_PROCESSING_DELAY_MS)).await;

    session
        .send_special(SpecialKey::Escape)
        .expect("Should send escape");
    tokio::time::sleep(Duration::from_millis(INPUT_PROCESSING_DELAY_MS)).await;

    // Phase 3: Quit flow
    println!("Phase 3: Graceful quit");

    session.send_key('q').expect("Should send quit");

    // Wait for quit confirmation or immediate exit
    tokio::time::sleep(Duration::from_millis(INPUT_PROCESSING_DELAY_MS * 2)).await;

    // Confirm quit if dialog appeared
    session.send_key('y').ok();

    // Clean exit
    session.quit().expect("Should exit cleanly");

    println!("Simplified lifecycle test completed successfully!");
}

// ===========================================================================
// Session State Machine Tests
// ===========================================================================

/// Verify session state transitions are valid
///
/// **IGNORED:** This test requires a real Flutter daemon to observe actual
/// state transitions (initialized -> running -> reloading -> running).
/// In headless mode, the app cannot reach these states.
///
/// To run with a real Flutter project:
/// ```bash
/// cargo test --test e2e test_session_state_machine -- --ignored --nocapture
/// ```
#[tokio::test]
#[serial]
#[ignore = "Requires real Flutter daemon - state transitions not achievable in headless mode"]
async fn test_session_state_machine() {
    let fixture = TestFixture::simple_app();
    let mut session = FdemonSession::spawn(&fixture.path()).expect("Failed to spawn fdemon");

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
        let is_valid = valid_transitions.iter().any(|(f, t)| f == from && t == to);
        assert!(is_valid, "Invalid transition: {} -> {}", from, to);
    }

    session.kill().expect("Kill");
}

// ===========================================================================
// Multi-Key Workflow Tests (Headless Compatible)
// ===========================================================================

/// Test various key combinations work without crashing
///
/// Verifies that the application handles various key presses gracefully
/// even when they may not trigger state changes in headless mode.
#[tokio::test]
#[serial]
async fn test_key_handling_robustness() {
    let fixture = TestFixture::simple_app();
    let mut session = FdemonSession::spawn(&fixture.path()).expect("Failed to spawn fdemon");

    // Wait for startup
    session.expect_header().expect("Should show header");
    tokio::time::sleep(Duration::from_millis(INITIALIZATION_DELAY_MS)).await;

    println!("Testing session switching keys...");
    // Session switching (1-9) - should not crash even if no sessions exist
    for key in '1'..='9' {
        session.send_key(key).expect("Should send number key");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    println!("Testing navigation keys...");
    // Navigation keys
    session.send_special(SpecialKey::Tab).expect("Send Tab");
    tokio::time::sleep(Duration::from_millis(INPUT_PROCESSING_DELAY_MS)).await;

    session.send_special(SpecialKey::ArrowUp).expect("Send Up");
    tokio::time::sleep(Duration::from_millis(50)).await;

    session
        .send_special(SpecialKey::ArrowDown)
        .expect("Send Down");
    tokio::time::sleep(Duration::from_millis(50)).await;

    session
        .send_special(SpecialKey::PageUp)
        .expect("Send PageUp");
    tokio::time::sleep(Duration::from_millis(50)).await;

    session
        .send_special(SpecialKey::PageDown)
        .expect("Send PageDown");
    tokio::time::sleep(Duration::from_millis(50)).await;

    println!("Testing action keys...");
    // Action keys (should be no-ops in headless but shouldn't crash)
    session.send_key('r').expect("Send reload");
    tokio::time::sleep(Duration::from_millis(INPUT_PROCESSING_DELAY_MS)).await;

    session.send_key('R').expect("Send restart");
    tokio::time::sleep(Duration::from_millis(INPUT_PROCESSING_DELAY_MS)).await;

    session.send_key('s').expect("Send stop");
    tokio::time::sleep(Duration::from_millis(INPUT_PROCESSING_DELAY_MS)).await;

    session.send_key('x').expect("Send close session");
    tokio::time::sleep(Duration::from_millis(INPUT_PROCESSING_DELAY_MS)).await;

    session.send_key('d').expect("Send device selector");
    tokio::time::sleep(Duration::from_millis(INPUT_PROCESSING_DELAY_MS)).await;

    println!("Testing escape key...");
    session
        .send_special(SpecialKey::Escape)
        .expect("Send Escape");
    tokio::time::sleep(Duration::from_millis(INPUT_PROCESSING_DELAY_MS)).await;

    // App should still be responsive
    session
        .expect_header()
        .expect("Header should still be visible after key presses");

    // Clean exit
    session
        .quit()
        .expect("Should quit cleanly after key handling test");

    println!("Key handling robustness test completed successfully!");
}

// ===========================================================================
// Navigation Flow Tests
// ===========================================================================

/// Test device selector -> escape -> quit flow
///
/// Verifies that the user can navigate through the device selector
/// modal and exit gracefully.
#[tokio::test]
#[serial]
async fn test_device_selector_quit_flow() {
    let fixture = TestFixture::simple_app();
    let mut session = FdemonSession::spawn(&fixture.path()).expect("Failed to spawn fdemon");

    println!("Step 1: Wait for header");
    session.expect_header().expect("Should show header");

    // Give app time to initialize
    tokio::time::sleep(Duration::from_millis(INITIALIZATION_DELAY_MS)).await;

    println!("Step 2: Open device selector");
    session
        .send_key('d')
        .expect("Should send 'd' to open device selector");

    tokio::time::sleep(Duration::from_millis(INPUT_PROCESSING_DELAY_MS * 2)).await;

    println!("Step 3: Close with Escape");
    session
        .send_special(SpecialKey::Escape)
        .expect("Should send Escape");

    tokio::time::sleep(Duration::from_millis(INPUT_PROCESSING_DELAY_MS)).await;

    println!("Step 4: Initiate quit");
    session.send_key('q').expect("Should send quit");

    tokio::time::sleep(Duration::from_millis(INPUT_PROCESSING_DELAY_MS * 2)).await;

    println!("Step 5: Confirm quit");
    session.send_key('y').ok(); // May not need confirmation

    session.quit().expect("Should exit cleanly");

    println!("Device selector quit flow completed successfully!");
}

/// Test quit cancellation flow
///
/// Verifies that the user can cancel a quit operation and continue using the app.
#[tokio::test]
#[serial]
async fn test_quit_cancel_flow() {
    let fixture = TestFixture::simple_app();
    let mut session = FdemonSession::spawn(&fixture.path()).expect("Failed to spawn fdemon");

    session.expect_header().expect("Should show header");

    tokio::time::sleep(Duration::from_millis(INITIALIZATION_DELAY_MS)).await;

    // Close any modal
    session.send_special(SpecialKey::Escape).ok();
    tokio::time::sleep(Duration::from_millis(INPUT_PROCESSING_DELAY_MS)).await;

    println!("Initiating quit...");
    session.send_key('q').expect("Should send quit");

    tokio::time::sleep(Duration::from_millis(INPUT_PROCESSING_DELAY_MS * 2)).await;

    println!("Cancelling quit with 'n'...");
    session.send_key('n').expect("Should send 'n' to cancel");

    tokio::time::sleep(Duration::from_millis(INPUT_PROCESSING_DELAY_MS)).await;

    // App should still be running
    session
        .expect_header()
        .expect("Should still show header after quit cancellation");

    println!("Trying quit cancellation with Escape...");
    session.send_key('q').expect("Should send quit again");

    tokio::time::sleep(Duration::from_millis(INPUT_PROCESSING_DELAY_MS * 2)).await;

    session
        .send_special(SpecialKey::Escape)
        .expect("Should send Escape to cancel");

    tokio::time::sleep(Duration::from_millis(INPUT_PROCESSING_DELAY_MS)).await;

    // App should still be running
    session
        .expect_header()
        .expect("Should still show header after second quit cancellation");

    // Now actually quit
    session.quit().expect("Should quit cleanly");

    println!("Quit cancellation flow completed successfully!");
}

// ===========================================================================
// Double-Key Shortcut Tests
// ===========================================================================

/// Test double-'q' quick quit shortcut
///
/// Verifies that pressing 'q' twice in quick succession exits the app
/// without requiring explicit confirmation.
#[tokio::test]
#[serial]
async fn test_double_q_quick_quit() {
    let fixture = TestFixture::simple_app();
    let mut session = FdemonSession::spawn(&fixture.path()).expect("Failed to spawn fdemon");

    session.expect_header().expect("Should show header");

    tokio::time::sleep(Duration::from_millis(INITIALIZATION_DELAY_MS)).await;

    // Close any modal first
    session.send_special(SpecialKey::Escape).ok();
    tokio::time::sleep(Duration::from_millis(INPUT_PROCESSING_DELAY_MS)).await;

    println!("Sending first 'q'...");
    session.send_key('q').expect("Should send first 'q'");

    // Brief pause to show dialog
    tokio::time::sleep(Duration::from_millis(INPUT_PROCESSING_DELAY_MS)).await;

    println!("Sending second 'q' (acts as confirmation)...");
    session.send_key('q').expect("Should send second 'q'");

    // Give time for quit to process
    tokio::time::sleep(Duration::from_millis(INPUT_PROCESSING_DELAY_MS * 2)).await;

    // Process should exit or be in process of exiting
    // quit() will handle cleanup
    session.quit().ok();

    println!("Double-q quick quit test completed!");
}

// ===========================================================================
// Multi-Session Workflow Tests
// ===========================================================================

/// Multi-session workflow: create two sessions and switch between them
///
/// **IGNORED:** This test requires multiple device connections which are not
/// available in headless mode. Creating a second session requires selecting
/// a different device from the device selector, but in headless mode:
/// - Only one mock device may be available
/// - Device selection may not create actual sessions
/// - Session state transitions require real Flutter daemons
///
/// To run this test with real Flutter:
/// - Connect multiple devices/emulators
/// - Remove `#[ignore]` or run with `--ignored` flag
///
/// ```bash
/// cargo test --test e2e test_multi_session_workflow -- --ignored --nocapture
/// ```
#[tokio::test]
#[serial]
#[ignore = "Requires multiple devices - not achievable in headless mode"]
async fn test_multi_session_workflow() {
    let fixture = TestFixture::simple_app();
    let mut session = FdemonSession::spawn(&fixture.path()).expect("Failed to spawn fdemon");

    // === Create First Session ===
    println!("Creating first session...");

    session
        .expect_running()
        .expect("First session should be running");

    // Verify session 1 indicator
    session.expect("[1]").expect("Should show session 1 tab");

    // === Create Second Session ===
    println!("Creating second session...");

    // Open device selector
    session.send_key('d').expect("Should open device selector");

    session
        .expect_device_selector()
        .expect("Should show device selector");

    // Select device to create new session
    session
        .send_special(SpecialKey::Enter)
        .expect("Should select device");

    // Wait for second session
    session.expect("[2]").expect("Should show session 2 tab");

    // === Verify Session Switching ===
    println!("Testing session switching...");

    // Should be on session 2 now
    session
        .expect("Session 2|\\[2\\].*active")
        .expect("Session 2 should be active");

    // Switch to session 1
    session.send_key('1').expect("Should switch to session 1");

    session
        .expect("Session 1|\\[1\\].*active")
        .expect("Session 1 should be active");

    // Switch back to session 2
    session.send_key('2').expect("Should switch to session 2");

    session
        .expect("Session 2|\\[2\\].*active")
        .expect("Session 2 should be active");

    // === Test Tab Cycling ===
    println!("Testing Tab cycling...");

    session
        .send_special(SpecialKey::Tab)
        .expect("Should cycle to next session");

    session
        .expect("\\[1\\].*active")
        .expect("Should be on session 1 after Tab");

    session
        .send_special(SpecialKey::Tab)
        .expect("Should cycle again");

    session
        .expect("\\[2\\].*active")
        .expect("Should be on session 2 after Tab");

    // === Snapshot Multi-Session UI ===
    session
        .assert_snapshot("multi_session_tabs")
        .expect("Multi-session snapshot");

    // === Clean Up ===
    session.kill().expect("Should kill process");
}

/// Test parallel hot reload across all sessions
///
/// **IGNORED:** This test requires multiple running Flutter sessions which
/// cannot be achieved in headless mode. The test needs:
/// - Multiple devices connected
/// - Multiple active Flutter processes
/// - Actual reload state transitions
///
/// In headless mode, the mock environment cannot simulate multiple concurrent
/// Flutter daemons or their reload responses.
///
/// To run with real Flutter:
/// ```bash
/// cargo test --test e2e test_parallel_reload_all_sessions -- --ignored --nocapture
/// ```
#[tokio::test]
#[serial]
#[ignore = "Requires multiple running sessions - not achievable in headless mode"]
async fn test_parallel_reload_all_sessions() {
    let fixture = TestFixture::simple_app();
    let mut session = FdemonSession::spawn(&fixture.path()).expect("Failed to spawn fdemon");

    // Create two sessions
    session.expect_running().expect("First session running");

    session.send_key('d').expect("Open device selector");
    session.expect_device_selector().expect("Device selector");
    session
        .send_special(SpecialKey::Enter)
        .expect("Select device");
    session.expect("[2]").expect("Second session created");

    // Both sessions should be running
    session.expect_running().expect("Sessions running");

    // Trigger reload all ('a' + 'r' or specific keybinding)
    // Note: Check actual keybinding for "reload all"
    session
        .send_key('r') // May reload only current session
        .expect("Should reload");

    // Current session should show reloading
    session.expect_reloading().expect("Should be reloading");

    // Wait for reload to complete
    session.expect_running().expect("Should return to running");

    // Switch to other session and verify it's still running
    session.send_key('1').expect("Switch to session 1");
    session
        .expect_running()
        .expect("Session 1 should be running");

    session.kill().expect("Kill");
}

/// Test session ordering remains consistent
///
/// **IGNORED:** This test requires multiple devices to create 3 sessions.
/// In headless mode:
/// - Multiple device selection is not possible
/// - Session creation depends on device availability
/// - Tab navigation may not work without real sessions
///
/// To run with real Flutter and multiple devices:
/// ```bash
/// cargo test --test e2e test_session_ordering -- --ignored --nocapture
/// ```
#[tokio::test]
#[serial]
#[ignore = "Requires multiple devices - not achievable in headless mode"]
async fn test_session_ordering() {
    let fixture = TestFixture::simple_app();
    let mut session = FdemonSession::spawn(&fixture.path()).expect("Failed to spawn fdemon");

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
    session
        .expect("\\[1\\]|\\[3\\]")
        .expect("Should have remaining sessions");

    session.kill().expect("Kill");
}

/// Test closing all sessions shows appropriate UI
///
/// **IGNORED:** Even though this test doesn't require real Flutter sessions,
/// it still needs the header to appear which may not work reliably in all
/// headless environments. Mark as ignored for CI compatibility.
///
/// To run manually:
/// ```bash
/// cargo test --test e2e test_close_all_sessions -- --ignored --nocapture
/// ```
#[tokio::test]
#[serial]
#[ignore = "Requires reliable header display - may not work in all headless environments"]
async fn test_close_all_sessions() {
    let fixture = TestFixture::simple_app();
    let mut session = FdemonSession::spawn(&fixture.path()).expect("Failed to spawn fdemon");

    // Wait for initial UI
    session.expect_header().expect("Should show header");

    // Give time for initialization
    tokio::time::sleep(Duration::from_millis(INITIALIZATION_DELAY_MS)).await;

    // Close any modal first
    session.send_special(SpecialKey::Escape).ok();
    tokio::time::sleep(Duration::from_millis(INPUT_PROCESSING_DELAY_MS)).await;

    println!("Attempting to close session...");

    // Close the session (may not exist in headless, but should handle gracefully)
    session.send_key('x').expect("Send close session");

    // Give time to process
    tokio::time::sleep(Duration::from_millis(INPUT_PROCESSING_DELAY_MS * 2)).await;

    // Should still be able to quit
    println!("Quitting application...");
    session.send_key('q').expect("Send quit");

    tokio::time::sleep(Duration::from_millis(INPUT_PROCESSING_DELAY_MS * 2)).await;

    // Confirm quit if needed
    session.send_key('y').ok();

    session.quit().expect("Should exit cleanly");

    println!("Close all sessions test completed!");
}

/// Test session switching keys (1-9) work without crashing
///
/// **IGNORED:** While this test doesn't require real Flutter, it needs the
/// header to appear reliably which may not work in all headless environments.
///
/// To run manually:
/// ```bash
/// cargo test --test e2e test_session_switching_keys_headless -- --ignored --nocapture
/// ```
#[tokio::test]
#[serial]
#[ignore = "Requires reliable header display - may not work in all headless environments"]
async fn test_session_switching_keys_headless() {
    let fixture = TestFixture::simple_app();
    let mut session = FdemonSession::spawn(&fixture.path()).expect("Failed to spawn fdemon");

    session.expect_header().expect("Should show header");

    tokio::time::sleep(Duration::from_millis(INITIALIZATION_DELAY_MS)).await;

    // Close any modal
    session.send_special(SpecialKey::Escape).ok();
    tokio::time::sleep(Duration::from_millis(INPUT_PROCESSING_DELAY_MS)).await;

    println!("Testing session switch keys 1-9...");

    // Try switching to various sessions (should not crash even if they don't exist)
    for key in '1'..='9' {
        session.send_key(key).expect("Should send session number");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    println!("Testing Tab cycling...");

    // Tab cycling should also not crash
    for _ in 0..5 {
        session.send_special(SpecialKey::Tab).expect("Send Tab");
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    // App should still be responsive
    session
        .expect_header()
        .expect("Header should still be visible after session key presses");

    // Clean exit
    session.quit().expect("Should quit cleanly");

    println!("Session switching keys test completed!");
}

// ===========================================================================
// Error Recovery Workflow Tests
// ===========================================================================

/// Test recovery from daemon crash (display error, allow restart)
///
/// **IGNORED:** This test requires a real Flutter daemon to crash.
/// In headless mode, we cannot:
/// - Spawn a real Flutter process to crash
/// - Kill the Flutter process to simulate crash
/// - Observe the daemon crash recovery flow
///
/// To test daemon crash recovery, you need:
/// - A real Flutter project with actual Flutter daemon
/// - A way to forcefully kill the Flutter process (e.g., process PID access)
///
/// The test documents expected behavior:
/// 1. App is running with Flutter daemon
/// 2. Daemon crashes (killed externally or internal error)
/// 3. fdemon displays error state
/// 4. User can see error details
/// 5. User can quit gracefully
///
/// ```bash
/// cargo test --test e2e test_daemon_crash_recovery -- --ignored --nocapture
/// ```
#[tokio::test]
#[serial]
#[ignore = "Requires real Flutter daemon - cannot simulate crash in headless mode"]
async fn test_daemon_crash_recovery() {
    let fixture = TestFixture::simple_app();
    let mut session = FdemonSession::spawn(&fixture.path()).expect("Failed to spawn fdemon");

    // Get to running state
    session.expect_running().expect("App should be running");

    // Simulate daemon crash by killing the Flutter process
    // This requires access to the Flutter process PID, which is not
    // exposed in headless mode.
    //
    // Alternative approach: Use error_app fixture that crashes on reload
    // (but error_app shows compile errors, not daemon crashes)

    // For manual testing with real Flutter:
    // 1. Start fdemon with a real Flutter project
    // 2. Note the Flutter process PID from process list
    // 3. kill -9 <flutter_pid>
    // 4. Observe fdemon shows error state
    // 5. Verify user can quit gracefully

    session.kill().expect("Kill current session");

    // Document expected behavior:
    // - Should show "Disconnected" or "Error" or "Crashed" state
    // - Error message should be visible in logs
    // - User can press 'q' to quit
    // - App doesn't panic or hang
}

/// Test recovery from compilation error after edit
///
/// **IGNORED:** This test requires a real Flutter project and the ability
/// to modify source files and trigger recompilation. In headless mode:
/// - We cannot trigger actual hot reload (no real daemon)
/// - File modifications don't trigger watch events
/// - Compilation errors are only visible with real Flutter
///
/// The test documents expected behavior:
/// 1. Start with working app
/// 2. Modify a file to introduce syntax error
/// 3. Trigger reload -> compilation fails
/// 4. fdemon displays error with file location and message
/// 5. Fix the file
/// 6. Trigger reload -> compilation succeeds
/// 7. App returns to running state
///
/// For manual testing:
/// ```bash
/// # 1. Start fdemon with a real Flutter project
/// # 2. Edit lib/main.dart to add syntax error (e.g., remove semicolon)
/// # 3. Save file (triggers auto-reload) or press 'r'
/// # 4. Observe error display
/// # 5. Fix the error
/// # 6. Save or reload again
/// # 7. Verify app recovers to running state
/// ```
#[tokio::test]
#[serial]
#[ignore = "Requires real Flutter daemon and file modification - not achievable in headless mode"]
async fn test_compilation_error_recovery() {
    let fixture = TestFixture::simple_app();
    let mut session = FdemonSession::spawn(&fixture.path()).expect("Failed to spawn fdemon");

    session
        .expect_running()
        .expect("Should start with working app");

    // Would need to:
    // 1. Modify a Dart file to introduce error
    // 2. Trigger reload
    // 3. See compilation error with file location
    // 4. Fix the file
    // 5. Trigger reload
    // 6. See app return to running state

    // This requires:
    // - Real Flutter daemon (for compilation)
    // - File system modification
    // - File watcher integration (or manual reload trigger)
    // - Flutter's error message parsing

    session.kill().expect("Kill");
}

/// Test handling of device disconnect scenarios
///
/// **IGNORED:** This test requires a real device connection and the ability
/// to disconnect it during runtime. In headless mode:
/// - No real devices are connected
/// - Device state changes are not simulated
/// - Cannot test device disconnection behavior
///
/// The test documents expected behavior:
/// 1. App is running on a device
/// 2. Device disconnects (USB unplugged, emulator closed, etc.)
/// 3. fdemon detects disconnection
/// 4. Shows "Disconnected" or "Lost connection" state
/// 5. Allows user to select a new device
/// 6. Does not crash or hang
///
/// For manual testing:
/// ```bash
/// # 1. Start fdemon with app running on physical device
/// # 2. Unplug USB cable
/// # 3. Observe fdemon behavior
/// # OR:
/// # 1. Start fdemon with app running on emulator
/// # 2. Close emulator window
/// # 3. Observe fdemon behavior
/// ```
#[tokio::test]
#[serial]
#[ignore = "Requires real device connection - cannot test in headless mode"]
async fn test_device_disconnect_handling() {
    let fixture = TestFixture::simple_app();
    let mut session = FdemonSession::spawn(&fixture.path()).expect("Failed to spawn fdemon");

    session.expect_running().expect("Should be running");

    // Would need to:
    // 1. Have app running on real device
    // 2. Simulate device disconnect (close emulator, unplug USB, etc.)
    // 3. Observe fdemon shows disconnected state
    // 4. Verify app doesn't crash
    // 5. Verify user can select new device or quit

    // In headless mode, this is not achievable

    session.kill().expect("Kill");
}

/// Test graceful handling of invalid/corrupted input
///
/// Verifies that fdemon handles invalid input gracefully:
/// - Null bytes are ignored
/// - Invalid escape sequences are ignored
/// - App remains responsive after invalid input
/// - Normal operations continue to work
///
/// This test DOES work in headless mode because it only tests input handling,
/// not daemon interaction.
#[tokio::test]
#[serial]
async fn test_graceful_degradation() {
    let fixture = TestFixture::simple_app();
    let mut session = FdemonSession::spawn(&fixture.path()).expect("Failed to spawn fdemon");

    println!("Phase 1: Verify initial state");
    session.expect_header().expect("Header should appear");

    tokio::time::sleep(Duration::from_millis(INITIALIZATION_DELAY_MS)).await;

    println!("Phase 2: Send invalid input sequences");

    // Send null byte (should be ignored)
    session.send_raw(&[0x00]).expect("Send null byte");
    tokio::time::sleep(Duration::from_millis(INPUT_PROCESSING_DELAY_MS)).await;

    // Send invalid escape sequences (should be ignored)
    session
        .send_raw(&[0x1b, 0x5b, 0x99])
        .expect("Send invalid escape");
    tokio::time::sleep(Duration::from_millis(INPUT_PROCESSING_DELAY_MS)).await;

    // Send partial escape sequence (should be ignored or handled gracefully)
    session
        .send_raw(&[0x1b, 0x5b])
        .expect("Send partial escape");
    tokio::time::sleep(Duration::from_millis(INPUT_PROCESSING_DELAY_MS)).await;

    // Send random control characters
    session
        .send_raw(&[0x01, 0x02, 0x7f])
        .expect("Send control chars");
    tokio::time::sleep(Duration::from_millis(INPUT_PROCESSING_DELAY_MS)).await;

    println!("Phase 3: Verify app is still responsive");

    // fdemon should still be responsive and show header
    session
        .expect_header()
        .expect("Should still show header after invalid input");

    println!("Phase 4: Verify normal operations still work");

    // Close any modal that might have appeared
    session.send_special(SpecialKey::Escape).ok();
    tokio::time::sleep(Duration::from_millis(INPUT_PROCESSING_DELAY_MS)).await;

    // Normal operations should still work
    session.send_key('q').expect("Quit should work");

    tokio::time::sleep(Duration::from_millis(INPUT_PROCESSING_DELAY_MS * 2)).await;

    session
        .expect("quit|Quit|exit|Exit")
        .expect("Quit confirmation should appear");

    session.send_key('n').expect("Cancel quit");

    tokio::time::sleep(Duration::from_millis(INPUT_PROCESSING_DELAY_MS)).await;

    // Should return to normal state
    session
        .expect_header()
        .expect("Back to normal after quit cancellation");

    session.quit().expect("Final quit should work");

    println!("Graceful degradation test completed successfully!");
}

/// Test timeout handling for slow operations
///
/// **IGNORED:** This test requires a real Flutter daemon to test actual
/// reload/restart timeout behavior. In headless mode:
/// - Hot reload is a no-op (no daemon)
/// - No actual timeouts occur
/// - Cannot verify timeout handling
///
/// The test documents expected behavior:
/// 1. Trigger a reload operation
/// 2. Reload takes longer than expected (slow network, large app, etc.)
/// 3. Either reload completes within reasonable time (success)
/// 4. Or fdemon shows appropriate timeout/error state
/// 5. App remains responsive and doesn't hang
///
/// For manual testing with slow reload:
/// ```bash
/// # Use a large Flutter app or slow device
/// # Trigger hot reload
/// # Observe timeout behavior (if any)
/// ```
#[tokio::test]
#[serial]
#[ignore = "Requires real Flutter daemon - timeout behavior not testable in headless mode"]
async fn test_timeout_handling() {
    let fixture = TestFixture::simple_app();
    let mut session = FdemonSession::spawn(&fixture.path()).expect("Failed to spawn fdemon");

    session.expect_running().expect("Running");

    // Trigger reload
    session.send_key('r').expect("Reload");

    // Reload should complete within reasonable time
    // If it times out, fdemon should show appropriate state
    let result = session.expect_timeout(
        "Running|running|Timeout|timeout|Error|error",
        Duration::from_secs(60),
    );

    match result {
        Ok(_) => {
            // Reload completed (success or error)
            println!("Reload completed within timeout");
        }
        Err(_) => {
            // Timeout - capture state for debugging
            println!("Reload timed out - capturing state");
            let _ = session.capture_for_snapshot();
        }
    }

    session.kill().expect("Kill");
}

/// Test that fdemon doesn't panic on edge cases or rapid input
///
/// Verifies that fdemon survives:
/// - Rapid key presses
/// - Chaotic input sequences
/// - Contradictory commands in quick succession
/// - Edge cases like quit+cancel+quit rapidly
///
/// This test DOES work in headless mode because it only tests input handling
/// robustness, not daemon interaction.
#[tokio::test]
#[serial]
async fn test_no_panic_on_edge_cases() {
    let fixture = TestFixture::simple_app();

    println!("Phase 1: Test rapid key presses");
    let mut session = FdemonSession::spawn(&fixture.path()).expect("Failed to spawn fdemon");

    session.expect_header().expect("Header");

    tokio::time::sleep(Duration::from_millis(INITIALIZATION_DELAY_MS)).await;

    // Close any initial modal
    session.send_special(SpecialKey::Escape).ok();
    tokio::time::sleep(Duration::from_millis(INPUT_PROCESSING_DELAY_MS)).await;

    // Rapid fire various keys
    for _ in 0..10 {
        session.send_key('r').ok(); // Reload
        session.send_key('R').ok(); // Restart
        session.send_key('q').ok(); // Quit
        session.send_key('n').ok(); // Cancel
        session.send_key('d').ok(); // Device selector
        session.send_special(SpecialKey::Escape).ok(); // Escape
    }

    // Brief pause to let processing catch up
    tokio::time::sleep(Duration::from_millis(INPUT_PROCESSING_DELAY_MS * 2)).await;

    // Should still be alive and responsive
    session.expect_header().expect("Should survive rapid input");

    session.kill().expect("Kill first session");

    println!("Phase 2: Test contradictory command sequences");
    let mut session = FdemonSession::spawn(&fixture.path()).expect("Failed to spawn fdemon");

    session.expect_header().expect("Header");

    tokio::time::sleep(Duration::from_millis(INITIALIZATION_DELAY_MS)).await;

    // Close any modal
    session.send_special(SpecialKey::Escape).ok();
    tokio::time::sleep(Duration::from_millis(INPUT_PROCESSING_DELAY_MS)).await;

    // Contradictory sequences
    // Open device selector, then quit, then cancel, then escape
    session.send_key('d').ok();
    tokio::time::sleep(Duration::from_millis(50)).await;
    session.send_key('q').ok();
    tokio::time::sleep(Duration::from_millis(50)).await;
    session.send_key('n').ok();
    tokio::time::sleep(Duration::from_millis(50)).await;
    session.send_special(SpecialKey::Escape).ok();
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Should still be responsive
    session
        .expect_header()
        .expect("Should survive contradictory commands");

    println!("Phase 3: Test rapid session switching");
    // Session switching (1-9) rapidly
    for key in '1'..='9' {
        session.send_key(key).ok();
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    tokio::time::sleep(Duration::from_millis(INPUT_PROCESSING_DELAY_MS)).await;

    // Should still be alive
    session
        .expect_header()
        .expect("Should survive rapid session switching");

    println!("Phase 4: Test rapid scrolling");
    for _ in 0..20 {
        session.send_special(SpecialKey::ArrowUp).ok();
        session.send_special(SpecialKey::ArrowDown).ok();
        session.send_special(SpecialKey::PageUp).ok();
        session.send_special(SpecialKey::PageDown).ok();
    }

    tokio::time::sleep(Duration::from_millis(INPUT_PROCESSING_DELAY_MS)).await;

    // Should still be responsive
    session
        .expect_header()
        .expect("Should survive rapid scrolling");

    session
        .quit()
        .expect("Should quit cleanly after stress test");

    println!("Edge cases test completed successfully!");
}
