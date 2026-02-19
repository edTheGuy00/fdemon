# Phase 4 Fixes: Review Action Items — Task Index

## Overview

Address all critical, major, and code quality issues identified in the Phase 4 code review (`workflow/reviews/features/devtools-integration-phase-4/`). Three critical runtime bugs prevent DevTools from functioning, four major issues affect correctness and maintainability, and several code quality issues violate project standards.

**Total Tasks:** 7
**Estimated Hours:** 12-18 hours

## Task Dependency Graph

```
Wave 1 (Critical fixes — parallel, no file contention)
┌──────────────────────────────┐   ┌──────────────────────────────┐
│ 01-fix-loading-stuck         │   │ 03-fix-tea-browser-launch    │
│ (update.rs, process.rs)      │   │ (devtools.rs, mod.rs,        │
│                              │   │  actions.rs)                 │
└──────────┬───────────────────┘   └──────────────────────────────┘
           │
           ▼
Wave 2 (Critical + Major — sequential after 01)
┌──────────────────────────────┐   ┌──────────────────────────────┐
│ 02-fix-vm-connection         │   │ 04-session-switch-reset      │
│ (update.rs, session.rs,      │   │ (session_lifecycle.rs,       │
│  state.rs, performance.rs)   │   │  state.rs)                   │
└──────────┬───────────────────┘   └──────────┬───────────────────┘
           │                                  │
           └──────────────┬───────────────────┘
                          ▼
Wave 3 (Major + Quality — parallel after 02, 04)
┌──────────────────┐ ┌──────────────────────┐ ┌──────────────────────┐
│ 05-code-quality  │ │ 06-cache-visible-    │ │ 07-object-group-     │
│   cleanup        │ │   nodes              │ │   disposal           │
│ (tui widgets,    │ │ (state.rs,           │ │ (actions.rs,         │
│  performance.rs) │ │  inspector.rs,       │ │  state.rs)           │
│                  │ │  mod.rs)             │ │                      │
└──────────────────┘ └──────────────────────┘ └──────────────────────┘
```

## Waves (Parallelizable Groups)

### Wave 1 (Critical — no file contention)
- **01-fix-loading-stuck** — Guard `loading = true` on `vm_connected`, send failure messages when hydration discards actions
- **03-fix-tea-browser-launch** — Move `open_url_in_browser` from handler to `UpdateAction` + `actions.rs`

### Wave 2 (Critical + Major — depends on Wave 1)
- **02-fix-vm-connection** — Fix `vm_shutdown_tx` leak, surface connection failures in DevTools panels
- **04-session-switch-reset** — Reset `DevToolsViewState` on session switch, add `reset()` methods

### Wave 3 (Major + Quality — depends on Wave 2)
- **05-code-quality-cleanup** — Extract `truncate_str`, add named constants, remove dead code, fix visibility
- **06-cache-visible-nodes** — Cache `visible_nodes()` result, invalidate on tree mutation
- **07-object-group-disposal** — Dispose VM object groups on refresh and DevTools exit

## Tasks

| # | Task | Status | Depends On | Est. Hours | Severity | Key Modules |
|---|------|--------|------------|------------|----------|-------------|
| 1 | [01-fix-loading-stuck](tasks/01-fix-loading-stuck.md) | Not Started | - | 2-3h | Critical | `handler/update.rs`, `process.rs`, `handler/devtools.rs` |
| 2 | [02-fix-vm-connection](tasks/02-fix-vm-connection.md) | Not Started | 1 | 2-3h | Critical | `handler/update.rs`, `handler/session.rs`, `state.rs` |
| 3 | [03-fix-tea-browser-launch](tasks/03-fix-tea-browser-launch.md) | Not Started | - | 1-2h | Critical | `handler/devtools.rs`, `handler/mod.rs`, `actions.rs` |
| 4 | [04-session-switch-reset](tasks/04-session-switch-reset.md) | Not Started | 1 | 1-2h | Major | `handler/session_lifecycle.rs`, `state.rs` |
| 5 | [05-code-quality-cleanup](tasks/05-code-quality-cleanup.md) | Not Started | 2, 4 | 2-3h | Minor | `widgets/devtools/*.rs` |
| 6 | [06-cache-visible-nodes](tasks/06-cache-visible-nodes.md) | Not Started | 2, 4 | 2-3h | Major | `state.rs`, `widgets/devtools/inspector.rs`, `widgets/devtools/mod.rs` |
| 7 | [07-object-group-disposal](tasks/07-object-group-disposal.md) | Not Started | 2, 4 | 2-3h | Major | `actions.rs`, `state.rs`, `handler/devtools.rs` |

## Success Criteria

Phase 4 fixes are complete when:

- [ ] Pressing `r` in Inspector with no VM shows an error (not stuck loading)
- [ ] VM Service connects reliably after hot restart / reconnect
- [ ] Connection failures are visible in DevTools panels
- [ ] `handle_open_browser_devtools` returns `UpdateAction`, no side effect in handler
- [ ] Switching sessions resets DevTools state
- [ ] `visible_nodes()` is cached, not rebuilt on every render frame
- [ ] VM object groups are disposed on refresh and DevTools exit
- [ ] No duplicate `truncate_str`; magic numbers extracted as constants
- [ ] All quality gates pass: `cargo fmt && cargo check && cargo clippy -- -D warnings && cargo test --lib`
- [ ] Manual verification of all scenarios from the re-review checklist in `ACTION_ITEMS.md`

## Notes

- Task 01 is the most impactful fix — it directly addresses the user-reported "Loading widget tree..." bug
- Task 02 is the root cause of "VM Service not connected" — `vm_shutdown_tx` leaks on `VmServiceDisconnected`
- Tasks 01 and 03 can be dispatched in parallel (no file contention)
- Tasks 02 and 04 share `state.rs` but touch different sections — can be parallelized with care
- Wave 3 tasks are independent of each other and can all run in parallel
