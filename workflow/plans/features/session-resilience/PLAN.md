# Plan: Session Resilience — Connection Recovery & Health Monitoring

## TL;DR

Three reliability gaps in session lifecycle management: (1) the `VmServiceReconnecting` message is fully wired in handlers and TUI but never emitted, leaving users blind during WebSocket reconnection backoff; (2) network polling tasks are not cleaned up on process exit or AppStop, creating zombie polling loops; (3) no heartbeat or health-check mechanism exists, so a hung Flutter process appears alive indefinitely. This plan addresses all three with minimal, targeted changes across the daemon and app layers.

---

## Background

Flutter Demon manages long-running Flutter sessions with concurrent WebSocket connections to the Dart VM Service, performance monitoring, and network profiling. When connections drop, processes hang, or sessions crash, the tool needs to detect these failures promptly and present clear status to the user.

Currently:
- The VM Service WebSocket client auto-reconnects transparently at the daemon layer (up to 10 attempts, 1–30s exponential backoff), but the TEA/UI layer is never informed — users see silence, not a "Reconnecting..." indicator.
- Process exit cleanup stops performance polling but leaves network polling running as a zombie.
- The only process death signal is stdout EOF; a hung process that produces no output stays "Running" forever.

---

## Affected Modules

- `crates/fdemon-daemon/src/vm_service/client.rs` — Reconnect loop, ConnectionState, new `getVersion` RPC
- `crates/fdemon-daemon/src/vm_service/protocol.rs` — VmServiceEvent type (new variant for connection state changes)
- `crates/fdemon-app/src/actions.rs` — `forward_vm_events`, `spawn_session`, `spawn_network_monitoring`
- `crates/fdemon-app/src/handler/session.rs` — `handle_session_exited`, `handle_session_message_state` (AppStop)
- `crates/fdemon-app/src/handler/update.rs` — Minor: verify existing handlers
- `crates/fdemon-app/src/message.rs` — Minor: verify existing message variants
- `crates/fdemon-app/src/session/handle.rs` — New health-tracking fields (Phase 3 only)

---

## Development Phases

### Phase 1: Network Polling Cleanup (Bug Fix)

**Goal**: Stop network polling zombie tasks on process exit and AppStop events.

**Priority**: High — this is a resource leak and correctness bug that exists today.

#### Steps

1. **Add network cleanup to `handle_session_exited`**
   - File: `crates/fdemon-app/src/handler/session.rs` (after the perf cleanup block ~line 134)
   - Mirror the existing perf cleanup pattern: `network_task_handle.take().abort()` + `network_shutdown_tx.take().send(true)`
   - Log at `info!` level matching the perf cleanup style

2. **Add network cleanup to AppStop handler**
   - File: `crates/fdemon-app/src/handler/session.rs` in `handle_session_message_state` (after perf cleanup ~line 187)
   - Same pattern as step 1

3. **Add unit tests**
   - Test that `handle_session_exited` clears `network_task_handle` and `network_shutdown_tx`
   - Test that AppStop event clears the same fields
   - Pattern: follow existing test at `handler/tests.rs:4252–4294` which covers `CloseCurrentSession` + network cleanup

**Milestone**: Network polling stops correctly in all session termination paths (exit, AppStop, close, VM disconnect).

---

### Phase 2: Emit VmServiceReconnecting Message

**Goal**: Surface VM Service reconnection status to the UI so users see "Reconnecting (2/10)..." instead of silence.

**Priority**: Medium — UX improvement; the reconnection itself already works, users just can't see it.

#### Approach Decision: Polling vs Event Channel

Two approaches were evaluated:

| Approach | Pros | Cons |
|----------|------|------|
| **A: New VmServiceEvent variant** — `run_client_task` emits a `ConnectionStateChanged` event through the existing `event_tx` channel | Clean separation, daemon layer owns its events | Requires changing `VmServiceEvent` from struct to enum or adding a new event type; protocol.rs refactor |
| **B: Poll `connection_state()` from `forward_vm_events`** — Add a `tokio::time::interval` arm that reads `client.connection_state()` and emits `VmServiceReconnecting` on transitions | No daemon-layer changes; uses existing public API | Polling interval introduces latency (up to 1 tick delay); slightly more complex select loop |

**Recommended: Approach A** — daemon layer emits events. This is cleaner, avoids polling, and aligns with the existing pattern where state changes originate from the source.

#### Steps

1. **Add connection lifecycle events to daemon event channel**
   - File: `crates/fdemon-daemon/src/vm_service/protocol.rs`
   - Add a `VmClientEvent` enum that wraps both `VmServiceEvent` (stream notifications) and new connection lifecycle variants:
     ```
     enum VmClientEvent {
         StreamEvent(VmServiceEvent),
         Reconnecting { attempt: u32, max_attempts: u32 },
         Reconnected,
         PermanentlyDisconnected,
     }
     ```
   - Update the `event_tx`/`event_rx` channel type from `mpsc::channel<VmServiceEvent>` to `mpsc::channel<VmClientEvent>`

