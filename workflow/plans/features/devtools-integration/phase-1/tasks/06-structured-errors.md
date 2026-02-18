## Task: Parse Flutter.Error Extension Events (Crash Log Fix)

**Objective**: Parse `Flutter.Error` events from the VM Service Extension stream and convert them to `LogEntry` items. **This is the primary fix for invisible widget crash logs** — errors that Flutter redirects away from stdout/stderr will now be captured directly.

**Depends on**: 02-capture-ws-uri, 05-vm-introspection

**Estimated Time**: 4-5 hours

### Scope

- **NEW** `crates/fdemon-daemon/src/vm_service/errors.rs` — Flutter.Error event parsing
- `crates/fdemon-daemon/src/vm_service/mod.rs` — Export error types

### Details

#### Background

When Flutter runs in `--machine` mode, `ext.flutter.inspector.structuredErrors` is enabled by default. This means `FlutterError.presentError` is replaced with `_reportStructuredError`, which posts errors via `developer.postEvent('Flutter.Error', errorJson)` to the VM Service Extension stream. **Errors never reach stdout/stderr**.

The Extension stream event looks like:

```json
{
  "jsonrpc": "2.0",
  "method": "streamNotify",
  "params": {
    "streamId": "Extension",
    "event": {
      "kind": "Extension",
      "extensionKind": "Flutter.Error",
      "extensionData": {
        "description": "A RenderFlex overflowed by 42 pixels on the right.",
        "renderedErrorText": "══╡ EXCEPTION CAUGHT BY RENDERING LIBRARY ╞══\n...",
        "errorsSinceReload": 1,
        "library": "rendering library",
        "stackTrace": "...",
        "diagnostics": [...]
      },
      "isolate": { "id": "isolates/1", "name": "main" },
      "timestamp": 1704067200000
    }
  }
}
```

#### Flutter Error Event Types

```rust
/// Parsed Flutter.Error event from VM Service Extension stream
#[derive(Debug, Clone)]
pub struct FlutterErrorEvent {
    /// Number of errors since last reload (1 = first error)
    pub errors_since_reload: i32,
    /// Full rendered error text (for first error only, None for subsequent)
    pub rendered_error_text: Option<String>,
    /// Short error description/summary
    pub description: String,
    /// Library where error occurred (e.g., "rendering library", "widgets library")
    pub library: Option<String>,
    /// Raw stack trace string
    pub stack_trace: Option<String>,
    /// Event timestamp from VM Service
    pub timestamp: Option<i64>,
}
```

#### Parsing Function

```rust
/// Check if a VM Service event is a Flutter.Error and parse it
pub fn parse_flutter_error(event: &StreamEvent) -> Option<FlutterErrorEvent> {
    // 1. Check kind == "Extension" and extensionKind == "Flutter.Error"
    // 2. Extract extensionData object
    // 3. Parse fields from extensionData
    // 4. Return FlutterErrorEvent
}
```

#### Conversion to LogEntry

```rust
/// Convert a FlutterErrorEvent to a LogEntry for display in the log view
pub fn flutter_error_to_log_entry(error: &FlutterErrorEvent) -> LogEntry {
    // Use rendered_error_text for first error (contains full exception block)
    // Use description for subsequent errors (shorter summary)
    let message = if let Some(ref rendered) = error.rendered_error_text {
        rendered.clone()
    } else {
        let prefix = error.library.as_deref().unwrap_or("Flutter");
        format!("[{}] {}", prefix, error.description)
    };

    LogEntry {
        level: LogLevel::Error,
        source: LogSource::VmService,
        message,
        timestamp: /* convert from error.timestamp or use now */,
        stack_trace: error.stack_trace.as_ref()
            .and_then(|st| ParsedStackTrace::parse(st)),
        // ... other fields
    }
}
```

#### Integration Point

This module provides parsing functions. The actual event routing happens in Task 08 (session integration), where the background VM Service event loop calls `parse_flutter_error()` on Extension events and sends resulting `LogEntry` items via the message channel.

### Acceptance Criteria

1. `parse_flutter_error()` correctly parses real Flutter.Error extension events
2. `flutter_error_to_log_entry()` produces a `LogEntry` with:
   - `level: LogLevel::Error`
   - `source: LogSource::VmService`
   - Full rendered error text for first error
   - Short description for subsequent errors
   - Parsed stack trace (via existing `ParsedStackTrace::parse()`)
3. Events with `extensionKind != "Flutter.Error"` return `None`
4. Malformed events are handled gracefully (no panics)
5. Comprehensive test coverage with real Flutter error JSON

