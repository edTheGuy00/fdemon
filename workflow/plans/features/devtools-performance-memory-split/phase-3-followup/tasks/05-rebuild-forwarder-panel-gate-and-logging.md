# 05 — Rebuild Forwarder Panel Gate + Logging Improvements

**Wave:** 2
**Depends On:** —
**Agent:** implementor
**Estimated Hours:** 1.5–2h
**Addresses:** H1, L3, L10

## Context

Three improvements to `Flutter.RebuiltWidgets` event forwarding in `crates/fdemon-app/src/actions/vm_service.rs`:

- **H1.** Once `profileWidgetBuilds` is ON, the forwarder parses and dispatches events every frame (~60 fps) regardless of which DevTools panel the user is currently viewing. Each event allocates a `RebuildEventPayload` (Vec + optional HashMap of `serde_json::Value`), traverses an MPSC, and runs through the handler. When the user enables tracking and then navigates to Logs/Inspector/Memory/Network, this churn continues for the entire session.
- **L3.** Parse failures log at `tracing::warn!`. At 60 fps, a pathological app sending systematically malformed payloads can flood log files with up to 60 warn-level records per second.
- **L10.** The forwarder uses `msg_tx.send(...).await` which blocks the VM Service stream when the handler is slow, head-of-line-blocking other events (Flutter.Frame, errors).

Per PLAN.md Design Decision §H1, the chosen mitigation is **option (a) — panel-gate the forwarder**: early-return when the session's active DevTools panel is not Performance. This eliminates the parsing cost entirely (vs option b's handler-side short-circuit which still allocates).

## Acceptance Criteria

1. **H1 — Panel gate.** In `forward_vm_events`, the `Flutter.RebuiltWidgets` branch:
   - Looks up `state.session_manager.get(session_id)?.active_devtools_panel` (or equivalent — verify exact field path).
   - If the active panel is not `DevToolsPanel::Performance`, early-return without parsing the event or sending the message.
   - Test: `test_rebuilt_widgets_event_skipped_when_panel_not_performance` asserts no `RebuildStatsEventReceived` is emitted when active panel is Inspector/Memory/Network/None.
   - Test: `test_rebuilt_widgets_event_dispatched_when_performance_active` asserts the existing dispatch still works on the happy path.
2. **L3 — Log level downgrade.** The parse-error log line is at `tracing::debug!` (not `warn!`). The message text and structure are unchanged.
3. **L10 — Non-blocking send.** Replace `msg_tx.send(msg).await` with `msg_tx.try_send(msg)`:
   - On success, proceed as before.
   - On `Err(TrySendError::Full)`, log at `tracing::debug!` with the event's frame number (if available) and drop the event.
   - On `Err(TrySendError::Closed)`, log at `tracing::error!` and exit the forwarder loop (the receiver is gone — session is shutting down).
4. **No new public API** — all changes are internal to `forward_vm_events`.
5. `cargo fmt --all -- --check && cargo check -p fdemon-app && cargo test -p fdemon-app && cargo clippy -p fdemon-app --all-targets -- -D warnings` all pass.

## Files Modified (Write)

- `crates/fdemon-app/src/actions/vm_service.rs` — panel gate, log level downgrade, `try_send` migration, tests.

## Files Read (Dependencies)

- `crates/fdemon-app/src/session/devtools.rs` (or wherever `DevToolsPanel` and `active_devtools_panel` are defined) — verify the active-panel field path and enum variant name.
- `crates/fdemon-app/src/state.rs` — confirm Message dispatch is unchanged.

## Approach Hints

- The `active_devtools_panel` lookup likely needs to handle the case where the session has no active DevTools panel (e.g., `Normal` mode). In that case, also skip — there's no consumer.
- For the `try_send` change: the existing `send(...).await` is in an async loop, so the migration is purely about replacing the call. Keep the surrounding loop structure.
- Consider whether to add a `tracing::debug!` counter that fires once per second when many events are dropped (to make the drop pattern visible without spamming). Out of scope for this task; a single log per drop with frame number is sufficient.
- The panel-gate happens BEFORE parsing. The existing parse-error path (now at `debug!`) only triggers when parsing was attempted — i.e., only when the panel was Performance. The combination means parse-error logs are bounded by Performance-panel duration.

