//! Frame timing parsing from Flutter.Frame Extension events.
//!
//! Flutter posts `Flutter.Frame` events via `developer.postEvent()` on the VM
//! Service Extension stream when a frame is rendered. These events contain
//! build and raster timing in string-encoded microsecond values.
//!
//! ## Event structure
//!
//! ```json
//! {
//!     "kind": "Extension",
//!     "extensionKind": "Flutter.Frame",
//!     "extensionData": {
//!         "number": "42",
//!         "startTime": "1704067200000",
//!         "elapsed": "12500",
//!         "build": "6200",
//!         "raster": "6300"
//!     },
//!     "isolate": { "id": "isolates/1234", "name": "main" },
//!     "timestamp": 1704067200000
//! }
//! ```
//!
//! These events arrive on the Extension stream, which is already subscribed to
//! in Phase 1. No new stream subscription is needed — only new parsing logic.
//!
//! ## String-encoded numbers
//!
//! Flutter's Extension event data encodes all numeric values as strings. The
//! [`parse_str_u64`] helper handles both string and integer JSON values for
//! defensive parsing.

use fdemon_core::performance::FrameTiming;
use fdemon_core::prelude::*;

use super::client::VmRequestHandle;
use super::protocol::StreamEvent;

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

/// Parse a `Flutter.Frame` Extension event into a [`FrameTiming`].
///
/// These events are posted by Flutter on the Extension stream with
/// `extensionKind == "Flutter.Frame"`. The `extensionData` contains
/// timing information in string-encoded microsecond values.
///
/// Returns `None` if the event is not a `Flutter.Frame` event or
/// if the data cannot be parsed.
pub fn parse_frame_timing(event: &StreamEvent) -> Option<FrameTiming> {
    // Must be an Extension event with extensionKind == "Flutter.Frame"
    if event.kind != "Extension" {
        return None;
    }

    let extension_kind = event.data.get("extensionKind").and_then(|v| v.as_str())?;

    if extension_kind != "Flutter.Frame" {
        return None;
    }

    let ext_data = event.data.get("extensionData")?;

    // Parse string-encoded numeric values
    let number = parse_str_u64(ext_data.get("number")?)?;
    let elapsed = parse_str_u64(ext_data.get("elapsed")?)?;
    let build = parse_str_u64(ext_data.get("build")?)?;
    let raster = parse_str_u64(ext_data.get("raster")?)?;

    Some(FrameTiming {
        number,
        build_micros: build,
        raster_micros: raster,
        elapsed_micros: elapsed,
        timestamp: chrono::Local::now(),
    })
}

/// Identify the kind of Flutter Extension event.
///
/// Flutter posts several kinds of Extension events via `developer.postEvent()`:
/// - `Flutter.Frame` — Frame timing data
/// - `Flutter.Error` — Structured errors (already handled in errors.rs)
/// - `Flutter.Navigation` — Route navigation events
/// - `Flutter.ServiceExtensionStateChanged` — Extension state changes
///
/// This function returns the extension kind string for classification.
pub fn flutter_extension_kind(event: &StreamEvent) -> Option<&str> {
    if event.kind != "Extension" {
        return None;
    }
    event.data.get("extensionKind").and_then(|v| v.as_str())
}

/// Check if a stream event is a Flutter.Frame event.
pub fn is_frame_event(event: &StreamEvent) -> bool {
    flutter_extension_kind(event) == Some("Flutter.Frame")
}

/// Parse a JSON value that may contain a u64 either as a string or as a
/// JSON number.
///
/// Flutter's Extension event data encodes all numeric values as strings
/// (e.g. `"42"`). This helper handles both string and integer JSON types for
/// defensive parsing.
pub fn parse_str_u64(value: &serde_json::Value) -> Option<u64> {
    value
        .as_str()
        .and_then(|s| s.parse::<u64>().ok())
        .or_else(|| value.as_u64())
}

// ---------------------------------------------------------------------------
// Frame tracking enablement
// ---------------------------------------------------------------------------

