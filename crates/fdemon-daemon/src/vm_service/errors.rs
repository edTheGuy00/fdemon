//! Parsing and conversion utilities for Flutter.Error extension events.
//!
//! When Flutter runs in `--machine` mode, `ext.flutter.inspector.structuredErrors`
//! is enabled by default. This redirects `FlutterError.presentError` to post errors
//! via `developer.postEvent('Flutter.Error', errorJson)` to the VM Service Extension
//! stream. These errors **never reach stdout/stderr**, so they must be captured here.
//!
//! This module provides:
//! - [`FlutterErrorEvent`] — Parsed Flutter.Error event data
//! - [`parse_flutter_error`] — Checks if a [`StreamEvent`] is a Flutter.Error and parses it
//! - [`flutter_error_to_log_entry`] — Converts a [`FlutterErrorEvent`] to a displayable [`LogEntry`]

use fdemon_core::{
    stack_trace::ParsedStackTrace,
    types::{LogEntry, LogLevel, LogSource},
};

use super::protocol::StreamEvent;

// ---------------------------------------------------------------------------
// Flutter error event types
// ---------------------------------------------------------------------------

/// Parsed Flutter.Error event from the VM Service Extension stream.
///
/// Flutter errors are structured as extension events with `extensionKind = "Flutter.Error"`.
/// For the first error after each reload, `rendered_error_text` contains the full formatted
/// exception block. Subsequent errors omit it (to avoid flooding the Extension stream) and
/// only carry a short `description`.
#[derive(Debug, Clone)]
pub struct FlutterErrorEvent {
    /// Number of errors since last reload (1 = first error after reload).
    pub errors_since_reload: i32,
    /// Full rendered error text (only present for the first error, `errorsSinceReload == 1`).
    pub rendered_error_text: Option<String>,
    /// Short error description/summary.
    pub description: String,
    /// Library where the error occurred (e.g., `"rendering library"`, `"widgets library"`).
    pub library: Option<String>,
    /// Raw stack trace string from the event.
    pub stack_trace: Option<String>,
    /// Event timestamp in milliseconds since epoch (from the VM Service event).
    pub timestamp: Option<i64>,
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

/// Check if a VM Service stream event is a `Flutter.Error` extension event and parse it.
///
/// Returns `Some(FlutterErrorEvent)` when the event matches, `None` for any other event
/// kind or when required fields are missing/malformed.
///
/// # Expected event structure
///
/// ```json
/// {
///   "kind": "Extension",
///   "extensionKind": "Flutter.Error",
///   "extensionData": {
///     "description": "A RenderFlex overflowed by 42 pixels on the right.",
///     "renderedErrorText": "══╡ EXCEPTION CAUGHT ...",
///     "errorsSinceReload": 1,
///     "library": "rendering library",
///     "stackTrace": "#0 ..."
///   }
/// }
/// ```
pub fn parse_flutter_error(event: &StreamEvent) -> Option<FlutterErrorEvent> {
    // 1. The event kind must be "Extension"
    if event.kind != "Extension" {
        return None;
    }

    // 2. The extensionKind must be "Flutter.Error" (in the flattened data)
    let extension_kind = event.data.get("extensionKind")?.as_str()?;
    if extension_kind != "Flutter.Error" {
        return None;
    }

    // 3. Extract the extensionData object
    let extension_data = event.data.get("extensionData")?;

    // 4. Parse required fields — description is required, others are optional
    let description = extension_data
        .get("description")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_default();

    let errors_since_reload = extension_data
        .get("errorsSinceReload")
        .and_then(|v| v.as_i64())
        .map(|n| n as i32)
        .unwrap_or(1);

    let rendered_error_text = extension_data
        .get("renderedErrorText")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());

    let library = extension_data
        .get("library")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());

    let stack_trace = extension_data
        .get("stackTrace")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());

    Some(FlutterErrorEvent {
        errors_since_reload,
        rendered_error_text,
        description,
        library,
        stack_trace,
        timestamp: event.timestamp,
    })
}

// ---------------------------------------------------------------------------
// Conversion
// ---------------------------------------------------------------------------

