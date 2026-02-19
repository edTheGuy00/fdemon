## Task: Frame Timing & Timeline RPCs

**Objective**: Parse frame timing data from Flutter's `Flutter.Frame` Extension events and implement the timeline flag configuration needed to enable frame event emission. This provides the raw frame timing data that Task 06 will aggregate into FPS and jank metrics.

**Depends on**: 01-performance-data-models (for `FrameTiming` type)

**Estimated Time**: 3-4 hours

### Scope

- `crates/fdemon-daemon/src/vm_service/timeline.rs`: **NEW** — Frame timing parsing from Extension events
- `crates/fdemon-daemon/src/vm_service/mod.rs`: Add `pub mod timeline` and re-exports

### Details

#### 1. How Flutter Frame Timing Works

Flutter posts frame timing data in two ways:

**A. `Flutter.Frame` Extension Events (preferred)**

Flutter posts `Flutter.Frame` events via `developer.postEvent()` on the Extension stream. These arrive as Extension-kind events that the existing forwarding loop already receives:

```json
{
    "kind": "Extension",
    "extensionKind": "Flutter.Frame",
    "extensionData": {
        "number": "42",
        "startTime": "1704067200000",
        "elapsed": "12500",
        "build": "6200",
        "raster": "6300"
    },
    "isolate": { "id": "isolates/1234", "name": "main" },
    "timestamp": 1704067200000
}
```

These events are already delivered to `forward_vm_events()` via the Extension stream subscription. Currently they're ignored (only `Flutter.Error` and `Logging` events are processed). This task adds parsing for `Flutter.Frame` events.

**B. Raw Timeline Stream (complex, not used)**

The VM Service Timeline stream emits Chrome Trace Format events at a very low level. Flutter DevTools uses these for the Performance view, but parsing raw trace events to extract frame boundaries is complex and brittle. The `Flutter.Frame` Extension events provide the same data in a much more consumable format.

#### 2. Flutter.Frame Event Parsing

```rust
use fdemon_core::performance::FrameTiming;

/// Parse a `Flutter.Frame` Extension event into a `FrameTiming`.
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

    let extension_kind = event.data.get("extensionKind")
        .and_then(|v| v.as_str())?;

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

/// Parse a JSON value that's a string containing a u64.
fn parse_str_u64(value: &serde_json::Value) -> Option<u64> {
    value.as_str()
        .and_then(|s| s.parse::<u64>().ok())
        .or_else(|| value.as_u64())
}
```

#### 3. Flutter.Navigation and Flutter.ServiceExtensionStateChanged

While parsing Extension events, also detect other useful Flutter extension events that arrive on the same stream. These are informational and can be logged:

```rust
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
    event.data.get("extensionKind")
        .and_then(|v| v.as_str())
}

/// Check if a stream event is a Flutter.Frame event.
pub fn is_frame_event(event: &StreamEvent) -> bool {
    flutter_extension_kind(event) == Some("Flutter.Frame")
}
```

#### 4. Frame Timing Availability

`Flutter.Frame` events are only emitted when the Flutter framework's `SchedulerBinding` is active and frames are being scheduled. Important edge cases:

- **App in background**: No frames are scheduled → no `Flutter.Frame` events
- **App idle (no animation)**: Frames may not be scheduled → sporadic events
- **Debug vs Profile mode**: Available in both, but profile mode has more accurate timing
- **Release mode**: Not available (VM Service not exposed)

The parsing code should be lenient about missing fields and return `None` rather than error.

#### 5. Enable Frame Event Emission (Optional Enhancement)

By default, Flutter emits `Flutter.Frame` events when there's a VM Service client connected. However, if frame events aren't arriving, it may be necessary to enable them explicitly via:

```rust
/// Enable frame timing event emission.
///
/// Calls `ext.flutter.profileWidgetBuilds` to ensure build timing is tracked.
/// This may already be enabled by default when a VM Service client is connected.
pub async fn enable_frame_tracking(
    handle: &VmRequestHandle,
    isolate_id: &str,
) -> Result<()> {
    // Attempt to enable profile widget builds — this is a best-effort call.
    // If the extension isn't available (profile mode), we silently continue
    // because Flutter.Frame events may still arrive.
    let result = handle.call_extension(
        "ext.flutter.profileWidgetBuilds",
        isolate_id,
        Some([("enabled".to_string(), "true".to_string())].into()),
    ).await;

    if let Err(ref e) = result {
        tracing::debug!("Could not enable profileWidgetBuilds: {e}");
    }

    Ok(())
}
```