### Testing

```rust
#[cfg(test)]
mod tests {
    #[test]
    fn test_parse_flutter_error_first_error_with_rendered_text() {
        let json = r#"{
            "kind": "Extension",
            "extensionKind": "Flutter.Error",
            "extensionData": {
                "description": "A RenderFlex overflowed by 42 pixels on the right.",
                "renderedErrorText": "══╡ EXCEPTION CAUGHT BY RENDERING LIBRARY ╞══\nThe following assertion was thrown during layout:\nA RenderFlex overflowed...",
                "errorsSinceReload": 1,
                "library": "rendering library",
                "stackTrace": "#0 RenderFlex.performLayout (...)"
            }
        }"#;
        // Parse and verify all fields
    }

    #[test]
    fn test_parse_flutter_error_subsequent_error_no_rendered_text() {
        // errorsSinceReload > 1, renderedErrorText is null
        // Should use description as message
    }

    #[test]
    fn test_parse_non_flutter_error_extension_returns_none() {
        // extensionKind = "Flutter.FirstFrame" should return None
    }

    #[test]
    fn test_flutter_error_to_log_entry_has_correct_level_and_source() {
        // Verify LogLevel::Error and LogSource::VmService
    }

    #[test]
    fn test_flutter_error_to_log_entry_parses_stack_trace() {
        // Verify stack trace is parsed via ParsedStackTrace::parse()
    }

    #[test]
    fn test_parse_malformed_extension_data_returns_none() {
        // Missing required fields
    }
}
```

### Notes

- The `rendered_error_text` field contains the full multi-line exception block that would normally appear on the console. It's only present for the first error after each reload (`errorsSinceReload == 1`)
- Subsequent errors only have `description` (to avoid flooding the Extension stream)
- The existing `ExceptionBlockParser` in `fdemon-core` is NOT modified — it remains as a fallback for when VM Service is unavailable
- The `diagnostics` field contains a `DiagnosticsNode` tree for rich error display — defer parsing this to a future enhancement (listed in PLAN.md Future Enhancements)
- Stack trace format from VM Service may differ slightly from stdout format — test with real data

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-daemon/src/vm_service/errors.rs` | New file: `FlutterErrorEvent`, `parse_flutter_error()`, `flutter_error_to_log_entry()`, 22 unit tests |
| `crates/fdemon-daemon/src/vm_service/mod.rs` | Added `pub mod errors;` and re-exports for `FlutterErrorEvent`, `parse_flutter_error`, `flutter_error_to_log_entry` |

### Notable Decisions/Tradeoffs

1. **Empty `extensionData` guard**: `parse_flutter_error` returns `None` when `extensionData` is absent entirely (the outer field is missing from the flattened `data` Value). This is the expected behavior for malformed events.

2. **Empty string filter for optional fields**: `rendered_error_text`, `library`, and `stack_trace` are filtered with `.filter(|s| !s.is_empty())` so that empty JSON strings (`""`) are coerced to `None`. This avoids downstream code having to handle empty-string optionals.

3. **Stack trace filtering**: `flutter_error_to_log_entry` filters out `ParsedStackTrace` instances that have zero frames after parsing. This means truly unparseable stack trace strings result in no `stack_trace` field on the `LogEntry`, keeping it clean. `LogEntry::with_stack_trace` is only called when frames exist.

4. **`description` defaults to empty string**: When `description` is absent from `extensionData`, it defaults to `""` rather than returning `None`. This matches the task spec (description is the key field, malformed events are handled gracefully without panics).

5. **Timestamp propagation**: The `timestamp` field comes from the top-level `StreamEvent.timestamp`, not from `extensionData`, matching the VM Service protocol structure.

### Testing Performed

- `cargo check --workspace` — PASS
- `cargo test -p fdemon-daemon` — PASS (202 tests, 0 failures, 3 ignored integration tests)
  - 22 new tests in `vm_service::errors::tests`
- `cargo clippy --workspace -- -D warnings` — PASS (no warnings)

### Risks/Limitations

1. **Stack trace format variance**: The stack trace in `extensionData.stackTrace` from VM Service may differ slightly from stdout format. The existing `ParsedStackTrace::parse()` handles both Dart VM and friendly formats, and gracefully returns zero frames for unrecognized formats.

2. **Future diagnostics field**: The `diagnostics` field in `extensionData` contains a `DiagnosticsNode` tree for rich error display; it is intentionally not parsed in this task (deferred to future enhancement per task Notes).
