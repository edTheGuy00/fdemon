# Phase 3: Process Health Monitoring — Task Index

## Overview

Detect hung Flutter processes and stale VM Service connections through periodic health checks. Currently, a hung process that produces no output stays "Running" forever, and a silently-dead VM Service connection shows stale DevTools data indefinitely. This phase adds two watchdog mechanisms (process-level and VM-level) plus proper exit code capture.

**Total Tasks:** 5

## Task Dependency Graph

```
┌──────────────────────┐     ┌──────────────────────┐
│  01-process-watchdog  │     │  02-get-version-rpc  │
│  (spawn_session arm)  │     │  (VmServiceClient)   │
└──────────┬───────────┘     └──────────┬───────────┘
           │                            │
           │                            ▼
           │              ┌──────────────────────────┐
           │              │  03-vm-heartbeat          │
           │              │  (forward_vm_events arm)  │
           │              └──────────┬───────────────┘
           │                         │
┌──────────┴───────────┐             │
│  04-wait-for-exit    │             │
│  (process.rs refactor)│            │
└──────────┬───────────┘             │
           │                         │
           └───────────┬─────────────┘
                       ▼
           ┌──────────────────────────┐
           │  05-health-monitoring-   │
           │  tests                   │
           └──────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Modules |
|---|------|--------|------------|---------|
| 1 | [01-process-watchdog](tasks/01-process-watchdog.md) | Not Started | - | `app/actions.rs` |
| 2 | [02-get-version-rpc](tasks/02-get-version-rpc.md) | Not Started | - | `daemon/vm_service/protocol.rs`, `daemon/vm_service/client.rs`, `daemon/vm_service/mod.rs` |
| 3 | [03-vm-heartbeat](tasks/03-vm-heartbeat.md) | Not Started | 02 | `app/actions.rs` |
| 4 | [04-wait-for-exit-task](tasks/04-wait-for-exit-task.md) | Not Started | - | `daemon/process.rs` |
| 5 | [05-health-monitoring-tests](tasks/05-health-monitoring-tests.md) | Not Started | 01, 02, 03, 04 | `daemon/vm_service/protocol.rs`, `daemon/process.rs`, `app/handler/tests.rs`, `app/actions.rs` |

## Execution Plan

| Wave | Tasks | Rationale |
|------|-------|-----------|
| 1 | Tasks 01, 02, 04 (parallel) | All three are independent: 01 modifies `spawn_session` in `actions.rs`, 02 adds a new method to `client.rs`/`protocol.rs`, 04 refactors `process.rs`. No file contention. |
| 2 | Task 03 | Depends on task 02 (`get_version` RPC used as the heartbeat probe). Modifies `forward_vm_events` in `actions.rs` — no conflict with task 01 which modifies `spawn_session`. |
| 3 | Task 05 | Tests for all four implementation tasks. |

## Success Criteria

Phase 3 is complete when:

- [ ] Hung process detected within 5 seconds via process watchdog (`spawn_session` polls `has_exited()`)
- [ ] Stale VM connection detected within ~90 seconds via heartbeat (3 x 30s failures of `getVersion` probe)
- [ ] Actual process exit code captured and surfaced in session log ("exited normally" / "exited with code N")
- [ ] `stdout_reader` no longer emits `DaemonEvent::Exited` — replaced by dedicated wait task
- [ ] No duplicate `Exited` events when watchdog and wait task race
- [ ] All existing tests pass (`cargo test --workspace`)
- [ ] `cargo clippy --workspace -- -D warnings` clean
- [ ] New tests cover: `VersionInfo` deserialization, exit code handling paths, constant validation

## Risk Assessment

### Task 04 Complexity

Task 04 (wait-for-exit-task) is the highest-risk task. It refactors `FlutterProcess` to move `Child` ownership into a dedicated async task, changing the struct's fields and the API surface of `has_exited()`, `is_running()`, `shutdown()`, and `Drop`. Two approaches are documented:
- **Simple**: `Arc<tokio::sync::Mutex<Child>>` — minimal changes, but lock contention risk
- **Clean**: Move `Child` entirely into the wait task, replace with oneshot channels — larger refactor but cleaner ownership

The implementer should assess both approaches and choose based on the number of callers affected.

### Interaction Between Task 01 and Task 04

Task 01 (watchdog) calls `process.has_exited()`. Task 04 changes `has_exited()` to be async or replaces it. If task 04 lands after task 01, the watchdog code must be updated. If both are in wave 1, coordinate the API change.

**Recommendation**: Implement task 01 first (simple, low-risk), then task 04 updates the watchdog as part of its refactor.

### Heartbeat During Reconnection (Task 03)

The heartbeat may fire during a reconnection backoff window. During reconnection, `getVersion` will fail with `Error::ChannelClosed`. The 3-failure threshold provides tolerance: with a 30s heartbeat interval, it takes 90s to declare dead. Reconnection with 10 attempts and exponential backoff takes at most ~127s (sum of 1+2+4+8+16+30+30+30+30+30). In the worst case, heartbeat may trigger disconnect during a long reconnection. This is acceptable — if reconnection takes >90s, the connection is effectively dead.

## Notes

- Phase 3 tasks are all in the daemon and app layers — no TUI changes required
- The process watchdog (task 01) and VM heartbeat (task 03) are independent safety nets at different layers: process-level (OS) and protocol-level (VM Service)
- Constants are intentionally conservative (5s watchdog, 30s heartbeat, 3 failures) — they can be made configurable in a future enhancement
- Task 04 may benefit from a spike/prototype phase before full implementation due to its refactoring scope
