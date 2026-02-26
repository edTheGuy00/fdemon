# Phase 2b: Reconnect Handler Fixes — Task Index

## Overview

Three pre-existing handler design issues uncovered during the phase-2 review. All relate to the `VmServiceConnected` handler being reused for reconnection without accounting for reconnect-specific concerns: performance state preservation, polling task cleanup, and multi-session `connection_status` guarding.

**Total Tasks:** 4

## Task Dependency Graph

```
┌──────────────────────────────────┐
│  01-reconnected-message-variant  │
│  (new Message + handler + mapping│
└──────────────┬───────────────────┘
               │
       ┌───────┴────────┐
       ▼                ▼
┌─────────────────┐  ┌──────────────────────────────┐
│  02-cleanup-    │  │  03-guard-connection-status   │
│  perf-on-       │  │  (multi-session fix)          │
│  reconnect      │  │                                │
└────────┬────────┘  └──────────────┬─────────────────┘
         │                          │
         └────────────┬─────────────┘
                      ▼
         ┌──────────────────────────────┐
         │  04-reconnect-handler-tests  │
         │  (depends on 01, 02, 03)    │
         └──────────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Modules |
|---|------|--------|------------|---------|
| 1 | [01-reconnected-message-variant](tasks/01-reconnected-message-variant.md) | Not Started | - | `app/message.rs`, `app/actions.rs`, `app/handler/update.rs` |
| 2 | [02-cleanup-perf-on-reconnect](tasks/02-cleanup-perf-on-reconnect.md) | Not Started | 01 | `app/handler/update.rs` |
| 3 | [03-guard-connection-status](tasks/03-guard-connection-status.md) | Not Started | - | `app/handler/update.rs` |
| 4 | [04-reconnect-handler-tests](tasks/04-reconnect-handler-tests.md) | Not Started | 01, 02, 03 | `app/handler/tests.rs` |

## Execution Plan

| Wave | Tasks | Rationale |
|------|-------|-----------|
| 1 | Task 01 + Task 03 (parallel) | 01 adds the new message variant; 03 is an independent guard fix — no file contention in the critical sections |
| 2 | Task 02 | Depends on task 01 (the new `VmServiceReconnected` handler is where perf cleanup goes) |
| 3 | Task 04 | Tests for all three fixes |

## Success Criteria

Phase 2b is complete when:

- [ ] `Message::VmServiceReconnected` exists and is used for reconnection (not `VmServiceConnected`)
- [ ] Reconnection preserves accumulated `PerformanceState` data (memory history, frame timings, etc.)
- [ ] Reconnection log message distinguishes from initial connection ("VM Service reconnected" vs "VM Service connected")
- [ ] Old performance polling task is aborted before spawning a new one on reconnect
- [ ] `connection_status` writes in `VmServiceConnected`, `VmServiceDisconnected`, and `VmServiceConnectionFailed` are guarded by active-session check
- [ ] Background session VM lifecycle events do not pollute foreground session's connection indicator
- [ ] All existing tests pass (`cargo test --workspace`)
- [ ] `cargo clippy --workspace -- -D warnings` clean

## Notes

- All three issues are pre-existing design gaps in the handler layer, not regressions from phase-2
- Task 01 is the largest change (new message variant + handler + mapping); tasks 02 and 03 are small targeted fixes
- The `VmServiceConnected` handler comment at `update.rs:1194` says "reset on hot-restart" — this is correct for hot-restart but wrong for plain WebSocket reconnection. Task 01 separates these concerns.
- Task 03 also covers `VmServiceConnectionFailed` which has the same unguarded `vm_connection_error` write