/// Convert a [`FlutterErrorEvent`] to a [`LogEntry`] for display in the log view.
///
/// For the first error after a reload (`errors_since_reload == 1`), the full
/// `rendered_error_text` is used as the message (this contains the formatted exception
/// block normally shown on the console). For subsequent errors, a shorter summary is
/// composed from the `library` prefix and `description`.
///
/// If a `stack_trace` string is present, it is parsed via [`ParsedStackTrace::parse`] and
/// attached to the entry.
pub fn flutter_error_to_log_entry(error: &FlutterErrorEvent) -> LogEntry {
    let message = if let Some(ref rendered) = error.rendered_error_text {
        rendered.clone()
    } else {
        let prefix = error.library.as_deref().unwrap_or("Flutter");
        format!("[{}] {}", prefix, error.description)
    };

    let stack_trace = error
        .stack_trace
        .as_deref()
        .map(ParsedStackTrace::parse)
        .filter(|t| t.has_frames());

    match stack_trace {
        Some(trace) => {
            LogEntry::with_stack_trace(LogLevel::Error, LogSource::VmService, message, trace)
        }
        None => LogEntry::new(LogLevel::Error, LogSource::VmService, message),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::vm_service::protocol::StreamEvent;

    // Helper: build a StreamEvent from raw JSON fields
    fn make_stream_event(kind: &str, extra_fields: serde_json::Value) -> StreamEvent {
        let mut data = extra_fields;
        // Merge kind into data so we can reconstruct the event
        // StreamEvent has kind separately and data flattened; we build data without `kind`
        // but we need to pass `kind` as the struct field.
        // Remove "kind" from data if accidentally included
        if let Some(obj) = data.as_object_mut() {
            obj.remove("kind");
        }

        StreamEvent {
            kind: kind.to_string(),
            isolate: None,
            timestamp: None,
            data,
        }
    }

    // Convenience: wrap the data fields that would come from JSON flattening
    fn flutter_error_event(
        extension_data: serde_json::Value,
        timestamp: Option<i64>,
    ) -> StreamEvent {
        StreamEvent {
            kind: "Extension".to_string(),
            isolate: None,
            timestamp,
            data: json!({
                "extensionKind": "Flutter.Error",
                "extensionData": extension_data
            }),
        }
    }

    // -------------------------------------------------------------------------
    // parse_flutter_error tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_parse_flutter_error_first_error_with_rendered_text() {
        let event = flutter_error_event(
            json!({
                "description": "A RenderFlex overflowed by 42 pixels on the right.",
                "renderedErrorText": "══╡ EXCEPTION CAUGHT BY RENDERING LIBRARY ╞══\nThe following assertion was thrown during layout:\nA RenderFlex overflowed...",
                "errorsSinceReload": 1,
                "library": "rendering library",
                "stackTrace": "#0 RenderFlex.performLayout (package:flutter/src/rendering/flex.dart:769:11)"
            }),
            Some(1_704_067_200_000),
        );

        let result = parse_flutter_error(&event);
        assert!(
            result.is_some(),
            "Expected Some for first Flutter.Error event"
        );

        let error = result.unwrap();
        assert_eq!(error.errors_since_reload, 1);
        assert_eq!(
            error.description,
            "A RenderFlex overflowed by 42 pixels on the right."
        );
        assert!(error.rendered_error_text.is_some());
        assert!(error
            .rendered_error_text
            .as_ref()
            .unwrap()
            .contains("EXCEPTION CAUGHT BY RENDERING LIBRARY"));
        assert_eq!(error.library.as_deref(), Some("rendering library"));
        assert!(error.stack_trace.is_some());
        assert_eq!(error.timestamp, Some(1_704_067_200_000));
    }

    #[test]
    fn test_parse_flutter_error_subsequent_error_no_rendered_text() {
        let event = flutter_error_event(
            json!({
                "description": "A RenderFlex overflowed by 12 pixels on the bottom.",
                "errorsSinceReload": 2,
                "library": "rendering library"
            }),
            None,
        );

        let result = parse_flutter_error(&event);
        assert!(
            result.is_some(),
            "Expected Some for subsequent Flutter.Error event"
        );

        let error = result.unwrap();
        assert_eq!(error.errors_since_reload, 2);
        assert!(
            error.rendered_error_text.is_none(),
            "No rendered text for subsequent errors"
        );
        assert_eq!(
            error.description,
            "A RenderFlex overflowed by 12 pixels on the bottom."
        );
        assert!(error.stack_trace.is_none());
        assert!(error.timestamp.is_none());
    }

    #[test]
    fn test_parse_non_flutter_error_extension_returns_none() {
        let event = StreamEvent {
            kind: "Extension".to_string(),
            isolate: None,
            timestamp: None,
            data: json!({
                "extensionKind": "Flutter.FirstFrame",
                "extensionData": {}
            }),
        };

        let result = parse_flutter_error(&event);
        assert!(
            result.is_none(),
            "Should return None for non-Flutter.Error extension kinds"
        );
    }

    #[test]
    fn test_parse_non_extension_kind_returns_none() {
        let event = make_stream_event(
            "Logging",
            json!({
                "logRecord": {
                    "message": { "valueAsString": "hello" }
                }
            }),
        );

        let result = parse_flutter_error(&event);
        assert!(
            result.is_none(),
            "Should return None for non-Extension kinds"
        );
    }

    #[test]
    fn test_parse_gc_event_returns_none() {
        let event = make_stream_event("GC", json!({ "newSpace": {}, "oldSpace": {} }));

        let result = parse_flutter_error(&event);
        assert!(result.is_none(), "Should return None for GC events");
    }

    #[test]
    fn test_parse_malformed_extension_data_returns_none() {
        // No extensionData field at all
        let event = StreamEvent {
            kind: "Extension".to_string(),
            isolate: None,
            timestamp: None,
            data: json!({
                "extensionKind": "Flutter.Error"
                // extensionData is missing
            }),
        };

        let result = parse_flutter_error(&event);
        assert!(
            result.is_none(),
            "Should return None when extensionData is missing"
        );
    }

    #[test]
    fn test_parse_missing_extension_kind_returns_none() {
        let event = StreamEvent {
            kind: "Extension".to_string(),
            isolate: None,
            timestamp: None,
            data: json!({
                "extensionData": {
                    "description": "Something"
                }
            }),
        };

        let result = parse_flutter_error(&event);
        assert!(
            result.is_none(),
            "Should return None when extensionKind is missing"
        );
    }

    #[test]
    fn test_parse_flutter_error_empty_rendered_text_treated_as_none() {
        // Empty string renderedErrorText should be treated as None
        let event = flutter_error_event(
            json!({
                "description": "Some error",
                "renderedErrorText": "",
                "errorsSinceReload": 1
            }),
            None,
        );

        let result = parse_flutter_error(&event).unwrap();
        assert!(
            result.rendered_error_text.is_none(),
            "Empty rendered text should be treated as None"
        );
    }

    #[test]
    fn test_parse_flutter_error_missing_description_uses_empty_string() {
        // description field is absent — should not panic, uses empty string
        let event = flutter_error_event(
            json!({
                "errorsSinceReload": 1,
                "library": "widgets library"
            }),
            None,
        );

        let result = parse_flutter_error(&event).unwrap();
        assert_eq!(
            result.description, "",
            "Missing description should default to empty string"
        );
    }

    #[test]
    fn test_parse_flutter_error_default_errors_since_reload() {
        // errorsSinceReload missing defaults to 1
        let event = flutter_error_event(
            json!({
                "description": "Some error"
            }),
            None,
        );

        let result = parse_flutter_error(&event).unwrap();
        assert_eq!(
            result.errors_since_reload, 1,
            "Missing errorsSinceReload defaults to 1"
        );
    }

    #[test]
    fn test_parse_flutter_error_with_timestamp() {
        let ts = 1_700_000_001_234_i64;
        let event = StreamEvent {
            kind: "Extension".to_string(),
            isolate: None,
            timestamp: Some(ts),
            data: json!({
                "extensionKind": "Flutter.Error",
                "extensionData": {
                    "description": "Error with timestamp"
                }
            }),
        };

        let result = parse_flutter_error(&event).unwrap();
        assert_eq!(result.timestamp, Some(ts));
    }

    // -------------------------------------------------------------------------
    // flutter_error_to_log_entry tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_flutter_error_to_log_entry_has_correct_level_and_source() {
        let error = FlutterErrorEvent {
            errors_since_reload: 1,
            rendered_error_text: Some("Full rendered error".to_string()),
            description: "Short description".to_string(),
            library: None,
            stack_trace: None,
            timestamp: None,
        };

        let entry = flutter_error_to_log_entry(&error);
        assert_eq!(entry.level, LogLevel::Error);
        assert_eq!(entry.source, LogSource::VmService);
    }

    #[test]
    fn test_flutter_error_to_log_entry_uses_rendered_text_for_first_error() {
        let rendered = "══╡ EXCEPTION CAUGHT BY RENDERING LIBRARY ╞══\nDetails...".to_string();
        let error = FlutterErrorEvent {
            errors_since_reload: 1,
            rendered_error_text: Some(rendered.clone()),
            description: "Short description".to_string(),
            library: Some("rendering library".to_string()),
            stack_trace: None,
            timestamp: None,
        };

        let entry = flutter_error_to_log_entry(&error);
        assert_eq!(
            entry.message, rendered,
            "Should use rendered_error_text when present"
        );
    }

    #[test]
    fn test_flutter_error_to_log_entry_uses_description_for_subsequent_errors() {
        let error = FlutterErrorEvent {
            errors_since_reload: 2,
            rendered_error_text: None,
            description: "RenderFlex overflowed by 42 pixels".to_string(),
            library: Some("rendering library".to_string()),
            stack_trace: None,
            timestamp: None,
        };

        let entry = flutter_error_to_log_entry(&error);
        assert_eq!(
            entry.message,
            "[rendering library] RenderFlex overflowed by 42 pixels"
        );
    }

    #[test]
    fn test_flutter_error_to_log_entry_uses_flutter_prefix_when_no_library() {
        let error = FlutterErrorEvent {
            errors_since_reload: 2,
            rendered_error_text: None,
            description: "Widget build error".to_string(),
            library: None,
            stack_trace: None,
            timestamp: None,
        };

        let entry = flutter_error_to_log_entry(&error);
        assert_eq!(entry.message, "[Flutter] Widget build error");
    }

    #[test]
    fn test_flutter_error_to_log_entry_parses_stack_trace() {
        let trace_str =
            "#0 RenderFlex.performLayout (package:flutter/src/rendering/flex.dart:769:11)\n\
             #1 _MyWidget.build (package:my_app/widgets/my_widget.dart:42:5)";

        let error = FlutterErrorEvent {
            errors_since_reload: 1,
            rendered_error_text: Some("Error text".to_string()),
            description: "RenderFlex overflowed".to_string(),
            library: Some("rendering library".to_string()),
            stack_trace: Some(trace_str.to_string()),
            timestamp: None,
        };

        let entry = flutter_error_to_log_entry(&error);
        assert!(
            entry.has_stack_trace(),
            "Entry should have a parsed stack trace"
        );
        assert!(
            entry.stack_trace_frame_count() >= 1,
            "Stack trace should have at least one frame"
        );
    }

    #[test]
    fn test_flutter_error_to_log_entry_no_stack_trace_when_none() {
        let error = FlutterErrorEvent {
            errors_since_reload: 1,
            rendered_error_text: Some("Error text".to_string()),
            description: "Some error".to_string(),
            library: None,
            stack_trace: None,
            timestamp: None,
        };

        let entry = flutter_error_to_log_entry(&error);
        assert!(
            !entry.has_stack_trace(),
            "Entry should have no stack trace when none provided"
        );
    }

    #[test]
    fn test_flutter_error_to_log_entry_no_stack_trace_when_unparseable() {
        let error = FlutterErrorEvent {
            errors_since_reload: 1,
            rendered_error_text: Some("Error text".to_string()),
            description: "Some error".to_string(),
            library: None,
            stack_trace: Some("not a real stack trace".to_string()),
            timestamp: None,
        };

        let entry = flutter_error_to_log_entry(&error);
        // ParsedStackTrace::parse on an unparseable string yields zero frames,
        // so we filter it out and the entry has no stack trace
        assert!(
            !entry.has_stack_trace(),
            "Entry should have no stack trace when input is unparseable"
        );
    }

    // -------------------------------------------------------------------------
    // Round-trip tests (parse + convert)
    // -------------------------------------------------------------------------

    #[test]
    fn test_round_trip_first_error() {
        let event = flutter_error_event(
            json!({
                "description": "A RenderFlex overflowed by 999 pixels on the bottom.",
                "renderedErrorText": "══╡ EXCEPTION CAUGHT ╞══\nDetails here",
                "errorsSinceReload": 1,
                "library": "rendering library",
                "stackTrace": "#0 main (package:my_app/main.dart:10:3)"
            }),
            Some(1_000_000_000),
        );

        let flutter_error = parse_flutter_error(&event).expect("should parse successfully");
        let entry = flutter_error_to_log_entry(&flutter_error);

        assert_eq!(entry.level, LogLevel::Error);
        assert_eq!(entry.source, LogSource::VmService);
        assert!(entry.message.contains("EXCEPTION CAUGHT"));
        assert!(entry.has_stack_trace());
    }

    #[test]
    fn test_round_trip_subsequent_error() {
        let event = flutter_error_event(
            json!({
                "description": "RenderFlex overflowed again",
                "errorsSinceReload": 3,
                "library": "widgets library"
            }),
            None,
        );

        let flutter_error = parse_flutter_error(&event).expect("should parse successfully");
        let entry = flutter_error_to_log_entry(&flutter_error);

        assert_eq!(entry.level, LogLevel::Error);
        assert_eq!(entry.source, LogSource::VmService);
        assert_eq!(
            entry.message,
            "[widgets library] RenderFlex overflowed again"
        );
        assert!(!entry.has_stack_trace());
    }
}
