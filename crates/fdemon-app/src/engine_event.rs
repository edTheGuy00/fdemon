//! Domain events emitted by the Engine for external consumers
//!
//! This module defines the `EngineEvent` enum, which is the primary extension
//! point for pro features (MCP server, remote SSH, etc.). Events are broadcast
//! after each message processing cycle via `Engine::subscribe()`.

use crate::session::SessionId;
use fdemon_core::{AppPhase, LogEntry};
use fdemon_daemon::Device;

/// Domain events emitted by the Engine for external consumers.
///
/// This is the primary extension point for pro features. An MCP server
/// or remote SSH client subscribes to these events via `Engine::subscribe()`.
///
/// Events are broadcast after each message processing cycle, so subscribers
/// see a consistent view of state changes.
#[derive(Debug, Clone)]
pub enum EngineEvent {
    // ─────────────────────────────────────────────────────────
    // Session Lifecycle
    // ─────────────────────────────────────────────────────────
    /// A new session was created (device selected, not yet running)
    SessionCreated {
        session_id: SessionId,
        device: Device,
    },

    /// A session's Flutter process has started
    SessionStarted {
        session_id: SessionId,
        device_id: String,
        device_name: String,
        platform: String,
        pid: Option<u32>,
    },

    /// A session has stopped (process exited or was killed)
    SessionStopped {
        session_id: SessionId,
        reason: Option<String>,
    },

    /// A session was removed from the session manager
    SessionRemoved { session_id: SessionId },

    // ─────────────────────────────────────────────────────────
    // App Phase Changes
    // ─────────────────────────────────────────────────────────
    /// The app phase changed for a session
    PhaseChanged {
        session_id: SessionId,
        old_phase: AppPhase,
        new_phase: AppPhase,
    },

    // ─────────────────────────────────────────────────────────
    // Hot Reload / Restart
    // ─────────────────────────────────────────────────────────
    /// Hot reload started for a session
    ReloadStarted { session_id: SessionId },

    /// Hot reload completed successfully
    ReloadCompleted { session_id: SessionId, time_ms: u64 },

    /// Hot reload failed
    ReloadFailed {
        session_id: SessionId,
        reason: String,
    },

    /// Hot restart started for a session
    RestartStarted { session_id: SessionId },

    /// Hot restart completed
    RestartCompleted { session_id: SessionId },

    // ─────────────────────────────────────────────────────────
    // Logging
    // ─────────────────────────────────────────────────────────
    /// A new log entry was added to a session
    LogEntry {
        session_id: SessionId,
        entry: LogEntry,
    },

    /// Batch of log entries (for high-volume logging)
    LogBatch {
        session_id: SessionId,
        entries: Vec<LogEntry>,
    },

    // ─────────────────────────────────────────────────────────
    // Device Discovery
    // ─────────────────────────────────────────────────────────
    /// Connected devices list was updated
    DevicesDiscovered { devices: Vec<Device> },

    // ─────────────────────────────────────────────────────────
    // File Watcher
    // ─────────────────────────────────────────────────────────
    /// Files changed (auto-reload may have been triggered)
    FilesChanged {
        count: usize,
        auto_reload_triggered: bool,
    },

    // ─────────────────────────────────────────────────────────
    // Engine Lifecycle
    // ─────────────────────────────────────────────────────────
    /// Engine is shutting down
    Shutdown,
}