This is optional — `Flutter.Frame` events typically arrive without explicit enablement. Include it as a best-effort call during connection setup.

### Acceptance Criteria

1. `parse_frame_timing()` correctly parses `Flutter.Frame` Extension events
2. `parse_frame_timing()` handles string-encoded numeric values (`"42"` → `42`)
3. `parse_frame_timing()` returns `None` for non-Frame events
4. `parse_frame_timing()` returns `None` for malformed data (missing fields)
5. `flutter_extension_kind()` correctly extracts the extension kind string
6. `is_frame_event()` returns `true` only for `Flutter.Frame` events
7. `parse_str_u64()` handles both string and integer JSON values
8. `enable_frame_tracking()` makes a best-effort extension call without failing
9. Module re-exported from `fdemon_daemon::vm_service`

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

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
```

### Notes

- **`Flutter.Frame` events arrive on the Extension stream** which is already subscribed in Phase 1. No new stream subscription is needed — only new parsing logic in the event dispatch.
- **String-encoded numbers** are a Flutter convention for Extension event data. All values in `extensionData` are strings (even numbers). The `parse_str_u64` helper handles both string and numeric JSON to be defensive.
- **This approach is simpler than raw Timeline parsing.** Flutter DevTools' performance view uses the raw Timeline stream for microsecond-precision trace events, but `Flutter.Frame` events provide the same build/raster breakdown at the frame level, which is sufficient for a TUI tool.
- **Frame events may not be emitted during app idle.** Task 06 must handle gaps in frame data gracefully (e.g., show "no frames" or stale FPS).
- **`enable_frame_tracking()` is best-effort.** `ext.flutter.profileWidgetBuilds` is a debug-only extension. In profile mode it may not be available, but `Flutter.Frame` events are still emitted. The function should not fail if the extension is unavailable.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-daemon/src/vm_service/timeline.rs` | NEW — `parse_frame_timing()`, `flutter_extension_kind()`, `is_frame_event()`, `parse_str_u64()`, `enable_frame_tracking()` with inline tests |
| `crates/fdemon-daemon/src/vm_service/mod.rs` | Added `pub mod timeline;` declaration and re-exports of all 5 public items |

### Notable Decisions/Tradeoffs

1. **`parse_str_u64` visibility**: Made `pub` (not `pub(crate)`) so it is accessible from the `mod.rs` re-export and can be used by downstream crates (e.g. Task 06 aggregation). The task file specifies it in re-exports.
2. **`enable_frame_tracking` error handling**: Always returns `Ok(())` — errors from `call_extension` are logged at `debug` level and then discarded, matching the best-effort semantics described in the task.
3. **`chrono::Local::now()`** used for `FrameTiming::timestamp` (not derived from the event's `startTime` field) — consistent with how `FrameTiming` is defined in `fdemon-core` and matching the task code exactly. The `startTime` field is present in `extensionData` but not extracted; Task 06 can use the `timestamp` field for ordering.

### Testing Performed

- `cargo fmt --all` — Passed (formatter made minor whitespace cleanup)
- `cargo check -p fdemon-daemon` — Passed
- `cargo test -p fdemon-daemon` — Passed (327 unit tests: 0 failed, 3 ignored; 11 new timeline tests all pass)
- `cargo clippy -p fdemon-daemon -- -D warnings` — Passed (0 warnings)

### Risks/Limitations

1. **No `startTime` extraction**: The `extensionData.startTime` field is present in the Flutter event but not captured in `FrameTiming`. If precise wall-clock frame start times are needed later, the parsing will need to be extended.
2. **`enable_frame_tracking` not tested end-to-end**: The function requires a live VM Service connection and cannot be unit-tested in isolation. It is covered structurally (the async function signature matches the task spec) but there are no integration tests for it here.
