## Task: Typed Protocol Messages

**Objective**: Define typed Rust structs for all Flutter daemon JSON-RPC events with proper serde deserialization, replacing the current raw message handling.

**Depends on**: None (foundational task)

---

### Scope

- `src/daemon/protocol.rs`: Extend existing with typed message enum
- `src/daemon/events.rs`: **NEW** - Define event-specific structs
- `src/daemon/mod.rs`: Re-export new types

---

### Implementation Details

#### Event Types to Define

Based on Flutter daemon protocol (from `flutter run --machine`):

```rust
// src/daemon/events.rs

use serde::{Deserialize, Serialize};

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

/// Device added event
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
```

#### Typed DaemonMessage Enum

```rust
// src/daemon/protocol.rs (extend existing)

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

impl DaemonMessage {
    /// Parse a JSON string into a typed DaemonMessage
    pub fn parse(json: &str) -> Option<Self> {
        let raw: RawMessage = serde_json::from_str(json).ok()?;
        Some(Self::from_raw(raw))
    }
    
    /// Convert from RawMessage to typed message
    pub fn from_raw(raw: RawMessage) -> Self {
        match raw {
            RawMessage::Response { id, result, error } => {
                DaemonMessage::Response { id, result, error }
            }
            RawMessage::Event { event, params } => {
                Self::parse_event(&event, params)
            }
        }
    }
    
    /// Parse an event by name
    fn parse_event(event: &str, params: serde_json::Value) -> Self {
        match event {
            "daemon.connected" => {
                serde_json::from_value(params)
                    .map(DaemonMessage::DaemonConnected)
                    .unwrap_or_else(|_| Self::unknown(event, params))
            }
            "daemon.logMessage" => {
                serde_json::from_value(params)
                    .map(DaemonMessage::DaemonLogMessage)
                    .unwrap_or_else(|_| Self::unknown(event, params))
            }
            "app.start" => {
                serde_json::from_value(params)
                    .map(DaemonMessage::AppStart)
                    .unwrap_or_else(|_| Self::unknown(event, params))
            }
            "app.started" => {
                serde_json::from_value(params)
                    .map(DaemonMessage::AppStarted)
                    .unwrap_or_else(|_| Self::unknown(event, params))
            }
            "app.stop" => {
                serde_json::from_value(params)
                    .map(DaemonMessage::AppStop)
                    .unwrap_or_else(|_| Self::unknown(event, params))
            }
            "app.log" => {
                serde_json::from_value(params)
                    .map(DaemonMessage::AppLog)
                    .unwrap_or_else(|_| Self::unknown(event, params))
            }
            "app.progress" => {
                serde_json::from_value(params)
                    .map(DaemonMessage::AppProgress)
                    .unwrap_or_else(|_| Self::unknown(event, params))
            }
            "app.debugPort" => {
                serde_json::from_value(params)
                    .map(DaemonMessage::AppDebugPort)
                    .unwrap_or_else(|_| Self::unknown(event, params))
            }
            "device.added" => {
                serde_json::from_value(params)
                    .map(DaemonMessage::DeviceAdded)
                    .unwrap_or_else(|_| Self::unknown(event, params))
            }
            "device.removed" => {
                serde_json::from_value(params)
                    .map(DaemonMessage::DeviceRemoved)
                    .unwrap_or_else(|_| Self::unknown(event, params))
            }
            _ => Self::unknown(event, params),
        }
    }
    
    fn unknown(event: &str, params: serde_json::Value) -> Self {
        DaemonMessage::UnknownEvent {
            event: event.to_string(),
            params,
        }
    }
    
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
            DaemonMessage::AppProgress(p) => {
                p.message.clone().unwrap_or_else(|| "Progress...".to_string())
            }
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
```

#### Update Core DaemonEvent

```rust
// src/core/events.rs - update to include typed messages

use crate::daemon::DaemonMessage;

/// Events from the Flutter daemon process
#[derive(Debug, Clone)]
pub enum DaemonEvent {
    /// Raw stdout line (before parsing)
    Stdout(String),
    
    /// Parsed daemon message
    Message(DaemonMessage),
    
    /// Stderr output
    Stderr(String),
    
    /// Daemon process exited
    Exited { code: Option<i32> },
    
    /// Process spawn failed
    SpawnFailed { reason: String },
}
```

---

### Acceptance Criteria

