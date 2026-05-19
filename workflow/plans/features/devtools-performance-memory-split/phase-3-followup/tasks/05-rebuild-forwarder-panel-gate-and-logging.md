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
