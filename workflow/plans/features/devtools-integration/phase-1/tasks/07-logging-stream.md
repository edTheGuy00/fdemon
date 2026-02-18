## Task: Parse VM Service Logging Stream Events

**Objective**: Parse `LogRecord` events from the VM Service Logging stream and convert them to `LogEntry` items with accurate log levels. This enables hybrid logging where apps using `dart:developer log()` get perfect level detection.

**Depends on**: 02-capture-ws-uri, 05-vm-introspection

**Estimated Time**: 3-4 hours

### Scope

- **NEW** `crates/fdemon-daemon/src/vm_service/logging.rs` — Logging stream handler
- `crates/fdemon-daemon/src/vm_service/mod.rs` — Export logging types

### Details

#### Background

The VM Service `Logging` stream emits structured `LogRecord` events for logs created via `dart:developer log()`. Apps using the `logging` package (which wraps `dart:developer`) also produce these events. Apps using `print()`, `Logger`, or `Talker` do NOT — those continue through the daemon's stdout.

The Logging stream event looks like:

```json
{
  "jsonrpc": "2.0",
  "method": "streamNotify",
  "params": {
    "streamId": "Logging",
    "event": {
      "kind": "Logging",
      "logRecord": {
        "message": {"type": "@Instance", "valueAsString": "User logged in"},
        "level": 800,
        "loggerName": {"type": "@Instance", "valueAsString": "AuthService"},
        "time": 1704067200000,
        "sequenceNumber": 42,
        "error": {"type": "@Instance", "valueAsString": null},
        "stackTrace": {"type": "@Instance", "valueAsString": null}
      },
      "isolate": {"id": "isolates/1", "name": "main"},
      "timestamp": 1704067200000
    }
  }
}
```

Note: LogRecord fields are `InstanceRef` objects with `valueAsString`, not plain strings.

#### VM Log Record Types

```rust
/// Parsed VM Service LogRecord event
#[derive(Debug, Clone)]
pub struct VmLogRecord {
    /// Log message text
    pub message: String,
    /// Log level (300-1200 range, maps to dart:developer levels)
    pub level: i32,
    /// Logger name (e.g., "AuthService", "HttpClient")
    pub logger_name: Option<String>,
    /// Timestamp in milliseconds since epoch
    pub time: i64,
    /// Sequence number for ordering
    pub sequence_number: i64,
    /// Error message if present
    pub error: Option<String>,
    /// Stack trace string if present
    pub stack_trace: Option<String>,
}
```

#### Level Mapping

```rust
/// Map VM Service log level to fdemon LogLevel.
/// VM levels follow the dart:developer convention.
pub fn vm_level_to_log_level(level: i32) -> LogLevel {
    match level {
        ..=499 => LogLevel::Debug,      // FINEST (300), FINER (400)
        500..=699 => LogLevel::Debug,   // FINE (500), CONFIG (700) — still debug-ish
        700..=799 => LogLevel::Debug,   // CONFIG
        800..=899 => LogLevel::Info,    // INFO (800)
        900..=999 => LogLevel::Warning, // WARNING (900)
        1000.. => LogLevel::Error,      // SEVERE (1000), SHOUT (1200)
    }
}
```

#### Parsing Function

```rust
/// Check if a VM Service event is a Logging event and parse the LogRecord
pub fn parse_log_record(event: &StreamEvent) -> Option<VmLogRecord> {
    if event.kind != "Logging" {
        return None;
    }

    // Extract logRecord from event data
    // Parse InstanceRef fields (extract valueAsString from each)
    // Handle null/missing fields gracefully
}

/// Extract valueAsString from a VM Service InstanceRef object
fn extract_value_as_string(value: &serde_json::Value) -> Option<String> {
    value.get("valueAsString")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}
```

#### Conversion to LogEntry

```rust
/// Convert a VmLogRecord to a LogEntry for display in the log view
pub fn vm_log_to_log_entry(record: &VmLogRecord) -> LogEntry {
    let level = vm_level_to_log_level(record.level);

    // Prefix with logger name if present
    let message = match &record.logger_name {
        Some(name) if !name.is_empty() => format!("[{}] {}", name, record.message),
        _ => record.message.clone(),
    };

    LogEntry {
        level,
        source: LogSource::VmService,
        message,
        timestamp: /* convert from record.time millis */,
        stack_trace: record.stack_trace.as_ref()
            .and_then(|st| ParsedStackTrace::parse(st)),
        // ... other fields
    }
}
```

### Acceptance Criteria

1. `parse_log_record()` correctly parses real VM Service Logging events
2. `InstanceRef` fields (`valueAsString`) are correctly extracted
3. `vm_level_to_log_level()` maps all dart:developer levels correctly:
   - FINEST (300) → Debug
   - INFO (800) → Info
   - WARNING (900) → Warning
   - SEVERE (1000) → Error
   - SHOUT (1200) → Error
4. `vm_log_to_log_entry()` produces a `LogEntry` with:
   - Correct `LogLevel` from VM level
   - `LogSource::VmService`
   - Logger name prefixed if present
   - Parsed stack trace if present
