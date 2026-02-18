//! VM Service Logging stream event parsing.
//!
//! This module parses `LogRecord` events from the VM Service `Logging` stream and
//! converts them to [`LogEntry`] items with accurate log levels.
//!
//! Apps using `dart:developer log()` (or the `logging` package that wraps it)
//! emit structured `LogRecord` events over the VM Service Logging stream. Apps
//! using `print()`, `Logger` (the print-based one), or `Talker` do NOT — those
//! continue through the daemon's stdout.
//!
//! # VM Log Level Mapping
//!
//! Dart's `dart:developer` package uses numeric log levels matching the Java
//! `java.util.logging` convention:
//!
//! | Dart Level | Value | fdemon Level |
//! |------------|-------|--------------|
//! | FINEST     | 300   | Debug        |
//! | FINER      | 400   | Debug        |
//! | FINE       | 500   | Debug        |
//! | CONFIG     | 700   | Debug        |
//! | INFO       | 800   | Info         |
//! | WARNING    | 900   | Warning      |
//! | SEVERE     | 1000  | Error        |
//! | SHOUT      | 1200  | Error        |

use chrono::{DateTime, Local, TimeZone};
use fdemon_core::{LogEntry, LogLevel, LogSource};
use serde_json::Value;

use super::protocol::StreamEvent;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// A parsed VM Service `LogRecord` event.
///
/// Fields match the `LogRecord` object in the VM Service protocol:
/// <https://github.com/dart-lang/sdk/blob/main/runtime/vm/service/service.md#logrecord>
#[derive(Debug, Clone)]
pub struct VmLogRecord {
    /// Log message text, extracted from the `message` `InstanceRef`.
    pub message: String,
    /// Dart log level (300–1200 range, following `dart:developer` convention).
    pub level: i32,
    /// Logger name extracted from the `loggerName` `InstanceRef` (e.g. `"AuthService"`).
    pub logger_name: Option<String>,
    /// Timestamp in milliseconds since epoch.
    pub time: i64,
    /// Monotonically-increasing sequence number for ordering.
    pub sequence_number: i64,
    /// Error message if present, extracted from the `error` `InstanceRef`.
    pub error: Option<String>,
    /// Stack trace string if present, extracted from the `stackTrace` `InstanceRef`.
    pub stack_trace: Option<String>,
}

// ---------------------------------------------------------------------------
// Level mapping
// ---------------------------------------------------------------------------

