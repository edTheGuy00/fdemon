//! JSON-RPC protocol handling for Flutter daemon

use serde::{Deserialize, Serialize};

// Import DaemonMessage and LogEntryInfo from fdemon_core
// The parsing methods are now implemented in fdemon_core to avoid orphan rule issues
pub use fdemon_core::{DaemonMessage, LogEntryInfo};

/// Strip the outer brackets from a daemon message
///
/// The Flutter daemon wraps all messages in `[...]` for resilience.
/// Returns the inner content if brackets are present.
pub fn strip_brackets(line: &str) -> Option<&str> {
    let trimmed = line.trim();
    if trimmed.starts_with('[') && trimmed.ends_with(']') {
        Some(&trimmed[1..trimmed.len() - 1])
    } else {
        None
    }
}

/// A raw daemon message (before parsing into typed events)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RawMessage {
    /// A response to a request we sent
    Response {
        id: serde_json::Value,
        #[serde(skip_serializing_if = "Option::is_none")]
        result: Option<serde_json::Value>,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<serde_json::Value>,
    },
    /// An event from the daemon (unsolicited)
    Event {
        event: String,
        params: serde_json::Value,
    },
}

impl RawMessage {
    /// Parse a JSON string into a RawMessage
    pub fn parse(json: &str) -> Option<Self> {
        serde_json::from_str(json).ok()
    }

    /// Check if this is an event
    pub fn is_event(&self) -> bool {
        matches!(self, RawMessage::Event { .. })
    }

    /// Get the event name if this is an event
    pub fn event_name(&self) -> Option<&str> {
        match self {
            RawMessage::Event { event, .. } => Some(event),
            _ => None,
        }
    }

    /// Get a human-readable summary of this message
    pub fn summary(&self) -> String {
        match self {
            RawMessage::Response { id, error, .. } => {
                if error.is_some() {
                    format!("Response #{}: error", id)
                } else {
                    format!("Response #{}: ok", id)
                }
            }
            RawMessage::Event { event, .. } => {
                format!("Event: {}", event)
            }
        }
    }
}

