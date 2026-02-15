## Task: Session Integration — Auto-Connect, Log Merging, Status Bar

**Objective**: Wire everything together: auto-connect `VmServiceClient` when `app.debugPort` arrives, route VM Service events as `Message` items through the TEA loop, merge VM logs with daemon logs, handle lifecycle (disconnect on stop), and show `[VM]` connection indicator in the status bar.

**Depends on**: 06-structured-errors, 07-logging-stream

**Estimated Time**: 6-8 hours

### Scope

- `crates/fdemon-app/src/message.rs` — Add VM Service message variants
- `crates/fdemon-app/src/session.rs` — Add `VmServiceClient` to `SessionHandle`
- `crates/fdemon-app/src/engine.rs` — Auto-connect on `app.debugPort`, spawn VM Service event task
- `crates/fdemon-app/src/handler/` — Handle VM Service messages in TEA update
- `crates/fdemon-tui/src/widgets/status_bar.rs` — Add `[VM]` indicator
- `crates/fdemon-app/src/session.rs` — Log merging with dedup

### Details

#### 1. VM Service Messages (TEA)

Add to `Message` enum in `crates/fdemon-app/src/message.rs`:

```rust
// ─────────────────────────────────────────────────
// VM Service (Phase 1 DevTools Integration)
// ─────────────────────────────────────────────────

/// VM Service WebSocket connected for a session
VmServiceConnected {
    session_id: SessionId,
},

/// VM Service connection failed
VmServiceConnectionFailed {
    session_id: SessionId,
    error: String,
},

/// VM Service disconnected (unexpected or graceful)
VmServiceDisconnected {
    session_id: SessionId,
},

/// VM Service received a Flutter.Error event (crash log)
VmServiceFlutterError {
    session_id: SessionId,
    log_entry: LogEntry,
},

/// VM Service received a log record from Logging stream
VmServiceLogRecord {
    session_id: SessionId,
    log_entry: LogEntry,
},
```

#### 2. SessionHandle Changes

Add `VmServiceClient` to `SessionHandle`:

```rust
pub struct SessionHandle {
    pub session: Session,
    pub process: Option<FlutterProcess>,
    pub cmd_sender: Option<CommandSender>,
    pub request_tracker: Arc<RequestTracker>,
    pub vm_client: Option<VmServiceClient>,  // NEW
}
```

Add `vm_connected: bool` to `Session` for UI indicator:

```rust
pub vm_connected: bool,  // NEW: true when VM Service WebSocket is connected
```

#### 3. Auto-Connect in Engine

When `app.debugPort` event is handled (from Task 02), spawn a VM Service connection task:

In `crates/fdemon-app/src/engine.rs`, add a new `UpdateAction` variant:

```rust
pub enum UpdateAction {
    // ... existing variants ...
    ConnectVmService {
        session_id: SessionId,
        ws_uri: String,
    },
}
```

The handler for `AppDebugPort` (from Task 02) returns this action:

```rust
// In handler/session.rs — after storing ws_uri
UpdateResult::action(UpdateAction::ConnectVmService {
    session_id,
    ws_uri: debug_port.ws_uri.clone(),
})
```

The engine's `handle_action` spawns the connection:

```rust
UpdateAction::ConnectVmService { session_id, ws_uri } => {
    let msg_tx = self.msg_tx.clone();
    tokio::spawn(async move {
        match VmServiceClient::connect(&ws_uri).await {
            Ok(client) => {
                // Discover main isolate and subscribe to streams
                if let Err(e) = client.discover_main_isolate().await {
                    tracing::warn!("Failed to discover isolate: {e}");
                }
                let errors = client.subscribe_phase1_streams().await;
                for err in &errors {
                    tracing::warn!("Stream subscription failed: {err}");
                }

                let _ = msg_tx.send(Message::VmServiceConnected { session_id });

                // Start event forwarding loop
                // ... (see step 4)
            }
            Err(e) => {
                let _ = msg_tx.send(Message::VmServiceConnectionFailed {
                    session_id,
                    error: e.to_string(),
                });
            }
        }
    });
}
```

#### 4. VM Service Event Forwarding

After connecting, spawn a task that reads VM Service events and forwards them as `Message` items:

```rust
async fn forward_vm_events(
    mut client: VmServiceClient,
    session_id: SessionId,
    msg_tx: mpsc::Sender<Message>,
) {
    while let Some(event) = client.event_receiver().recv().await {
        // Try parsing as Flutter.Error
        if let Some(flutter_error) = parse_flutter_error(&event.params.event) {
            let log_entry = flutter_error_to_log_entry(&flutter_error);
            let _ = msg_tx.send(Message::VmServiceFlutterError {
                session_id,
                log_entry,
            });
            continue;
        }

        // Try parsing as LogRecord
        if let Some(log_record) = parse_log_record(&event.params.event) {
            let log_entry = vm_log_to_log_entry(&log_record);
            let _ = msg_tx.send(Message::VmServiceLogRecord {
                session_id,
                log_entry,
            });
        }
    }

    // Client disconnected
    let _ = msg_tx.send(Message::VmServiceDisconnected { session_id });
}
```

