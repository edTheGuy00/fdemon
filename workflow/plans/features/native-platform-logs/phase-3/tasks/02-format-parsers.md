## Task: Pluggable Output Format Parsers

**Objective**: Create a format parser module that dispatches line-by-line output parsing based on the configured `OutputFormat`, reusing existing parsers where possible.

**Depends on**: None (but will use `OutputFormat` enum from task 01 — can develop in parallel using a local enum that gets replaced)

### Scope

- `crates/fdemon-daemon/src/native_logs/formats.rs` — **NEW** file
- `crates/fdemon-daemon/src/native_logs/mod.rs` — Add `pub mod formats;`
- `crates/fdemon-daemon/src/native_logs/android.rs` — May need to make `parse_threadtime_line` pub(crate) or extract
- `crates/fdemon-daemon/src/native_logs/macos.rs` — May need to make `parse_syslog_line` pub(crate) or extract

### Details

Create `formats.rs` with a central dispatch function:

```rust
use crate::native_logs::NativeLogEvent;
use fdemon_core::types::LogLevel;

/// Parse a single output line using the specified format.
///
/// Returns `None` if the line cannot be parsed (blank line, header, etc.)
pub fn parse_line(
    format: &OutputFormat,
    line: &str,
    source_name: &str,
) -> Option<NativeLogEvent> {
    match format {
        OutputFormat::Raw => parse_raw(line, source_name),
        OutputFormat::Json => parse_json(line, source_name),
        OutputFormat::LogcatThreadtime => parse_logcat_threadtime(line),
        OutputFormat::Syslog => parse_syslog(line, source_name),
    }
}
```

#### Raw Format

Simplest parser — every non-empty line becomes a log event:

```rust
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
```

#### JSON Format

Parse JSON objects with flexible field names:

```rust
fn parse_json(line: &str, source_name: &str) -> Option<NativeLogEvent> {
    let v: serde_json::Value = serde_json::from_str(line.trim()).ok()?;
    let obj = v.as_object()?;

    // Message: try "message", "msg", "text"
    let message = obj.get("message")
        .or_else(|| obj.get("msg"))
        .or_else(|| obj.get("text"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    if message.is_empty() {
        return None;
    }

    // Tag: try "tag", "source", "logger" — fall back to source_name
    let tag = obj.get("tag")
        .or_else(|| obj.get("source"))
        .or_else(|| obj.get("logger"))
        .and_then(|v| v.as_str())
        .unwrap_or(source_name)
        .to_string();

    // Level: try "level", "severity", "priority"
    let level = obj.get("level")
        .or_else(|| obj.get("severity"))
        .or_else(|| obj.get("priority"))
        .and_then(|v| v.as_str())
        .map(parse_json_level)
        .unwrap_or(LogLevel::Info);

    // Timestamp: try "timestamp", "time", "ts"
    let timestamp = obj.get("timestamp")
        .or_else(|| obj.get("time"))
        .or_else(|| obj.get("ts"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    Some(NativeLogEvent { tag, level, message, timestamp })
}

fn parse_json_level(s: &str) -> LogLevel {
    match s.to_lowercase().as_str() {
        "trace" | "verbose" | "debug" => LogLevel::Debug,
        "info" | "information" => LogLevel::Info,
        "warn" | "warning" => LogLevel::Warning,
        "error" | "err" | "fatal" | "critical" => LogLevel::Error,
        _ => LogLevel::Info,
    }
}
```

#### Logcat Threadtime Format

Delegate to the existing `parse_threadtime_line()` from `android.rs`:

```rust
fn parse_logcat_threadtime(line: &str) -> Option<NativeLogEvent> {
    // Reuse android::parse_threadtime_line() and convert LogcatLine → NativeLogEvent
    let logcat_line = super::android::parse_threadtime_line(line)?;
    Some(super::android::logcat_line_to_event(logcat_line))
}
```

This requires making `parse_threadtime_line` and `logcat_line_to_event` `pub(crate)` in `android.rs`.

#### Syslog Format

Delegate to the existing `parse_syslog_line()` from `macos.rs`:

```rust
fn parse_syslog(line: &str, source_name: &str) -> Option<NativeLogEvent> {
    // Reuse macos::parse_syslog_line() and convert SyslogLine → NativeLogEvent
    let syslog_line = super::macos::parse_syslog_line(line)?;
    Some(super::macos::syslog_line_to_event(syslog_line, source_name))
}
```

This requires making `parse_syslog_line` and `syslog_line_to_event` `pub(crate)` in `macos.rs`.

### Layer Boundary Consideration

The `OutputFormat` enum is defined in `fdemon-app/src/config/types.rs` (task 01) as part of config. But `formats.rs` lives in `fdemon-daemon`. Options:

1. **Move `OutputFormat` to `fdemon-core`** — cleanest, allows both daemon and app to use it
2. **Duplicate a simple format enum in daemon** — avoids core dependency change but duplicates
3. **Pass format as a string and parse in daemon** — loose coupling but stringly typed

**Recommended: Option 1** — move `OutputFormat` to `fdemon-core/src/types.rs`. It's a small enum with no dependencies, and it represents a domain concept (log output format).

### Acceptance Criteria

1. `parse_line()` correctly dispatches to all 4 format parsers
2. Raw parser: non-empty lines produce events with tag=source_name, level=Info
3. JSON parser: handles all field name aliases (message/msg/text, tag/source/logger, level/severity/priority)
4. JSON parser: gracefully returns None for invalid JSON or missing message
5. Logcat threadtime parser: delegates to existing `parse_threadtime_line()` and produces identical results
6. Syslog parser: delegates to existing `parse_syslog_line()` and produces identical results
7. Empty lines and whitespace-only lines return None for all formats

### Testing

```rust
#[test]
fn test_raw_format_basic_line() { ... }

#[test]
fn test_raw_format_empty_line_returns_none() { ... }

#[test]
fn test_json_format_standard_fields() {
    let line = r#"{"level": "error", "tag": "MyApp", "message": "something failed"}"#;
    // Verify: level=Error, tag="MyApp", message="something failed"
}

#[test]
fn test_json_format_alternate_field_names() {
    let line = r#"{"severity": "warn", "logger": "http", "msg": "timeout"}"#;
    // Verify: level=Warning, tag="http", message="timeout"
}

#[test]
fn test_json_format_missing_message_returns_none() { ... }

#[test]
fn test_json_format_invalid_json_returns_none() { ... }

#[test]
fn test_logcat_threadtime_delegates_to_existing_parser() { ... }

#[test]
fn test_syslog_delegates_to_existing_parser() { ... }
```

### Notes

- The `serde_json` dependency is already in the workspace — no new dependency needed
- For the logcat-threadtime and syslog delegating parsers, ensure the `android.rs` and `macos.rs` parser functions are `pub(crate)` but not `pub` (internal to the daemon crate)
- If making the existing parsers `pub(crate)` requires touching too many tests, consider extracting just the parsing functions to a shared submodule within `native_logs/`