2. **Emit lifecycle events from `run_client_task`**
   - File: `crates/fdemon-daemon/src/vm_service/client.rs`
   - At `ConnectionState::Reconnecting { attempt }` transition (~line 617): send `VmClientEvent::Reconnecting { attempt, max_attempts: MAX_RECONNECT_ATTEMPTS }`
   - At `ConnectionState::Connected` after reconnect (~line 637): send `VmClientEvent::Reconnected`
   - At final `ConnectionState::Disconnected` (~line 610, 631): send `VmClientEvent::PermanentlyDisconnected`
   - Export `MAX_RECONNECT_ATTEMPTS` as a public constant

3. **Update `forward_vm_events` to handle new event types**
   - File: `crates/fdemon-app/src/actions.rs`
   - Match on `VmClientEvent::Reconnecting` → send `Message::VmServiceReconnecting { session_id, attempt, max_attempts }`
   - Match on `VmClientEvent::Reconnected` → send `Message::VmServiceConnected { session_id }` (reuses existing handler which resets status)
   - Match on `VmClientEvent::PermanentlyDisconnected` → let the `None` branch handle it (channel closes after this)
   - Update all `VmClientEvent::StreamEvent(event)` matches to unwrap the inner event

4. **Update `VmServiceClient` event receiver API**
   - File: `crates/fdemon-daemon/src/vm_service/client.rs`
   - `event_receiver()` now returns `&mut mpsc::Receiver<VmClientEvent>` instead of `VmServiceEvent`
   - Update all callers

5. **Add unit tests**
   - Test that `VmServiceReconnecting` message flows through to state correctly (handler already exists and is tested, but add an integration-style test from emit to state change)
   - Test that `Reconnected` resets `VmConnectionStatus` to `Connected`

**Milestone**: Users see "Reconnecting (N/10)..." in the DevTools tab bar and panel headers during VM Service backoff windows.

---

### Phase 3: Process Health Monitoring

**Goal**: Detect hung Flutter processes and stale VM Service connections through periodic health checks.

**Priority**: Medium-low — edge case (hung processes are uncommon), but when it happens, the session is permanently stuck.

#### Steps

1. **Add process watchdog to `spawn_session` event loop**
   - File: `crates/fdemon-app/src/actions.rs` (~line 409)
   - Add a third `tokio::select!` arm with `tokio::time::interval(Duration::from_secs(5))`
   - On tick: call `process.has_exited()` (already exists on `FlutterProcess`, lines 278–280)
   - If exited: send `DaemonEvent::Exited { code: None }` and set `process_exited = true`; break
   - This catches the case where the process dies without closing stdout cleanly (e.g., SIGKILL)
   - Watchdog interval: 5 seconds (balances responsiveness vs overhead)

2. **Add `getVersion` RPC to VmServiceClient** (optional, lightweight probe)
   - File: `crates/fdemon-daemon/src/vm_service/client.rs`
   - Add `pub async fn get_version(&self) -> Result<(u32, u32)>` calling `"getVersion"` RPC
   - This is the lightest possible VM Service probe — no isolate ID needed, returns major/minor version
   - Useful for Phase 3 step 3 and general diagnostic purposes

3. **Add VM Service heartbeat to `forward_vm_events`**
   - File: `crates/fdemon-app/src/actions.rs` (~line 949)
   - Add a third `tokio::select!` arm with `tokio::time::interval(Duration::from_secs(30))`
   - On tick: call `client.get_version()` (or `client.get_vm()`) with a `tokio::time::timeout(5s)` wrapper
   - On timeout or error: increment a failure counter
   - After 3 consecutive failures: break the loop (triggers `VmServiceDisconnected`)
   - On success: reset failure counter
   - This detects a silently-dead VM Service connection where no close frame was received

4. **Capture actual process exit code**
   - File: `crates/fdemon-daemon/src/process.rs`
   - In `stdout_reader`: after the read loop exits, call `child.try_wait()` to get the actual exit code
   - Challenge: `stdout_reader` does not own `child`. Two options:
     - Pass an `Arc<Mutex<Child>>` to `stdout_reader` (moderate refactor)
     - Add a separate `wait_for_exit` task that calls `child.wait().await` and sends exit code
   - **Recommended**: spawn a separate `wait_for_exit` task alongside stdout/stderr readers. This task calls `child.wait().await` and sends `DaemonEvent::Exited { code: Some(code) }`. Remove the `Exited` emission from `stdout_reader`. This cleanly separates concerns and always captures the exit code.

5. **Add health status to `SessionHandle`** (optional, for UI display)
   - File: `crates/fdemon-app/src/session/handle.rs`
   - Add `pub last_heartbeat: Option<std::time::Instant>` field
   - Update on each successful watchdog check
   - Could be used to show "Last seen: 30s ago" in UI for unresponsive sessions

6. **Add unit tests**
   - Test that the watchdog detects a mock process exit
   - Test that VM heartbeat failure triggers `VmServiceDisconnected` after 3 failures
   - Test that `wait_for_exit` task sends the actual exit code

**Milestone**: Hung processes are detected within 5 seconds; stale VM connections are detected within ~90 seconds (3 × 30s interval). Session status accurately reflects process liveness.

