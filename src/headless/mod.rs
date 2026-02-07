//! Headless mode - JSON event output for E2E testing
//!
//! This module provides a headless (non-TUI) mode for fdemon that outputs
//! structured JSON events to stdout. This enables reliable parsing in test scripts,
//! avoiding the complexity of parsing ANSI escape codes from the TUI.
//!
//! # Event Format
//!
//! Events are output as NDJSON (newline-delimited JSON), one event per line.
//! Each event has an "event" field indicating its type, along with event-specific data.
//!
//! # Example Output
//!
//! ```json
//! {"event":"daemon_connected","device":"linux","timestamp":1704700001000}
//! {"event":"app_started","session_id":"abc-123","device":"linux","timestamp":1704700002000}
//! {"event":"log","level":"info","message":"Flutter initialized","session_id":"abc-123","timestamp":1704700003000}
//! ```

pub mod runner;

use chrono::Utc;
use serde::Serialize;
use std::io::{self, Write};
use tracing::error;

/// Events emitted in headless mode
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "event", rename_all = "snake_case")]
#[allow(dead_code)] // Future functionality - variants will be used when headless mode is fully implemented
pub enum HeadlessEvent {
    /// Flutter daemon connected successfully
    DaemonConnected { device: String, timestamp: i64 },

    /// Flutter daemon disconnected
    DaemonDisconnected {
        device: String,
        reason: Option<String>,
        timestamp: i64,
    },

    /// Device detected/discovered
    DeviceDetected {
        device_id: String,
        device_name: String,
        platform: String,
        timestamp: i64,
    },

    /// Flutter app started successfully
    AppStarted {
        session_id: String,
        device: String,
        timestamp: i64,
    },

    /// Flutter app stopped
    AppStopped {
        session_id: String,
        reason: Option<String>,
        timestamp: i64,
    },

    /// Hot reload initiated
    HotReloadStarted { session_id: String, timestamp: i64 },

    /// Hot reload completed successfully
    HotReloadCompleted {
        session_id: String,
        duration_ms: u64,
        timestamp: i64,
    },

    /// Hot reload failed
    HotReloadFailed {
        session_id: String,
        error: String,
        timestamp: i64,
    },

    /// Log entry from Flutter app
    Log {
        level: String,
        message: String,
        session_id: Option<String>,
        timestamp: i64,
    },

    /// Error occurred
    Error {
        message: String,
        fatal: bool,
        timestamp: i64,
    },

    /// New session created
    SessionCreated {
        session_id: String,
        device: String,
        timestamp: i64,
    },

    /// Session removed/ended
    SessionRemoved { session_id: String, timestamp: i64 },
}

#[allow(dead_code)] // Future functionality - constructors will be used when headless mode is fully implemented
impl HeadlessEvent {
    /// Emit this event to stdout as JSON
    pub fn emit(&self) {
        // Serialize to JSON
        let json = match serde_json::to_string(self) {
            Ok(json) => json,
            Err(e) => {
                error!("Failed to serialize headless event: {}", e);
                return;
            }
        };

        // Write to stdout with newline (NDJSON format)
        let mut stdout = io::stdout().lock();
        if let Err(e) = writeln!(stdout, "{}", json) {
            error!("Failed to write headless event to stdout: {}", e);
            return;
        }

        // Flush to ensure immediate output
        if let Err(e) = stdout.flush() {
            error!("Failed to flush headless stdout: {}", e);
        }
    }

    /// Get current timestamp in milliseconds
    fn now() -> i64 {
        Utc::now().timestamp_millis()
    }

    // ─────────────────────────────────────────────────────────
    // Convenience constructors
    // ─────────────────────────────────────────────────────────

    pub fn daemon_connected(device: &str) -> Self {
        Self::DaemonConnected {
            device: device.to_string(),
            timestamp: Self::now(),
        }
    }

    pub fn daemon_disconnected(device: &str, reason: Option<String>) -> Self {
        Self::DaemonDisconnected {
            device: device.to_string(),
            reason,
            timestamp: Self::now(),
        }
    }

    pub fn device_detected(device_id: &str, device_name: &str, platform: &str) -> Self {
        Self::DeviceDetected {
            device_id: device_id.to_string(),
            device_name: device_name.to_string(),
            platform: platform.to_string(),
            timestamp: Self::now(),
        }
    }

    pub fn app_started(session_id: &str, device: &str) -> Self {
        Self::AppStarted {
            session_id: session_id.to_string(),
            device: device.to_string(),
            timestamp: Self::now(),
        }
    }