1. [ ] `DaemonMessage` enum covers all common Flutter daemon events
2. [ ] All event structs use `#[serde(rename_all = "camelCase")]`
3. [ ] Optional fields use `#[serde(default)]`
4. [ ] `DaemonMessage::parse()` correctly parses JSON into typed events
5. [ ] Unknown events fall back to `UnknownEvent` variant (no panics)
6. [ ] Malformed JSON in known events falls back to `UnknownEvent`
7. [ ] `app_id()` helper returns app ID for app-related events
8. [ ] `is_error()` helper identifies error messages
9. [ ] `summary()` provides human-readable descriptions
10. [ ] `DaemonEvent` updated to include `Message(DaemonMessage)` variant
11. [ ] All new types are re-exported from `daemon/mod.rs`
12. [ ] Unit tests cover parsing of each event type

---

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_daemon_connected() {
        let json = r#"{"event":"daemon.connected","params":{"version":"0.6.1","pid":12345}}"#;
        if let Some(inner) = strip_brackets(&format!("[{}]", json)) {
            // For events without brackets
        }
        let msg = DaemonMessage::parse(json);
        assert!(matches!(msg, Some(DaemonMessage::DaemonConnected(_))));
        if let Some(DaemonMessage::DaemonConnected(c)) = msg {
            assert_eq!(c.version, "0.6.1");
            assert_eq!(c.pid, 12345);
        }
    }

    #[test]
    fn test_parse_app_log() {
        let json = r#"{"event":"app.log","params":{"appId":"abc123","log":"flutter: Hello World","error":false}}"#;
        let msg = DaemonMessage::parse(json).unwrap();
        assert!(matches!(msg, DaemonMessage::AppLog(_)));
        if let DaemonMessage::AppLog(log) = msg {
            assert_eq!(log.log, "flutter: Hello World");
            assert!(!log.error);
        }
    }

    #[test]
    fn test_parse_app_log_error() {
        let json = r#"{"event":"app.log","params":{"appId":"abc","log":"Error message","error":true,"stackTrace":"at main.dart:10"}}"#;
        let msg = DaemonMessage::parse(json).unwrap();
        assert!(msg.is_error());
    }

    #[test]
    fn test_parse_app_progress() {
        let json = r#"{"event":"app.progress","params":{"appId":"abc","id":"1","message":"Compiling...","finished":false}}"#;
        let msg = DaemonMessage::parse(json).unwrap();
        if let DaemonMessage::AppProgress(p) = msg {
            assert_eq!(p.message, Some("Compiling...".to_string()));
            assert!(!p.finished);
        }
    }

    #[test]
    fn test_parse_app_start() {
        let json = r#"{"event":"app.start","params":{"appId":"abc123","deviceId":"iphone","directory":"/path/to/app","supportsRestart":true}}"#;
        let msg = DaemonMessage::parse(json).unwrap();
        assert!(matches!(msg, DaemonMessage::AppStart(_)));
        assert_eq!(msg.app_id(), Some("abc123"));
    }

    #[test]
    fn test_parse_device_added() {
        let json = r#"{"event":"device.added","params":{"id":"emulator-5554","name":"Pixel 4","platform":"android","emulator":true}}"#;
        let msg = DaemonMessage::parse(json).unwrap();
        if let DaemonMessage::DeviceAdded(d) = msg {
            assert_eq!(d.name, "Pixel 4");
            assert!(d.emulator);
        }
    }

    #[test]
    fn test_parse_response_success() {
        let json = r#"{"id":1,"result":{"code":0}}"#;
        let msg = DaemonMessage::parse(json).unwrap();
        assert!(matches!(msg, DaemonMessage::Response { .. }));
        assert!(!msg.is_error());
    }

    #[test]
    fn test_parse_response_error() {
        let json = r#"{"id":1,"error":"Something failed"}"#;
        let msg = DaemonMessage::parse(json).unwrap();
        assert!(msg.is_error());
    }

    #[test]
    fn test_unknown_event_fallback() {
        let json = r#"{"event":"some.future.event","params":{"foo":"bar"}}"#;
        let msg = DaemonMessage::parse(json).unwrap();
        assert!(matches!(msg, DaemonMessage::UnknownEvent { .. }));
    }

    #[test]
    fn test_malformed_event_fallback() {
        // app.start missing required fields
        let json = r#"{"event":"app.start","params":{"incomplete":true}}"#;
        let msg = DaemonMessage::parse(json).unwrap();
        // Should fall back to UnknownEvent, not panic
        assert!(matches!(msg, DaemonMessage::UnknownEvent { .. }));
    }

    #[test]
    fn test_summary_messages() {
        let log_json = r#"{"event":"app.log","params":{"appId":"a","log":"Hello","error":false}}"#;
        let msg = DaemonMessage::parse(log_json).unwrap();
        assert_eq!(msg.summary(), "Hello");

        let connected_json = r#"{"event":"daemon.connected","params":{"version":"1.0.0","pid":123}}"#;
        let msg = DaemonMessage::parse(connected_json).unwrap();
        assert!(msg.summary().contains("1.0.0"));
    }

    #[test]
    fn test_invalid_json_returns_none() {
        assert!(DaemonMessage::parse("not json").is_none());
        assert!(DaemonMessage::parse("{incomplete").is_none());
    }
}
```

---

### Notes

- Keep `RawMessage` enum for backwards compatibility and initial parsing
- `DaemonMessage` is the higher-level typed abstraction
- Use `#[serde(default)]` liberally for optional fields to handle protocol variations
- The Flutter daemon protocol is not formally documented; these structs are based on observed behavior
- Consider adding more event types as discovered during testing
- Stack traces can be very long; consider truncation in display (not in storage)

