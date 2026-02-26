# Phase 3b: Review Fixes — Task Index

## Overview

Follow-up fixes from the phase-3 code review. Two critical bugs (heartbeat counter not reset on reconnection, duplicate exit events from watchdog race), plus dead code cleanup, idempotency hardening, and test hygiene improvements.

**Review:** [Phase 3 Review](../../reviews/features/session-resilience-phase-3/REVIEW.md)
**Total Tasks:** 5

## Task Dependency Graph

```
┌────────────────────────────────┐  ┌────────────────────────────────┐
│  01-reset-heartbeat-on-        │  │  02-fix-duplicate-exit-race    │
│  reconnect                     │  │  (spawn_session watchdog)      │
│  (forward_vm_events)           │  │                                │
└───────────────┬────────────────┘  └───────────────┬────────────────┘
                │                                   │
                │                   ┌───────────────┴────────────────┐
                │                   │  03-exit-handler-idempotency   │
                │                   │  (session.rs + test)           │
                │                   └───────────────┬────────────────┘
                │                                   │
┌───────────────┴────────────────┐                  │
│  04-cleanup-get-version        │                  │
│  (client.rs + protocol.rs)     │                  │
└───────────────┬────────────────┘                  │
                │                                   │
                └───────────────┬───────────────────┘
                                ▼
                ┌────────────────────────────────────┐
                │  05-test-and-style-fixes            │
                │  (tests.rs, process.rs)             │
                └────────────────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Severity | Modules |
|---|------|--------|------------|----------|---------|
| 1 | [01-reset-heartbeat-on-reconnect](tasks/01-reset-heartbeat-on-reconnect.md) | Pending | - | Critical | `app/actions.rs` |
| 2 | [02-fix-duplicate-exit-race](tasks/02-fix-duplicate-exit-race.md) | Pending | - | Critical | `app/actions.rs` |
| 3 | [03-exit-handler-idempotency](tasks/03-exit-handler-idempotency.md) | Pending | 02 | Major | `app/handler/session.rs`, `app/handler/tests.rs` |
| 4 | [04-cleanup-get-version](tasks/04-cleanup-get-version.md) | Pending | - | Major | `daemon/vm_service/client.rs`, `daemon/vm_service/protocol.rs`, `app/actions.rs` |
| 5 | [05-test-and-style-fixes](tasks/05-test-and-style-fixes.md) | Pending | 01, 02, 03, 04 | Minor | `app/handler/tests.rs`, `daemon/process.rs` |

## Execution Plan

| Wave | Tasks | Rationale |
|------|-------|-----------|
| 1 | Tasks 01, 02, 04 (parallel) | All independent: 01 modifies `forward_vm_events`, 02 modifies `spawn_session`, 04 modifies `client.rs`/`protocol.rs`. No file contention. |
| 2 | Task 03 | Depends on task 02 (the idempotency guard is defense-in-depth for the race fix). Modifies `session.rs` + adds tests. |
| 3 | Task 05 | Test cleanup depends on all implementation tasks being stable. |

## Success Criteria

Phase 3b is complete when:

- [ ] `consecutive_failures` reset to 0 on `Reconnected` and `Reconnecting` events
- [ ] Watchdog checks `process_exited` flag before synthesizing `Exited` event
- [ ] `handle_session_exited` returns early when session is already `Stopped`
- [ ] `get_version()` accessible from `VmRequestHandle` and used by heartbeat (or removed if unused)
- [ ] `VersionInfo` has `#[serde(rename_all = "camelCase")]` matching module convention
- [ ] No duplicate test (`test_session_exited_updates_session_phase` removed)
- [ ] New tests follow `test_<function>_<scenario>_<expected_result>` naming
- [ ] Platform-dependent tests guarded with `#[cfg(unix)]`
- [ ] Double-exit idempotency test exists
- [ ] All existing tests pass (`cargo test --workspace`)
- [ ] `cargo clippy --workspace -- -D warnings` clean
