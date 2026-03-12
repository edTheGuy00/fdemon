//! Pluggable output format parsers for native log sources.
//!
//! Provides a central dispatch function [`parse_line`] that routes a raw output
//! line to the correct format-specific parser based on the configured
//! [`OutputFormat`].
//!
//! ## Supported Formats
//!
//! | Variant               | Parser                        | Delegates to         |
//! |-----------------------|-------------------------------|----------------------|
//! | [`OutputFormat::Raw`]            | [`parse_raw`]        | —                    |
//! | [`OutputFormat::Json`]           | [`parse_json`]       | —                    |
//! | [`OutputFormat::LogcatThreadtime`] | [`parse_logcat_threadtime`] | `android::parse_threadtime_line` |
//! | [`OutputFormat::Syslog`]         | [`parse_syslog`]     | `macos::parse_syslog_line`  |

use fdemon_core::{LogLevel, OutputFormat};

use super::NativeLogEvent;

/// Parse a single output line using the specified format.
///
/// Returns `None` if the line cannot be parsed (blank line, header line,
/// malformed JSON, unrecognized format, etc.).
pub fn parse_line(format: &OutputFormat, line: &str, source_name: &str) -> Option<NativeLogEvent> {
    match format {
        OutputFormat::Raw => parse_raw(line, source_name),
        OutputFormat::Json => parse_json(line, source_name),
        OutputFormat::LogcatThreadtime => parse_logcat_threadtime(line),
        OutputFormat::Syslog => parse_syslog(line, source_name),
    }
}

/// Parse a raw (unstructured) log line.
///
/// Every non-empty, non-whitespace-only line becomes a log event.
/// The tag is set to `source_name` and the level is always [`LogLevel::Info`].
fn parse_raw(line: &str, source_name: &str) -> Option<NativeLogEvent> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(NativeLogEvent {
        tag: source_name.to_string(),
        level: LogLevel::Info,
        message: trimmed.to_string(),
        timestamp: None,
    })
}

