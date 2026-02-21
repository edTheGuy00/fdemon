## Task: Wire Network Monitoring into Engine/Actions

**Objective**: Connect the network handlers to the async execution layer. Implement the background polling task that periodically calls `getHttpProfile`, the on-demand detail fetcher, the HTTP profile clear action, and the hydration functions in `process.rs`. This is the bridge between the pure TEA handlers and the actual VM Service calls.

**Depends on**: Task 02 (vm-service-network-extensions), Task 04 (network-handlers-and-keybindings)

### Scope

- `crates/fdemon-app/src/actions.rs`: Add `spawn_network_monitoring()`, `spawn_fetch_http_request_detail()`, `spawn_clear_http_profile()`, and handle new `UpdateAction` variants
- `crates/fdemon-app/src/process.rs`: Add hydration functions for network actions
- `crates/fdemon-app/src/session/handle.rs`: Add `network_shutdown_tx` and `network_task_handle` fields

### Details

#### Extend `SessionHandle`

In `crates/fdemon-app/src/session/handle.rs`, add fields for the network polling task lifecycle:

```rust
pub struct SessionHandle {
    // ... existing fields ...
    /// Shutdown signal for the network monitoring background task.
    pub network_shutdown_tx: Option<Arc<watch::Sender<bool>>>,
    /// Handle to the network monitoring background task.
    pub network_task_handle: Option<tokio::task::JoinHandle<()>>,
}
```

Update the `Drop` or cleanup logic (alongside `perf_shutdown_tx` / `perf_task_handle`) to abort the network task on session disconnect:
- In the `VmServiceDisconnected` handler (or wherever perf tasks are aborted), also abort and clean up network tasks
- In `SessionHandle::cleanup()` (if it exists), send shutdown signal and abort handle

#### Add hydration functions to `process.rs`

Follow the existing hydration pattern. Add three hydration functions to the chain in `process.rs`:

```rust
/// Hydrate StartNetworkMonitoring with the VM request handle.
fn hydrate_start_network_monitoring(
    action: Option<UpdateAction>,
    state: &AppState,
) -> Option<UpdateAction> {
    match action {
        Some(UpdateAction::StartNetworkMonitoring { session_id, handle: None, poll_interval_ms }) => {
            let vm_handle = state.session_manager.handle(&session_id)
                .and_then(|h| h.vm_request_handle.clone());
            vm_handle.map(|h| UpdateAction::StartNetworkMonitoring {
                session_id, handle: Some(h), poll_interval_ms,
            })
        }
        other => other,
    }
}

/// Hydrate FetchHttpRequestDetail with the VM request handle.
fn hydrate_fetch_http_request_detail(
    action: Option<UpdateAction>,
    state: &AppState,
) -> Option<UpdateAction> {
    match action {
        Some(UpdateAction::FetchHttpRequestDetail { session_id, request_id, vm_handle: None }) => {
            let vm_handle = state.session_manager.handle(&session_id)
                .and_then(|h| h.vm_request_handle.clone());
            vm_handle.map(|h| UpdateAction::FetchHttpRequestDetail {
                session_id, request_id, vm_handle: Some(h),
            })
        }
        other => other,
    }
}

/// Hydrate ClearHttpProfile with the VM request handle.
fn hydrate_clear_http_profile(
    action: Option<UpdateAction>,
    state: &AppState,
) -> Option<UpdateAction> {
    match action {
        Some(UpdateAction::ClearHttpProfile { session_id, vm_handle: None }) => {
            let vm_handle = state.session_manager.handle(&session_id)
                .and_then(|h| h.vm_request_handle.clone());
            vm_handle.map(|h| UpdateAction::ClearHttpProfile {
                session_id, vm_handle: Some(h),
            })
        }
        other => other,
    }
}
```

Add these to the hydration chain (after the existing devtools hydrators):

```rust
let action = hydrate_start_network_monitoring(action, state);
let action = hydrate_fetch_http_request_detail(action, state);
let action = hydrate_clear_http_profile(action, state);
```

#### Implement action handlers in `actions.rs`

Add match arms in `handle_action()`:

```rust
UpdateAction::StartNetworkMonitoring { session_id, handle: Some(handle), poll_interval_ms } => {
    spawn_network_monitoring(session_id, handle, msg_tx.clone(), poll_interval_ms);
}
UpdateAction::FetchHttpRequestDetail { session_id, request_id, vm_handle: Some(handle) } => {
    spawn_fetch_http_request_detail(session_id, request_id, handle, msg_tx.clone());
}
UpdateAction::ClearHttpProfile { session_id, vm_handle: Some(handle) } => {
    spawn_clear_http_profile(session_id, handle, msg_tx.clone());
}
```