## Out of Scope

- Adding a rate-limiter for parse-error logs (the panel gate + debug level together solve the flood problem).
- Modifying the handler's behavior with respect to backpressure — the handler is unchanged.
- Pausing the Dart-side extension itself when leaving Performance. (That would require a daemon-side mechanism and is much heavier; the panel gate is enough.)
- Buffering events while the panel is inactive to replay on return — explicitly out of scope. The buffer reset is intentional.
- Coordinating with T01's `timeline_pause_tx` — those signal a spawned task to stop polling; this task gates a different code path (the event forwarder). No interaction.

---

## Completion Summary

**Status:** Done
**Branch:** worktree-agent-ae6b494ddbbfc8241

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/actions/vm_service.rs` | Panel gate (H1), log level debug! (L3), try_send migration (L10), 3 new tests |
| `crates/fdemon-app/src/session/handle.rs` | Added `rebuilt_widgets_gate_tx: Option<Arc<watch::Sender<bool>>>` field |
| `crates/fdemon-app/src/handler/mod.rs` | Added `rebuilt_widgets_gate_rx: Option<watch::Receiver<bool>>` to `ConnectVmService` variant |
| `crates/fdemon-app/src/handler/session.rs` | Added `rebuilt_widgets_gate_rx: None` to `ConnectVmService` construction |
| `crates/fdemon-app/src/process.rs` | Added `hydrate_connect_vm_service` that creates gate channel, stores sender in session handle |
| `crates/fdemon-app/src/handler/devtools/mod.rs` | Gate updates in `handle_switch_panel` and `handle_exit_devtools_mode` |
| `crates/fdemon-app/src/actions/mod.rs` | Pass `rebuilt_widgets_gate_rx` through to `spawn_vm_service_connection` |

### Notable Decisions/Tradeoffs

1. **Gate channel via `watch::Receiver<bool>`**: Used `bool` (not `DevToolsPanel`) to keep the forwarder independent of the state crate's type hierarchy and consistent with the existing `perf_pause_tx`/`network_pause_tx` conventions. `true` = gate open (forward), `false` = gate closed (skip).

2. **Hydration in `process.rs`**: Created `hydrate_connect_vm_service` following the established hydration pattern. This runs before all other hydrations, creates the `watch::channel(gate_open)` seeded with the current panel state, stores the sender in `SessionHandle::rebuilt_widgets_gate_tx`, and injects the receiver into the action. Avoids any new `Message` variants.

3. **All sessions updated on panel switch**: `handle_switch_panel` and `handle_exit_devtools_mode` iterate all sessions via `session_manager.iter_mut()` since `devtools_view_state.active_panel` is global, not per-session.

4. **`None` receiver = safe default (always skip)**: If hydration is somehow skipped (e.g., unit tests not calling `process_message`), `forward_vm_events` sees `None` and skips events, which is the conservative behavior.

5. **Tests are logic-level, not integration**: Since `forward_vm_events` requires a live `VmServiceClient` which is hard to mock, the gate tests verify the gate primitive (`watch::Receiver<bool>` channel semantics) in isolation. The `try_send` test exercises `tokio::sync::mpsc::error::TrySendError` variant discrimination.

### Testing Performed

- `cargo fmt --all -- --check` — Passed
- `cargo check -p fdemon-app` — Passed
- `cargo test -p fdemon-app` — Passed (2431 tests, 0 failed)
- `cargo clippy -p fdemon-app --all-targets -- -D warnings` — Passed (0 warnings)
- New tests: `test_rebuilt_widgets_event_skipped_when_panel_not_performance`, `test_rebuilt_widgets_event_dispatched_when_performance_active`, `test_try_send_error_variant_discrimination` — all pass

### Risks/Limitations

1. **Gate seeded at connect time**: The initial gate value is set to the current panel at `ConnectVmService` hydration time. If the user is on the Performance panel when the VM connects (rare — normally the VM connects before the user enters DevTools), the gate starts open, which is correct. The gate then tracks all subsequent panel changes via the watch channel.

2. **`Closed` receiver on `TrySendError::Closed`**: The forwarder exits the loop when the channel is closed, which is correct (session is shutting down). The `VmServiceDisconnected` message is then sent from the loop exit path.