---

## Edge Cases & Risks

### Race Conditions
- **Risk:** Process watchdog and stdout EOF both detect exit simultaneously, producing duplicate `DaemonEvent::Exited` messages.
- **Mitigation:** Guard with `process_exited` flag (already exists in `spawn_session` loop). First event sets it; second is ignored.

### Reconnection + Heartbeat Interaction
- **Risk:** VM heartbeat probe fires during a reconnection backoff window, sees "disconnected", and prematurely triggers `VmServiceDisconnected`.
- **Mitigation:** The heartbeat runs in `forward_vm_events`, which is a consumer of the event channel. During reconnection, the background task is reconnecting — `client.get_version()` would fail because `cmd_rx` is being serviced by `run_io_loop` or the task is between connections. The client should return an appropriate error that the heartbeat classifies as "transient". Use the failure counter (3 consecutive) to avoid premature disconnect.

### Network Polling Error Tolerance
- **Risk:** The network polling loop (`actions.rs:1755–1764`) intentionally retries on errors (for transient isolate-paused states). The Phase 1 fix stops it via shutdown signal, not by changing its error tolerance.
- **Mitigation:** Phase 1 only adds explicit cleanup calls in missing paths. The polling loop's retry behavior is not modified.

### VmClientEvent Channel Backpressure
- **Risk:** Adding lifecycle events to the event channel (capacity 256) could cause backpressure if the consumer is slow.
- **Mitigation:** Lifecycle events are rare (at most ~10 reconnect attempts). No meaningful backpressure risk.

### Backward Compatibility
- **Risk:** Changing `VmServiceEvent` to `VmClientEvent` is a breaking API change for the daemon crate.
- **Mitigation:** The daemon crate is an internal workspace dependency, not a published crate. All consumers are within the workspace.

---

## Task Dependency Graph

```
Phase 1 (Bug Fix)                Phase 2 (Reconnecting UI)
├── 01-network-cleanup-exit      ├── 04-vm-client-event-type
├── 02-network-cleanup-appstop   ├── 05-emit-reconnect-events
└── 03-network-cleanup-tests     ├── 06-forward-events-update
                                 └── 07-reconnecting-tests

Phase 2b (Reconnect Handler Fixes)     Phase 3 (Health Monitoring)
├── 01-reconnected-message-variant     ├── 01-process-watchdog        ─┐
├── 02-cleanup-perf-on-reconnect       ├── 02-get-version-rpc         ─┤ Wave 1
├── 03-guard-connection-status         ├── 04-wait-for-exit-task      ─┘
└── 04-reconnect-handler-tests         ├── 03-vm-heartbeat (→02)       Wave 2
                                       └── 05-health-monitoring-tests  Wave 3
```

---

## Success Criteria

### Phase 1 Complete When:
- [ ] `handle_session_exited` clears `network_task_handle` and `network_shutdown_tx`
- [ ] AppStop handler clears `network_task_handle` and `network_shutdown_tx`
- [ ] All existing tests pass + new tests for both cleanup paths
- [ ] `cargo clippy --workspace -- -D warnings` clean

### Phase 2 Complete When:
- [ ] `VmServiceReconnecting` message is emitted during WebSocket reconnection backoff
- [ ] DevTools tab bar shows "Reconnecting (N/10)..." during backoff window
- [ ] Inspector/Performance/Network panels show reconnection status
- [ ] `VmConnectionStatus` resets to `Connected` after successful reconnection
- [ ] All existing tests pass + new tests for reconnection message flow

### Phase 2b Complete When:
- [ ] `Message::VmServiceReconnected` variant exists and is used for WebSocket reconnection
- [ ] Reconnection preserves accumulated `PerformanceState` (memory history, frame timings, etc.)
- [ ] Old performance polling task aborted before spawning new one on reconnect
- [ ] `connection_status` writes guarded by active-session check in all VM lifecycle handlers
- [ ] Background session VM events don't pollute foreground connection indicator
- [ ] All existing tests pass + new tests for all three fix areas

### Phase 3 Complete When:
- [ ] Hung process detected within 5 seconds via watchdog
- [ ] Stale VM connection detected within ~90 seconds via heartbeat
- [ ] Actual process exit code captured and surfaced in session log
- [ ] All existing tests pass + new tests for watchdog and heartbeat

---

## Future Enhancements

- **Auto-restart on crash**: Optionally restart a session when the Flutter process crashes (requires user confirmation or config flag)
- **Escalate reload to restart**: Detect "needs full restart" in hot reload failure reasons and auto-suggest/trigger hot restart
- **Session health dashboard**: Show per-session health indicators in the tab bar (green/yellow/red dots)
- **Configurable timeouts**: Expose watchdog interval, heartbeat interval, and failure threshold in `.fdemon/config.toml`

---

## References

- Existing deferred work note: `workflow/plans/features/devtools-integration/phase-5/tasks/02-connection-state-ui.md:257`
- Dart VM Service Protocol: `getVersion` and `getVM` RPCs
- TEA pattern: `docs/ARCHITECTURE.md` § Data Flow