impl EngineEvent {
    /// Returns a short string label for this event type (for logging/debugging).
    pub fn event_type(&self) -> &'static str {
        match self {
            Self::SessionCreated { .. } => "session_created",
            Self::SessionStarted { .. } => "session_started",
            Self::SessionStopped { .. } => "session_stopped",
            Self::SessionRemoved { .. } => "session_removed",
            Self::PhaseChanged { .. } => "phase_changed",
            Self::ReloadStarted { .. } => "reload_started",
            Self::ReloadCompleted { .. } => "reload_completed",
            Self::ReloadFailed { .. } => "reload_failed",
            Self::RestartStarted { .. } => "restart_started",
            Self::RestartCompleted { .. } => "restart_completed",
            Self::LogEntry { .. } => "log_entry",
            Self::LogBatch { .. } => "log_batch",
            Self::DevicesDiscovered { .. } => "devices_discovered",
            Self::FilesChanged { .. } => "files_changed",
            Self::Shutdown => "shutdown",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fdemon_core::LogSource;

    #[test]
    fn test_engine_event_type_labels() {
        let event = EngineEvent::Shutdown;
        assert_eq!(event.event_type(), "shutdown");

        let event = EngineEvent::ReloadStarted { session_id: 1 };
        assert_eq!(event.event_type(), "reload_started");

        let event = EngineEvent::PhaseChanged {
            session_id: 1,
            old_phase: AppPhase::Initializing,
            new_phase: AppPhase::Running,
        };
        assert_eq!(event.event_type(), "phase_changed");

        let event = EngineEvent::FilesChanged {
            count: 5,
            auto_reload_triggered: true,
        };
        assert_eq!(event.event_type(), "files_changed");
    }

    #[test]
    fn test_engine_event_clone() {
        let event = EngineEvent::SessionStarted {
            session_id: 1,
            device_id: "device-1".to_string(),
            device_name: "Pixel 6".to_string(),
            platform: "android".to_string(),
            pid: Some(12345),
        };
        let cloned = event.clone();
        assert_eq!(cloned.event_type(), "session_started");
    }

    #[test]
    fn test_engine_event_all_variants_have_labels() {
        // Ensure all variants have event_type labels
        let events = vec![
            EngineEvent::SessionCreated {
                session_id: 1,
                device: Device {
                    id: "test".to_string(),
                    name: "Test Device".to_string(),
                    platform: "ios".to_string(),
                    emulator: false,
                    category: None,
                    platform_type: None,
                    ephemeral: false,
                    emulator_id: None,
                },
            },
            EngineEvent::SessionStarted {
                session_id: 1,
                device_id: "d1".to_string(),
                device_name: "Device".to_string(),
                platform: "ios".to_string(),
                pid: None,
            },
            EngineEvent::SessionStopped {
                session_id: 1,
                reason: None,
            },
            EngineEvent::SessionRemoved { session_id: 1 },
            EngineEvent::PhaseChanged {
                session_id: 1,
                old_phase: AppPhase::Initializing,
                new_phase: AppPhase::Running,
            },
            EngineEvent::ReloadStarted { session_id: 1 },
            EngineEvent::ReloadCompleted {
                session_id: 1,
                time_ms: 500,
            },
            EngineEvent::ReloadFailed {
                session_id: 1,
                reason: "error".to_string(),
            },
            EngineEvent::RestartStarted { session_id: 1 },
            EngineEvent::RestartCompleted { session_id: 1 },
            EngineEvent::LogEntry {
                session_id: 1,
                entry: LogEntry::info(LogSource::App, "test"),
            },
            EngineEvent::LogBatch {
                session_id: 1,
                entries: vec![],
            },
            EngineEvent::DevicesDiscovered { devices: vec![] },
            EngineEvent::FilesChanged {
                count: 0,
                auto_reload_triggered: false,
            },
            EngineEvent::Shutdown,
        ];

        for event in events {
            let label = event.event_type();
            assert!(!label.is_empty());
            // Labels should be snake_case
            assert_eq!(label, label.to_lowercase());
            assert!(!label.contains(' '));
        }
    }

    #[test]
    fn test_engine_event_debug() {
        // EngineEvent should implement Debug
        let event = EngineEvent::Shutdown;
        let debug_str = format!("{:?}", event);
        assert!(debug_str.contains("Shutdown"));
    }

    #[test]
    fn test_log_entry_variant() {
        // Test LogEntry variant can hold a LogEntry
        let entry = LogEntry::error(LogSource::App, "Test error");
        let event = EngineEvent::LogEntry {
            session_id: 42,
            entry: entry.clone(),
        };

        assert_eq!(event.event_type(), "log_entry");

        // Verify we can clone it
        let cloned = event.clone();
        assert_eq!(cloned.event_type(), "log_entry");
    }

    #[test]
    fn test_log_batch_variant() {
        // Test LogBatch variant can hold multiple LogEntry instances
        let entries = vec![
            LogEntry::info(LogSource::App, "Entry 1"),
            LogEntry::error(LogSource::Flutter, "Entry 2"),
            LogEntry::warn(LogSource::Daemon, "Entry 3"),
        ];

        let event = EngineEvent::LogBatch {
            session_id: 1,
            entries: entries.clone(),
        };

        assert_eq!(event.event_type(), "log_batch");

        // Verify cloning works with batch
        let cloned = event.clone();
        assert_eq!(cloned.event_type(), "log_batch");
    }

    #[test]
    fn test_device_variant() {
        // Test that Device can be cloned in SessionCreated
        let device = Device {
            id: "test-device".to_string(),
            name: "Test Device".to_string(),
            platform: "android".to_string(),
            emulator: true,
            category: Some("mobile".to_string()),
            platform_type: Some("android".to_string()),
            ephemeral: false,
            emulator_id: Some("emulator-5554".to_string()),
        };

        let event = EngineEvent::SessionCreated {
            session_id: 1,
            device: device.clone(),
        };

        assert_eq!(event.event_type(), "session_created");

        // Verify cloning works
        let cloned = event.clone();
        assert_eq!(cloned.event_type(), "session_created");
    }

    #[test]
    fn test_devices_discovered_variant() {
        // Test DevicesDiscovered with multiple devices
        let devices = vec![
            Device {
                id: "d1".to_string(),
                name: "Device 1".to_string(),
                platform: "ios".to_string(),
                emulator: false,
                category: None,
                platform_type: None,
                ephemeral: false,
                emulator_id: None,
            },
            Device {
                id: "d2".to_string(),
                name: "Device 2".to_string(),
                platform: "android".to_string(),
                emulator: true,
                category: None,
                platform_type: None,
                ephemeral: false,
                emulator_id: None,
            },
        ];

        let event = EngineEvent::DevicesDiscovered {
            devices: devices.clone(),
        };

        assert_eq!(event.event_type(), "devices_discovered");

        // Verify cloning works
        let cloned = event.clone();
        assert_eq!(cloned.event_type(), "devices_discovered");
    }
}
