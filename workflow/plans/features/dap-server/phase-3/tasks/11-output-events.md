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

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-dap/src/adapter/mod.rs` | Added `LogOutput` variant to `DebugEvent` enum; added `log_level_to_category` public helper function; added `emit_output` convenience method to `DapAdapter`; added `LogOutput` arm to `handle_debug_event` that emits a DAP `"output"` event with correct category, newline-terminated output, and optional source location; added 15 new unit tests. |
| `crates/fdemon-dap/src/server/mod.rs` | Added `broadcast::Sender<DebugEvent>` field (`log_event_tx`) to `DapServerHandle`; added `log_event_sender()` accessor; updated `start()` to create the broadcast channel; updated `accept_loop` signature to accept the sender; updated session spawn in accept loop to subscribe each session via `log_event_tx.subscribe()`. |
| `crates/fdemon-dap/src/server/session.rs` | Added `broadcast` import; updated `DapClientSession::run` and `run_on` to accept a `broadcast::Receiver<DebugEvent>` for log events; added `event_tx/event_rx` channel to `run_on` so adapter-generated events are stamped and forwarded; added `log_event_rx` select arm that forwards received debug events to the adapter (if attached); handles `Lagged` and `Closed` broadcast errors gracefully. |
| `crates/fdemon-dap/src/service.rs` | Updated `start_stdio` to create a dummy broadcast channel for the `log_event_tx` field in the returned `DapServerHandle` (stdio mode has no TCP accept loop but must satisfy the struct requirement). |
| `crates/fdemon-dap/src/transport/stdio.rs` | Updated all `DapClientSession::run_on` call sites (both production and test) to pass a dummy broadcast receiver. |
| `crates/fdemon-dap/src/lib.rs` | Added doc comment entry for `adapter::log_level_to_category`. |
| `crates/fdemon-app/src/engine.rs` | Added `dap_log_event_tx: Option<broadcast::Sender<DapDebugEvent>>` field to `Engine`; added `sync_dap_log_sender` private method that reads the sender from `dap_server_handle` once per TEA cycle (using `try_lock` to avoid blocking); calls `sync_dap_log_sender` in `process_message` after the TEA cycle; in `emit_events`, when new logs arrive and DAP has active clients, sends `DebugEvent::LogOutput` for each log entry to the broadcast channel. |

### Notable Decisions/Tradeoffs

1. **Broadcast channel over per-session mpsc**: Using `broadcast::Sender<DebugEvent>` in `DapServerHandle` lets the Engine push log events to all active sessions without tracking individual session channels. This matches the one-to-many nature of log forwarding (one Engine, multiple IDE clients). Lagging receivers are dropped automatically by tokio, preventing one slow client from blocking others.

2. **Sender sync via `try_lock` once per TEA cycle**: Rather than storing the broadcast sender in the Engine and updating it through an explicit API, `sync_dap_log_sender` reads it from `dap_server_handle` once per `process_message` call. This keeps the action handler (which deposits the handle asynchronously) decoupled from the Engine while ensuring the Engine always has a current sender without holding the lock during log forwarding.

3. **`DebugEvent::LogOutput` uses `String` for level**: Avoids adding `fdemon-core::LogLevel` as a dependency to the `DebugEvent` enum type signature. The engine converts `LogLevel` to a lowercase string before constructing the event, and the adapter uses `log_level_to_category` to map back to a DAP category.

4. **`run_on` now owns the adapter event channel**: `run_on` was updated to create its own `(event_tx, event_rx)` pair and store `event_tx` in the session, matching the pattern in `run_on_with_backend`. This enables the `event_rx` select arm so adapter-generated events (e.g., from `LogOutput` handling) are forwarded to the client.

5. **Dummy broadcast channel for stdio mode**: `DapService::start_stdio` creates a `broadcast::channel(1)` and immediately drops the receiver. This satisfies the `DapServerHandle` struct field without any functional impact on stdio sessions.

### Testing Performed

- `cargo fmt --all` - Passed
- `cargo check --workspace` - Passed
- `cargo test --workspace --lib` - Passed (3255 tests: 1267 + 360 + 460 + 372 + 796, up 15 from 3240)
- `cargo clippy --workspace -- -D warnings` - Passed

### Risks/Limitations

1. **Engine integration requires DAP server to be running**: `dap_log_event_tx` is only populated after the DAP server starts and the handle is deposited in `dap_server_handle`. Log events before `DapServerStarted` is processed are silently dropped. This is correct behavior (acceptance criterion 6: no output events when no DAP session is active).

2. **No source location for Flutter log entries**: `LogEntry` in `fdemon-core` does not have `source_uri` or `line` fields, so all forwarded log events have `source_uri: None` and `line: None`. Source-linked output events (clickable links in IDE) require the VM Service to provide source info, which is not available at the log forwarding layer.

3. **Session attachment still uses NoopBackend**: The TCP accept loop uses `NoopBackend` (no real VM Service connection). `LogOutput` events are forwarded to all sessions via the broadcast channel, but the adapter's `handle_debug_event` is only called when the session has an adapter (i.e., after `attach` succeeds). With `NoopBackend`, `attach` fails, so the adapter is never created and log events are silently discarded. Full end-to-end log forwarding requires Phase 4 Engine wiring (real backend construction on attach).