// DaemonMessage enum and all methods moved to fdemon-core/events.rs
// This avoids orphan rule violations

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_brackets_valid() {
        assert_eq!(
            strip_brackets(r#"[{"event":"test"}]"#),
            Some(r#"{"event":"test"}"#)
        );
    }

    #[test]
    fn test_strip_brackets_whitespace() {
        assert_eq!(strip_brackets("  [content]  "), Some("content"));
    }

    #[test]
    fn test_strip_brackets_invalid() {
        assert_eq!(strip_brackets("no brackets"), None);
        assert_eq!(strip_brackets("[missing end"), None);
        assert_eq!(strip_brackets("missing start]"), None);
    }

    #[test]
    fn test_parse_event() {
        let json = r#"{"event":"app.log","params":{"message":"hello"}}"#;
        let msg = RawMessage::parse(json).unwrap();
        assert!(msg.is_event());
        assert_eq!(msg.event_name(), Some("app.log"));
    }

    #[test]
    fn test_parse_response() {
        let json = r#"{"id":1,"result":"0.1.0"}"#;
        let msg = RawMessage::parse(json).unwrap();
        assert!(!msg.is_event());
    }

    #[test]
    fn test_parse_invalid_json() {
        assert!(RawMessage::parse("not json").is_none());
    }

    #[test]
    fn test_message_summary() {
        let event = RawMessage::parse(r#"{"event":"app.log","params":{}}"#).unwrap();
        assert_eq!(event.summary(), "Event: app.log");

        let response = RawMessage::parse(r#"{"id":1,"result":"ok"}"#).unwrap();
        assert_eq!(response.summary(), "Response #1: ok");

        let error_resp = RawMessage::parse(r#"{"id":2,"error":"failed"}"#).unwrap();
        assert_eq!(error_resp.summary(), "Response #2: error");
    }

    // DaemonMessage tests

    #[test]
    fn test_daemon_message_parse_daemon_connected() {
        let json = r#"{"event":"daemon.connected","params":{"version":"0.6.1","pid":12345}}"#;
        let msg = DaemonMessage::parse(json);
        assert!(matches!(msg, Some(DaemonMessage::DaemonConnected(_))));
        if let Some(DaemonMessage::DaemonConnected(c)) = msg {
            assert_eq!(c.version, "0.6.1");
            assert_eq!(c.pid, 12345);
        }
    }

    #[test]
    fn test_daemon_message_parse_app_log() {
        let json = r#"{"event":"app.log","params":{"appId":"abc123","log":"flutter: Hello World","error":false}}"#;
        let msg = DaemonMessage::parse(json).unwrap();
        assert!(matches!(msg, DaemonMessage::AppLog(_)));
        if let DaemonMessage::AppLog(log) = msg {
            assert_eq!(log.log, "flutter: Hello World");
            assert!(!log.error);
        }
    }

    #[test]
    fn test_daemon_message_parse_app_log_error() {
        let json = r#"{"event":"app.log","params":{"appId":"abc","log":"Error message","error":true,"stackTrace":"at main.dart:10"}}"#;
        let msg = DaemonMessage::parse(json).unwrap();
        assert!(msg.is_error());
        if let DaemonMessage::AppLog(log) = msg {
            assert!(log.error);
            assert_eq!(log.stack_trace, Some("at main.dart:10".to_string()));
        }
    }

    #[test]
    fn test_daemon_message_parse_app_progress() {
        let json = r#"{"event":"app.progress","params":{"appId":"abc","id":"1","message":"Compiling...","finished":false}}"#;
        let msg = DaemonMessage::parse(json).unwrap();
        if let DaemonMessage::AppProgress(p) = msg {
            assert_eq!(p.message, Some("Compiling...".to_string()));
            assert!(!p.finished);
        } else {
            panic!("Expected AppProgress");
        }
    }

    #[test]
    fn test_daemon_message_parse_app_start() {
        let json = r#"{"event":"app.start","params":{"appId":"abc123","deviceId":"iphone","directory":"/path/to/app","supportsRestart":true}}"#;
        let msg = DaemonMessage::parse(json).unwrap();
        assert!(matches!(msg, DaemonMessage::AppStart(_)));
        assert_eq!(msg.app_id(), Some("abc123"));
    }

    #[test]
    fn test_daemon_message_parse_app_started() {
        let json = r#"{"event":"app.started","params":{"appId":"abc123"}}"#;
        let msg = DaemonMessage::parse(json).unwrap();
        assert!(matches!(msg, DaemonMessage::AppStarted(_)));
        assert_eq!(msg.app_id(), Some("abc123"));
    }

    #[test]
    fn test_daemon_message_parse_app_stop() {
        let json = r#"{"event":"app.stop","params":{"appId":"abc123"}}"#;
        let msg = DaemonMessage::parse(json).unwrap();
        assert!(matches!(msg, DaemonMessage::AppStop(_)));
        assert!(!msg.is_error());
    }

    #[test]
    fn test_daemon_message_parse_app_stop_with_error() {
        let json = r#"{"event":"app.stop","params":{"appId":"abc123","error":"Crashed"}}"#;
        let msg = DaemonMessage::parse(json).unwrap();
        assert!(msg.is_error());
        if let DaemonMessage::AppStop(stop) = msg {
            assert_eq!(stop.error, Some("Crashed".to_string()));
        }
    }

    #[test]
    fn test_daemon_message_parse_device_added() {
        let json = r#"{"event":"device.added","params":{"id":"emulator-5554","name":"Pixel 4","platform":"android","emulator":true}}"#;
        let msg = DaemonMessage::parse(json).unwrap();
        if let DaemonMessage::DeviceAdded(d) = msg {
            assert_eq!(d.name, "Pixel 4");
            assert!(d.emulator);
            assert_eq!(d.platform, "android");
        } else {
            panic!("Expected DeviceAdded");
        }
    }

    #[test]
    fn test_daemon_message_parse_device_removed() {
        let json = r#"{"event":"device.removed","params":{"id":"emulator-5554","name":"Pixel 4","platform":"android"}}"#;
        let msg = DaemonMessage::parse(json).unwrap();
        assert!(matches!(msg, DaemonMessage::DeviceRemoved(_)));
    }

    #[test]
    fn test_daemon_message_parse_app_debug_port() {
        let json = r#"{"event":"app.debugPort","params":{"appId":"abc","port":8080,"wsUri":"ws://localhost:8080"}}"#;
        let msg = DaemonMessage::parse(json).unwrap();
        if let DaemonMessage::AppDebugPort(d) = msg {
            assert_eq!(d.port, 8080);
            assert_eq!(d.ws_uri, "ws://localhost:8080");
        } else {
            panic!("Expected AppDebugPort");
        }
    }

    #[test]
    fn test_daemon_message_parse_response_success() {
        let json = r#"{"id":1,"result":{"code":0}}"#;
        let msg = DaemonMessage::parse(json).unwrap();
        assert!(matches!(msg, DaemonMessage::Response { .. }));
        assert!(!msg.is_error());
    }

    #[test]
    fn test_daemon_message_parse_response_error() {
        let json = r#"{"id":1,"error":"Something failed"}"#;
        let msg = DaemonMessage::parse(json).unwrap();
        assert!(msg.is_error());
    }

    #[test]
    fn test_daemon_message_unknown_event_fallback() {
        let json = r#"{"event":"some.future.event","params":{"foo":"bar"}}"#;
        let msg = DaemonMessage::parse(json).unwrap();
        assert!(matches!(msg, DaemonMessage::UnknownEvent { .. }));
        if let DaemonMessage::UnknownEvent { event, .. } = msg {
            assert_eq!(event, "some.future.event");
        }
    }

    #[test]
    fn test_daemon_message_malformed_event_fallback() {
        // app.start missing required fields
        let json = r#"{"event":"app.start","params":{"incomplete":true}}"#;
        let msg = DaemonMessage::parse(json).unwrap();
        // Should fall back to UnknownEvent, not panic
        assert!(matches!(msg, DaemonMessage::UnknownEvent { .. }));
    }

    #[test]
    fn test_daemon_message_summary() {
        let log_json = r#"{"event":"app.log","params":{"appId":"a","log":"Hello","error":false}}"#;
        let msg = DaemonMessage::parse(log_json).unwrap();
        assert_eq!(msg.summary(), "Hello");

        let connected_json =
            r#"{"event":"daemon.connected","params":{"version":"1.0.0","pid":123}}"#;
        let msg = DaemonMessage::parse(connected_json).unwrap();
        assert!(msg.summary().contains("1.0.0"));

        let started_json = r#"{"event":"app.started","params":{"appId":"a"}}"#;
        let msg = DaemonMessage::parse(started_json).unwrap();
        assert_eq!(msg.summary(), "App started");
    }

    #[test]
    fn test_daemon_message_app_id_helper() {
        // App events should return app_id
        let json = r#"{"event":"app.log","params":{"appId":"test-app","log":"msg","error":false}}"#;
        let msg = DaemonMessage::parse(json).unwrap();
        assert_eq!(msg.app_id(), Some("test-app"));

        // Non-app events should return None
        let json = r#"{"event":"daemon.connected","params":{"version":"1.0","pid":1}}"#;
        let msg = DaemonMessage::parse(json).unwrap();
        assert_eq!(msg.app_id(), None);

        // Device events should return None
        let json = r#"{"event":"device.added","params":{"id":"d","name":"n","platform":"p"}}"#;
        let msg = DaemonMessage::parse(json).unwrap();
        assert_eq!(msg.app_id(), None);
    }

    #[test]
    fn test_daemon_message_invalid_json_returns_none() {
        assert!(DaemonMessage::parse("not json").is_none());
        assert!(DaemonMessage::parse("{incomplete").is_none());
    }

    #[test]
    fn test_daemon_message_daemon_log_message() {
        let json =
            r#"{"event":"daemon.logMessage","params":{"level":"warning","message":"Low memory"}}"#;
        let msg = DaemonMessage::parse(json).unwrap();
        if let DaemonMessage::DaemonLogMessage(m) = msg {
            assert_eq!(m.level, "warning");
            assert_eq!(m.message, "Low memory");
        } else {
            panic!("Expected DaemonLogMessage");
        }
    }

    // ─────────────────────────────────────────────────────────
    // Enhanced Logging Tests (Task 07)
    // ─────────────────────────────────────────────────────────

    #[test]
    fn test_parse_flutter_log_basic() {
        let (level, msg) = DaemonMessage::parse_flutter_log("flutter: Hello World", false);
        assert_eq!(level, fdemon_core::LogLevel::Info);
        assert_eq!(msg, "Hello World");
    }

    #[test]
    fn test_parse_flutter_log_error_flag() {
        let (level, msg) = DaemonMessage::parse_flutter_log("Some error occurred", true);
        assert_eq!(level, fdemon_core::LogLevel::Error);
        assert_eq!(msg, "Some error occurred");
    }

    #[test]
    fn test_parse_flutter_log_exception_in_message() {
        let (level, _) =
            DaemonMessage::parse_flutter_log("flutter: Exception: Something went wrong", false);
        assert_eq!(level, fdemon_core::LogLevel::Error);
    }

    #[test]
    fn test_parse_flutter_log_warning() {
        let (level, _) =
            DaemonMessage::parse_flutter_log("flutter: Warning: deprecated API used", false);
        assert_eq!(level, fdemon_core::LogLevel::Warning);
    }

    #[test]
    fn test_detect_log_level_error_patterns() {
        assert_eq!(
            DaemonMessage::detect_log_level("Error occurred"),
            fdemon_core::LogLevel::Error
        );
        assert_eq!(
            DaemonMessage::detect_log_level("An exception was thrown"),
            fdemon_core::LogLevel::Error
        );
        assert_eq!(
            DaemonMessage::detect_log_level("Build failed"),
            fdemon_core::LogLevel::Error
        );
        assert_eq!(
            DaemonMessage::detect_log_level("Fatal error"),
            fdemon_core::LogLevel::Error
        );
    }

    #[test]
    fn test_detect_log_level_warning_patterns() {
        assert_eq!(
            DaemonMessage::detect_log_level("Warning: check this"),
            fdemon_core::LogLevel::Warning
        );
        assert_eq!(
            DaemonMessage::detect_log_level("This is deprecated"),
            fdemon_core::LogLevel::Warning
        );
    }

    #[test]
    fn test_detect_log_level_debug_patterns() {
        assert_eq!(
            DaemonMessage::detect_log_level("debug: value is 5"),
            fdemon_core::LogLevel::Debug
        );
        assert_eq!(
            DaemonMessage::detect_log_level("[debug] trace info"),
            fdemon_core::LogLevel::Debug
        );
    }

    #[test]
    fn test_detect_log_level_default() {
        assert_eq!(
            DaemonMessage::detect_log_level("Normal message"),
            fdemon_core::LogLevel::Info
        );
    }

    #[test]
    fn test_app_log_to_log_entry() {
        use fdemon_core::AppLog;

        let app_log = AppLog {
            app_id: "test".to_string(),
            log: "flutter: Hello from app".to_string(),
            error: false,
            stack_trace: None,
        };

        let msg = DaemonMessage::AppLog(app_log);
        let entry = msg.to_log_entry().unwrap();

        assert_eq!(entry.level, fdemon_core::LogLevel::Info);
        assert_eq!(entry.message, "Hello from app");
        assert!(matches!(entry.source, fdemon_core::LogSource::Flutter));
    }

    #[test]
    fn test_daemon_log_message_to_log_entry() {
        use fdemon_core::DaemonLogMessage;

        let daemon_msg = DaemonLogMessage {
            level: "error".to_string(),
            message: "Something went wrong".to_string(),
            stack_trace: None,
        };

        let msg = DaemonMessage::DaemonLogMessage(daemon_msg);
        let entry = msg.to_log_entry().unwrap();

        assert_eq!(entry.level, fdemon_core::LogLevel::Error);
        assert_eq!(entry.message, "Something went wrong");
    }

    #[test]
    fn test_app_progress_finished_only() {
        use fdemon_core::AppProgress;

        let progress_ongoing = AppProgress {
            app_id: "test".to_string(),
            id: "1".to_string(),
            progress_id: None,
            message: Some("Compiling...".to_string()),
            finished: false,
        };

        let msg_ongoing = DaemonMessage::AppProgress(progress_ongoing);
        assert!(msg_ongoing.to_log_entry().is_none()); // Skip ongoing

        let progress_finished = AppProgress {
            app_id: "test".to_string(),
            id: "1".to_string(),
            progress_id: None,
            message: Some("Compilation complete".to_string()),
            finished: true,
        };

        let msg_finished = DaemonMessage::AppProgress(progress_finished);
        assert!(msg_finished.to_log_entry().is_some()); // Show finished
    }

    #[test]
    fn test_app_stop_error_level() {
        use fdemon_core::AppStop;

        let stop_normal = AppStop {
            app_id: "test".to_string(),
            error: None,
        };
        let entry = DaemonMessage::AppStop(stop_normal).to_log_entry().unwrap();
        assert_eq!(entry.level, fdemon_core::LogLevel::Warning);

        let stop_error = AppStop {
            app_id: "test".to_string(),
            error: Some("Crash!".to_string()),
        };
        let entry = DaemonMessage::AppStop(stop_error).to_log_entry().unwrap();
        assert_eq!(entry.level, fdemon_core::LogLevel::Error);
    }

    #[test]
    fn test_app_log_strips_flutter_prefix() {
        use fdemon_core::AppLog;

        let app_log = AppLog {
            app_id: "test".to_string(),
            log: "flutter: Button pressed".to_string(),
            error: false,
            stack_trace: None,
        };

        let msg = DaemonMessage::AppLog(app_log);
        let entry = msg.to_log_entry().unwrap();

        // Should strip "flutter: " prefix
        assert_eq!(entry.message, "Button pressed");
    }

    #[test]
    fn test_app_log_with_stack_trace() {
        use fdemon_core::AppLog;

        let app_log = AppLog {
            app_id: "test".to_string(),
            log: "Exception: Null check failed".to_string(),
            error: true,
            stack_trace: Some("at main.dart:42\nat widget.dart:100".to_string()),
        };

        let msg = DaemonMessage::AppLog(app_log);
        let entry = msg.to_log_entry().unwrap();

        assert_eq!(entry.level, fdemon_core::LogLevel::Error);
        assert!(entry.stack_trace.is_some());
        assert!(entry.stack_trace.as_ref().unwrap().contains("main.dart:42"));
    }

    // ─────────────────────────────────────────────────────────
    // Logger Package Detection Tests (Task 09)
    // ─────────────────────────────────────────────────────────

    #[test]
    fn test_logger_trace_prefix() {
        assert_eq!(
            DaemonMessage::detect_log_level("Trace: Very detailed info"),
            fdemon_core::LogLevel::Debug
        );
        assert_eq!(
            DaemonMessage::detect_log_level("│  Trace: message"),
            fdemon_core::LogLevel::Debug
        );
    }

    #[test]
    fn test_logger_debug_emoji() {
        assert_eq!(
            DaemonMessage::detect_log_level("🐛 Debug: Debugging info"),
            fdemon_core::LogLevel::Debug
        );
        assert_eq!(
            DaemonMessage::detect_log_level("│ 🐛  Debug: message"),
            fdemon_core::LogLevel::Debug
        );
    }

    #[test]
    fn test_logger_info_emoji() {
        assert_eq!(
            DaemonMessage::detect_log_level("💡 Info: General info"),
            fdemon_core::LogLevel::Info
        );
        assert_eq!(
            DaemonMessage::detect_log_level("│ 💡  Info: message"),
            fdemon_core::LogLevel::Info
        );
    }

    #[test]
    fn test_logger_warning_emoji() {
        assert_eq!(
            DaemonMessage::detect_log_level("⚠️ Warning: Something wrong"),
            fdemon_core::LogLevel::Warning
        );
        assert_eq!(
            DaemonMessage::detect_log_level("│ ⚠  Warning: message"),
            fdemon_core::LogLevel::Warning
        );
    }

    #[test]
    fn test_logger_error_emoji() {
        assert_eq!(
            DaemonMessage::detect_log_level("⛔ Error: Something failed"),
            fdemon_core::LogLevel::Error
        );
        assert_eq!(
            DaemonMessage::detect_log_level("│ ⛔  Error: message"),
            fdemon_core::LogLevel::Error
        );
        assert_eq!(
            DaemonMessage::detect_log_level("❌ Error: failure"),
            fdemon_core::LogLevel::Error
        );
    }

    #[test]
    fn test_logger_fatal_emoji() {
        assert_eq!(
            DaemonMessage::detect_log_level("🔥 Fatal: Critical failure"),
            fdemon_core::LogLevel::Error
        );
        assert_eq!(
            DaemonMessage::detect_log_level("│ 🔥  Fatal: message"),
            fdemon_core::LogLevel::Error
        );
    }

    // ─────────────────────────────────────────────────────────
    // Talker Package Detection Tests (Task 09)
    // ─────────────────────────────────────────────────────────

    #[test]
    fn test_talker_verbose() {
        assert_eq!(
            DaemonMessage::detect_log_level("[verbose] Detailed message"),
            fdemon_core::LogLevel::Debug
        );
    }

    #[test]
    fn test_talker_debug() {
        assert_eq!(
            DaemonMessage::detect_log_level("[debug] Debug message"),
            fdemon_core::LogLevel::Debug
        );
    }

    #[test]
    fn test_talker_info() {
        assert_eq!(
            DaemonMessage::detect_log_level("[info] Info message"),
            fdemon_core::LogLevel::Info
        );
    }

    #[test]
    fn test_talker_warning() {
        assert_eq!(
            DaemonMessage::detect_log_level("[warning] Warning message"),
            fdemon_core::LogLevel::Warning
        );
        assert_eq!(
            DaemonMessage::detect_log_level("[warn] Warning message"),
            fdemon_core::LogLevel::Warning
        );
    }

    #[test]
    fn test_talker_error() {
        assert_eq!(
            DaemonMessage::detect_log_level("[error] Error message"),
            fdemon_core::LogLevel::Error
        );
        assert_eq!(
            DaemonMessage::detect_log_level("[exception] Exception occurred"),
            fdemon_core::LogLevel::Error
        );
    }

    #[test]
    fn test_talker_critical() {
        assert_eq!(
            DaemonMessage::detect_log_level("[critical] Critical failure"),
            fdemon_core::LogLevel::Error
        );
        assert_eq!(
            DaemonMessage::detect_log_level("[fatal] Fatal error"),
            fdemon_core::LogLevel::Error
        );
    }

    // ─────────────────────────────────────────────────────────
    // Edge Cases (Task 09)
    // ─────────────────────────────────────────────────────────

    #[test]
    fn test_box_drawing_with_level() {
        // Logger package wraps messages in boxes
        assert_eq!(
            DaemonMessage::detect_log_level("│ 💡  Info: Login successful"),
            fdemon_core::LogLevel::Info
        );
        assert_eq!(
            DaemonMessage::detect_log_level("│ 🐛  Debug: User data loaded"),
            fdemon_core::LogLevel::Debug
        );
    }

    #[test]
    fn test_case_insensitive_prefixes() {
        assert_eq!(
            DaemonMessage::detect_log_level("ERROR: something failed"),
            fdemon_core::LogLevel::Error
        );
        assert_eq!(
            DaemonMessage::detect_log_level("Warning: be careful"),
            fdemon_core::LogLevel::Warning
        );
        assert_eq!(
            DaemonMessage::detect_log_level("DEBUG: verbose output"),
            fdemon_core::LogLevel::Debug
        );
    }

    #[test]
    fn test_info_colon_prefix() {
        assert_eq!(
            DaemonMessage::detect_log_level("Info: Application started"),
            fdemon_core::LogLevel::Info
        );
    }

    #[test]
    fn test_crash_keyword() {
        assert_eq!(
            DaemonMessage::detect_log_level("App crashed unexpectedly"),
            fdemon_core::LogLevel::Error
        );
    }

    #[test]
    fn test_caution_keyword() {
        assert_eq!(
            DaemonMessage::detect_log_level("Caution: low memory"),
            fdemon_core::LogLevel::Warning
        );
    }
}
