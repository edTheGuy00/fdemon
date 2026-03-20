## Task: Implement Progress Reporting for Hot Reload/Restart

**Objective**: Emit DAP progress events (`progressStart`, `progressUpdate`, `progressEnd`) during hot reload and hot restart operations so the IDE shows a progress indicator. Also emit `dart.hotReloadComplete` and `dart.hotRestartComplete` custom events on completion.

**Depends on**: 02-expand-backend-trait

**Estimated Time**: 3–4 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-dap/src/adapter/handlers.rs`: Modify `handle_hot_reload` and `handle_hot_restart` to emit progress events and completion events
- `crates/fdemon-dap/src/adapter/events.rs`: Add hot reload/restart completion event emission
- `crates/fdemon-dap/src/protocol/types.rs`: Add progress event types

### Details

#### Check client capability:

During `initialize`, the client sends `supportsProgressReporting: bool`. Store this on the adapter:

```rust
// In session.rs or wherever InitializeRequestArguments is parsed:
self.client_supports_progress = args.supports_progress_reporting.unwrap_or(false);
```

#### Progress events:

```rust
async fn handle_hot_reload(&mut self, request: &DapRequest) -> DapResponse {
    let progress_id = format!("hot-reload-{}", self.next_progress_id());

    // Emit progressStart if client supports it
    if self.client_supports_progress {
        self.send_event("progressStart", json!({
            "progressId": progress_id,
            "title": "Hot Reload",
            "cancellable": false,
        }));
    }

    let result = self.backend.hot_reload().await;

    // Emit progressEnd
    if self.client_supports_progress {
        self.send_event("progressEnd", json!({
            "progressId": progress_id,
        }));
    }

    // Emit completion event
    self.send_event("dart.hotReloadComplete", json!({}));

    match result {
        Ok(()) => DapResponse::success(request, json!({})),
        Err(e) => DapResponse::error(request, &format!("Hot reload failed: {}", e)),
    }
}
```

Similarly for `handle_hot_restart` — emit `progressStart(title: "Hot Restart")`, `progressEnd`, and `dart.hotRestartComplete`.

#### Progress ID management:

Add `next_progress_id: u64` counter to `DapAdapter`. Each progress event pair gets a unique ID.

#### Make hot reload/restart awaitable:

Currently `backend.hot_reload()` sends `Message::HotReload` and returns immediately (fire-and-forget). To report completion, the backend needs to either:
1. Actually await the reload completion (blocking until the reload succeeds/fails)
2. Return immediately and emit the completion event later via the event channel

Option 1 is simpler for this task. Check if `VmServiceBackend::hot_reload` already awaits completion. If it's fire-and-forget (sends message via `msg_tx` and returns), change it to await a response or add a completion callback.

If changing the backend is complex, use Option 2: emit `progressStart` in the handler, return success immediately, and emit `progressEnd` + completion event when the `EngineEvent::ReloadCompleted` or similar event arrives in `events.rs`.

### Acceptance Criteria

1. Progress indicator appears in IDE during hot reload/restart
2. Progress events only sent if client advertises `supportsProgressReporting`
3. `dart.hotReloadComplete` emitted after successful hot reload
4. `dart.hotRestartComplete` emitted after successful hot restart
5. Progress ID is unique per operation
6. 6+ new unit tests

### Testing

```rust
#[tokio::test]
async fn test_hot_reload_emits_progress_events() {
    // client_supports_progress = true
    // Call handle_hot_reload
    // Verify progressStart and progressEnd events emitted
}

#[tokio::test]
async fn test_hot_reload_no_progress_when_unsupported() {
    // client_supports_progress = false
    // Call handle_hot_reload
    // Verify no progress events
}

#[tokio::test]
async fn test_hot_reload_emits_completion_event() {
    // Call handle_hot_reload
    // Verify dart.hotReloadComplete event emitted
}
```

### Notes

- The `progressStart` event has a `cancellable` field — set to `false` since hot reload/restart cannot be cancelled mid-operation.
- If the reload fails, still emit `progressEnd` (the IDE expects the progress to be properly closed).
- `dart.hotReloadComplete` and `dart.hotRestartComplete` are custom events expected by the Dart-Code extension for updating its internal state.
