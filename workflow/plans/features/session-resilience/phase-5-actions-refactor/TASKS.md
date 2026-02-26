# Phase 5: Heartbeat Bug Fix + Actions Refactor — Task Index

## Overview

Fix the heartbeat failure counter bug (stale `consecutive_failures` not reset on reconnection events) and refactor the oversized `actions.rs` (2,081 lines — 4x the 500-line standard) into focused submodules.

**Total Tasks:** 7

## Task Dependency Graph

```
Wave 1 — Independent (can be parallelized)
┌───────────────────────────────────┐
│ 01-fix-heartbeat-counter-reset    │  bug fix (2 lines + test)
└───────────────────────────────────┘

Wave 2 — Sequential refactoring (each moves code out of mod.rs)
┌───────────────────────────────────┐
│ 02-extract-session-module         │  spawn_session + execute_task → session.rs
└───────────────┬───────────────────┘
                │
┌───────────────┴───────────────────┐
│ 03-extract-vm-service-module      │  vm connection + heartbeat → vm_service.rs
└───────────────┬───────────────────┘
                │
┌───────────────┴───────────────────┐
│ 04-extract-performance-module     │  perf polling → performance.rs
└───────────────┬───────────────────┘
                │
┌───────────────┴───────────────────┐
│ 05-extract-inspector-module       │  widget tree + overlay + layout + disposal → inspector.rs
└───────────────┬───────────────────┘
                │
┌───────────────┴───────────────────┐
│ 06-extract-network-module         │  HTTP profile + detail + clear + browser → network.rs
└───────────────┬───────────────────┘
                │
┌───────────────┴───────────────────┐
│ 07-verify-and-cleanup             │  final line counts, re-exports, doc headers
└───────────────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Modules |
|---|------|--------|------------|---------|
| 1 | [01-fix-heartbeat-counter-reset](tasks/01-fix-heartbeat-counter-reset.md) | Not Started | - | `actions.rs` |
| 2 | [02-extract-session-module](tasks/02-extract-session-module.md) | Not Started | 1 | `actions/mod.rs`, `actions/session.rs` **NEW** |
| 3 | [03-extract-vm-service-module](tasks/03-extract-vm-service-module.md) | Not Started | 2 | `actions/mod.rs`, `actions/vm_service.rs` **NEW** |
| 4 | [04-extract-performance-module](tasks/04-extract-performance-module.md) | Not Started | 3 | `actions/mod.rs`, `actions/performance.rs` **NEW** |
| 5 | [05-extract-inspector-module](tasks/05-extract-inspector-module.md) | Not Started | 4 | `actions/mod.rs`, `actions/inspector.rs` **NEW** |
| 6 | [06-extract-network-module](tasks/06-extract-network-module.md) | Not Started | 5 | `actions/mod.rs`, `actions/network.rs` **NEW** |
| 7 | [07-verify-and-cleanup](tasks/07-verify-and-cleanup.md) | Not Started | 6 | all `actions/*.rs` |

## Target Module Structure

```
crates/fdemon-app/src/actions/
├── mod.rs            (~350 lines)  — constants, SessionTaskMap, handle_action dispatcher, re-exports
├── session.rs        (~320 lines)  — spawn_session, execute_task
├── vm_service.rs     (~250 lines)  — spawn_vm_service_connection, forward_vm_events (heartbeat)
├── performance.rs    (~220 lines)  — spawn_performance_polling
├── inspector.rs      (~440 lines)  — widget tree, overlay toggle, layout explorer, group disposal
└── network.rs        (~340 lines)  — HTTP profile polling, detail fetch, clear, browser util
```

This mirrors the existing `handler/devtools/` decomposition pattern (inspector, performance, network).

## Success Criteria

Phase 5 is complete when:

- [ ] `consecutive_failures = 0` is set in both `Reconnecting` and `Reconnected` arms
- [ ] `actions.rs` is replaced by `actions/` directory module
- [ ] No single file in `actions/` exceeds ~500 lines (CODE_STANDARDS.md threshold)
- [ ] All existing public API (`handle_action`, `execute_task`, `SessionTaskMap`) re-exported from `actions/mod.rs`
- [ ] Each submodule has a `//!` doc header
- [ ] `cargo fmt --all` clean
- [ ] `cargo check --workspace` passes
- [ ] `cargo test --workspace` passes
- [ ] `cargo clippy --workspace -- -D warnings` clean
- [ ] No behavioral changes (pure refactoring after task 01)

## Notes

- Task 01 (bug fix) is applied to the monolithic file first, before the split begins. This keeps the bug fix reviewable as an isolated diff.
- Tasks 02-06 are sequential because each extraction reduces `mod.rs` and the next task references updated line numbers. Parallelizing would cause merge conflicts.
- The `spawn` submodule (device discovery, emulator launch, etc.) already exists separately and is NOT part of this refactor.
- The Phase 3b task `01-reset-heartbeat-on-reconnect.md` has a false-positive completion summary — the fix was never applied to source code.
