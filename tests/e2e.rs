//! E2E Integration Tests for Flutter Demon
//!
//! Run with: cargo test --test e2e

// Test submodules
mod e2e {
    mod daemon_interaction;
    mod hot_reload;
    pub mod mock_daemon;
    pub mod pty_utils;
    mod session_management;
}

use flutter_demon::app::state::AppState;
use flutter_demon::daemon::Device;

// ─────────────────────────────────────────────────────────
// Test Data Helpers
// ─────────────────────────────────────────────────────────

/// Create a test device with minimal required fields
pub fn test_device(id: &str, name: &str, platform: &str) -> Device {
    Device {
        id: id.to_string(),
        name: name.to_string(),
        platform: platform.to_string(),
        emulator: platform == "android",
        category: Some("mobile".to_string()),
        platform_type: Some(platform.to_string()),
        ephemeral: false,
        emulator_id: None,
    }
}

/// Create an Android emulator device
pub fn android_emulator(id: &str) -> Device {
    test_device(id, "Android Emulator", "android")
}

/// Create an iOS simulator device
pub fn ios_simulator(id: &str) -> Device {
    let mut device = test_device(id, "iPhone Simulator", "ios");
    device.emulator = true;
    device
}

/// Create a physical iOS device
pub fn ios_device(id: &str) -> Device {
    test_device(id, "iPhone", "ios")
}

/// Create a test AppState with default configuration
pub fn test_app_state() -> AppState {
    AppState::new()
}

/// Create a test AppState with a specific project path
pub fn test_app_state_with_project(path: &str) -> AppState {
    let mut state = AppState::new();
    state.project_path = std::path::PathBuf::from(path);
    state
}

/// Generate a unique app ID for testing
pub fn test_app_id() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(1);
    format!("test-app-{}", COUNTER.fetch_add(1, Ordering::SeqCst))
}

/// Generate a unique session ID for testing
pub fn test_session_id() -> flutter_demon::app::session::SessionId {
    flutter_demon::app::session::next_session_id()
}

// ─────────────────────────────────────────────────────────
// Fixture Loading
// ─────────────────────────────────────────────────────────

/// Load a JSON fixture file from tests/fixtures/daemon_responses/
pub fn load_fixture(name: &str) -> String {
    let path = format!(
        "{}/tests/fixtures/daemon_responses/{}.json",
        env!("CARGO_MANIFEST_DIR"),
        name
    );
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Failed to load fixture {}: {}", name, e))
}

/// Load and parse a fixture as a DaemonMessage
pub fn load_daemon_message(name: &str) -> flutter_demon::daemon::DaemonMessage {
    let json = load_fixture(name);
    flutter_demon::daemon::DaemonMessage::parse(&json)
        .unwrap_or_else(|| panic!("Failed to parse fixture {} as DaemonMessage", name))
}

/// Load a fixture containing a sequence of events
pub fn load_event_sequence(name: &str) -> Vec<serde_json::Value> {
    let json = load_fixture(name);
    serde_json::from_str(&json)
        .unwrap_or_else(|e| panic!("Failed to parse fixture {} as array: {}", name, e))
}

// ─────────────────────────────────────────────────────────
// Assertions
// ─────────────────────────────────────────────────────────

/// Assert that state phase matches expected
#[macro_export]
macro_rules! assert_phase {
    ($state:expr, $phase:pat) => {
        assert!(
            matches!($state.phase, $phase),
            "Expected phase {:?}, got {:?}",
            stringify!($phase),
            $state.phase
        );
    };
}

/// Assert that a session exists with expected app_id
#[macro_export]
macro_rules! assert_session_running {
    ($state:expr, $session_id:expr) => {
        let session = $state.session_manager.get($session_id);
        assert!(session.is_some(), "Session {:?} not found", $session_id);
        assert!(
            session.unwrap().session.app_id.is_some(),
            "Session {:?} has no app_id",
            $session_id
        );
    };
}

// ─────────────────────────────────────────────────────────
// Async Test Helpers
// ─────────────────────────────────────────────────────────

/// Create a timeout for async operations in tests
pub async fn with_timeout<T, F: std::future::Future<Output = T>>(
    duration_ms: u64,
    future: F,
) -> Result<T, &'static str> {
    tokio::time::timeout(std::time::Duration::from_millis(duration_ms), future)
        .await
        .map_err(|_| "Operation timed out")
}

// ─────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────

#[cfg(test)]
mod test_helpers {
    use super::*;

    #[test]
    fn test_test_device() {
        let device = test_device("device-123", "Test Device", "ios");
        assert_eq!(device.id, "device-123");
        assert_eq!(device.name, "Test Device");
        assert_eq!(device.platform, "ios");
        assert!(!device.emulator); // iOS is not emulator by default
    }

    #[test]
    fn test_android_emulator() {
        let device = android_emulator("emulator-5554");
        assert_eq!(device.id, "emulator-5554");
        assert_eq!(device.name, "Android Emulator");
        assert_eq!(device.platform, "android");
        assert!(device.emulator);
    }

    #[test]
    fn test_ios_simulator() {
        let device = ios_simulator("sim-123");
        assert_eq!(device.id, "sim-123");
        assert_eq!(device.name, "iPhone Simulator");
        assert_eq!(device.platform, "ios");
        assert!(device.emulator);
    }

    #[test]
    fn test_ios_device() {
        let device = ios_device("iphone-123");
        assert_eq!(device.id, "iphone-123");
        assert_eq!(device.name, "iPhone");
        assert_eq!(device.platform, "ios");
        assert!(!device.emulator);
    }

    #[test]
    fn test_test_app_state() {
        let state = test_app_state();
        assert!(state.project_path.as_os_str().is_empty());
    }

    #[test]
    fn test_test_app_state_with_project() {
        let state = test_app_state_with_project("/test/path");
        assert_eq!(state.project_path.to_str().unwrap(), "/test/path");
    }

    #[test]
    fn test_test_app_id_unique() {
        let id1 = test_app_id();
        let id2 = test_app_id();
        assert_ne!(id1, id2);
        assert!(id1.starts_with("test-app-"));
        assert!(id2.starts_with("test-app-"));
    }

    #[test]
    fn test_test_session_id_unique() {
        let id1 = test_session_id();
        let id2 = test_session_id();
        assert_ne!(id1, id2);
    }
}
