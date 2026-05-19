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
use super::request_api::VmRequestApi;

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

    // Detect shader compilation from event data when available.
    // Some Flutter versions expose a `shaderCompilation` boolean field.
    // Defaults to false when the field is absent or not a boolean.
    let shader_compilation = ext_data
        .get("shaderCompilation")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    Some(FrameTiming {
        number,
        build_micros: build,
        raster_micros: raster,
        elapsed_micros: elapsed,
        timestamp: chrono::Local::now(),
        // Phase breakdown requires timeline event data (deferred to a future task).
        phases: None,
        shader_compilation,
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
            crate::vm_service::extensions::ext::PROFILE_WIDGET_BUILDS,
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
// VM Timeline RPCs
// ---------------------------------------------------------------------------

use fdemon_core::timeline::{parse_vm_timeline, TimelineEvent};
use serde_json::json;
use std::collections::HashMap;

/// Wrap the VM Service `getVMTimelineMicros` RPC. Returns the current VM
/// timeline clock value in microseconds.
///
/// This is a raw VM Service method, not a Flutter extension. It requires no
/// `isolateId` parameter.
///
/// # Errors
///
/// Returns [`Error::Protocol`] if the response does not contain a `timestamp`
/// field. Returns [`Error::ChannelClosed`] if the background WebSocket task
/// has exited.
pub async fn get_vm_timeline_micros<H: VmRequestApi>(handle: &H) -> Result<u64> {
    let response = handle.request("getVMTimelineMicros", None).await?;
    let ts = response
        .get("timestamp")
        .and_then(|v| v.as_i64())
        .ok_or_else(|| Error::protocol("getVMTimelineMicros response missing timestamp"))?;
    // Clamp negative values to 0 (defensive; the VM should never send a
    // negative timestamp, but as_i64() accepts them).
    Ok(ts.max(0) as u64)
}

/// Fetch a slice of the VM timeline (`getVMTimeline`) covering the window
/// `[since_micros, since_micros + extent_micros)` and return it as a vector
/// of parsed [`TimelineEvent`]s with thread classification applied.
///
/// `thread_name_map` is the caller's persistent `tid → thread name` cache —
/// it is updated in place as metadata events arrive. Pass a fresh
/// `HashMap::new()` on the very first call; reuse for subsequent calls so
/// that thread-name associations from earlier responses are available for
/// later ones.
///
/// Returns an empty vec if the VM had no events in the window.
///
/// # Cast safety
///
/// `timeOriginMicros` and `timeExtentMicros` use `i64` per the VM Service
/// protocol. Values are clamped to `i64::MAX` before the cast — sub-`i64::MAX`
/// inputs round-trip cleanly; pathological values are silently clamped to the
/// maximum, which the VM Service will reject as a window-too-large error and
/// the polling loop will recover on the next tick.
///
/// # Errors
///
/// Returns [`Error::Protocol`] if the response is missing the `traceEvents`
/// field or contains malformed events. Returns [`Error::ChannelClosed`] if
/// the background WebSocket task has exited.
pub async fn fetch_timeline_chunk<H: VmRequestApi>(
    handle: &H,
    since_micros: u64,
    extent_micros: u64,
    thread_name_map: &mut HashMap<i64, String>,
) -> Result<Vec<TimelineEvent>> {
    // VM Service protocol uses i64 for these parameters.
    // Clamp to i64::MAX before casting to avoid undefined behaviour on
    // pathological values (see doc comment above).
    const I64_MAX_AS_U64: u64 = i64::MAX as u64;
    let params = json!({
        "timeOriginMicros": since_micros.min(I64_MAX_AS_U64) as i64,
        "timeExtentMicros": extent_micros.min(I64_MAX_AS_U64) as i64,
    });
    let response = handle.request("getVMTimeline", Some(params)).await?;
    parse_vm_timeline(&response, thread_name_map)
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

    /// Build an Extension stream event with arbitrary extensionData payload.
    fn make_extension_event(
        extension_kind: &str,
        extension_data: serde_json::Value,
    ) -> StreamEvent {
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
                "extensionKind": extension_kind,
                "extensionData": extension_data,
            }),
        }
    }

    #[test]
    fn test_parse_frame_timing_with_shader_compilation() {
        let event = make_extension_event(
            "Flutter.Frame",
            json!({
                "number": "42",
                "elapsed": "20000",
                "build": "5000",
                "raster": "15000",
                "shaderCompilation": true,
            }),
        );
        let timing = parse_frame_timing(&event).unwrap();
        assert!(timing.shader_compilation);
        assert!(timing.phases.is_none());
    }

    #[test]
    fn test_parse_frame_timing_without_shader_field_defaults_false() {
        let event = make_extension_event(
            "Flutter.Frame",
            json!({
                "number": "1",
                "elapsed": "10000",
                "build": "5000",
                "raster": "5000",
            }),
        );
        let timing = parse_frame_timing(&event).unwrap();
        assert!(!timing.shader_compilation);
    }

    #[test]
    fn test_parse_frame_timing_new_fields_populated() {
        let event = make_extension_event(
            "Flutter.Frame",
            json!({
                "number": "1",
                "elapsed": "10000",
                "build": "5000",
                "raster": "5000",
            }),
        );
        let timing = parse_frame_timing(&event).unwrap();
        assert_eq!(timing.phases, None);
        assert!(!timing.shader_compilation);
    }

    #[test]
    fn test_parse_frame_timing_shader_compilation_false_value() {
        // Explicit false value should also work correctly.
        let event = make_extension_event(
            "Flutter.Frame",
            json!({
                "number": "5",
                "elapsed": "8000",
                "build": "4000",
                "raster": "4000",
                "shaderCompilation": false,
            }),
        );
        let timing = parse_frame_timing(&event).unwrap();
        assert!(!timing.shader_compilation);
    }

    // ── get_vm_timeline_micros ────────────────────────────────────────────────

    /// Build a VmRequestHandle wired to a live channel where the test acts as
    /// the fake VM responder. Returns the handle and the command receiver.
    ///
    /// The returned handle's `request()` method sends a
    /// `ClientCommand::SendRequest` to the receiver. The test reads the
    /// command, inspects the method/params, and sends a response via the
    /// embedded oneshot sender.
    fn make_mock_handle() -> (
        super::super::client::VmRequestHandle,
        tokio::sync::mpsc::Receiver<crate::vm_service::client::ClientCommand>,
    ) {
        super::super::client::VmRequestHandle::new_with_test_channel()
    }

    #[tokio::test]
    async fn get_vm_timeline_micros_parses_timestamp() {
        use crate::vm_service::client::ClientCommand;

        let (handle, mut cmd_rx) = make_mock_handle();

        // Spawn a fake responder that answers getVMTimelineMicros.
        tokio::spawn(async move {
            if let Some(ClientCommand::SendRequest { response_tx, .. }) = cmd_rx.recv().await {
                let _ = response_tx.send(Ok(json!({
                    "type": "Timestamp",
                    "timestamp": 12345_i64
                })));
            }
        });

        let result = get_vm_timeline_micros(&handle).await;
        assert_eq!(result.unwrap(), 12345u64);
    }

    #[tokio::test]
    async fn get_vm_timeline_micros_missing_timestamp_errors() {
        use crate::vm_service::client::ClientCommand;

        let (handle, mut cmd_rx) = make_mock_handle();

        tokio::spawn(async move {
            if let Some(ClientCommand::SendRequest { response_tx, .. }) = cmd_rx.recv().await {
                // Response is missing the "timestamp" field.
                let _ = response_tx.send(Ok(json!({ "type": "Timestamp" })));
            }
        });

        let result = get_vm_timeline_micros(&handle).await;
        assert!(result.is_err(), "missing timestamp should produce an error");
        assert!(
            matches!(
                result.unwrap_err(),
                fdemon_core::error::Error::Protocol { .. }
            ),
            "expected Protocol error"
        );
    }

    #[tokio::test]
    async fn get_vm_timeline_micros_negative_clamped_to_zero() {
        use crate::vm_service::client::ClientCommand;

        let (handle, mut cmd_rx) = make_mock_handle();

        tokio::spawn(async move {
            if let Some(ClientCommand::SendRequest { response_tx, .. }) = cmd_rx.recv().await {
                let _ = response_tx.send(Ok(json!({ "type": "Timestamp", "timestamp": -1_i64 })));
            }
        });

        let result = get_vm_timeline_micros(&handle).await;
        assert_eq!(
            result.unwrap(),
            0u64,
            "negative timestamp should clamp to 0"
        );
    }

    // ── fetch_timeline_chunk ──────────────────────────────────────────────────

    #[tokio::test]
    async fn fetch_timeline_chunk_sends_correct_method() {
        use crate::vm_service::client::ClientCommand;

        let (handle, mut cmd_rx) = make_mock_handle();

        // Capture the method name then respond with an empty timeline.
        let responder = tokio::spawn(async move {
            if let Some(ClientCommand::SendRequest {
                method,
                response_tx,
                ..
            }) = cmd_rx.recv().await
            {
                assert_eq!(method, "getVMTimeline");
                let _ = response_tx.send(Ok(json!({ "type": "Timeline", "traceEvents": [] })));
            }
        });

        let mut map = HashMap::new();
        let _ = fetch_timeline_chunk(&handle, 0, 50_000, &mut map).await;
        let _ = responder.await;
    }

    #[tokio::test]
    async fn fetch_timeline_chunk_sends_origin_and_extent() {
        use crate::vm_service::client::ClientCommand;

        let (handle, mut cmd_rx) = make_mock_handle();

        let responder = tokio::spawn(async move {
            if let Some(ClientCommand::SendRequest {
                params,
                response_tx,
                ..
            }) = cmd_rx.recv().await
            {
                let p = params.expect("params must be present");
                assert_eq!(
                    p.get("timeOriginMicros").and_then(|v| v.as_i64()),
                    Some(12_345_000_i64),
                    "timeOriginMicros must match since_micros cast to i64"
                );
                assert_eq!(
                    p.get("timeExtentMicros").and_then(|v| v.as_i64()),
                    Some(50_000_i64),
                    "timeExtentMicros must match extent_micros cast to i64"
                );
                let _ = response_tx.send(Ok(json!({ "type": "Timeline", "traceEvents": [] })));
            }
        });

        let mut map = HashMap::new();
        let _ = fetch_timeline_chunk(&handle, 12_345_000, 50_000, &mut map).await;
        let _ = responder.await;
    }

    #[tokio::test]
    async fn fetch_timeline_chunk_parses_empty_response() {
        use crate::vm_service::client::ClientCommand;

        let (handle, mut cmd_rx) = make_mock_handle();

        tokio::spawn(async move {
            if let Some(ClientCommand::SendRequest { response_tx, .. }) = cmd_rx.recv().await {
                let _ = response_tx.send(Ok(json!({
                    "type": "Timeline",
                    "traceEvents": [],
                    "timeOriginMicros": 0_i64,
                    "timeExtentMicros": 50_000_i64
                })));
            }
        });

        let mut map = HashMap::new();
        let events = fetch_timeline_chunk(&handle, 0, 50_000, &mut map)
            .await
            .expect("should succeed");
        assert!(
            events.is_empty(),
            "empty traceEvents should produce empty vec"
        );
    }

    #[tokio::test]
    async fn fetch_timeline_chunk_parses_metadata_and_ui_event_fixture() {
        use crate::vm_service::client::ClientCommand;
        use fdemon_core::timeline::{TimelinePhase, TimelineThread};

        let (handle, mut cmd_rx) = make_mock_handle();

        // Fixture: metadata for UI (tid=1) + Raster (tid=2) threads, then
        // a Frame event (UI), a Raster event (Raster), and a GC event (Other).
        tokio::spawn(async move {
            if let Some(ClientCommand::SendRequest { response_tx, .. }) = cmd_rx.recv().await {
                let _ = response_tx.send(Ok(json!({
                    "type": "Timeline",
                    "traceEvents": [
                        // Metadata: UI thread
                        { "ph": "M", "name": "thread_name", "pid": 1, "tid": 1,
                          "args": { "name": "io.flutter.1.ui (1)" } },
                        // Metadata: Raster thread
                        { "ph": "M", "name": "thread_name", "pid": 1, "tid": 2,
                          "args": { "name": "io.flutter.1.raster (2)" } },
                        // Frame event on UI thread (Complete, with frame_number)
                        { "ph": "X", "name": "Frame", "cat": "Embedder",
                          "pid": 1, "tid": 1, "ts": 12_350_000_u64, "dur": 8_000_u64,
                          "args": { "frame_number": "42" } },
                        // Raster event on Raster thread
                        { "ph": "X", "name": "Raster", "cat": "Embedder",
                          "pid": 1, "tid": 2, "ts": 12_355_000_u64, "dur": 5_000_u64 },
                        // GC event on a different (Other) thread
                        { "ph": "X", "name": "GC", "cat": "Dart",
                          "pid": 1, "tid": 99, "ts": 12_360_000_u64, "dur": 1_000_u64 }
                    ]
                })));
            }
        });

        let mut map = HashMap::new();
        let events = fetch_timeline_chunk(&handle, 12_345_000, 50_000, &mut map)
            .await
            .expect("should parse successfully");

        // 3 non-metadata events returned; metadata excluded.
        assert_eq!(events.len(), 3, "metadata events must not be in the output");

        // Frame event: UI thread, Complete phase, frame_number = 42.
        assert_eq!(events[0].name, "Frame");
        assert_eq!(events[0].thread, TimelineThread::Ui);
        assert_eq!(events[0].phase, TimelinePhase::Complete);
        assert_eq!(events[0].frame_number, Some(42));
        assert_eq!(events[0].ts, 12_350_000);
        assert_eq!(events[0].dur, Some(8_000));

        // Raster event: Raster thread.
        assert_eq!(events[1].name, "Raster");
        assert_eq!(events[1].thread, TimelineThread::Raster);

        // GC event: Other thread (tid 99 has no metadata).
        assert_eq!(events[2].name, "GC");
        assert_eq!(events[2].thread, TimelineThread::Other);

        // thread_name_map must be populated with the two metadata entries.
        assert_eq!(
            map.get(&1).map(|s| s.as_str()),
            Some("io.flutter.1.ui (1)"),
            "UI thread name must be in thread_name_map"
        );
        assert_eq!(
            map.get(&2).map(|s| s.as_str()),
            Some("io.flutter.1.raster (2)"),
            "Raster thread name must be in thread_name_map"
        );
    }

    #[tokio::test]
    async fn fetch_timeline_chunk_accumulates_thread_name_map_across_calls() {
        use crate::vm_service::client::ClientCommand;
        use fdemon_core::timeline::TimelineThread;

        let (handle, mut cmd_rx) = make_mock_handle();

        // First response: metadata for tid=1 (UI) + one event on tid=1.
        // Second response: one event on tid=2 (Raster) — no metadata this time.
        tokio::spawn(async move {
            // First call
            if let Some(ClientCommand::SendRequest { response_tx, .. }) = cmd_rx.recv().await {
                let _ = response_tx.send(Ok(json!({
                    "type": "Timeline",
                    "traceEvents": [
                        { "ph": "M", "name": "thread_name", "pid": 1, "tid": 1,
                          "args": { "name": "io.flutter.1.ui (1)" } },
                        { "ph": "M", "name": "thread_name", "pid": 1, "tid": 2,
                          "args": { "name": "io.flutter.1.raster (2)" } },
                        { "ph": "X", "name": "Build", "cat": "Dart",
                          "pid": 1, "tid": 1, "ts": 1000_u64, "dur": 100_u64 }
                    ]
                })));
            }
            // Second call — no metadata events; event on tid=2.
            if let Some(ClientCommand::SendRequest { response_tx, .. }) = cmd_rx.recv().await {
                let _ = response_tx.send(Ok(json!({
                    "type": "Timeline",
                    "traceEvents": [
                        { "ph": "X", "name": "Raster", "cat": "Embedder",
                          "pid": 1, "tid": 2, "ts": 2000_u64, "dur": 200_u64 }
                    ]
                })));
            }
        });

        let mut map = HashMap::new();

        // First call populates map with tid→name entries.
        let first = fetch_timeline_chunk(&handle, 0, 1_000, &mut map)
            .await
            .expect("first call should succeed");
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].thread, TimelineThread::Ui);

        // Second call: no metadata in response, but map carries over tid=2.
        let second = fetch_timeline_chunk(&handle, 1_000, 1_000, &mut map)
            .await
            .expect("second call should succeed");
        assert_eq!(second.len(), 1);
        assert_eq!(
            second[0].thread,
            TimelineThread::Raster,
            "raster thread must be classified correctly using map from first call"
        );
    }

    #[tokio::test]
    async fn fetch_timeline_chunk_propagates_request_errors() {
        use crate::vm_service::client::ClientCommand;

        let (handle, mut cmd_rx) = make_mock_handle();

        // Responder sends back an Err, simulating a VM Service error.
        tokio::spawn(async move {
            if let Some(ClientCommand::SendRequest { response_tx, .. }) = cmd_rx.recv().await {
                let _ = response_tx.send(Err(fdemon_core::error::Error::vm_service(
                    "simulated VM service error",
                )));
            }
        });

        let mut map = HashMap::new();
        let result = fetch_timeline_chunk(&handle, 0, 50_000, &mut map).await;
        assert!(
            result.is_err(),
            "errors from the transport must be propagated"
        );
    }
}
