# Phase 3: Panel-Aware Monitoring Lifecycle — Task Index

## Overview

Stop polling entirely when the user isn't viewing DevTools panels. Phase 2 reduced RPC frequency via interval scaling and alloc-only gating; Phase 3 eliminates all unnecessary polling by pausing the entire performance polling loop outside DevTools, pausing network polling when not on the Network tab, and deferring performance monitoring startup until the user actually opens DevTools. Target: zero VM Service polling RPCs when viewing logs.

**Total Tasks:** 4
**Estimated Hours:** 5-8 hours

## Task Dependency Graph

```
┌───────────────────────────────────────┐
│  01-pause-perf-when-not-devtools      │
│  (actions/performance.rs,             │
│   handler/devtools/mod.rs,            │
│   session/handle.rs, message.rs,      │
│   handler/update.rs)                  │
└───────────────┬───────────────────────┘
                │
                ▼
┌───────────────────────────────────────┐
│  02-pause-network-on-tab-switch       │
│  (actions/network.rs,                 │
│   handler/devtools/mod.rs,            │
│   handler/devtools/network.rs,        │
│   session/handle.rs, message.rs)      │
└───────────────┬───────────────────────┘
                │
                ▼
┌───────────────────────────────────────┐
│  03-lazy-start-monitoring             │
│  (handler/update.rs,                  │
│   handler/devtools/mod.rs,            │
│   actions/performance.rs)             │
└───────────────┬───────────────────────┘
                │
                ▼
┌───────────────────────────────────────┐
│  04-update-docs                       │
│  (docs/ARCHITECTURE.md)              │
│  Agent: doc_maintainer                │
└───────────────────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Est. Hours | Modules |
|---|------|--------|------------|------------|---------|
| 1 | [01-pause-perf-when-not-devtools](tasks/01-pause-perf-when-not-devtools.md) | Done | Phase 2 task 05 | 1.5-2h | `actions/performance.rs`, `handler/devtools/mod.rs`, `session/handle.rs`, `message.rs`, `handler/update.rs` |
| 2 | [02-pause-network-on-tab-switch](tasks/02-pause-network-on-tab-switch.md) | Done | 1 | 1.5-2h | `actions/network.rs`, `handler/devtools/mod.rs`, `handler/devtools/network.rs`, `session/handle.rs`, `message.rs` |
| 3 | [03-lazy-start-monitoring](tasks/03-lazy-start-monitoring.md) | Done | 1 | 1.5-2h | `handler/update.rs`, `handler/devtools/mod.rs`, `actions/performance.rs` |
| 4 | [04-update-docs](tasks/04-update-docs.md) | Done | 3 | 0.5h | `docs/ARCHITECTURE.md` |

## File Overlap Analysis

| Task | Files Modified (Write) | Files Read (Dependencies) |
|------|----------------------|--------------------------|
| 01-pause-perf-when-not-devtools | `crates/fdemon-app/src/actions/performance.rs`, `crates/fdemon-app/src/handler/devtools/mod.rs`, `crates/fdemon-app/src/session/handle.rs`, `crates/fdemon-app/src/message.rs`, `crates/fdemon-app/src/handler/update.rs` | `crates/fdemon-app/src/state.rs` (UiMode, DevToolsPanel) |
| 02-pause-network-on-tab-switch | `crates/fdemon-app/src/actions/network.rs`, `crates/fdemon-app/src/handler/devtools/mod.rs`, `crates/fdemon-app/src/handler/devtools/network.rs`, `crates/fdemon-app/src/session/handle.rs`, `crates/fdemon-app/src/message.rs` | `crates/fdemon-app/src/state.rs` (DevToolsPanel) |
| 03-lazy-start-monitoring | `crates/fdemon-app/src/handler/update.rs`, `crates/fdemon-app/src/handler/devtools/mod.rs`, `crates/fdemon-app/src/actions/performance.rs` | `crates/fdemon-app/src/state.rs` (UiMode), `crates/fdemon-app/src/session/handle.rs` (perf_shutdown_tx sentinel) |
| 04-update-docs | `docs/ARCHITECTURE.md` | Task 03 completion summary |

### Overlap Matrix

| Task Pair | Shared Write Files | Isolation Strategy |
|-----------|-------------------|-------------------|
| 01 + 02 | `handler/devtools/mod.rs`, `session/handle.rs`, `message.rs` | Sequential (same branch) |
| 01 + 03 | `handler/devtools/mod.rs`, `actions/performance.rs` | Sequential (same branch) |
| 02 + 03 | `handler/devtools/mod.rs` | Sequential (same branch) |
| 04 | No overlap with 01-03 (only writes `docs/ARCHITECTURE.md`) | Sequential (depends on 03) |

**Note:** All three implementation tasks share `handler/devtools/mod.rs` as a write target. The entire phase is a single sequential chain. This mirrors Phase 2's structure — all changes converge on the same DevTools handler and session handle modules. Parallelism is not possible without risking merge conflicts.

## Success Criteria

Phase 3 is complete when:

- [ ] Zero VM Service polling RPCs fire when the user is viewing logs (not in DevTools)
- [ ] Memory polling (`getMemoryUsage` + `getIsolate`) pauses when not in DevTools mode
- [ ] Network polling (`getHttpProfile`) pauses when not viewing the Network panel
- [ ] Monitoring resumes within one interval when switching back to the relevant DevTools panel
- [ ] Immediate data fetch fires on resume (no stale data on panel re-entry)
- [ ] Performance monitoring does not start until the user first enters DevTools
- [ ] Frame timing (event-driven, no polling) remains active at all times regardless of panel
- [ ] VM connect/reconnect while already in DevTools starts monitoring immediately
- [ ] Debug mode behavior is unchanged (same polling lifecycle, just panel-gated)
- [ ] All existing tests pass (`cargo test --workspace`)
- [ ] New tests cover pause/resume and lazy-start behavior

## Notes

- All tasks are sequential because they converge on `handler/devtools/mod.rs`. Each task builds on the prior one's changes.
- The BUG.md numbered these tasks 07-09 (global numbering). This TASKS.md uses 01-03 (phase-local numbering) plus 04 for docs. Mapping: BUG 07 → here 01, BUG 08 → here 02, BUG 09 → here 03.
- Frame timing comes from the VM's extension stream (push-based events), not from the performance polling task. It is completely unaffected by pausing/stopping the polling loop.
- The `watch::channel<bool>` pattern established in Phase 2 (`alloc_pause_tx`) is reused for both `perf_pause_tx` and `network_pause_tx`. Convention: `true` = paused, `false` = active.
- Network monitoring is already demand-started (only starts when user enters Network panel). Task 02 adds pause/resume for when the user leaves and returns to the Network tab without exiting DevTools.
