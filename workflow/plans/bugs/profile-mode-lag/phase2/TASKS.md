# Phase 2: Core Fix — Mode-Aware Polling — Task Index

## Overview

Reduce VM Service pressure in profile/release modes by deduplicating RPCs, preventing burst recovery, scaling intervals by build mode, and gating allocation profiling on panel visibility. Target: reduce from ~8 RPCs/sec to <= 2 RPCs/sec in profile mode with the reporter's aggressive config.

**Total Tasks:** 5
**Estimated Hours:** 5-8 hours

## Task Dependency Graph

```
┌──────────────────────────┐     ┌──────────────────────────┐
│  01-dedup-memory-rpc     │     │  02-missed-tick-skip     │
│  (actions/performance.rs │     │  (actions/performance.rs │
│   + daemon/performance)  │     │   + actions/network.rs)  │
└────────────┬─────────────┘     └────────────┬─────────────┘
             │                                │
             └──────────────┬─────────────────┘
                            ▼
             ┌──────────────────────────────┐
             │  03-thread-flutter-mode      │
             │  (handler/mod.rs, update.rs, │
             │   devtools/mod.rs, process,  │
             │   actions/*, config/types)   │
             └──────────────┬───────────────┘
                            │
                            ▼
             ┌──────────────────────────────┐
             │  04-scale-intervals-by-mode  │
             │  (actions/performance.rs,    │
             │   actions/network.rs)        │
             └──────────────┬───────────────┘
                            │
                            ▼
             ┌──────────────────────────────┐
             │  05-gate-alloc-on-panel      │
             │  (actions/performance.rs,    │
             │   handler/devtools/mod.rs,   │
             │   session/handle.rs,         │
             │   message.rs)               │
             └─────────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Est. Hours | Modules |
|---|------|--------|------------|------------|---------|
| 1 | [01-dedup-memory-rpc](tasks/01-dedup-memory-rpc.md) | Not Started | - | 1-1.5h | `daemon/vm_service/performance.rs`, `actions/performance.rs` |
| 2 | [02-missed-tick-skip](tasks/02-missed-tick-skip.md) | Not Started | - | 0.5h | `actions/performance.rs`, `actions/network.rs` |
| 3 | [03-thread-flutter-mode](tasks/03-thread-flutter-mode.md) | Not Started | 1, 2 | 1.5-2h | `handler/mod.rs`, `handler/update.rs`, `handler/devtools/mod.rs`, `process.rs`, `actions/mod.rs`, `actions/performance.rs`, `actions/network.rs` |
| 4 | [04-scale-intervals-by-mode](tasks/04-scale-intervals-by-mode.md) | Not Started | 3 | 1-1.5h | `actions/performance.rs`, `actions/network.rs` |
| 5 | [05-gate-alloc-on-panel](tasks/05-gate-alloc-on-panel.md) | Not Started | 4 | 1.5-2h | `actions/performance.rs`, `handler/devtools/mod.rs`, `session/handle.rs`, `message.rs` |

## File Overlap Analysis

| Task | Files Modified (Write) | Files Read (Dependencies) |
|------|----------------------|--------------------------|
| 01-dedup-memory-rpc | `crates/fdemon-daemon/src/vm_service/performance.rs`, `crates/fdemon-app/src/actions/performance.rs` | `crates/fdemon-core/src/performance.rs` (domain types) |
| 02-missed-tick-skip | `crates/fdemon-app/src/actions/performance.rs`, `crates/fdemon-app/src/actions/network.rs` | - |
| 03-thread-flutter-mode | `crates/fdemon-app/src/handler/mod.rs`, `crates/fdemon-app/src/handler/update.rs`, `crates/fdemon-app/src/handler/devtools/mod.rs`, `crates/fdemon-app/src/process.rs`, `crates/fdemon-app/src/actions/mod.rs`, `crates/fdemon-app/src/actions/performance.rs`, `crates/fdemon-app/src/actions/network.rs` | `crates/fdemon-app/src/config/types.rs` (FlutterMode), `crates/fdemon-app/src/session/session.rs` (launch_config access) |
| 04-scale-intervals-by-mode | `crates/fdemon-app/src/actions/performance.rs`, `crates/fdemon-app/src/actions/network.rs` | `crates/fdemon-app/src/config/types.rs` (FlutterMode) |
| 05-gate-alloc-on-panel | `crates/fdemon-app/src/actions/performance.rs`, `crates/fdemon-app/src/handler/devtools/mod.rs`, `crates/fdemon-app/src/session/handle.rs`, `crates/fdemon-app/src/message.rs` | `crates/fdemon-app/src/state.rs` (DevToolsPanel) |

### Overlap Matrix

| Task Pair | Shared Write Files | Isolation Strategy |
|-----------|-------------------|-------------------|
| 01 + 02 | `actions/performance.rs` | Sequential (same branch) |
| 03 depends on 01, 02 | N/A (sequential by design) | Sequential (same branch) |
| 04 depends on 03 | N/A (sequential by design) | Sequential (same branch) |
| 05 depends on 04 | `actions/performance.rs` overlap | Sequential (same branch) |

**Note:** All five tasks share `actions/performance.rs` as a write target. The entire phase is a single sequential chain. This is inherent to the bugfix — all changes converge on the same polling loop. Parallelism is not possible without risking merge conflicts.

## Success Criteria

Phase 2 is complete when:

- [ ] `getMemoryUsage` is called once per memory tick, not twice (dedup)
- [ ] `MissedTickBehavior::Skip` is set on all three polling intervals (memory, alloc, network)
- [ ] `FlutterMode` flows from `LaunchConfig` through `UpdateAction` to both polling functions
- [ ] Profile/release mode intervals are scaled up: effective >= 2000ms perf, >= 5000ms alloc, >= 3000ms network
- [ ] Allocation profiling pauses when the Performance panel is not visible
- [ ] No visible lag in profile mode with the reporter's original config (`performance_refresh_ms=500`, `allocation_profile_interval_ms=1000`, `network_poll_interval_ms=1000`)
- [ ] Debug mode behavior is unchanged (same intervals, same polling lifecycle)
- [ ] All existing tests pass (`cargo test --workspace`)
- [ ] New tests cover mode-aware interval scaling and allocation panel gating

## Notes

- All tasks are sequential because they converge on `actions/performance.rs`. Each task builds on the prior one's changes.
- Task 01 and 02 are listed as independent in the BUG.md dependency graph, but they overlap on `actions/performance.rs`, so they must execute sequentially to avoid merge conflicts.
- The `FlutterMode` is accessed via `session.launch_config.as_ref().map(|c| c.mode)`. When `launch_config` is `None` (bare `flutter run`), default to `FlutterMode::Debug` to preserve current behavior.
- Network monitoring is already demand-driven (starts only on panel switch to Network tab). Task 03 threads mode through this existing path; Task 04 scales the interval.
- The BUG.md numbered these tasks 02-06 (global numbering). This TASKS.md uses 01-05 (phase-local numbering). Mapping: BUG 02→here 01, BUG 03→here 02, BUG 04→here 03, BUG 05→here 04, BUG 06→here 05.
