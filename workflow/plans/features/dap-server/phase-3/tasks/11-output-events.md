## Task: Output Events — Wire Log Pipeline to DAP

**Objective**: Forward Flutter application log output to connected DAP clients as DAP `output` events, so logs appear in the IDE's debug console.

**Depends on**: 10-session-integration

**Estimated Time**: 2-3 hours

### Scope

- `crates/fdemon-dap/src/adapter/mod.rs` — Output event emission
- `crates/fdemon-app/src/handler/dap.rs` — Log forwarding to DAP adapter
- `crates/fdemon-app/src/engine.rs` — Log event routing when DAP is active

### Details

#### Log-to-DAP Category Mapping

Map fdemon's `LogLevel` (from `fdemon-core`) to DAP output categories:

| `LogLevel` | DAP `category` | Notes |
|------------|----------------|-------|
| `Error` | `"stderr"` | Red/highlighted in most IDE consoles |
| `Warning` | `"console"` | Informational |
| `Info` | `"stdout"` | Standard output |
| `Debug` | `"console"` | Debug-level messages |
| `Verbose` | `"console"` | Verbose output (may be filtered by IDE) |

#### Output Event Structure

```rust
impl DapAdapter<B> {
    /// Emit a log entry as a DAP output event.
    pub async fn emit_log(&self, message: &str, level: LogLevel, source: Option<&DapSource>) {
        let category = match level {
            LogLevel::Error => "stderr",
            LogLevel::Info => "stdout",
            _ => "console",
        };

        let mut body = serde_json::json!({
            "category": category,
            "output": if message.ends_with('\n') {
                message.to_string()
            } else {
                format!("{}\n", message)
            },
        });

        // Include source location if available
        if let Some(src) = source {
            body["source"] = serde_json::to_value(src).unwrap_or_default();
        }

        let event = DapEvent::new("output", Some(body));
        let _ = self.event_tx.send(DapMessage::Event(event)).await;
    }
}
```

#### Engine Log Routing

When a DAP session is active, the Engine forwards log entries:

```rust
// In the Engine's log handling path:

// Forward to DAP if active
if let Some(dap_debug_event_tx) = &self.dap_debug_event_tx {
    let dap_event = DebugEvent::LogOutput {
        message: log_entry.message.clone(),
        level: log_entry.level,
        source_uri: log_entry.source_uri.clone(),
        line: log_entry.line,
    };
    let _ = dap_debug_event_tx.send(dap_event).await;
}
```

#### Add `LogOutput` to DebugEvent

Extend the `DebugEvent` enum (from Task 03):

```rust
pub enum DebugEvent {
    // ... existing variants ...
    LogOutput {
        message: String,
        level: String,  // Use string to avoid fdemon-core dependency in event enum
        source_uri: Option<String>,
        line: Option<i32>,
    },
}
```

#### Handle LogOutput in Adapter

```rust
DebugEvent::LogOutput { message, level, source_uri, line } => {
    let category = match level.as_str() {
        "error" => "stderr",
        "info" => "stdout",
        _ => "console",
    };

    let source = source_uri.as_ref().map(|uri| DapSource {
        name: Some(uri.rsplit('/').next().unwrap_or(uri).to_string()),
        path: dart_uri_to_path(uri),
        source_reference: None,
        presentation_hint: None,
    });

    let mut body = serde_json::json!({
        "category": category,
        "output": format!("{}\n", message),
    });

    if let Some(src) = source {
        body["source"] = serde_json::to_value(&src).unwrap_or_default();
        if let Some(line) = line {
            body["line"] = serde_json::json!(line);
        }
    }

    let event = DapEvent::new("output", Some(body));
    let _ = self.event_tx.send(DapMessage::Event(event)).await;
}
```

#### Output for Debug Console Messages

Besides app logs, emit output events for debugging lifecycle messages:

```rust
// On attach success:
self.emit_output("console", "Flutter Demon: Attached to VM Service\n").await;

// On breakpoint hit:
// (the stopped event handles this, but a console message is helpful)

// On hot reload (Phase 4):
// self.emit_output("console", "Hot reload completed in 450ms\n").await;
```

### Acceptance Criteria

1. Flutter app stdout appears in the IDE's debug console as `category: "stdout"`
2. Flutter app errors appear as `category: "stderr"` (highlighted in IDE)
3. Debug/info messages appear as `category: "console"`
4. Output events include source location when available
5. Each output message ends with a newline (DAP convention)
6. No output events are sent when no DAP session is active
7. Output events are correctly sequenced (monotonic `seq` numbers)
8. Works in Helix (output appears in the editor console area)
9. Works in Zed (output appears in the debug console panel)

### Testing

```rust
#[test]
fn test_output_event_structure() {
    let event = DapEvent::output("stderr", "Error: null check\n");
    let json = serde_json::to_value(DapMessage::Event(event)).unwrap();
    assert_eq!(json["body"]["category"], "stderr");
    assert_eq!(json["body"]["output"], "Error: null check\n");
}

#[test]
fn test_output_event_with_source() {
    let body = serde_json::json!({
        "category": "stderr",
        "output": "Error: null check\n",
        "source": {
            "name": "main.dart",
            "path": "/home/user/app/lib/main.dart"
        },
        "line": 42
    });
    let event = DapEvent::new("output", Some(body));
    let json = serde_json::to_value(DapMessage::Event(event)).unwrap();
    assert_eq!(json["body"]["source"]["path"], "/home/user/app/lib/main.dart");
    assert_eq!(json["body"]["line"], 42);
}

#[test]
fn test_log_level_to_category() {
    assert_eq!(log_level_to_category("error"), "stderr");
    assert_eq!(log_level_to_category("info"), "stdout");
    assert_eq!(log_level_to_category("debug"), "console");
    assert_eq!(log_level_to_category("warning"), "console");
}
```

### Notes

- Output messages MUST end with `\n` — IDEs concatenate output events and expect newline-terminated lines
- Both Zed and Helix display `output` events in their debug console/output area
- The `"telemetry"` category should never be used for user-visible output (it's hidden by most IDEs)
- Consider rate-limiting output events if the Flutter app is extremely chatty — a burst of thousands of log lines could overwhelm the DAP transport. A simple debounce/batch would help, but defer to Phase 4 if not needed.
- Source locations in output events allow IDEs to create clickable links to source files

---

## Completion Summary

**Status:** Not Started
