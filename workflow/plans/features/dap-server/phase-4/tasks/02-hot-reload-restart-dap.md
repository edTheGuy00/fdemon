## Task: Hot Reload/Restart via Custom DAP Requests

**Objective**: Implement `hotReload` and `hotRestart` custom DAP requests so IDEs can trigger Flutter hot reload/restart through the debug toolbar. These are the same custom request names used by Flutter's official DAP adapter, ensuring compatibility with VS Code's Dart extension.

**Depends on**: 01-wire-debug-event-channel

**Estimated Time**: 3–4 hours

### Scope

- `crates/fdemon-dap/src/adapter/mod.rs`: Add `handle_hot_reload()` and `handle_hot_restart()` request handlers
- `crates/fdemon-dap/src/adapter/mod.rs`: Register in `handle_request()` dispatch match
- `crates/fdemon-app/src/handler/dap_backend.rs`: Add `hot_reload()` and `hot_restart()` to `DebugBackend` trait and `VmServiceBackend` impl
- `crates/fdemon-dap/src/protocol/types.rs`: Add capabilities for restart support

### Details

#### Custom Request Format

```json
// hotReload request
{
  "type": "request",
  "seq": 10,
  "command": "hotReload",
  "arguments": {
    "reason": "manual"  // optional: "manual" | "save"
  }
}

// hotRestart request
{
  "type": "request",
  "seq": 11,
  "command": "hotRestart",
  "arguments": {
    "reason": "manual"  // optional: "manual" | "save"
  }
}
```

#### Implementation

```rust
// In DebugBackend trait:
async fn hot_reload(&self) -> Result<(), String>;
async fn hot_restart(&self) -> Result<(), String>;

// In VmServiceBackend:
async fn hot_reload(&self) -> Result<(), String> {
    // Send Message::HotReload via msg_tx
    // This goes through the existing TEA pipeline which calls
    // FlutterController::reload()
    self.msg_tx.send(Message::HotReload)
        .await
        .map_err(|e| format!("Failed to send hot reload: {}", e))
}

async fn hot_restart(&self) -> Result<(), String> {
    self.msg_tx.send(Message::HotRestart)
        .await
        .map_err(|e| format!("Failed to send hot restart: {}", e))
}
```

#### Adapter Handler

```rust
fn handle_hot_reload(&self, request: &DapRequest) -> DapResponse {
    match self.backend.hot_reload().await {
        Ok(()) => DapResponse::success(request, None),
        Err(e) => DapResponse::error(request, &e),
    }
}
```

#### Custom Events After Completion

After hot reload/restart completes, send custom DAP events so the IDE can update its UI:

```json
// After hot reload succeeds:
{ "type": "event", "event": "dart.hotReloadComplete", "body": {} }

// After hot restart succeeds:
{ "type": "event", "event": "dart.hotRestartComplete", "body": {} }
```

These events require subscribing to `EngineEvent::ReloadCompleted` / `EngineEvent::RestartCompleted` in the adapter's event loop and emitting the corresponding DAP events.

#### Integration with VmServiceBackend

`VmServiceBackend` holds a `msg_tx: Sender<Message>`. Use it to send `Message::HotReload` / `Message::HotRestart` which flows through the existing TEA pipeline. The backend does NOT need to call VM Service RPCs directly — the Engine handles the reload/restart lifecycle.

### Acceptance Criteria

1. `hotReload` custom DAP request triggers Flutter hot reload and returns success
2. `hotRestart` custom DAP request triggers Flutter hot restart and returns success
3. Unknown custom requests return error response with `success: false`
4. `hotReload` / `hotRestart` return error when no Flutter session is running
5. All existing tests pass
6. 10+ new unit tests

### Testing

```rust
#[tokio::test]
async fn test_hot_reload_request_dispatches_to_backend() {
    let (adapter, backend) = create_test_adapter_with_mock();
    let req = make_request("hotReload", json!({"reason": "manual"}));
    let resp = adapter.handle_request(&req).await;
    assert!(resp.success);
    assert!(backend.hot_reload_called());
}

#[tokio::test]
async fn test_hot_restart_request_dispatches_to_backend() {
    let (adapter, backend) = create_test_adapter_with_mock();
    let req = make_request("hotRestart", json!({}));
    let resp = adapter.handle_request(&req).await;
    assert!(resp.success);
}

#[tokio::test]
async fn test_unknown_custom_request_returns_error() {
    let req = make_request("unknownCustomCommand", json!({}));
    let resp = adapter.handle_request(&req).await;
    assert!(!resp.success);
}
```

### Notes

- The `msg_tx` sender is already available on `VmServiceBackend`. No new channels needed.
- Hot restart creates a new isolate — breakpoint re-application is handled by Task 10 (breakpoint persistence).
- The `reason` field in arguments is optional and informational — it does not change behavior.
- Consider whether `hotReload` should be blocked while the debugger is paused. The official Dart adapter allows it, so we should too.
