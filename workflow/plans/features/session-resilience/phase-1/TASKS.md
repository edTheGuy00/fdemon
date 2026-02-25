# Phase 1: Network Polling Cleanup — Task Index

## Overview

Fix zombie network polling tasks that persist after session termination. Two of the five session cleanup paths (`handle_session_exited` and `AppStop`) clean up performance polling but skip network polling, creating resource leaks.

**Total Tasks:** 3

## Task Dependency Graph

```
┌──────────────────────────────┐     ┌──────────────────────────────┐
│  01-network-cleanup-exit     │     │  02-network-cleanup-appstop  │
│  (independent)               │     │  (independent)               │
└──────────────┬───────────────┘     └──────────────┬───────────────┘
               │                                    │
               └────────────┬───────────────────────┘
                            ▼
               ┌──────────────────────────────────┐
               │  03-network-cleanup-tests         │
               │  (depends on 01, 02)              │
               └──────────────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Modules |
|---|------|--------|------------|---------|
| 1 | [01-network-cleanup-exit](tasks/01-network-cleanup-exit.md) | Done | - | `handler/session.rs` |
| 2 | [02-network-cleanup-appstop](tasks/02-network-cleanup-appstop.md) | Done | - | `handler/session.rs` |
| 3 | [03-network-cleanup-tests](tasks/03-network-cleanup-tests.md) | Done | 01, 02 | `handler/tests.rs` |

## Cleanup Path Audit

Before Phase 1, the five session termination paths have this coverage:

| # | Termination Path | File | Perf Cleanup | Network Cleanup |
|---|-----------------|------|:------------:|:---------------:|
| 1 | `VmServiceDisconnected` | `handler/update.rs:1278` | ✅ | ✅ |
| 2 | `CloseCurrentSession` | `handler/session_lifecycle.rs:114` | ✅ | ✅ |
| 3 | `handle_session_exited` | `handler/session.rs:95` | ✅ | ✅ |
| 4 | `AppStop` in `handle_session_message_state` | `handler/session.rs:161` | ✅ | ✅ |
| 5 | `handle_session_spawn_failed` | `handler/session_lifecycle.rs:39` | N/A | N/A |

Path 5 is not a bug — monitoring never starts if the process fails to spawn.

## Success Criteria

Phase 1 is complete when:

- [x] `handle_session_exited` clears `network_task_handle` and `network_shutdown_tx`
- [x] `AppStop` handler clears `network_task_handle` and `network_shutdown_tx`
- [x] New tests verify both cleanup paths
- [x] All existing tests pass (`cargo test --workspace`)
- [x] `cargo clippy --workspace -- -D warnings` clean
- [x] `cargo fmt --all --check` clean

## Notes

- The cleanup pattern is well-established: `handle.take().abort()` + `tx.take().send(true)` + `tracing::info!`
- `NetworkState` has no `monitoring_active` flag (unlike `PerformanceState`), so there is no equivalent flag to reset
- Both fixes are in the same file (`handler/session.rs`), making tasks 01 and 02 small and focused
- Tasks 01 and 02 can be implemented in parallel; task 03 depends on both