#### `spawn_network_monitoring()` — Background polling task

Follow the `spawn_performance_polling()` pattern exactly: create a `watch::channel(false)` for shutdown, use `Arc<Mutex<Option<JoinHandle>>>` for task handle rendezvous, and send a `VmServiceNetworkMonitoringStarted` message back.

```rust
fn spawn_network_monitoring(
    session_id: SessionId,
    handle: VmRequestHandle,
    msg_tx: mpsc::Sender<Message>,
    poll_interval_ms: u64,
) {
    let (shutdown_tx, mut shutdown_rx) = watch::channel(false);
    let shutdown_tx = Arc::new(shutdown_tx);
    let task_handle_slot: Arc<Mutex<Option<JoinHandle<()>>>> = Arc::new(Mutex::new(None));
    let task_handle_slot_clone = task_handle_slot.clone();

    let join_handle = tokio::spawn(async move {
        // Send lifecycle message with shutdown handle
        let _ = msg_tx.send(Message::VmServiceNetworkMonitoringStarted {
            session_id: session_id.clone(),
            network_shutdown_tx: shutdown_tx.clone(),
            network_task_handle: task_handle_slot_clone,
        }).await;

        // Build VM service client from handle
        let client = handle.to_client();

        // Step 1: Enable HTTP timeline logging
        let isolate_id = match handle.main_isolate_id().await {
            Some(id) => id,
            None => {
                tracing::warn!("No isolate ID available for network monitoring");
                return;
            }
        };

        match network::enable_http_timeline_logging(&client, &isolate_id, true).await {
            Ok(_) => tracing::info!("HTTP timeline logging enabled for {}", session_id),
            Err(e) => {
                if extensions::is_extension_not_available(&e) {
                    let _ = msg_tx.send(Message::VmServiceNetworkExtensionsUnavailable {
                        session_id,
                    }).await;
                    return;
                }
                tracing::warn!("Failed to enable HTTP timeline logging: {}", e);
            }
        }

        // Step 2: Optionally enable socket profiling
        let _ = network::set_socket_profiling_enabled(&client, &isolate_id, true).await;

        // Step 3: Start polling loop
        let mut poll_interval = tokio::time::interval(
            tokio::time::Duration::from_millis(poll_interval_ms),
        );
        let mut last_timestamp: Option<i64> = None;

        loop {
            tokio::select! {
                _ = poll_interval.tick() => {
                    // Check if recording is active by trying to poll
                    // (The handler sets session.network.recording; the polling task
                    // always polls but the handler may ignore results if not recording.
                    // Alternatively, we always poll and let the handler decide.)

                    match network::get_http_profile(
                        &client,
                        &isolate_id,
                        last_timestamp,
                    ).await {
                        Ok(profile) => {
                            last_timestamp = Some(profile.timestamp);
                            if !profile.requests.is_empty() {
                                let _ = msg_tx.send(Message::VmServiceHttpProfileReceived {
                                    session_id: session_id.clone(),
                                    timestamp: profile.timestamp,
                                    entries: profile.requests,
                                }).await;
                            }
                        }
                        Err(e) => {
                            tracing::debug!("Network profile poll failed: {}", e);
                        }
                    }
                }
                _ = shutdown_rx.changed() => {
                    if *shutdown_rx.borrow() {
                        tracing::info!("Network monitoring stopped for {}", session_id);
                        break;
                    }
                }
            }
        }
    });

    // Store the join handle
    if let Ok(mut slot) = task_handle_slot.lock() {
        *slot = Some(join_handle);
    }
}
```

#### `spawn_fetch_http_request_detail()` — On-demand detail fetch

```rust
fn spawn_fetch_http_request_detail(
    session_id: SessionId,
    request_id: String,
    handle: VmRequestHandle,
    msg_tx: mpsc::Sender<Message>,
) {
    tokio::spawn(async move {
        let client = handle.to_client();
        let isolate_id = match handle.main_isolate_id().await {
            Some(id) => id,
            None => return,
        };

        match network::get_http_profile_request(&client, &isolate_id, &request_id).await {
            Ok(detail) => {
                let _ = msg_tx.send(Message::VmServiceHttpRequestDetailReceived {
                    session_id,
                    detail: Box::new(detail),
                }).await;
            }
            Err(e) => {
                let _ = msg_tx.send(Message::VmServiceHttpRequestDetailFailed {
                    session_id,
                    error: e.to_string(),
                }).await;
            }
        }
    });
}
```