/// Enable frame timing event emission.
///
/// Calls `ext.flutter.profileWidgetBuilds` to ensure build timing is tracked.
/// This is a best-effort call — if the extension is unavailable (e.g. in
/// profile mode where debug extensions are disabled), the call fails silently
/// because `Flutter.Frame` events are still emitted by the framework.
///
/// # Errors
///
/// Always returns `Ok(())`. Errors from the VM Service call are logged at
/// `debug` level and then discarded.
pub async fn enable_frame_tracking(handle: &VmRequestHandle, isolate_id: &str) -> Result<()> {
    // Attempt to enable profile widget builds — this is a best-effort call.
    // If the extension isn't available (profile mode), we silently continue
    // because Flutter.Frame events may still arrive.
    let result = handle
        .call_extension(
            "ext.flutter.profileWidgetBuilds",
            isolate_id,
            Some([("enabled".to_string(), "true".to_string())].into()),
        )
        .await;

    if let Err(ref e) = result {
        tracing::debug!("Could not enable profileWidgetBuilds: {e}");
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    use crate::vm_service::protocol::{IsolateRef, StreamEvent};

    fn make_frame_event(number: &str, elapsed: &str, build: &str, raster: &str) -> StreamEvent {
        StreamEvent {
            kind: "Extension".to_string(),
            isolate: Some(IsolateRef {
                id: "isolates/1234".to_string(),
                name: "main".to_string(),
                number: None,
                is_system_isolate: Some(false),
            }),
            timestamp: Some(1704067200000),
            data: json!({
                "extensionKind": "Flutter.Frame",
                "extensionData": {
                    "number": number,
                    "startTime": "1704067200000",
                    "elapsed": elapsed,
                    "build": build,
                    "raster": raster
                }
            }),
        }
    }

    #[test]
    fn test_parse_frame_timing_basic() {
        let event = make_frame_event("42", "12500", "6200", "6300");
        let timing = parse_frame_timing(&event).unwrap();
        assert_eq!(timing.number, 42);
        assert_eq!(timing.elapsed_micros, 12500);
        assert_eq!(timing.build_micros, 6200);
        assert_eq!(timing.raster_micros, 6300);
    }

    #[test]
    fn test_parse_frame_timing_janky() {
        let event = make_frame_event("100", "25000", "12000", "13000");
        let timing = parse_frame_timing(&event).unwrap();
        assert!(timing.is_janky()); // 25ms > 16.667ms
        assert!((timing.elapsed_ms() - 25.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_parse_frame_timing_smooth() {
        let event = make_frame_event("101", "8000", "4000", "4000");
        let timing = parse_frame_timing(&event).unwrap();
        assert!(!timing.is_janky()); // 8ms < 16.667ms
    }

    #[test]
    fn test_parse_frame_timing_not_extension() {
        let event = StreamEvent {
            kind: "GC".to_string(),
            isolate: None,
            timestamp: None,
            data: json!({}),
        };
        assert!(parse_frame_timing(&event).is_none());
    }

    #[test]
    fn test_parse_frame_timing_wrong_extension_kind() {
        let event = StreamEvent {
            kind: "Extension".to_string(),
            isolate: None,
            timestamp: None,
            data: json!({
                "extensionKind": "Flutter.Error",
                "extensionData": {}
            }),
        };
        assert!(parse_frame_timing(&event).is_none());
    }

    #[test]
    fn test_parse_frame_timing_missing_data() {
        let event = StreamEvent {
            kind: "Extension".to_string(),
            isolate: None,
            timestamp: None,
            data: json!({
                "extensionKind": "Flutter.Frame",
                "extensionData": {
                    "number": "1"
                    // missing elapsed, build, raster
                }
            }),
        };
        assert!(parse_frame_timing(&event).is_none());
    }

    #[test]
    fn test_parse_str_u64_string() {
        assert_eq!(parse_str_u64(&json!("42")), Some(42));
    }

    #[test]
    fn test_parse_str_u64_integer() {
        assert_eq!(parse_str_u64(&json!(42)), Some(42));
    }

    #[test]
    fn test_parse_str_u64_invalid() {
        assert_eq!(parse_str_u64(&json!("abc")), None);
        assert_eq!(parse_str_u64(&json!(null)), None);
    }

    #[test]
    fn test_flutter_extension_kind() {
        let frame = make_frame_event("1", "10000", "5000", "5000");
        assert_eq!(flutter_extension_kind(&frame), Some("Flutter.Frame"));

        let non_ext = StreamEvent {
            kind: "GC".to_string(),
            isolate: None,
            timestamp: None,
            data: json!({}),
        };
        assert_eq!(flutter_extension_kind(&non_ext), None);
    }

    #[test]
    fn test_is_frame_event() {
        let frame = make_frame_event("1", "10000", "5000", "5000");
        assert!(is_frame_event(&frame));

        let error = StreamEvent {
            kind: "Extension".to_string(),
            isolate: None,
            timestamp: None,
            data: json!({ "extensionKind": "Flutter.Error" }),
        };
        assert!(!is_frame_event(&error));
    }
}