    pub fn app_stopped(session_id: &str, reason: Option<String>) -> Self {
        Self::AppStopped {
            session_id: session_id.to_string(),
            reason,
            timestamp: Self::now(),
        }
    }

    pub fn hot_reload_started(session_id: &str) -> Self {
        Self::HotReloadStarted {
            session_id: session_id.to_string(),
            timestamp: Self::now(),
        }
    }

    pub fn hot_reload_completed(session_id: &str, duration_ms: u64) -> Self {
        Self::HotReloadCompleted {
            session_id: session_id.to_string(),
            duration_ms,
            timestamp: Self::now(),
        }
    }

    pub fn hot_reload_failed(session_id: &str, error: String) -> Self {
        Self::HotReloadFailed {
            session_id: session_id.to_string(),
            error,
            timestamp: Self::now(),
        }
    }

    pub fn log(level: &str, message: String, session_id: Option<String>) -> Self {
        Self::Log {
            level: level.to_string(),
            message,
            session_id,
            timestamp: Self::now(),
        }
    }

    pub fn error(message: String, fatal: bool) -> Self {
        Self::Error {
            message,
            fatal,
            timestamp: Self::now(),
        }
    }

    pub fn session_created(session_id: &str, device: &str) -> Self {
        Self::SessionCreated {
            session_id: session_id.to_string(),
            device: device.to_string(),
            timestamp: Self::now(),
        }
    }

    pub fn session_removed(session_id: &str) -> Self {
        Self::SessionRemoved {
            session_id: session_id.to_string(),
            timestamp: Self::now(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_daemon_connected_serialization() {
        let event = HeadlessEvent::daemon_connected("linux");
        let json = serde_json::to_string(&event).expect("serialization failed");

        // Parse back to ensure valid JSON
        let value: serde_json::Value = serde_json::from_str(&json).expect("invalid JSON");

        assert_eq!(value["event"], "daemon_connected");
        assert_eq!(value["device"], "linux");
        assert!(value["timestamp"].is_number());
    }

    #[test]
    fn test_app_started_serialization() {
        let event = HeadlessEvent::app_started("session-123", "android");
        let json = serde_json::to_string(&event).expect("serialization failed");

        let value: serde_json::Value = serde_json::from_str(&json).expect("invalid JSON");

        assert_eq!(value["event"], "app_started");
        assert_eq!(value["session_id"], "session-123");
        assert_eq!(value["device"], "android");
        assert!(value["timestamp"].is_number());
    }

    #[test]
    fn test_hot_reload_completed_serialization() {
        let event = HeadlessEvent::hot_reload_completed("session-456", 1250);
        let json = serde_json::to_string(&event).expect("serialization failed");

        let value: serde_json::Value = serde_json::from_str(&json).expect("invalid JSON");

        assert_eq!(value["event"], "hot_reload_completed");
        assert_eq!(value["session_id"], "session-456");
        assert_eq!(value["duration_ms"], 1250);
        assert!(value["timestamp"].is_number());
    }

    #[test]
    fn test_log_serialization() {
        let event = HeadlessEvent::log(
            "info",
            "Application started".to_string(),
            Some("session-789".to_string()),
        );
        let json = serde_json::to_string(&event).expect("serialization failed");

        let value: serde_json::Value = serde_json::from_str(&json).expect("invalid JSON");

        assert_eq!(value["event"], "log");
        assert_eq!(value["level"], "info");
        assert_eq!(value["message"], "Application started");
        assert_eq!(value["session_id"], "session-789");
        assert!(value["timestamp"].is_number());
    }

    #[test]
    fn test_error_serialization() {
        let event = HeadlessEvent::error("Connection failed".to_string(), true);
        let json = serde_json::to_string(&event).expect("serialization failed");

        let value: serde_json::Value = serde_json::from_str(&json).expect("invalid JSON");

        assert_eq!(value["event"], "error");
        assert_eq!(value["message"], "Connection failed");
        assert_eq!(value["fatal"], true);
        assert!(value["timestamp"].is_number());
    }

    #[test]
    fn test_device_detected_serialization() {
        let event = HeadlessEvent::device_detected("linux-x64", "Linux Desktop", "linux");
        let json = serde_json::to_string(&event).expect("serialization failed");

        let value: serde_json::Value = serde_json::from_str(&json).expect("invalid JSON");

        assert_eq!(value["event"], "device_detected");
        assert_eq!(value["device_id"], "linux-x64");
        assert_eq!(value["device_name"], "Linux Desktop");
        assert_eq!(value["platform"], "linux");
        assert!(value["timestamp"].is_number());
    }
}
