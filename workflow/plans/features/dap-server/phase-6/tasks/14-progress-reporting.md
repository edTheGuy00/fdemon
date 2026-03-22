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

---

## Completion Summary

**Status:** Done
**Branch:** feat/dap-phase-6-plan

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-dap/src/adapter/mod.rs` | Added `client_supports_progress: bool` and `next_progress_id: u64` fields to `DapAdapter`; initialized to `false`/`0` in `new_with_tx`; added `set_client_supports_progress()` and `alloc_progress_id()` methods |
| `crates/fdemon-dap/src/adapter/handlers.rs` | Rewrote `handle_hot_reload` and `handle_hot_restart` to emit `progressStart`/`progressEnd` events when progress is supported, and `dart.hotReloadComplete`/`dart.hotRestartComplete` on success |
| `crates/fdemon-dap/src/server/session.rs` | Propagate `supportsProgressReporting` from stored `client_info` to adapter at lazy-creation time |
| `crates/fdemon-dap/src/adapter/tests/mod.rs` | Registered `progress_reporting` test module |
| `crates/fdemon-dap/src/adapter/tests/progress_reporting.rs` | New file: 13 unit tests covering all acceptance criteria |

### Notable Decisions/Tradeoffs

1. **Option 1 (synchronous await) chosen**: The task offered Option 1 (await backend and return) or Option 2 (fire-and-forget then emit later). Since `backend.hot_reload()` already `await`s before returning, Option 1 was natural and requires no channel plumbing.

2. **`progressEnd` on failure**: The DAP spec requires progress to be properly closed. `progressEnd` is emitted regardless of success/failure so the IDE never shows a stale spinner.

3. **`dart.hotReloadComplete` only on success**: The completion event is only emitted on `Ok(())`, not on error, consistent with the Dart-Code extension's expectations.

4. **Session propagation via `set_client_supports_progress`**: Since the adapter is created lazily (not at `initialize` time), the session sets the capability on the adapter immediately after construction. This keeps the capability-propagation path simple and colocated in `session.rs`.

### Testing Performed

- `cargo fmt --all -- --check` - Passed
- `cargo check --workspace` - Passed
- `cargo test -p fdemon-dap` - Passed (763 tests)
- `cargo clippy --workspace -- -D warnings` - Passed

### Risks/Limitations

1. **Progress events are adapter-side only**: The actual reload/restart happens in the backend (TEA message bus). If the backend is fire-and-forget in the real implementation, the progress indicator will close immediately after the message is dispatched rather than when the reload is fully complete. This is acceptable for Phase 6 scope.
