## Task: Create E2E Test Utilities Module

**Objective**: Create the `tests/e2e/mod.rs` module with test helper functions, fixture loading, and common test setup utilities.

**Depends on**: 01-add-dependencies

### Scope

- `tests/e2e/mod.rs` - **NEW** Main E2E test module with utilities
- `tests/e2e.rs` - **NEW** Test entry point (required by Cargo)

### Details

Create the E2E test infrastructure module that will be used by all integration tests.

**File: `tests/e2e.rs`** (entry point):
```rust
//! E2E Integration Tests for Flutter Demon
//!
//! Run with: cargo test --test e2e

mod e2e;

// Re-export for test files
pub use e2e::*;
```

**File: `tests/e2e/mod.rs`**:
```rust
//! E2E test utilities and mock infrastructure
//!
//! This module provides:
//! - Test helper functions for creating test data
//! - Fixture loading utilities
//! - Mock daemon infrastructure (in mock_daemon.rs)
//! - Common test assertions

pub mod mock_daemon;

// Test modules
mod daemon_interaction;
mod hot_reload;
mod session_management;

use flutter_demon::daemon::Device;
use flutter_demon::app::state::AppState;
use flutter_demon::app::session::{Session, SessionHandle};
use flutter_demon::daemon::RequestTracker;
use std::sync::Arc;
use uuid::Uuid;

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
    format!("test-app-{}", &Uuid::new_v4().to_string()[..8])
}

/// Generate a unique session ID for testing
pub fn test_session_id() -> flutter_demon::app::session::SessionId {
    flutter_demon::app::session::SessionId::new()
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
    tokio::time::timeout(
        std::time::Duration::from_millis(duration_ms),
        future,
    )
    .await
    .map_err(|_| "Operation timed out")
}
```

### Acceptance Criteria

1. `tests/e2e.rs` exists and compiles
2. `tests/e2e/mod.rs` exists with helper functions
3. `cargo test --test e2e` runs (even if no tests exist yet)
4. Helper functions compile and are usable:
   - `test_device()`, `android_emulator()`, `ios_simulator()`
   - `test_app_state()`, `test_app_id()`
   - `load_fixture()`, `load_daemon_message()`

### Testing

```bash
# Verify module compiles
cargo test --test e2e --no-run

# Verify helpers work (add a simple test)
cargo test --test e2e test_helpers
```

### Notes

- This module does NOT include `MockFlutterDaemon` - that's Task 04
- Uses `flutter_demon` library crate for type imports
- The `tests/e2e.rs` file is required by Cargo's test discovery
- Fixture loading uses `CARGO_MANIFEST_DIR` for reliable paths
- Keep helpers focused on data creation, not behavior testing
