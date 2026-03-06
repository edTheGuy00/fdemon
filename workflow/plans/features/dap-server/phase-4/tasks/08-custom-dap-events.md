## Task: Emit Custom DAP Events for IDE Integration

**Objective**: Send Flutter-specific custom DAP events that IDEs use for rich integration: `dart.debuggerUris` (VM Service URI on attach), `flutter.appStarted` (when session reaches Running), `flutter.appStart` (device/mode metadata), and `dart.serviceExtensionAdded` (when Flutter extensions register).

**Depends on**: 01-wire-debug-event-channel

**Estimated Time**: 2–3 hours

### Scope

- `crates/fdemon-dap/src/adapter/mod.rs`: Emit custom events at appropriate lifecycle points
- `crates/fdemon-dap/src/server/session.rs`: Forward EngineEvent-triggered custom events

### Details

#### Events to Implement

##### 1. `dart.debuggerUris` — On Attach

Sent immediately after a successful `attach` response. IDEs use this to connect their own Dart tooling to the VM Service.

```json
{
  "type": "event",
  "event": "dart.debuggerUris",
  "body": {
    "vmServiceUri": "ws://127.0.0.1:12345/ws"
  }
}
```

**Implementation**: In `handle_attach()`, after the backend connects successfully, emit this event with the VM Service WebSocket URI from the session's connection info.

Add `ws_uri() -> Option<String>` to `DebugBackend` trait if not already present. `VmServiceBackend` returns the URI from its `VmRequestHandle`.

##### 2. `flutter.appStart` — On Attach

Provides device and mode metadata for the debug session.

```json
{
  "type": "event",
  "event": "flutter.appStart",
  "body": {
    "deviceId": "emulator-5554",
    "mode": "debug",
    "supportsRestart": true
  }
}
```

**Implementation**: In `handle_attach()`, retrieve device info and mode from the backend or session metadata.

##### 3. `flutter.appStarted` — When Session is Running

Sent when the Flutter app is fully started and ready for interaction.

```json
{
  "type": "event",
  "event": "flutter.appStarted",
  "body": {}
}
```

**Implementation**: Listen for `EngineEvent` indicating the session phase reached `Running`. The adapter subscribes to engine events via the `debug_event_rx` channel — add an `AppStarted` variant to `DebugEvent` and forward it from the TEA handler when the session phase transitions to `Running`.

Alternatively, if `AppPhase::Running` is already reached before the DAP client connects, emit this event immediately after attach.

##### 4. `dart.serviceExtensionAdded` — On Extension Registration

Sent when a Flutter service extension registers (e.g., `ext.flutter.inspector.show`).

```json
{
  "type": "event",
  "event": "dart.serviceExtensionAdded",
  "body": {
    "extensionRPC": "ext.flutter.inspector.show",
    "isolateId": "isolates/1234567890"
  }
}
```

**Implementation**: Listen for `VmServiceEvent::ServiceExtensionAdded` events. These are already parsed in the VM Service client. Forward them through the debug event channel as `DebugEvent::ServiceExtensionAdded { extension, isolate_id }`.

**Note**: This event is lower priority. Many IDEs don't use it. Implement if straightforward, otherwise defer.

### Acceptance Criteria

1. `dart.debuggerUris` sent after successful attach with correct VM Service URI
2. `flutter.appStarted` sent when session reaches Running phase
3. Custom events are well-formed JSON matching Flutter DAP convention
4. Events are only sent when a DAP client is connected (no sending to void)
5. All existing tests pass
6. 6+ new unit tests

### Testing

```rust
#[tokio::test]
async fn test_attach_emits_debugger_uris() {
    let (adapter, event_rx) = create_test_adapter();
    adapter.handle_attach(&attach_request).await;
    let events = collect_events(event_rx);
    assert!(events.iter().any(|e| e.event == "dart.debuggerUris"));
}

#[tokio::test]
async fn test_attach_emits_app_start() {
    let (adapter, event_rx) = create_test_adapter();
    adapter.handle_attach(&attach_request).await;
    let events = collect_events(event_rx);
    assert!(events.iter().any(|e| e.event == "flutter.appStart"));
}

#[tokio::test]
async fn test_app_started_event_on_running() {
    // Simulate DebugEvent::AppStarted
    // Verify flutter.appStarted event emitted
}
```

### Notes

- These events are Flutter/Dart-convention, not DAP-standard. Non-Flutter-aware IDEs will simply ignore them.
- The `dart.debuggerUris` event is particularly important for VS Code's Dart extension, which uses it to connect supplementary tooling (DevTools browser, etc.).
- `flutter.appStart.supportsRestart` should match whether hot restart is available (debug builds: true, profile/release: false).
- Zed may or may not consume these events currently. They are forward-compatible and add zero cost.