/// Map a Dart VM Service log level integer to a [`LogLevel`].
///
/// Dart's `dart:developer` package uses numeric levels matching the
/// `java.util.logging` convention. The mapping is:
///
/// - `..=799`  → [`LogLevel::Debug`]   (FINEST 300, FINER 400, FINE 500, CONFIG 700)
/// - `800..=899` → [`LogLevel::Info`]  (INFO 800)
/// - `900..=999` → [`LogLevel::Warning`] (WARNING 900)
/// - `1000..`  → [`LogLevel::Error`]   (SEVERE 1000, SHOUT 1200)
pub fn vm_level_to_log_level(level: i32) -> LogLevel {
    match level {
        ..=799 => LogLevel::Debug,
        800..=899 => LogLevel::Info,
        900..=999 => LogLevel::Warning,
        _ => LogLevel::Error,
    }
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

/// Check if a VM Service event is a `Logging` event and parse its `LogRecord`.
///
/// Returns `None` if the event kind is not `"Logging"` or if required fields
/// are missing or malformed.
pub fn parse_log_record(event: &StreamEvent) -> Option<VmLogRecord> {
    if event.kind != "Logging" {
        return None;
    }

    let log_record = event.data.get("logRecord")?;

    // Extract the `message` field — required.
    let message = extract_value_as_string(log_record.get("message")?)?;

    // Extract the `level` field — required. The VM Service sends this as an
    // integer, not an InstanceRef.
    let level = log_record.get("level")?.as_i64()? as i32;

    // Extract optional `loggerName` — may be null.
    let logger_name = log_record
        .get("loggerName")
        .and_then(extract_value_as_string);

    // Extract optional `time` — defaults to 0 if missing.
    let time = log_record.get("time").and_then(Value::as_i64).unwrap_or(0);

    // Extract optional `sequenceNumber` — defaults to 0 if missing.
    let sequence_number = log_record
        .get("sequenceNumber")
        .and_then(Value::as_i64)
        .unwrap_or(0);

    // Extract optional `error` — may be null.
    let error = log_record.get("error").and_then(extract_value_as_string);

    // Extract optional `stackTrace` — may be null.
    let stack_trace = log_record
        .get("stackTrace")
        .and_then(extract_value_as_string);

    Some(VmLogRecord {
        message,
        level,
        logger_name,
        time,
        sequence_number,
        error,
        stack_trace,
    })
}

/// Extract `valueAsString` from a VM Service `InstanceRef` object.
///
/// The `InstanceRef` format is `{"type": "@Instance", "valueAsString": "..."}`.
/// Returns `None` if the field is absent or its value is JSON `null`.
fn extract_value_as_string(value: &Value) -> Option<String> {
    value
        .get("valueAsString")
        .and_then(Value::as_str)
        .map(str::to_owned)
}

// ---------------------------------------------------------------------------
// Conversion to LogEntry
// ---------------------------------------------------------------------------

/// Convert a [`VmLogRecord`] to a [`LogEntry`] for display in the log view.
///
/// - The [`LogLevel`] is derived from the VM log level integer.
/// - The [`LogSource`] is always [`LogSource::VmService`].
/// - If a logger name is present and non-empty, the message is prefixed with
///   `[LoggerName] `.
/// - A stack trace is parsed if present.
pub fn vm_log_to_log_entry(record: &VmLogRecord) -> LogEntry {
    use fdemon_core::stack_trace::ParsedStackTrace;

    let level = vm_level_to_log_level(record.level);

    // Prefix message with logger name when available.
    let message = match &record.logger_name {
        Some(name) if !name.is_empty() => format!("[{}] {}", name, record.message),
        _ => record.message.clone(),
    };

    // Parse stack trace if present.
    let stack_trace = record.stack_trace.as_deref().map(ParsedStackTrace::parse);

    // Convert milliseconds-since-epoch to DateTime<Local>.
    // Falls back to Local::now() if conversion fails (e.g., time is 0 or out of range).
    let timestamp = millis_to_datetime(record.time);

    // Construct the entry manually so we can supply the VM timestamp rather
    // than using Local::now() inside LogEntry::new().
    let base = LogEntry::new(level, LogSource::VmService, message);

    LogEntry {
        timestamp,
        stack_trace,
        ..base
    }
}

/// Convert milliseconds since epoch to a [`DateTime<Local>`].
///
/// Returns `Local::now()` if the timestamp cannot be represented (e.g. 0 or
/// out of range).
fn millis_to_datetime(millis: i64) -> DateTime<Local> {
    let secs = millis / 1000;
    let nanos = ((millis % 1000) * 1_000_000) as u32;
    Local
        .timestamp_opt(secs, nanos)
        .single()
        .unwrap_or_else(Local::now)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // Helper: deserialize a JSON object into a StreamEvent.
    fn parse_event(json: &str) -> StreamEvent {
        serde_json::from_str(json).expect("test JSON must be valid StreamEvent")
    }

    // ── vm_level_to_log_level ──────────────────────────────────────────────

    #[test]
    fn test_vm_level_to_log_level_finest_is_debug() {
        assert_eq!(vm_level_to_log_level(300), LogLevel::Debug);
    }

    #[test]
    fn test_vm_level_to_log_level_finer_is_debug() {
        assert_eq!(vm_level_to_log_level(400), LogLevel::Debug);
    }

    #[test]
    fn test_vm_level_to_log_level_fine_is_debug() {
        assert_eq!(vm_level_to_log_level(500), LogLevel::Debug);
    }

    #[test]
    fn test_vm_level_to_log_level_config_is_debug() {
        assert_eq!(vm_level_to_log_level(700), LogLevel::Debug);
    }

    #[test]
    fn test_vm_level_to_log_level_info() {
        assert_eq!(vm_level_to_log_level(800), LogLevel::Info);
    }

    #[test]
    fn test_vm_level_to_log_level_info_upper_boundary() {
        assert_eq!(vm_level_to_log_level(899), LogLevel::Info);
    }

    #[test]
    fn test_vm_level_to_log_level_warning() {
        assert_eq!(vm_level_to_log_level(900), LogLevel::Warning);
    }

    #[test]
    fn test_vm_level_to_log_level_warning_upper_boundary() {
        assert_eq!(vm_level_to_log_level(999), LogLevel::Warning);
    }

    #[test]
    fn test_vm_level_to_log_level_severe() {
        assert_eq!(vm_level_to_log_level(1000), LogLevel::Error);
    }

    #[test]
    fn test_vm_level_to_log_level_shout() {
        assert_eq!(vm_level_to_log_level(1200), LogLevel::Error);
    }

    #[test]
    fn test_vm_level_to_log_level_zero_is_debug() {
        assert_eq!(vm_level_to_log_level(0), LogLevel::Debug);
    }

    // ── extract_value_as_string ────────────────────────────────────────────

    #[test]
    fn test_extract_value_as_string_returns_value() {
        let value = serde_json::json!({"type": "@Instance", "valueAsString": "hello"});
        assert_eq!(extract_value_as_string(&value), Some("hello".to_string()));
    }

    #[test]
    fn test_extract_value_as_string_null_returns_none() {
        let value = serde_json::json!({"type": "@Instance", "valueAsString": null});
        assert_eq!(extract_value_as_string(&value), None);
    }

    #[test]
    fn test_extract_value_as_string_missing_field_returns_none() {
        let value = serde_json::json!({"type": "@Instance"});
        assert_eq!(extract_value_as_string(&value), None);
    }

    #[test]
    fn test_extract_value_as_string_non_string_value_returns_none() {
        // valueAsString as a number — should return None (not a str)
        let value = serde_json::json!({"valueAsString": 42});
        assert_eq!(extract_value_as_string(&value), None);
    }

    // ── parse_log_record ───────────────────────────────────────────────────

    #[test]
    fn test_parse_non_logging_event_returns_none() {
        let json = r#"{
            "kind": "Extension",
            "extensionKind": "Flutter.Error",
            "extensionData": {}
        }"#;
        assert!(parse_log_record(&parse_event(json)).is_none());
    }

    #[test]
    fn test_parse_log_record_with_logger_name() {
        let json = r#"{
            "kind": "Logging",
            "logRecord": {
                "message": {"type": "@Instance", "valueAsString": "User logged in"},
                "level": 800,
                "loggerName": {"type": "@Instance", "valueAsString": "AuthService"},
                "time": 1704067200000,
                "sequenceNumber": 42,
                "error": {"type": "@Instance", "valueAsString": null},
                "stackTrace": {"type": "@Instance", "valueAsString": null}
            }
        }"#;
        let record = parse_log_record(&parse_event(json)).unwrap();
        assert_eq!(record.message, "User logged in");
        assert_eq!(record.logger_name, Some("AuthService".to_string()));
        assert_eq!(record.level, 800);
        assert_eq!(record.time, 1_704_067_200_000);
        assert_eq!(record.sequence_number, 42);
        assert!(record.error.is_none());
        assert!(record.stack_trace.is_none());
    }

    #[test]
    fn test_parse_log_record_without_logger_name() {
        let json = r#"{
            "kind": "Logging",
            "logRecord": {
                "message": {"type": "@Instance", "valueAsString": "plain message"},
                "level": 800,
                "loggerName": {"type": "@Instance", "valueAsString": null},
                "time": 0,
                "sequenceNumber": 1,
                "error": {"type": "@Instance", "valueAsString": null},
                "stackTrace": {"type": "@Instance", "valueAsString": null}
            }
        }"#;
        let record = parse_log_record(&parse_event(json)).unwrap();
        assert_eq!(record.message, "plain message");
        assert!(record.logger_name.is_none());
    }

    #[test]
    fn test_parse_log_record_missing_log_record_field_returns_none() {
        let json = r#"{"kind": "Logging"}"#;
        assert!(parse_log_record(&parse_event(json)).is_none());
    }

    #[test]
    fn test_parse_log_record_missing_message_returns_none() {
        let json = r#"{
            "kind": "Logging",
            "logRecord": {
                "level": 800,
                "time": 0,
                "sequenceNumber": 1
            }
        }"#;
        assert!(parse_log_record(&parse_event(json)).is_none());
    }

    #[test]
    fn test_parse_log_record_missing_level_returns_none() {
        let json = r#"{
            "kind": "Logging",
            "logRecord": {
                "message": {"type": "@Instance", "valueAsString": "hello"},
                "time": 0,
                "sequenceNumber": 1
            }
        }"#;
        assert!(parse_log_record(&parse_event(json)).is_none());
    }

    #[test]
    fn test_parse_log_record_with_error_and_stack_trace() {
        let json = r##"{
            "kind": "Logging",
            "logRecord": {
                "message": {"type": "@Instance", "valueAsString": "Unhandled exception"},
                "level": 1000,
                "loggerName": {"type": "@Instance", "valueAsString": null},
                "time": 1704067200000,
                "sequenceNumber": 7,
                "error": {"type": "@Instance", "valueAsString": "Null check operator used on a null value"},
                "stackTrace": {"type": "@Instance", "valueAsString": "#0 main (package:app/main.dart:15:3)"}
            }
        }"##;
        let record = parse_log_record(&parse_event(json)).unwrap();
        assert_eq!(record.level, 1000);
        assert_eq!(
            record.error,
            Some("Null check operator used on a null value".to_string())
        );
        assert_eq!(
            record.stack_trace,
            Some("#0 main (package:app/main.dart:15:3)".to_string())
        );
    }

    #[test]
    fn test_parse_log_record_optional_fields_default() {
        // time and sequenceNumber are absent — should default to 0.
        let json = r#"{
            "kind": "Logging",
            "logRecord": {
                "message": {"type": "@Instance", "valueAsString": "minimal"},
                "level": 800
            }
        }"#;
        let record = parse_log_record(&parse_event(json)).unwrap();
        assert_eq!(record.time, 0);
        assert_eq!(record.sequence_number, 0);
        assert!(record.logger_name.is_none());
        assert!(record.error.is_none());
        assert!(record.stack_trace.is_none());
    }

    // ── vm_log_to_log_entry ────────────────────────────────────────────────

    #[test]
    fn test_vm_log_to_log_entry_prefixes_logger_name() {
        let record = VmLogRecord {
            message: "User logged in".to_string(),
            level: 800,
            logger_name: Some("AuthService".to_string()),
            time: 1_704_067_200_000,
            sequence_number: 42,
            error: None,
            stack_trace: None,
        };
        let entry = vm_log_to_log_entry(&record);
        assert_eq!(entry.message, "[AuthService] User logged in");
        assert_eq!(entry.level, LogLevel::Info);
        assert_eq!(entry.source, LogSource::VmService);
        assert!(!entry.has_stack_trace());
    }

    #[test]
    fn test_vm_log_to_log_entry_no_logger_name_no_prefix() {
        let record = VmLogRecord {
            message: "raw message".to_string(),
            level: 800,
            logger_name: None,
            time: 0,
            sequence_number: 1,
            error: None,
            stack_trace: None,
        };
        let entry = vm_log_to_log_entry(&record);
        assert_eq!(entry.message, "raw message");
    }

    #[test]
    fn test_vm_log_to_log_entry_empty_logger_name_no_prefix() {
        let record = VmLogRecord {
            message: "raw message".to_string(),
            level: 800,
            logger_name: Some(String::new()),
            time: 0,
            sequence_number: 1,
            error: None,
            stack_trace: None,
        };
        let entry = vm_log_to_log_entry(&record);
        assert_eq!(entry.message, "raw message");
    }

    #[test]
    fn test_vm_log_to_log_entry_correct_level() {
        let levels = [
            (300, LogLevel::Debug),
            (800, LogLevel::Info),
            (900, LogLevel::Warning),
            (1000, LogLevel::Error),
            (1200, LogLevel::Error),
        ];
        for (vm_level, expected) in levels {
            let record = VmLogRecord {
                message: "test".to_string(),
                level: vm_level,
                logger_name: None,
                time: 0,
                sequence_number: 0,
                error: None,
                stack_trace: None,
            };
            assert_eq!(
                vm_log_to_log_entry(&record).level,
                expected,
                "level {vm_level} should map to {expected:?}"
            );
        }
    }

    #[test]
    fn test_vm_log_to_log_entry_source_is_vm_service() {
        let record = VmLogRecord {
            message: "test".to_string(),
            level: 800,
            logger_name: None,
            time: 0,
            sequence_number: 0,
            error: None,
            stack_trace: None,
        };
        let entry = vm_log_to_log_entry(&record);
        assert_eq!(entry.source, LogSource::VmService);
    }

    #[test]
    fn test_vm_log_to_log_entry_with_stack_trace() {
        let record = VmLogRecord {
            message: "crash".to_string(),
            level: 1000,
            logger_name: None,
            time: 0,
            sequence_number: 0,
            error: Some("NullPointerException".to_string()),
            stack_trace: Some("#0 main (package:app/main.dart:10:3)".to_string()),
        };
        let entry = vm_log_to_log_entry(&record);
        assert!(entry.has_stack_trace());
        assert_eq!(entry.level, LogLevel::Error);
    }

    #[test]
    fn test_millis_to_datetime_zero_does_not_panic() {
        // Should not panic — returns some valid DateTime.
        let dt = millis_to_datetime(0);
        // 1970-01-01 00:00:00 UTC — just check it doesn't crash.
        let _ = dt.format("%Y").to_string();
    }

    #[test]
    fn test_millis_to_datetime_reasonable_value() {
        // 2024-01-01 00:00:00 UTC = 1704067200000 ms
        let dt = millis_to_datetime(1_704_067_200_000);
        assert_eq!(dt.format("%Y").to_string(), "2024");
    }
}
