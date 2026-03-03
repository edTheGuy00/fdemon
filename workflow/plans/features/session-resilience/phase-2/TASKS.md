# Phase 2: Emit VmServiceReconnecting Message — Task Index

## Overview

Surface VM Service reconnection status to the UI so users see "Reconnecting (2/10)..." instead of silence during WebSocket backoff. The entire downstream pipeline (Message variant, handler, state field, all four TUI panels) is already wired — the only missing piece is emitting events from the daemon's reconnection loop and forwarding them through the app layer.

**Total Tasks:** 7 (4 original + 3 review fixes)

## Task Dependency Graph

```
┌──────────────────────────────────┐
│  04-vm-client-event-type         │
│  (foundation: daemon refactor)   │
└──────────────┬───────────────────┘
               │
       ┌───────┴────────┐
       ▼                ▼
┌─────────────────┐  ┌──────────────────────────────┐
│  05-emit-       │  │  06-forward-events-update     │
│  reconnect-     │  │  (app: handle VmClientEvent)  │
│  events         │  │                                │
│  (daemon: emit) │  │                                │
└────────┬────────┘  └──────────────┬─────────────────┘
         │                          │
         └────────────┬─────────────┘
                      ▼
         ┌──────────────────────────────┐
         │  07-reconnecting-tests       │
         │  (depends on 05, 06)         │
         └──────────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Modules |
|---|------|--------|------------|---------|
| 4 | [04-vm-client-event-type](tasks/04-vm-client-event-type.md) | Done | - | `daemon/vm_service/protocol.rs`, `daemon/vm_service/client.rs`, `daemon/vm_service/mod.rs` |
| 5 | [05-emit-reconnect-events](tasks/05-emit-reconnect-events.md) | Done | 04 | `daemon/vm_service/client.rs` |
| 6 | [06-forward-events-update](tasks/06-forward-events-update.md) | Done | 04 | `app/actions.rs` |
| 7 | [07-reconnecting-tests](tasks/07-reconnecting-tests.md) | Done | 05, 06 | `daemon/vm_service/client.rs` tests, `app/handler/tests.rs` |
| 8 | [08-log-lifecycle-send-failures](tasks/08-log-lifecycle-send-failures.md) | Not Started | - | `daemon/vm_service/client.rs` |
| 9 | [09-fix-stale-doc-example](tasks/09-fix-stale-doc-example.md) | Not Started | - | `daemon/vm_service/mod.rs` |
| 10 | [10-permanently-disconnected-test](tasks/10-permanently-disconnected-test.md) | Not Started | - | `app/handler/tests.rs` |

## Review Fixes (Wave 4)

Three issues identified during code review that should be fixed before merge:

| # | Review Issue | Severity | Description |
|---|-------------|----------|-------------|
| 8 | Issue #1 | Major | Lifecycle event `try_send` failures silently dropped — add `warn!` logging |
| 9 | Issue #4 | Minor | Stale doc example references `event.params.stream_id` on `VmClientEvent` |
| 10 | Issue #5 | Minor | No test for `PermanentlyDisconnected` → `VmServiceDisconnected` path |

Tasks 08, 09, 10 are independent and can all be executed in parallel.

## Execution Plan

| Wave | Tasks | Rationale |
|------|-------|-----------|
| 1 | Task 04 | Foundation: creates VmClientEvent type, changes channel plumbing |
| 2 | Tasks 05 + 06 (parallel) | 05 is daemon-only (client.rs), 06 is app-only (actions.rs) — no file contention |
| 3 | Task 07 | Tests depend on both 05 and 06 being complete |
| 4 | Tasks 08 + 09 + 10 (parallel) | Review fixes — all independent, no file contention |

**Compilation note:** After Task 04 alone, `cargo check -p fdemon-daemon` passes but `cargo check -p fdemon-app` fails (actions.rs expects old event type). Task 06 fixes the app crate. Full workspace compilation succeeds only after both Tasks 04 and 06 are done.

## Pre-existing Wiring (No Changes Needed)

These components are already complete and waiting for events to flow:

| Component | Location | Status |
|-----------|----------|--------|
| `Message::VmServiceReconnecting` variant | `message.rs:709-715` | Exists |
| `handle_vm_service_reconnecting` handler | `handler/devtools/mod.rs:242-258` | Exists |
| `VmConnectionStatus::Reconnecting` state | `state.rs:64-117` | Exists |
| DevTools tab bar reconnecting indicator | `tui/widgets/devtools/mod.rs:268-293` | Exists |
| Inspector panel reconnecting message | `tui/widgets/devtools/inspector/mod.rs:219-228` | Exists |
| Performance panel reconnecting message | `tui/widgets/devtools/performance/mod.rs:217-227` | Exists |
| Network panel reconnecting message | `tui/widgets/devtools/network/mod.rs:308-315` | Exists |

## Success Criteria

Phase 2 is complete when:

- [ ] `VmClientEvent` enum exists with `StreamEvent`, `Reconnecting`, `Reconnected`, `PermanentlyDisconnected` variants
- [ ] `run_client_task` emits lifecycle events through the event channel at correct state transitions
- [ ] `forward_vm_events` translates `VmClientEvent::Reconnecting` → `Message::VmServiceReconnecting`
- [ ] `forward_vm_events` translates `VmClientEvent::Reconnected` → `Message::VmServiceConnected`
- [ ] Users see "Reconnecting (N/10)..." in DevTools during backoff (via existing UI wiring)
- [ ] `VmConnectionStatus` resets to `Connected` after successful reconnection
- [ ] All existing tests pass (`cargo test --workspace`)
- [ ] `cargo clippy --workspace -- -D warnings` clean
- [ ] `cargo fmt --all --check` clean

## Notes

- The daemon crate is an internal workspace dependency, not published — changing event types is safe
- `VmServiceEvent` (stream notifications) is NOT renamed — it continues to exist inside `VmClientEvent::StreamEvent`
- The event channel capacity (256) is more than sufficient for lifecycle events (at most ~10 reconnect attempts)
- `connection_status` lives on `DevToolsViewState` (global), not per-session — the handler guards with `active_id == Some(session_id)` so only the visible session's status is shown
- `event_tx.try_send()` (non-blocking) is used for stream events; lifecycle events should use the same pattern for consistency
