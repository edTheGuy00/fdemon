## Task: Parse `daemon.devtools` Event + ServeDevTools Response in Protocol Parser

**Objective**: Wire `daemon.devtools` event parsing (and the `daemon.devtools.serve` response) into `protocol.rs` so it produces `DaemonMessage::DevToolsServed { ... }` instead of falling through to `_ => unknown_event(...)`.

**Depends on**: 02-daemon-message-devtools-served

**Estimated Time**: 1-2 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-daemon/src/protocol.rs` (lines 115-151): Add an arm for `event == "daemon.devtools"` parsing `params.host` (string) and `params.port` (u16) into `DaemonMessage::DevToolsServed`.
- `crates/fdemon-daemon/src/protocol.rs`: Add response handling for `daemon.devtools.serve`:
  - On success result with `{host, port}`: emit `DevToolsServed`.
  - On error result with `code: -32601`: emit `DevToolsServeFailed { reason: "Method not supported on this Flutter SDK" }`.
  - On other error: emit `DevToolsServeFailed { reason: <message> }`.
  - Use `RequestTracker` (or whatever mechanism `commands.rs` uses) to correlate response → original request id.

**Files Read (Dependencies):**
- `workflow/plans/bugs/browser-devtools-dds-registration/RESEARCH.md`: For sample wire traces.
- `crates/fdemon-daemon/src/commands.rs`: For `RequestTracker` patterns.
- `crates/fdemon-core/src/events.rs`: For `DaemonMessage` variants.

### Details

Event parsing sketch (adjust to the existing parser style — likely a `match event_name` block):

```rust
match event_name {
    // ... existing arms ...
    "daemon.devtools" => {
        let host = params.get("host").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let port = params.get("port").and_then(|v| v.as_u64()).unwrap_or(0) as u16;
        let pid = params.get("pid").and_then(|v| v.as_u64()).map(|p| p as u32);
        DaemonMessage::DevToolsServed { host, port, pid }
    }
    _ => unknown_event(event, params),
}
```

Response handling — if the parser branches between events and responses by presence of `"method"` vs `"result"`/`"error"`:

```rust
fn parse_response(msg: &Value, tracker: &mut RequestTracker) -> Option<DaemonMessage> {
    let id = msg.get("id").and_then(|v| v.as_str())?;
    let original = tracker.take(id)?;

    match original.method.as_str() {
        "daemon.devtools.serve" => {
            if let Some(result) = msg.get("result") {
                let host = result.get("host")?.as_str()?.to_string();
                let port = result.get("port")?.as_u64()? as u16;
                let pid = result.get("pid").and_then(|v| v.as_u64()).map(|p| p as u32);
                Some(DaemonMessage::DevToolsServed { host, port, pid })
            } else if let Some(error) = msg.get("error") {
                let code = error.get("code").and_then(|v| v.as_i64()).unwrap_or(0);
                let message = error.get("message").and_then(|v| v.as_str()).unwrap_or("unknown");
                let reason = if code == -32601 {
                    "Method not supported on this Flutter SDK".to_string()
                } else {
                    format!("daemon.devtools.serve failed: {} (code {})", message, code)
                };
                Some(DaemonMessage::DevToolsServeFailed { reason })
            } else {
                None
            }
        }
        _ => /* existing response handlers */ None,
    }
}
```

### Acceptance Criteria

1. `daemon.devtools` events with valid host+port produce `DaemonMessage::DevToolsServed`.
2. `daemon.devtools.serve` responses with `result` shape produce `DevToolsServed`.
3. `daemon.devtools.serve` responses with error code `-32601` produce `DevToolsServeFailed { reason: "Method not supported ..." }`.
4. Other errors produce `DevToolsServeFailed` with the daemon's error message.
5. No existing event/response handling is broken (regression tests for `app.start`, `app.debugPort`, etc. continue to pass).
6. New unit tests cover all four cases using fixtures from RESEARCH.md.

### Testing

```rust
#[test]
fn parses_daemon_devtools_event() {
    let json = r#"{"event":"daemon.devtools","params":{"host":"127.0.0.1","port":9100}}"#;
    let msg = parse_event(serde_json::from_str(json).unwrap()).unwrap();
    assert!(matches!(msg, DaemonMessage::DevToolsServed { ref host, port, .. } if host == "127.0.0.1" && port == 9100));
}

#[test]
fn parses_devtools_serve_response_method_not_found() {
    let mut tracker = RequestTracker::new();
    tracker.register("devtools-serve", "daemon.devtools.serve");
    let json = r#"{"id":"devtools-serve","error":{"code":-32601,"message":"Method not found"}}"#;
    let msg = parse_response(&serde_json::from_str(json).unwrap(), &mut tracker).unwrap();
    assert!(matches!(msg, DaemonMessage::DevToolsServeFailed { ref reason } if reason.contains("Method not supported")));
}

#[test]
fn parses_devtools_serve_response_success() { /* ... */ }
```

### Notes

- Reuse existing helpers (e.g., `as_str_or_default`) if present in `protocol.rs`.
- Don't `unwrap` — return `DaemonMessage::ParseError` or similar on malformed JSON (existing pattern).
- Be defensive about types: `port` may come as a JSON number that's > u16 max in pathological cases; clamp or error.
