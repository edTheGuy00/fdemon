## Task: Fix ws_uri Capture and Add LogSource::VmService

**Objective**: Fix the broken `ws_uri` capture pipeline so the VM Service WebSocket URI is stored in the session and synced to SharedState. Also add the `LogSource::VmService` variant needed by later tasks.

**Depends on**: None

**Estimated Time**: 2-3 hours

### Scope

- `crates/fdemon-core/src/types.rs`: Add `VmService` variant to `LogSource` enum
- `crates/fdemon-app/src/session.rs`: Add `ws_uri: Option<String>` field to `Session` struct
- `crates/fdemon-app/src/handler/session.rs`: Add `AppDebugPort` handler in `handle_session_message_state()`
- `crates/fdemon-app/src/engine.rs`: Sync `ws_uri` to `SharedState.devtools_uri` (replace hardcoded `None`)

### Details

#### 1. Add `LogSource::VmService` (fdemon-core)

In `crates/fdemon-core/src/types.rs`, add to the `LogSource` enum (after `Watcher`):

```rust
/// VM Service / DevTools messages (structured logs, errors)
VmService,
```

Update the `prefix()` method:
```rust
LogSource::VmService => "vm",
```

Update any exhaustive match statements that handle `LogSource` variants.

#### 2. Add `ws_uri` to Session (fdemon-app)

In `crates/fdemon-app/src/session.rs`, add after the `app_id` field (~line 254):

```rust
/// VM Service WebSocket URI (from app.debugPort event)
pub ws_uri: Option<String>,
```

Initialize to `None` in `Session::new()`. Clear on session stop (when `app_id` is cleared).

#### 3. Handle AppDebugPort (fdemon-app)

In `crates/fdemon-app/src/handler/session.rs`, add to `handle_session_message_state()` after the `AppStop` handler:

```rust
// Handle app.debugPort event — capture VM Service URI
if let DaemonMessage::AppDebugPort(debug_port) = msg {
    if let Some(handle) = state.session_manager.get_mut(session_id) {
        if handle.session.app_id.as_ref() == Some(&debug_port.app_id) {
            handle.session.ws_uri = Some(debug_port.ws_uri.clone());
            tracing::info!(
                "Session {} VM Service ready: ws_uri={}",
                session_id,
                debug_port.ws_uri
            );
        }
    }
}
```

#### 4. Sync to SharedState (fdemon-app)

In `crates/fdemon-app/src/engine.rs`, line ~303, replace:
```rust
app_state.devtools_uri = None; // Not tracked in Session yet
```
with:
```rust
app_state.devtools_uri = session.ws_uri.clone();
```

### Acceptance Criteria

1. `LogSource::VmService` variant exists and compiles with all match arms updated
2. `Session` struct has `ws_uri: Option<String>` field
3. When `app.debugPort` daemon event arrives, `ws_uri` is stored in the session
4. `SharedState.devtools_uri` is populated from `session.ws_uri` (no longer `None`)
5. `ws_uri` is cleared when session stops (same time `app_id` clears)
6. Existing tests pass — no regressions
7. New tests cover the `AppDebugPort` handler

### Testing

```rust
#[cfg(test)]
mod tests {
    #[test]
    fn test_handle_app_debug_port_stores_ws_uri() {
        // Create state with a session that has app_id = "test-app"
        // Send DaemonMessage::AppDebugPort { app_id: "test-app", port: 8080, ws_uri: "ws://..." }
        // Assert session.ws_uri == Some("ws://...")
    }

    #[test]
    fn test_handle_app_debug_port_ignores_wrong_app_id() {
        // Create state with session app_id = "test-app"
        // Send AppDebugPort with app_id = "other-app"
        // Assert session.ws_uri remains None
    }

    #[test]
    fn test_ws_uri_cleared_on_app_stop() {
        // Set ws_uri, then send AppStop
        // Assert ws_uri is None
    }

    #[test]
    fn test_log_source_vm_service_prefix() {
        assert_eq!(LogSource::VmService.prefix(), "vm");
    }
}
```

### Notes

- The `AppDebugPort` event struct already exists in `fdemon-core/src/events.rs` — no changes needed there
- This task is independent of the WebSocket dependencies (Task 01) since it only stores the URI string
- The `ExceptionBlockParser` is NOT modified — it remains as-is for fallback

---

## Completion Summary

**Status:** Not Started
