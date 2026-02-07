//! JSON-RPC protocol handling for Flutter daemon

use serde::{Deserialize, Serialize};

use fdemon_core::ansi::{contains_word, strip_ansi_codes};
use fdemon_core::types::{LogLevel, LogSource};
use fdemon_core::DaemonMessage;

/// Intermediate log entry info produced by DaemonMessage conversion
#[derive(Debug, Clone)]
pub struct LogEntryInfo {
    pub level: LogLevel,
    pub source: LogSource,
    pub message: String,
    pub stack_trace: Option<String>,
}

/// Strip the outer brackets from a daemon message
///
/// The Flutter daemon wraps all messages in `[...]` for resilience.
/// Returns the inner content if brackets are present.
pub(crate) fn strip_brackets(line: &str) -> Option<&str> {
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
pub(crate) enum RawMessage {
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

#[allow(dead_code)]
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

// ─────────────────────────────────────────────────────────
// JSON-RPC Protocol Parsing (Free Functions)
// ─────────────────────────────────────────────────────────

/// Parses a JSON-RPC message from Flutter's --machine stdout.
///
/// This function handles both bracketed lines (e.g., `[{...}]`) and raw JSON strings.
/// The Flutter daemon wraps messages in brackets for resilience, but this function
/// accepts both formats for flexibility.
///
/// # Arguments
/// * `line` - Line from Flutter daemon stdout (may or may not have brackets)
///
/// # Returns
/// * `Some(DaemonMessage)` if valid JSON-RPC
/// * `None` if parsing fails
pub fn parse_daemon_message(line: &str) -> Option<DaemonMessage> {
    // Strip brackets if present, otherwise use line as-is
    let json = strip_brackets(line).unwrap_or(line);

    let raw = RawMessage::parse(json)?;
    match raw {
        RawMessage::Event { event, params } => Some(parse_event(&event, params)),
        RawMessage::Response { id, result, error } => {
            Some(DaemonMessage::Response { id, result, error })
        }
    }
}

/// Parse an event by name and parameters
fn parse_event(event: &str, params: serde_json::Value) -> DaemonMessage {
    match event {
        "daemon.connected" => serde_json::from_value(params.clone())
            .map(DaemonMessage::DaemonConnected)
            .unwrap_or_else(|_| unknown_event(event, params)),
        "daemon.logMessage" => serde_json::from_value(params.clone())
            .map(DaemonMessage::DaemonLogMessage)
            .unwrap_or_else(|_| unknown_event(event, params)),
        "app.start" => serde_json::from_value(params.clone())
            .map(DaemonMessage::AppStart)
            .unwrap_or_else(|_| unknown_event(event, params)),
        "app.started" => serde_json::from_value(params.clone())
            .map(DaemonMessage::AppStarted)
            .unwrap_or_else(|_| unknown_event(event, params)),
        "app.stop" => serde_json::from_value(params.clone())
            .map(DaemonMessage::AppStop)
            .unwrap_or_else(|_| unknown_event(event, params)),
        "app.log" => serde_json::from_value(params.clone())
            .map(DaemonMessage::AppLog)
            .unwrap_or_else(|_| unknown_event(event, params)),
        "app.progress" => serde_json::from_value(params.clone())
            .map(DaemonMessage::AppProgress)
            .unwrap_or_else(|_| unknown_event(event, params)),
        "app.debugPort" => serde_json::from_value(params.clone())
            .map(DaemonMessage::AppDebugPort)
            .unwrap_or_else(|_| unknown_event(event, params)),
        "device.added" => serde_json::from_value(params.clone())
            .map(DaemonMessage::DeviceAdded)
            .unwrap_or_else(|_| unknown_event(event, params)),
        "device.removed" => serde_json::from_value(params.clone())
            .map(DaemonMessage::DeviceRemoved)
            .unwrap_or_else(|_| unknown_event(event, params)),
        _ => unknown_event(event, params),
    }
}

/// Create an unknown event fallback
fn unknown_event(event: &str, params: serde_json::Value) -> DaemonMessage {
    DaemonMessage::UnknownEvent {
        event: event.to_string(),
        params,
    }
}

/// Converts a DaemonMessage to a displayable log entry.
///
/// Not all daemon messages produce log entries (e.g., Response messages).
///
/// # Arguments
/// * `msg` - The daemon message to convert
///
/// # Returns
/// * `Some(LogEntryInfo)` if the message should be logged
/// * `None` if the message should not be logged
pub fn to_log_entry(msg: &DaemonMessage) -> Option<LogEntryInfo> {
    match msg {
        DaemonMessage::AppLog(log) => {
            let (level, message) = parse_flutter_log(&log.log, log.error);
            Some(LogEntryInfo {
                level,
                source: LogSource::Flutter,
                message,
                stack_trace: log.stack_trace.clone(),
            })
        }
        DaemonMessage::DaemonLogMessage(msg) => {
            let level = match msg.level.as_str() {
                "error" => LogLevel::Error,
                "warning" => LogLevel::Warning,
                "status" => LogLevel::Info,
                _ => LogLevel::Debug,
            };
            Some(LogEntryInfo {
                level,
                source: LogSource::Daemon,
                message: msg.message.clone(),
                stack_trace: msg.stack_trace.clone(),
            })
        }
        DaemonMessage::AppProgress(progress) => {
            // Only show progress messages that are meaningful
            if progress.finished {
                progress.message.as_ref().map(|msg| LogEntryInfo {
                    level: LogLevel::Info,
                    source: LogSource::Flutter,
                    message: msg.clone(),
                    stack_trace: None,
                })
            } else {
                // Skip in-progress messages to reduce noise
                None
            }
        }
        DaemonMessage::AppStart(start) => Some(LogEntryInfo {
            level: LogLevel::Info,
            source: LogSource::App,
            message: format!("App starting on {}", start.device_id),
            stack_trace: None,
        }),
        DaemonMessage::AppStarted(_) => Some(LogEntryInfo {
            level: LogLevel::Info,
            source: LogSource::App,
            message: "App started".to_string(),
            stack_trace: None,
        }),
        DaemonMessage::AppStop(stop) => {
            let message = if let Some(err) = &stop.error {
                format!("App stopped with error: {}", err)
            } else {
                "App stopped".to_string()
            };
            Some(LogEntryInfo {
                level: if stop.error.is_some() {
                    LogLevel::Error
                } else {
                    LogLevel::Warning
                },
                source: LogSource::App,
                message,
                stack_trace: None,
            })
        }
        DaemonMessage::AppDebugPort(debug) => Some(LogEntryInfo {
            level: LogLevel::Info,
            source: LogSource::App,
            message: format!("DevTools available at port {}", debug.port),
            stack_trace: None,
        }),
        DaemonMessage::DeviceAdded(device) => Some(LogEntryInfo {
            level: LogLevel::Debug,
            source: LogSource::App,
            message: format!("Device connected: {} ({})", device.name, device.platform),
            stack_trace: None,
        }),
        DaemonMessage::DeviceRemoved(device) => Some(LogEntryInfo {
            level: LogLevel::Debug,
            source: LogSource::App,
            message: format!("Device disconnected: {}", device.name),
            stack_trace: None,
        }),
        DaemonMessage::DaemonConnected(conn) => Some(LogEntryInfo {
            level: LogLevel::Debug,
            source: LogSource::Daemon,
            message: format!("Daemon connected (v{}, pid {})", conn.version, conn.pid),
            stack_trace: None,
        }),
        _ => None, // UnknownEvent, Response handled separately
    }
}

/// Parses a raw Flutter log line, detecting level and stripping prefixes.
///
/// Strips ANSI escape codes and the "flutter: " prefix, then detects
/// the log level based on content patterns.
///
/// # Arguments
/// * `raw` - Raw log line from Flutter
/// * `is_error` - Whether this log was marked as an error
///
/// # Returns
/// * `(LogLevel, String)` - Detected level and cleaned message
pub fn parse_flutter_log(raw: &str, is_error: bool) -> (LogLevel, String) {
    // Strip ANSI escape codes first (from Logger package, etc.)
    let cleaned = strip_ansi_codes(raw);
    let message = cleaned.trim();

    // Check for error indicators
    if is_error {
        return (LogLevel::Error, message.to_string());
    }

    // Check for common patterns - strip "flutter: " prefix
    if let Some(content) = message.strip_prefix("flutter: ") {
        let level = detect_log_level(content);
        return (level, content.to_string());
    }

    // Check for error patterns in content
    if message.contains("Exception:") || message.contains("Error:") || message.starts_with("E/") {
        return (LogLevel::Error, message.to_string());
    }

    // Check for warning patterns
    if message.contains("Warning:") || message.starts_with("W/") {
        return (LogLevel::Warning, message.to_string());
    }

    // Default to info
    (LogLevel::Info, message.to_string())
}

/// Detects the log level from message content using pattern matching.
///
/// Supports standard patterns plus Logger/Talker package formats:
/// - Logger: emoji indicators (🔥⛔⚠️💡🐛) and prefixes (Trace:, Debug:, etc.)
/// - Talker: bracketed prefixes ([verbose], [debug], [info], etc.)
///
/// # Arguments
/// * `message` - The message content to analyze
///
/// # Returns
/// * `LogLevel` - Detected log level (defaults to Info)
pub fn detect_log_level(message: &str) -> LogLevel {
    // ─────────────────────────────────────────────────────────
    // Emoji-based detection (Logger package uses these)
    // Check emojis first - they're unambiguous indicators
    // ─────────────────────────────────────────────────────────

    // Fatal/Critical indicators (check first - highest priority)
    if message.contains('🔥') || message.contains('💀') {
        return LogLevel::Error;
    }

    // Error indicators
    if message.contains('⛔') || message.contains('❌') || message.contains('🚫') {
        return LogLevel::Error;
    }

    // Warning indicators
    if message.contains('⚠') || message.contains('⚡') {
        return LogLevel::Warning;
    }

    // Info indicators
    if message.contains('💡') || message.contains('ℹ') {
        return LogLevel::Info;
    }

    // Debug indicators
    if message.contains('🐛') || message.contains('🔍') {
        return LogLevel::Debug;
    }

    let lower = message.to_lowercase();

    // ─────────────────────────────────────────────────────────
    // Prefix-based detection (Logger/Talker package formats)
    // ─────────────────────────────────────────────────────────

    // Logger package prefixes (with colon)
    if lower.contains("fatal:") || lower.contains("critical:") {
        return LogLevel::Error;
    }
    if lower.contains("error:") || lower.contains("exception:") {
        return LogLevel::Error;
    }
    if lower.contains("warning:") || lower.contains("warn:") {
        return LogLevel::Warning;
    }
    if lower.contains("info:") {
        return LogLevel::Info;
    }
    if lower.contains("debug:") || lower.contains("trace:") {
        return LogLevel::Debug;
    }

    // Talker package format (bracketed)
    if lower.contains("[critical]") || lower.contains("[fatal]") {
        return LogLevel::Error;
    }
    if lower.contains("[error]") || lower.contains("[exception]") {
        return LogLevel::Error;
    }
    if lower.contains("[warning]") || lower.contains("[warn]") {
        return LogLevel::Warning;
    }
    if lower.contains("[info]") {
        return LogLevel::Info;
    }
    if lower.contains("[debug]") || lower.contains("[verbose]") || lower.contains("[trace]") {
        return LogLevel::Debug;
    }

    // ─────────────────────────────────────────────────────────
    // Dart exception type detection
    // Handles CamelCase exception types like RangeError, TypeError, FormatException
    // Pattern: "SomethingError (params):" or "SomethingError: message"
    // ─────────────────────────────────────────────────────────

    // Check for Dart exception patterns (TypeNameError or TypeNameException)
    // These are CamelCase but indicate real errors
    if lower.contains("error (") || lower.contains("exception (") {
        return LogLevel::Error;
    }

    // ─────────────────────────────────────────────────────────
    // Word boundary detection (prevents false positives)
    // Uses word boundaries to avoid matching identifiers like
    // "ErrorTestingPage", "handleError", "errorCount"
    // ─────────────────────────────────────────────────────────

    // Error keywords - must be at word boundaries
    // Include common word variations (crashed, crashing, etc.)
    if contains_word(message, "error")
        || contains_word(message, "exception")
        || contains_word(message, "failed")
        || contains_word(message, "failure")
        || contains_word(message, "fatal")
        || contains_word(message, "crash")
        || contains_word(message, "crashed")
        || contains_word(message, "crashing")
    {
        return LogLevel::Error;
    }

    // Warning keywords - must be at word boundaries
    if contains_word(message, "warning")
        || contains_word(message, "deprecated")
        || contains_word(message, "caution")
    {
        return LogLevel::Warning;
    }

    // Debug keywords
    if lower.starts_with("debug") || contains_word(message, "verbose") {
        return LogLevel::Debug;
    }

    LogLevel::Info
}

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
        let msg = parse_daemon_message(json);
        assert!(matches!(msg, Some(DaemonMessage::DaemonConnected(_))));
        if let Some(DaemonMessage::DaemonConnected(c)) = msg {
            assert_eq!(c.version, "0.6.1");
            assert_eq!(c.pid, 12345);
        }
    }

    #[test]
    fn test_daemon_message_parse_app_log() {
        let json = r#"{"event":"app.log","params":{"appId":"abc123","log":"flutter: Hello World","error":false}}"#;
        let msg = parse_daemon_message(json).unwrap();
        assert!(matches!(msg, DaemonMessage::AppLog(_)));
        if let DaemonMessage::AppLog(log) = msg {
            assert_eq!(log.log, "flutter: Hello World");
            assert!(!log.error);
        }
    }

    #[test]
    fn test_daemon_message_parse_app_log_error() {
        let json = r#"{"event":"app.log","params":{"appId":"abc","log":"Error message","error":true,"stackTrace":"at main.dart:10"}}"#;
        let msg = parse_daemon_message(json).unwrap();
        assert!(msg.is_error());
        if let DaemonMessage::AppLog(log) = msg {
            assert!(log.error);
            assert_eq!(log.stack_trace, Some("at main.dart:10".to_string()));
        }
    }

    #[test]
    fn test_daemon_message_parse_app_progress() {
        let json = r#"{"event":"app.progress","params":{"appId":"abc","id":"1","message":"Compiling...","finished":false}}"#;
        let msg = parse_daemon_message(json).unwrap();
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
        let msg = parse_daemon_message(json).unwrap();
        assert!(matches!(msg, DaemonMessage::AppStart(_)));
        assert_eq!(msg.app_id(), Some("abc123"));
    }

    #[test]
    fn test_daemon_message_parse_app_started() {
        let json = r#"{"event":"app.started","params":{"appId":"abc123"}}"#;
        let msg = parse_daemon_message(json).unwrap();
        assert!(matches!(msg, DaemonMessage::AppStarted(_)));
        assert_eq!(msg.app_id(), Some("abc123"));
    }

    #[test]
    fn test_daemon_message_parse_app_stop() {
        let json = r#"{"event":"app.stop","params":{"appId":"abc123"}}"#;
        let msg = parse_daemon_message(json).unwrap();
        assert!(matches!(msg, DaemonMessage::AppStop(_)));
        assert!(!msg.is_error());
    }

    #[test]
    fn test_daemon_message_parse_app_stop_with_error() {
        let json = r#"{"event":"app.stop","params":{"appId":"abc123","error":"Crashed"}}"#;
        let msg = parse_daemon_message(json).unwrap();
        assert!(msg.is_error());
        if let DaemonMessage::AppStop(stop) = msg {
            assert_eq!(stop.error, Some("Crashed".to_string()));
        }
    }

    #[test]
    fn test_daemon_message_parse_device_added() {
        let json = r#"{"event":"device.added","params":{"id":"emulator-5554","name":"Pixel 4","platform":"android","emulator":true}}"#;
        let msg = parse_daemon_message(json).unwrap();
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
        let msg = parse_daemon_message(json).unwrap();
        assert!(matches!(msg, DaemonMessage::DeviceRemoved(_)));
    }

    #[test]
    fn test_daemon_message_parse_app_debug_port() {
        let json = r#"{"event":"app.debugPort","params":{"appId":"abc","port":8080,"wsUri":"ws://localhost:8080"}}"#;
        let msg = parse_daemon_message(json).unwrap();
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
        let msg = parse_daemon_message(json).unwrap();
        assert!(matches!(msg, DaemonMessage::Response { .. }));
        assert!(!msg.is_error());
    }

    #[test]
    fn test_daemon_message_parse_response_error() {
        let json = r#"{"id":1,"error":"Something failed"}"#;
        let msg = parse_daemon_message(json).unwrap();
        assert!(msg.is_error());
    }

    #[test]
    fn test_daemon_message_unknown_event_fallback() {
        let json = r#"{"event":"some.future.event","params":{"foo":"bar"}}"#;
        let msg = parse_daemon_message(json).unwrap();
        assert!(matches!(msg, DaemonMessage::UnknownEvent { .. }));
        if let DaemonMessage::UnknownEvent { event, .. } = msg {
            assert_eq!(event, "some.future.event");
        }
    }

    #[test]
    fn test_daemon_message_malformed_event_fallback() {
        // app.start missing required fields
        let json = r#"{"event":"app.start","params":{"incomplete":true}}"#;
        let msg = parse_daemon_message(json).unwrap();
        // Should fall back to UnknownEvent, not panic
        assert!(matches!(msg, DaemonMessage::UnknownEvent { .. }));
    }

    #[test]
    fn test_daemon_message_summary() {
        let log_json = r#"{"event":"app.log","params":{"appId":"a","log":"Hello","error":false}}"#;
        let msg = parse_daemon_message(log_json).unwrap();
        assert_eq!(msg.summary(), "Hello");

        let connected_json =
            r#"{"event":"daemon.connected","params":{"version":"1.0.0","pid":123}}"#;
        let msg = parse_daemon_message(connected_json).unwrap();
        assert!(msg.summary().contains("1.0.0"));

        let started_json = r#"{"event":"app.started","params":{"appId":"a"}}"#;
        let msg = parse_daemon_message(started_json).unwrap();
        assert_eq!(msg.summary(), "App started");
    }

    #[test]
    fn test_daemon_message_app_id_helper() {
        // App events should return app_id
        let json = r#"{"event":"app.log","params":{"appId":"test-app","log":"msg","error":false}}"#;
        let msg = parse_daemon_message(json).unwrap();
        assert_eq!(msg.app_id(), Some("test-app"));

        // Non-app events should return None
        let json = r#"{"event":"daemon.connected","params":{"version":"1.0","pid":1}}"#;
        let msg = parse_daemon_message(json).unwrap();
        assert_eq!(msg.app_id(), None);

        // Device events should return None
        let json = r#"{"event":"device.added","params":{"id":"d","name":"n","platform":"p"}}"#;
        let msg = parse_daemon_message(json).unwrap();
        assert_eq!(msg.app_id(), None);
    }

    #[test]
    fn test_daemon_message_invalid_json_returns_none() {
        assert!(parse_daemon_message("not json").is_none());
        assert!(parse_daemon_message("{incomplete").is_none());
    }

    #[test]
    fn test_daemon_message_daemon_log_message() {
        let json =
            r#"{"event":"daemon.logMessage","params":{"level":"warning","message":"Low memory"}}"#;
        let msg = parse_daemon_message(json).unwrap();
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
        let (level, msg) = parse_flutter_log("flutter: Hello World", false);
        assert_eq!(level, fdemon_core::LogLevel::Info);
        assert_eq!(msg, "Hello World");
    }

    #[test]
    fn test_parse_flutter_log_error_flag() {
        let (level, msg) = parse_flutter_log("Some error occurred", true);
        assert_eq!(level, fdemon_core::LogLevel::Error);
        assert_eq!(msg, "Some error occurred");
    }

    #[test]
    fn test_parse_flutter_log_exception_in_message() {
        let (level, _) = parse_flutter_log("flutter: Exception: Something went wrong", false);
        assert_eq!(level, fdemon_core::LogLevel::Error);
    }

    #[test]
    fn test_parse_flutter_log_warning() {
        let (level, _) = parse_flutter_log("flutter: Warning: deprecated API used", false);
        assert_eq!(level, fdemon_core::LogLevel::Warning);
    }

    #[test]
    fn test_detect_log_level_error_patterns() {
        assert_eq!(
            detect_log_level("Error occurred"),
            fdemon_core::LogLevel::Error
        );
        assert_eq!(
            detect_log_level("An exception was thrown"),
            fdemon_core::LogLevel::Error
        );
        assert_eq!(
            detect_log_level("Build failed"),
            fdemon_core::LogLevel::Error
        );
        assert_eq!(
            detect_log_level("Fatal error"),
            fdemon_core::LogLevel::Error
        );
    }

    #[test]
    fn test_detect_log_level_warning_patterns() {
        assert_eq!(
            detect_log_level("Warning: check this"),
            fdemon_core::LogLevel::Warning
        );
        assert_eq!(
            detect_log_level("This is deprecated"),
            fdemon_core::LogLevel::Warning
        );
    }

    #[test]
    fn test_detect_log_level_debug_patterns() {
        assert_eq!(
            detect_log_level("debug: value is 5"),
            fdemon_core::LogLevel::Debug
        );
        assert_eq!(
            detect_log_level("[debug] trace info"),
            fdemon_core::LogLevel::Debug
        );
    }

    #[test]
    fn test_detect_log_level_default() {
        assert_eq!(
            detect_log_level("Normal message"),
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
        let entry = to_log_entry(&msg).unwrap();

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
        let entry = to_log_entry(&msg).unwrap();

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
        assert!(to_log_entry(&msg_ongoing).is_none()); // Skip ongoing

        let progress_finished = AppProgress {
            app_id: "test".to_string(),
            id: "1".to_string(),
            progress_id: None,
            message: Some("Compilation complete".to_string()),
            finished: true,
        };

        let msg_finished = DaemonMessage::AppProgress(progress_finished);
        assert!(to_log_entry(&msg_finished).is_some()); // Show finished
    }

    #[test]
    fn test_app_stop_error_level() {
        use fdemon_core::AppStop;

        let stop_normal = AppStop {
            app_id: "test".to_string(),
            error: None,
        };
        let entry = to_log_entry(&DaemonMessage::AppStop(stop_normal)).unwrap();
        assert_eq!(entry.level, fdemon_core::LogLevel::Warning);

        let stop_error = AppStop {
            app_id: "test".to_string(),
            error: Some("Crash!".to_string()),
        };
        let entry = to_log_entry(&DaemonMessage::AppStop(stop_error)).unwrap();
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
        let entry = to_log_entry(&msg).unwrap();

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
        let entry = to_log_entry(&msg).unwrap();

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
            detect_log_level("Trace: Very detailed info"),
            fdemon_core::LogLevel::Debug
        );
        assert_eq!(
            detect_log_level("│  Trace: message"),
            fdemon_core::LogLevel::Debug
        );
    }

    #[test]
    fn test_logger_debug_emoji() {
        assert_eq!(
            detect_log_level("🐛 Debug: Debugging info"),
            fdemon_core::LogLevel::Debug
        );
        assert_eq!(
            detect_log_level("│ 🐛  Debug: message"),
            fdemon_core::LogLevel::Debug
        );
    }

    #[test]
    fn test_logger_info_emoji() {
        assert_eq!(
            detect_log_level("💡 Info: General info"),
            fdemon_core::LogLevel::Info
        );
        assert_eq!(
            detect_log_level("│ 💡  Info: message"),
            fdemon_core::LogLevel::Info
        );
    }

    #[test]
    fn test_logger_warning_emoji() {
        assert_eq!(
            detect_log_level("⚠️ Warning: Something wrong"),
            fdemon_core::LogLevel::Warning
        );
        assert_eq!(
            detect_log_level("│ ⚠  Warning: message"),
            fdemon_core::LogLevel::Warning
        );
    }

    #[test]
    fn test_logger_error_emoji() {
        assert_eq!(
            detect_log_level("⛔ Error: Something failed"),
            fdemon_core::LogLevel::Error
        );
        assert_eq!(
            detect_log_level("│ ⛔  Error: message"),
            fdemon_core::LogLevel::Error
        );
        assert_eq!(
            detect_log_level("❌ Error: failure"),
            fdemon_core::LogLevel::Error
        );
    }

    #[test]
    fn test_logger_fatal_emoji() {
        assert_eq!(
            detect_log_level("🔥 Fatal: Critical failure"),
            fdemon_core::LogLevel::Error
        );
        assert_eq!(
            detect_log_level("│ 🔥  Fatal: message"),
            fdemon_core::LogLevel::Error
        );
    }

    // ─────────────────────────────────────────────────────────
    // Talker Package Detection Tests (Task 09)
    // ─────────────────────────────────────────────────────────

    #[test]
    fn test_talker_verbose() {
        assert_eq!(
            detect_log_level("[verbose] Detailed message"),
            fdemon_core::LogLevel::Debug
        );
    }

    #[test]
    fn test_talker_debug() {
        assert_eq!(
            detect_log_level("[debug] Debug message"),
            fdemon_core::LogLevel::Debug
        );
    }

    #[test]
    fn test_talker_info() {
        assert_eq!(
            detect_log_level("[info] Info message"),
            fdemon_core::LogLevel::Info
        );
    }

    #[test]
    fn test_talker_warning() {
        assert_eq!(
            detect_log_level("[warning] Warning message"),
            fdemon_core::LogLevel::Warning
        );
        assert_eq!(
            detect_log_level("[warn] Warning message"),
            fdemon_core::LogLevel::Warning
        );
    }

    #[test]
    fn test_talker_error() {
        assert_eq!(
            detect_log_level("[error] Error message"),
            fdemon_core::LogLevel::Error
        );
        assert_eq!(
            detect_log_level("[exception] Exception occurred"),
            fdemon_core::LogLevel::Error
        );
    }

    #[test]
    fn test_talker_critical() {
        assert_eq!(
            detect_log_level("[critical] Critical failure"),
            fdemon_core::LogLevel::Error
        );
        assert_eq!(
            detect_log_level("[fatal] Fatal error"),
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
            detect_log_level("│ 💡  Info: Login successful"),
            fdemon_core::LogLevel::Info
        );
        assert_eq!(
            detect_log_level("│ 🐛  Debug: User data loaded"),
            fdemon_core::LogLevel::Debug
        );
    }

    #[test]
    fn test_case_insensitive_prefixes() {
        assert_eq!(
            detect_log_level("ERROR: something failed"),
            fdemon_core::LogLevel::Error
        );
        assert_eq!(
            detect_log_level("Warning: be careful"),
            fdemon_core::LogLevel::Warning
        );
        assert_eq!(
            detect_log_level("DEBUG: verbose output"),
            fdemon_core::LogLevel::Debug
        );
    }

    #[test]
    fn test_info_colon_prefix() {
        assert_eq!(
            detect_log_level("Info: Application started"),
            fdemon_core::LogLevel::Info
        );
    }

    #[test]
    fn test_crash_keyword() {
        assert_eq!(
            detect_log_level("App crashed unexpectedly"),
            fdemon_core::LogLevel::Error
        );
    }

    #[test]
    fn test_caution_keyword() {
        assert_eq!(
            detect_log_level("Caution: low memory"),
            fdemon_core::LogLevel::Warning
        );
    }
}