#### 5. TEA Handlers for VM Messages

In `handler/update.rs` or a new `handler/vm_service.rs`:

```rust
Message::VmServiceConnected { session_id } => {
    if let Some(handle) = state.session_manager.get_mut(session_id) {
        handle.session.vm_connected = true;
        handle.session.add_log(LogEntry::info(
            LogSource::App,
            "VM Service connected — enhanced logging active",
        ));
    }
    UpdateResult::none()
}

Message::VmServiceConnectionFailed { session_id, error } => {
    if let Some(handle) = state.session_manager.get_mut(session_id) {
        tracing::warn!("VM Service connection failed for session {session_id}: {error}");
        // Don't show error to user — daemon logs still work
    }
    UpdateResult::none()
}

Message::VmServiceDisconnected { session_id } => {
    if let Some(handle) = state.session_manager.get_mut(session_id) {
        handle.session.vm_connected = false;
    }
    UpdateResult::none()
}

Message::VmServiceFlutterError { session_id, log_entry } => {
    if let Some(handle) = state.session_manager.get_mut(session_id) {
        handle.session.add_log(log_entry);
    }
    UpdateResult::none()
}

Message::VmServiceLogRecord { session_id, log_entry } => {
    if let Some(handle) = state.session_manager.get_mut(session_id) {
        handle.session.add_log(log_entry);
    }
    UpdateResult::none()
}
```

#### 6. Log Deduplication

Some logs may appear in both VM Service Logging stream and daemon stdout (rare, but possible). Add simple dedup:

```rust
/// Check if a log entry is a duplicate of a recent entry
fn is_duplicate_log(logs: &VecDeque<LogEntry>, entry: &LogEntry, threshold_ms: i64) -> bool {
    logs.iter().rev().take(10).any(|existing| {
        existing.message == entry.message
            && (existing.timestamp - entry.timestamp).abs() < threshold_ms
    })
}
```

Call before `add_log()` for VM Service entries.

#### 7. Disconnect on Session Stop

When a session stops or is closed, disconnect the VM Service client:

```rust
// In session cleanup (handle_session_exited or close_session)
if let Some(vm_client) = handle.vm_client.take() {
    vm_client.disconnect().await;
}
```

#### 8. Status Bar `[VM]` Indicator

In `crates/fdemon-tui/src/widgets/status_bar.rs`, add a `[VM]` badge when `session.vm_connected == true`:

```rust
// After existing status indicators
if session.vm_connected {
    spans.push(Span::styled("[VM] ", Style::default().fg(palette::STATUS_GREEN)));
}
```

### Acceptance Criteria

1. VM Service auto-connects when `app.debugPort` event arrives (no user action)
2. `Flutter.Error` events appear as error log entries in the session log view
3. VM Service `LogRecord` events appear with correct log levels
4. Duplicate logs are filtered (same message within 100ms threshold)
5. `[VM]` indicator appears in status bar when connected
6. `[VM]` indicator disappears when disconnected
7. Session stop disconnects VM Service client gracefully
8. Connection failure is handled silently (daemon logs still work as fallback)
9. All new message variants are handled in the TEA update function
10. Existing tests pass — no regressions in daemon log processing
11. New tests cover VM Service message handling

### Testing

```rust
#[cfg(test)]
mod tests {
    #[test]
    fn test_vm_service_connected_sets_flag() {
        // Process VmServiceConnected message
        // Assert session.vm_connected == true
    }

    #[test]
    fn test_vm_service_disconnected_clears_flag() {
        // Process VmServiceDisconnected message
        // Assert session.vm_connected == false
    }

    #[test]
    fn test_vm_service_flutter_error_adds_log() {
        // Process VmServiceFlutterError with a LogEntry
        // Assert log appears in session.logs
    }

    #[test]
    fn test_vm_service_log_record_adds_log() {
        // Process VmServiceLogRecord with a LogEntry
        // Assert log appears with correct level
    }

    #[test]
    fn test_duplicate_log_detection() {
        // Add a log, then add same message within 100ms
        // Assert second is filtered
    }

    #[test]
    fn test_connection_failure_does_not_crash() {
        // Process VmServiceConnectionFailed
        // Assert no panic, state unchanged
    }
}
```

### Notes

- This is the largest task in Phase 1 — consider splitting if implementation exceeds 500 lines
- The event forwarding task runs alongside the daemon stdout/stderr reader tasks — same pattern
- Log dedup threshold of 100ms matches the config default (`dedupe_threshold_ms = 100`)
- The `VmServiceClient` handle needs to be stored somewhere accessible for disconnect — `SessionHandle` is the natural place
- Connection status logging should use `tracing::info!` / `tracing::warn!` (never `println!`)
- Consider adding `EngineEvent::VmServiceConnected` / `VmServiceDisconnected` for the event broadcast system

---

## Completion Summary

**Status:** Not Started