5. Events with `kind != "Logging"` return `None`
6. Malformed events are handled gracefully (no panics)
7. Comprehensive test coverage

### Testing

```rust
#[cfg(test)]
mod tests {
    #[test]
    fn test_vm_level_to_log_level_info() {
        assert_eq!(vm_level_to_log_level(800), LogLevel::Info);
    }

    #[test]
    fn test_vm_level_to_log_level_warning() {
        assert_eq!(vm_level_to_log_level(900), LogLevel::Warning);
    }

    #[test]
    fn test_vm_level_to_log_level_severe() {
        assert_eq!(vm_level_to_log_level(1000), LogLevel::Error);
    }

    #[test]
    fn test_vm_level_to_log_level_finest_is_debug() {
        assert_eq!(vm_level_to_log_level(300), LogLevel::Debug);
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
    }

    #[test]
    fn test_parse_log_record_without_logger_name() {
        // loggerName valueAsString is null
    }

    #[test]
    fn test_vm_log_to_log_entry_prefixes_logger_name() {
        // Logger name "AuthService" → message "[AuthService] User logged in"
    }

    #[test]
    fn test_parse_non_logging_event_returns_none() {
        // kind = "Extension" should return None
    }

    #[test]
    fn test_extract_value_as_string_null_returns_none() {
        // {"type": "@Instance", "valueAsString": null} → None
    }

    #[test]
    fn test_vm_log_with_error_and_stack_trace() {
        // LogRecord with error and stackTrace fields populated
    }
}
```

### Notes

- The `InstanceRef` format (`{"type": "@Instance", "valueAsString": "..."}`) is a VM Service convention — the actual value is always in `valueAsString`
- `valueAsString` can be `null` for `error` and `stackTrace` fields — handle as `Option`
- The `time` field is milliseconds since epoch — convert using `chrono::DateTime`
- Logger name prefixing (`[AuthService] msg`) matches how the `logging` package displays in DevTools
- This module provides parsing functions — actual event routing happens in Task 08
- Popular packages that use `dart:developer log()`: `logging`, `logger` (the one from pub.dev that wraps dart:developer — not to be confused with the `Logger` package that uses `print()`)

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-daemon/src/vm_service/logging.rs` | Created new module with `VmLogRecord`, `vm_level_to_log_level`, `parse_log_record`, `extract_value_as_string`, `vm_log_to_log_entry`, `millis_to_datetime`, and 25 unit tests |
| `crates/fdemon-daemon/src/vm_service/mod.rs` | Added `pub mod logging;` and re-exports for `VmLogRecord`, `parse_log_record`, `vm_level_to_log_level`, `vm_log_to_log_entry`; also updated module doc to mention logging and errors modules |
| `crates/fdemon-daemon/Cargo.toml` | Added `chrono.workspace = true` to support `DateTime<Local>` timestamp conversion |

### Notable Decisions/Tradeoffs

1. **Level mapping boundary at 799**: The task spec showed `..=499`, `500..=699`, `700..=799` all mapping to Debug — simplified to a single `..=799 => Debug` arm which is semantically equivalent and matches the intent.

2. **`chrono` dependency added to fdemon-daemon**: The `vm_log_to_log_entry` function converts `record.time` (milliseconds since epoch) into a `DateTime<Local>` so the log entry gets the correct VM timestamp rather than `Local::now()`. This required adding `chrono` to the daemon crate's dependencies.

3. **Timestamp conversion fallback**: If `millis_to_datetime` receives an invalid or zero milliseconds value, it falls back to `Local::now()` rather than panicking or returning an error, satisfying the "handle malformed events gracefully" requirement.

4. **`extract_value_as_string` is private**: The function is an implementation detail for parsing `InstanceRef` objects — it is not exported from the module or crate, consistent with the task specification (`fn`, not `pub fn`).

5. **Raw string delimiter level**: The test `test_parse_log_record_with_error_and_stack_trace` uses `r##"..."##` instead of `r#"..."#` to avoid the Rust parser treating `"#0` inside the JSON as the end of the raw string delimiter.

6. **mod.rs concurrent edit**: The task warned another agent may be editing `mod.rs`. Re-read the file before editing and found `pub mod errors;` already added — preserved it and appended only the logging additions.

### Testing Performed

- `cargo check --workspace` — Passed
- `cargo test -p fdemon-daemon` — Passed (233 tests: 233 passed, 0 failed, 3 ignored)
  - All 25 new `vm_service::logging::tests::*` tests passed
- `cargo clippy --workspace -- -D warnings` — Passed (zero warnings)

### Risks/Limitations

1. **No integration test against a real VM Service**: All tests use synthetic JSON. Real VM Service events may differ slightly (e.g., extra fields, different nesting), but the `#[serde(flatten)]` on `StreamEvent.data` provides forward-compatibility.

2. **Level field is an integer in the test data**: The task's JSON example shows `"level": 800` (plain integer), not `"level": {"valueAsString": "800"}`. This implementation parses `level` as a plain integer, which matches the actual VM Service protocol — the integer value is used directly, not wrapped in an `InstanceRef`.
