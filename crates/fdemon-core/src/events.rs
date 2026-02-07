//! Domain event definitions

use serde::{Deserialize, Serialize};

// ─────────────────────────────────────────────────────────
// Event Structs (moved from daemon/events.rs)
// ─────────────────────────────────────────────────────────

/// Connected event - sent when daemon is ready
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DaemonConnected {
    pub version: String,
    pub pid: u32,
}

/// Log message from the daemon itself (not the app)
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DaemonLogMessage {
    pub level: String,
    pub message: String,
    #[serde(default)]
    pub stack_trace: Option<String>,
}

/// App start event - when app begins launching
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppStart {
    pub app_id: String,
    pub device_id: String,
    pub directory: String,
    #[serde(default)]
    pub launch_mode: Option<String>,
    pub supports_restart: bool,
}

/// App started event - when app is fully running
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppStarted {
    pub app_id: String,
}

/// App log event - Flutter print() and debug output
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppLog {
    pub app_id: String,
    pub log: String,
    #[serde(default)]
    pub error: bool,
    #[serde(default)]
    pub stack_trace: Option<String>,
}

/// Progress notification during build/reload
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppProgress {
    pub app_id: String,
    pub id: String,
    #[serde(default)]
    pub progress_id: Option<String>,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub finished: bool,
}

/// App stop event
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppStop {
    pub app_id: String,
    #[serde(default)]
    pub error: Option<String>,
}

/// Debug port information for DevTools
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppDebugPort {
    pub app_id: String,
    pub port: u16,
    pub ws_uri: String,
}

/// Device added/removed event
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceInfo {
    pub id: String,
    pub name: String,
    pub platform: String,
    #[serde(default)]
    pub emulator: bool,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub platform_type: Option<String>,
    #[serde(default)]
    pub ephemeral: bool,
}

// ─────────────────────────────────────────────────────────
// DaemonMessage Enum (moved from daemon/protocol.rs)
// ─────────────────────────────────────────────────────────

/// Fully typed daemon message
#[derive(Debug, Clone)]
pub enum DaemonMessage {
    // Connection
    DaemonConnected(DaemonConnected),
    DaemonLogMessage(DaemonLogMessage),

    // App lifecycle
    AppStart(AppStart),
    AppStarted(AppStarted),
    AppStop(AppStop),
    AppLog(AppLog),
    AppProgress(AppProgress),
    AppDebugPort(AppDebugPort),

    // Devices
    DeviceAdded(DeviceInfo),
    DeviceRemoved(DeviceInfo),

    // Responses
    Response {
        id: serde_json::Value,
        result: Option<serde_json::Value>,
        error: Option<serde_json::Value>,
    },

    // Fallback for unknown events
    UnknownEvent {
        event: String,
        params: serde_json::Value,
    },
}

// ─────────────────────────────────────────────────────────
// Pure Methods (moved from daemon/protocol.rs)
// ─────────────────────────────────────────────────────────

impl DaemonMessage {
    /// Get the app ID if this message relates to an app
    pub fn app_id(&self) -> Option<&str> {
        match self {
            DaemonMessage::AppStart(e) => Some(&e.app_id),
            DaemonMessage::AppStarted(e) => Some(&e.app_id),
            DaemonMessage::AppStop(e) => Some(&e.app_id),
            DaemonMessage::AppLog(e) => Some(&e.app_id),
            DaemonMessage::AppProgress(e) => Some(&e.app_id),
            DaemonMessage::AppDebugPort(e) => Some(&e.app_id),
            _ => None,
        }
    }

    /// Check if this is an error message
    pub fn is_error(&self) -> bool {
        match self {
            DaemonMessage::AppLog(log) => log.error,
            DaemonMessage::AppStop(stop) => stop.error.is_some(),
            DaemonMessage::Response { error, .. } => error.is_some(),
            _ => false,
        }
    }

    /// Get a human-readable summary
    pub fn summary(&self) -> String {
        match self {
            DaemonMessage::DaemonConnected(c) => {
                format!("Daemon connected (v{})", c.version)
            }
            DaemonMessage::DaemonLogMessage(m) => {
                format!("[{}] {}", m.level, m.message)
            }
            DaemonMessage::AppStart(s) => {
                format!("App starting on {}", s.device_id)
            }
            DaemonMessage::AppStarted(_) => "App started".to_string(),
            DaemonMessage::AppStop(s) => {
                if let Some(err) = &s.error {
                    format!("App stopped: {}", err)
                } else {
                    "App stopped".to_string()
                }
            }
            DaemonMessage::AppLog(log) => log.log.clone(),
            DaemonMessage::AppProgress(p) => p
                .message
                .clone()
                .unwrap_or_else(|| "Progress...".to_string()),
            DaemonMessage::AppDebugPort(d) => {
                format!("DevTools at port {}", d.port)
            }
            DaemonMessage::DeviceAdded(d) => {
                format!("Device added: {} ({})", d.name, d.platform)
            }
            DaemonMessage::DeviceRemoved(d) => {
                format!("Device removed: {}", d.name)
            }
            DaemonMessage::Response { id, error, .. } => {
                if error.is_some() {
                    format!("Response #{}: error", id)
                } else {
                    format!("Response #{}: ok", id)
                }
            }
            DaemonMessage::UnknownEvent { event, .. } => {
                format!("Event: {}", event)
            }
        }
    }
}