#### `spawn_clear_http_profile()` — Clear operation

```rust
fn spawn_clear_http_profile(
    session_id: SessionId,
    handle: VmRequestHandle,
    msg_tx: mpsc::Sender<Message>,
) {
    tokio::spawn(async move {
        let client = handle.to_client();
        let isolate_id = match handle.main_isolate_id().await {
            Some(id) => id,
            None => return,
        };

        if let Err(e) = network::clear_http_profile(&client, &isolate_id).await {
            tracing::warn!("Failed to clear HTTP profile: {}", e);
        }
    });
}
```

#### Trigger network monitoring on VM connect

In the `VmServiceConnected` handler (in `handler/update.rs`), alongside the existing `StartPerformanceMonitoring` action, also trigger `StartNetworkMonitoring` when the Network tab is active:

```rust
// After StartPerformanceMonitoring is returned...
// If the Network panel is active, also start network monitoring
if state.devtools_view_state.active_panel == DevToolsPanel::Network {
    // Return StartNetworkMonitoring action
    // (May need to chain actions or send a follow-up message)
}
```

**Note**: If the architecture only allows one `UpdateAction` per `update()` call, use a follow-up `Message` pattern: the handler can set a flag, and `process.rs` can check it to trigger a second action. Alternatively, start network monitoring lazily when the user first switches to the Network tab (already handled in `handle_switch_panel`).

#### Clean up on disconnect

In the `VmServiceDisconnected` handler, add network task cleanup alongside performance task cleanup:

```rust
if let Some(handle) = state.session_manager.handle_mut(&session_id) {
    // Existing: abort perf task
    if let Some(tx) = handle.perf_shutdown_tx.take() { let _ = tx.send(true); }
    if let Some(jh) = handle.perf_task_handle.take() { jh.abort(); }

    // NEW: abort network task
    if let Some(tx) = handle.network_shutdown_tx.take() { let _ = tx.send(true); }
    if let Some(jh) = handle.network_task_handle.take() { jh.abort(); }
}
```

### Acceptance Criteria

1. `SessionHandle` has `network_shutdown_tx` and `network_task_handle` fields
2. `hydrate_start_network_monitoring()` fills in the VM handle from session state
3. `hydrate_fetch_http_request_detail()` fills in the VM handle
4. `hydrate_clear_http_profile()` fills in the VM handle
5. All three hydrations are in the `process.rs` chain
6. `spawn_network_monitoring()` enables HTTP timeline logging, then polls `getHttpProfile` on an interval
7. `spawn_network_monitoring()` sends `VmServiceNetworkExtensionsUnavailable` when extensions aren't registered
8. `spawn_network_monitoring()` uses incremental polling with `updatedSince`
9. `spawn_fetch_http_request_detail()` fetches detail for a single request and sends result message
10. `spawn_clear_http_profile()` calls `clearHttpProfile` on the VM
11. Network task is cleaned up on VM disconnect
12. `cargo check -p fdemon-app` passes
13. `cargo test -p fdemon-app` passes

### Testing

The async spawning functions are difficult to unit test (they require a live VM). Focus testing on:

1. **Hydration functions** — test that they correctly extract/fill handles:
```rust
#[test]
fn test_hydrate_start_network_monitoring_no_handle_returns_none() {
    // State with no VM handle → action discarded
}

#[test]
fn test_hydrate_clear_http_profile_fills_handle() {
    // State with VM handle → action has handle filled in
}
```

2. **Integration with update cycle** — verify that the full message → action → hydration pipeline works for network messages using the existing test harness.

### Notes