---

### Files to Create/Modify

| File | Action | Description |
|------|--------|-------------|
| `src/daemon/events.rs` | CREATE | Event struct definitions |
| `src/daemon/protocol.rs` | MODIFY | Add `DaemonMessage` enum and parsing |
| `src/daemon/mod.rs` | MODIFY | Re-export new types |
| `src/core/events.rs` | MODIFY | Add `Message(DaemonMessage)` variant |

---

## Completion Summary

**Status**: ✅ Done

**Date**: 2026-01-03

### Files Modified

| File | Action | Description |
|------|--------|-------------|
| `src/daemon/events.rs` | CREATED | All 9 typed event structs: `DaemonConnected`, `DaemonLogMessage`, `AppStart`, `AppStarted`, `AppLog`, `AppProgress`, `AppStop`, `AppDebugPort`, `DeviceInfo` |
| `src/daemon/protocol.rs` | MODIFIED | Added `DaemonMessage` enum with 12 variants and `parse()`, `from_raw()`, `app_id()`, `is_error()`, `summary()` methods |
| `src/daemon/mod.rs` | MODIFIED | Added `events` module and re-exported all typed event structs and `DaemonMessage` |
| `src/core/events.rs` | MODIFIED | Added `Message(DaemonMessage)` variant to `DaemonEvent` enum |
| `src/core/types.rs` | MODIFIED | Added `LogSource::Daemon` variant for daemon-level messages |
| `src/app/handler.rs` | MODIFIED | Added `handle_daemon_message()` function to process typed messages with proper log levels and sources |
| `src/tui/widgets/log_view.rs` | MODIFIED | Added styling for `LogSource::Daemon` (yellow color) |

### Notable Decisions/Tradeoffs

1. **Clone on parse**: The `parse_event()` method clones `params` before deserializing to avoid consuming the value on failure (needed for fallback to `UnknownEvent`).

2. **LogSource::Daemon**: Added a new `Daemon` log source variant to distinguish daemon infrastructure messages from Flutter app messages.

3. **Fallback behavior**: Malformed JSON for known event types gracefully falls back to `UnknownEvent` rather than panicking.

4. **Handler integration**: The `handle_daemon_message()` function properly routes log levels and sources based on message type (e.g., `AppProgress` -> `Debug`, error logs -> `LogLevel::Error`).

### Testing Performed

```bash
cargo check     # ✅ Passes
cargo test      # ✅ 110 tests pass (94 lib + 16 integration)
cargo clippy    # ✅ No warnings
cargo fmt       # ✅ Formatting applied
```

### Acceptance Criteria Status

- [x] `DaemonMessage` enum covers all common Flutter daemon events
- [x] All event structs use `#[serde(rename_all = "camelCase")]`
- [x] Optional fields use `#[serde(default)]`
- [x] `DaemonMessage::parse()` correctly parses JSON into typed events
- [x] Unknown events fall back to `UnknownEvent` variant (no panics)
- [x] Malformed JSON in known events falls back to `UnknownEvent`
- [x] `app_id()` helper returns app ID for app-related events
- [x] `is_error()` helper identifies error messages
- [x] `summary()` provides human-readable descriptions
- [x] `DaemonEvent` updated to include `Message(DaemonMessage)` variant
- [x] All new types are re-exported from `daemon/mod.rs`
- [x] Unit tests cover parsing of each event type (18 new tests)

### Risks/Limitations

- The Flutter daemon protocol is not formally documented; struct definitions are based on observed behavior and may need updates for edge cases.
- Stack traces in `AppLog` can be very long; truncation is deferred to display layer (not storage).