/// Parse a JSON log line.
///
/// Supports flexible field name aliases:
/// - Message: `"message"`, `"msg"`, `"text"`
/// - Tag: `"tag"`, `"source"`, `"logger"` (falls back to `source_name`)
/// - Level: `"level"`, `"severity"`, `"priority"` (falls back to [`LogLevel::Info`])
/// - Timestamp: `"timestamp"`, `"time"`, `"ts"`
///
/// Returns `None` for invalid JSON, non-object values, or missing/empty message.
fn parse_json(line: &str, source_name: &str) -> Option<NativeLogEvent> {
    let v: serde_json::Value = serde_json::from_str(line.trim()).ok()?;
    let obj = v.as_object()?;

    // Message: try "message", "msg", "text"
    let message = obj
        .get("message")
        .or_else(|| obj.get("msg"))
        .or_else(|| obj.get("text"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    if message.is_empty() {
        return None;
    }

    // Tag: try "tag", "source", "logger" — fall back to source_name
    let tag = obj
        .get("tag")
        .or_else(|| obj.get("source"))
        .or_else(|| obj.get("logger"))
        .and_then(|v| v.as_str())
        .unwrap_or(source_name)
        .to_string();

    // Level: try "level", "severity", "priority"
    let level = obj
        .get("level")
        .or_else(|| obj.get("severity"))
        .or_else(|| obj.get("priority"))
        .and_then(|v| v.as_str())
        .map(parse_json_level)
        .unwrap_or(LogLevel::Info);

    // Timestamp: try "timestamp", "time", "ts"
    let timestamp = obj
        .get("timestamp")
        .or_else(|| obj.get("time"))
        .or_else(|| obj.get("ts"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    Some(NativeLogEvent {
        tag,
        level,
        message,
        timestamp,
    })
}

/// Map a JSON-style level string to a [`LogLevel`].
///
/// Recognized strings (case-insensitive):
/// - `"trace"` / `"verbose"` / `"debug"` → [`LogLevel::Debug`]
/// - `"info"` / `"information"` → [`LogLevel::Info`]
/// - `"warn"` / `"warning"` → [`LogLevel::Warning`]
/// - `"error"` / `"err"` / `"fatal"` / `"critical"` → [`LogLevel::Error`]
/// - Anything else → [`LogLevel::Info`]
fn parse_json_level(s: &str) -> LogLevel {
    match s.to_lowercase().as_str() {
        "trace" | "verbose" | "debug" => LogLevel::Debug,
        "info" | "information" => LogLevel::Info,
        "warn" | "warning" => LogLevel::Warning,
        "error" | "err" | "fatal" | "critical" => LogLevel::Error,
        _ => LogLevel::Info,
    }
}

/// Parse an Android logcat threadtime-format line.
///
/// Delegates to [`super::android::parse_threadtime_line`] and
/// [`super::android::logcat_line_to_event`]. Returns `None` for header lines,
/// blank lines, or lines that do not match the threadtime format.
fn parse_logcat_threadtime(line: &str) -> Option<NativeLogEvent> {
    let logcat_line = super::android::parse_threadtime_line(line)?;
    super::android::logcat_line_to_event(&logcat_line)
}

/// Parse a macOS `log stream --style compact` (syslog) format line.
///
/// Delegates to [`super::macos::parse_syslog_line`] and
/// [`super::macos::syslog_line_to_event`]. The `source_name` parameter is
/// accepted for API consistency but is not used; the tag is derived from the
/// log line's subsystem/category fields instead.
///
/// Returns `None` for header lines, blank lines, or non-matching lines.
#[cfg(target_os = "macos")]
fn parse_syslog(line: &str, _source_name: &str) -> Option<NativeLogEvent> {
    let syslog_line = super::macos::parse_syslog_line(line)?;
    Some(super::macos::syslog_line_to_event(&syslog_line))
}

/// Stub for non-macOS platforms: syslog format is only supported on macOS.
///
/// Always returns `None` on non-macOS platforms. Use [`OutputFormat::Raw`] or
/// [`OutputFormat::Json`] for cross-platform sources.
#[cfg(not(target_os = "macos"))]
fn parse_syslog(_line: &str, _source_name: &str) -> Option<NativeLogEvent> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Raw format ──────────────────────────────────────────────────────────

    #[test]
    fn test_raw_format_basic_line() {
        let event =
            parse_line(&OutputFormat::Raw, "Hello from custom source", "my-source").unwrap();
        assert_eq!(event.tag, "my-source");
        assert_eq!(event.level, LogLevel::Info);
        assert_eq!(event.message, "Hello from custom source");
        assert!(event.timestamp.is_none());
    }

    #[test]
    fn test_raw_format_trims_whitespace() {
        let event = parse_line(&OutputFormat::Raw, "  hello  ", "src").unwrap();
        assert_eq!(event.message, "hello");
        assert_eq!(event.tag, "src");
    }

    #[test]
    fn test_raw_format_empty_line_returns_none() {
        assert!(parse_line(&OutputFormat::Raw, "", "src").is_none());
    }

    #[test]
    fn test_raw_format_whitespace_only_returns_none() {
        assert!(parse_line(&OutputFormat::Raw, "   ", "src").is_none());
        assert!(parse_line(&OutputFormat::Raw, "\t", "src").is_none());
        assert!(parse_line(&OutputFormat::Raw, "\n", "src").is_none());
    }

    // ── JSON format ─────────────────────────────────────────────────────────

    #[test]
    fn test_json_format_standard_fields() {
        let line = r#"{"level": "error", "tag": "MyApp", "message": "something failed"}"#;
        let event = parse_line(&OutputFormat::Json, line, "fallback").unwrap();
        assert_eq!(event.level, LogLevel::Error);
        assert_eq!(event.tag, "MyApp");
        assert_eq!(event.message, "something failed");
    }

    #[test]
    fn test_json_format_alternate_field_names() {
        let line = r#"{"severity": "warn", "logger": "http", "msg": "timeout"}"#;
        let event = parse_line(&OutputFormat::Json, line, "fallback").unwrap();
        assert_eq!(event.level, LogLevel::Warning);
        assert_eq!(event.tag, "http");
        assert_eq!(event.message, "timeout");
    }

    #[test]
    fn test_json_format_text_field_alias() {
        let line = r#"{"priority": "debug", "source": "network", "text": "connecting"}"#;
        let event = parse_line(&OutputFormat::Json, line, "fallback").unwrap();
        assert_eq!(event.level, LogLevel::Debug);
        assert_eq!(event.tag, "network");
        assert_eq!(event.message, "connecting");
    }

    #[test]
    fn test_json_format_tag_falls_back_to_source_name() {
        let line = r#"{"message": "no tag here"}"#;
        let event = parse_line(&OutputFormat::Json, line, "my-source").unwrap();
        assert_eq!(event.tag, "my-source");
        assert_eq!(event.level, LogLevel::Info);
    }

    #[test]
    fn test_json_format_timestamp_aliases() {
        // "timestamp"
        let line = r#"{"message": "msg", "timestamp": "2024-01-01T00:00:00Z"}"#;
        let event = parse_line(&OutputFormat::Json, line, "src").unwrap();
        assert_eq!(event.timestamp, Some("2024-01-01T00:00:00Z".to_string()));

        // "time"
        let line = r#"{"message": "msg", "time": "2024-01-01"}"#;
        let event = parse_line(&OutputFormat::Json, line, "src").unwrap();
        assert_eq!(event.timestamp, Some("2024-01-01".to_string()));

        // "ts"
        let line = r#"{"message": "msg", "ts": "12345"}"#;
        let event = parse_line(&OutputFormat::Json, line, "src").unwrap();
        assert_eq!(event.timestamp, Some("12345".to_string()));
    }

    #[test]
    fn test_json_format_missing_message_returns_none() {
        let line = r#"{"level": "info", "tag": "foo"}"#;
        assert!(parse_line(&OutputFormat::Json, line, "src").is_none());
    }

    #[test]
    fn test_json_format_empty_message_returns_none() {
        let line = r#"{"message": "", "tag": "foo"}"#;
        assert!(parse_line(&OutputFormat::Json, line, "src").is_none());
    }

    #[test]
    fn test_json_format_invalid_json_returns_none() {
        assert!(parse_line(&OutputFormat::Json, "not json", "src").is_none());
        assert!(parse_line(&OutputFormat::Json, "{broken", "src").is_none());
        assert!(parse_line(&OutputFormat::Json, "", "src").is_none());
    }

    #[test]
    fn test_json_format_non_object_returns_none() {
        assert!(parse_line(&OutputFormat::Json, r#"["array"]"#, "src").is_none());
        assert!(parse_line(&OutputFormat::Json, r#""string""#, "src").is_none());
    }

    #[test]
    fn test_json_level_all_mappings() {
        let cases = [
            ("trace", LogLevel::Debug),
            ("verbose", LogLevel::Debug),
            ("debug", LogLevel::Debug),
            ("info", LogLevel::Info),
            ("information", LogLevel::Info),
            ("warn", LogLevel::Warning),
            ("warning", LogLevel::Warning),
            ("error", LogLevel::Error),
            ("err", LogLevel::Error),
            ("fatal", LogLevel::Error),
            ("critical", LogLevel::Error),
            ("unknown", LogLevel::Info),
        ];
        for (s, expected) in cases {
            assert_eq!(
                parse_json_level(s),
                expected,
                "parse_json_level({s:?}) should be {expected:?}"
            );
        }
    }

    #[test]
    fn test_json_level_case_insensitive() {
        assert_eq!(parse_json_level("ERROR"), LogLevel::Error);
        assert_eq!(parse_json_level("Warning"), LogLevel::Warning);
        assert_eq!(parse_json_level("DEBUG"), LogLevel::Debug);
    }

    // ── Logcat threadtime format ─────────────────────────────────────────────

    #[test]
    fn test_logcat_threadtime_delegates_to_existing_parser() {
        let line = "03-10 14:30:00.123  1234  5678 I GoLog   : Hello from Go";
        let event =
            parse_line(&OutputFormat::LogcatThreadtime, line, "ignored-source-name").unwrap();
        assert_eq!(event.tag, "GoLog");
        assert_eq!(event.level, LogLevel::Info);
        assert_eq!(event.message, "Hello from Go");
        assert_eq!(event.timestamp, Some("03-10 14:30:00.123".to_string()));
    }

    #[test]
    fn test_logcat_threadtime_error_priority() {
        let line = "03-10 14:30:00.123  1234  5678 E AndroidRuntime: FATAL EXCEPTION";
        let event = parse_line(&OutputFormat::LogcatThreadtime, line, "src").unwrap();
        assert_eq!(event.level, LogLevel::Error);
        assert_eq!(event.tag, "AndroidRuntime");
    }

    #[test]
    fn test_logcat_threadtime_non_matching_line_returns_none() {
        assert!(parse_line(&OutputFormat::LogcatThreadtime, "not a logcat line", "src").is_none());
        assert!(parse_line(
            &OutputFormat::LogcatThreadtime,
            "--------- beginning of main",
            "src"
        )
        .is_none());
        assert!(parse_line(&OutputFormat::LogcatThreadtime, "", "src").is_none());
    }

    // ── Syslog format (macOS only) ───────────────────────────────────────────

    #[cfg(target_os = "macos")]
    #[test]
    fn test_syslog_delegates_to_existing_parser() {
        let line = "2024-03-10 14:30:00.123 I  my_app[5678:abcde] (MyPlugin) [com.example.plugin:default] Hello from plugin";
        let event = parse_line(&OutputFormat::Syslog, line, "ignored").unwrap();
        assert_eq!(event.tag, "com.example.plugin");
        assert_eq!(event.level, LogLevel::Info);
        assert_eq!(event.message, "Hello from plugin");
        assert_eq!(event.timestamp, Some("2024-03-10 14:30:00.123".to_string()));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn test_syslog_header_line_returns_none() {
        assert!(parse_line(
            &OutputFormat::Syslog,
            "Filtering the log data using \"process == \\\"my_app\\\"\"",
            "src"
        )
        .is_none());
        assert!(parse_line(
            &OutputFormat::Syslog,
            "Timestamp               Ty Process[PID:TID]",
            "src"
        )
        .is_none());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn test_syslog_empty_line_returns_none() {
        assert!(parse_line(&OutputFormat::Syslog, "", "src").is_none());
        assert!(parse_line(&OutputFormat::Syslog, "   ", "src").is_none());
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn test_syslog_non_macos_returns_none() {
        let line = "2024-03-10 14:30:00.123 I  my_app[5678:abcde] hello";
        assert!(parse_line(&OutputFormat::Syslog, line, "src").is_none());
    }

    // ── JSON format edge cases (Phase 3 Task 05) ─────────────────────────────

    #[test]
    fn test_json_format_ignores_unknown_fields() {
        // Extra fields ("extra", "nested" object) must be silently ignored.
        let line = r#"{"message": "hello", "extra": "ignored", "nested": {"deep": true}}"#;
        let event = parse_line(&OutputFormat::Json, line, "test").unwrap();
        assert_eq!(event.message, "hello");
    }

    #[test]
    fn test_json_format_string_level_only_numeric_defaults_to_info() {
        // A numeric value for the level field cannot be parsed as a string;
        // the parser should fall back to the default LogLevel::Info.
        let line = r#"{"message": "hello", "level": 3}"#;
        let event = parse_line(&OutputFormat::Json, line, "test").unwrap();
        assert_eq!(event.message, "hello");
        assert_eq!(
            event.level,
            LogLevel::Info,
            "numeric level should default to Info"
        );
    }
}