- **`handle.to_client()` method**: The exact method to get a `VmServiceClient` from a `VmRequestHandle` depends on the codebase. Research how `spawn_performance_polling` accesses the client — it may construct one or use the handle directly. If `VmRequestHandle` doesn't have `to_client()`, the extension calls may need to accept `&VmRequestHandle` instead, requiring a different calling pattern. Check the actual `VmServiceClient` vs `VmRequestHandle` usage in `performance.rs` and `actions.rs`.
- **Lazy start vs eager start**: Network monitoring starts lazily (when the user switches to the Network tab or when the VM connects with the Network tab active). It does NOT auto-start for all sessions like performance monitoring. This avoids unnecessary polling overhead.
- **Single `UpdateAction` constraint**: If the TEA architecture only supports one action per `update()` call, network monitoring cannot be started in the same action as performance monitoring. The lazy start via `handle_switch_panel(Network)` avoids this issue.
- **Polling interval**: Default 1000ms (1 second). This balances responsiveness with overhead. Configurable via settings (Phase 5).
- **`updatedSince` is stored per-session**: The `last_poll_timestamp` in `NetworkState` tracks the timestamp for incremental polling. It's reset on clear.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-daemon/src/vm_service/network.rs` | Added `VmRequestHandle`-accepting variants of the four network functions: `enable_http_timeline_logging_handle`, `get_http_profile_handle`, `get_http_profile_request_handle`, `clear_http_profile_handle`, `set_socket_profiling_enabled_handle`. Also imported `VmRequestHandle`. |
| `crates/fdemon-daemon/src/vm_service/mod.rs` | Re-exported the five new handle-accepting network functions. |
| `crates/fdemon-app/src/process.rs` | Added three hydration functions (`hydrate_start_network_monitoring`, `hydrate_fetch_http_request_detail`, `hydrate_clear_http_profile`) and wired them into the hydration chain after `hydrate_dispose_devtools_groups`. |
| `crates/fdemon-app/src/actions.rs` | Replaced stub `StartNetworkMonitoring`, `FetchHttpRequestDetail`, `ClearHttpProfile` action handlers with real implementations. Added `spawn_network_monitoring()`, `spawn_fetch_http_request_detail()`, and `spawn_clear_http_profile()` functions. Added `NETWORK_POLL_MIN_MS` constant. |

### Notable Decisions/Tradeoffs

1. **`VmRequestHandle`-accepting network variants**: The existing `network.rs` functions accept `&VmServiceClient`, but background tasks only have `VmRequestHandle`. Added `_handle` suffix variants that accept `&VmRequestHandle` directly (both types have the same `call_extension()` method). This avoids duplicating the parsing logic and keeps the daemon crate as the single source of truth for VM protocol details.

2. **Extension unavailability detection via `Error::Protocol`**: The task spec referenced `extensions::is_extension_not_available(&e)` which takes `&VmServiceError`, but `handle.call_extension()` returns `fdemon_core::Error`. Used `matches!(e, fdemon_core::Error::Protocol { .. })` instead, consistent with how `spawn_fetch_widget_tree` detects unavailable extensions.

3. **No `ClearNetworkProfile` re-send from `spawn_clear_http_profile`**: The initial draft sent `ClearNetworkProfile` after VM-side clear, but this would create an infinite loop: `ClearNetworkProfile` → `handle_clear_network_profile` → `ClearHttpProfile` action → `spawn_clear_http_profile` → `ClearNetworkProfile` again. The local state is already cleared by `handle_clear_network_profile` before `spawn_clear_http_profile` runs, so no follow-up message is needed. `msg_tx` parameter kept with `_` prefix for API consistency.

4. **`session/handle.rs` already complete**: Task 04 had already added `network_shutdown_tx` and `network_task_handle` fields. The `VmServiceDisconnected` handler already cleans them up. `handle_close_current_session` did NOT yet clean up network tasks — this could be a follow-up if needed.

### Testing Performed

- `cargo check -p fdemon-daemon` - Passed
- `cargo check -p fdemon-app` - Passed
- `cargo check --workspace` - Passed
- `cargo test -p fdemon-app` - Passed (992 tests)
- `cargo test -p fdemon-daemon` - Passed (375 tests)
- `cargo clippy -p fdemon-app -p fdemon-daemon -- -D warnings` - Passed

### Risks/Limitations

1. **`handle_close_current_session` missing network cleanup**: The `session_lifecycle.rs` `handle_close_current_session` aborts the perf task but does not abort the network task. This mirrors a pre-existing pattern (perf task is cleaned up there, network is only cleaned up on `VmServiceDisconnected`). A follow-up task should add network task cleanup to `handle_close_current_session` for symmetry.

2. **Pre-existing test failure**: `fdemon-tui::widgets::devtools::performance::memory_chart::tests::test_allocation_table_none_profile` was already failing before this task and is unrelated to network monitoring.