// ─────────────────────────────────────────────────────────
// Parsing and Conversion Methods (moved from daemon/protocol.rs)
// ─────────────────────────────────────────────────────────

use crate::ansi::{contains_word, strip_ansi_codes};
use crate::types::{LogLevel, LogSource};

/// Intermediate log entry info from parsed daemon message
#[derive(Debug, Clone)]
pub struct LogEntryInfo {
    pub level: LogLevel,
    pub source: LogSource,
    pub message: String,
    pub stack_trace: Option<String>,
}

impl DaemonMessage {
    /// Parse a JSON string into a typed DaemonMessage
    pub fn parse(json: &str) -> Option<Self> {
        // RawMessage is defined in daemon/protocol.rs, so we need to import it
        // For now, we'll parse directly using serde_json
        let value: serde_json::Value = serde_json::from_str(json).ok()?;

        // Check if it's an event or response
        if let Some(event) = value.get("event").and_then(|v| v.as_str()) {
            let params = value
                .get("params")
                .cloned()
                .unwrap_or(serde_json::Value::Null);
            Some(Self::parse_event(event, params))
        } else if value.get("id").is_some() {
            // Response
            let id = value.get("id").cloned().unwrap_or(serde_json::Value::Null);
            let result = value.get("result").cloned();
            let error = value.get("error").cloned();
            Some(DaemonMessage::Response { id, result, error })
        } else {
            None
        }
    }

    /// Parse an event by name
    fn parse_event(event: &str, params: serde_json::Value) -> Self {
        match event {
            "daemon.connected" => serde_json::from_value(params.clone())
                .map(DaemonMessage::DaemonConnected)
                .unwrap_or_else(|_| Self::unknown(event, params)),
            "daemon.logMessage" => serde_json::from_value(params.clone())
                .map(DaemonMessage::DaemonLogMessage)
                .unwrap_or_else(|_| Self::unknown(event, params)),
            "app.start" => serde_json::from_value(params.clone())
                .map(DaemonMessage::AppStart)
                .unwrap_or_else(|_| Self::unknown(event, params)),
            "app.started" => serde_json::from_value(params.clone())
                .map(DaemonMessage::AppStarted)
                .unwrap_or_else(|_| Self::unknown(event, params)),
            "app.stop" => serde_json::from_value(params.clone())
                .map(DaemonMessage::AppStop)
                .unwrap_or_else(|_| Self::unknown(event, params)),
            "app.log" => serde_json::from_value(params.clone())
                .map(DaemonMessage::AppLog)
                .unwrap_or_else(|_| Self::unknown(event, params)),
            "app.progress" => serde_json::from_value(params.clone())
                .map(DaemonMessage::AppProgress)
                .unwrap_or_else(|_| Self::unknown(event, params)),
            "app.debugPort" => serde_json::from_value(params.clone())
                .map(DaemonMessage::AppDebugPort)
                .unwrap_or_else(|_| Self::unknown(event, params)),
            "device.added" => serde_json::from_value(params.clone())
                .map(DaemonMessage::DeviceAdded)
                .unwrap_or_else(|_| Self::unknown(event, params)),
            "device.removed" => serde_json::from_value(params.clone())
                .map(DaemonMessage::DeviceRemoved)
                .unwrap_or_else(|_| Self::unknown(event, params)),
            _ => Self::unknown(event, params),
        }
    }

    fn unknown(event: &str, params: serde_json::Value) -> Self {
        DaemonMessage::UnknownEvent {
            event: event.to_string(),
            params,
        }
    }

    /// Extract a clean log message for display
    pub fn to_log_entry(&self) -> Option<LogEntryInfo> {
        match self {
            DaemonMessage::AppLog(log) => {
                let (level, message) = Self::parse_flutter_log(&log.log, log.error);
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

    /// Parse a flutter log message to extract level and clean message
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
            let level = Self::detect_log_level(content);
            return (level, content.to_string());
        }

        // Check for error patterns in content
        if message.contains("Exception:") || message.contains("Error:") || message.starts_with("E/")
        {
            return (LogLevel::Error, message.to_string());
        }

        // Check for warning patterns
        if message.contains("Warning:") || message.starts_with("W/") {
            return (LogLevel::Warning, message.to_string());
        }

        // Default to info
        (LogLevel::Info, message.to_string())
    }

    /// Detect log level from message content
    ///
    /// Supports standard patterns plus Logger/Talker package formats:
    /// - Logger: emoji indicators (🔥⛔⚠️💡🐛) and prefixes (Trace:, Debug:, etc.)
    /// - Talker: bracketed prefixes ([verbose], [debug], [info], etc.)
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
}

// ─────────────────────────────────────────────────────────
// DaemonEvent (original content from core/events.rs)
// ─────────────────────────────────────────────────────────

/// Events from the Flutter daemon process
#[derive(Debug, Clone)]
pub enum DaemonEvent {
    /// Raw stdout line from daemon (JSON-RPC wrapped)
    Stdout(String),

    /// Parsed daemon message
    Message(DaemonMessage),

    /// Stderr output (usually errors/warnings)
    Stderr(String),

    /// Daemon process has exited
    Exited { code: Option<i32> },

    /// Process spawn failed
    SpawnFailed { reason: String },
}
