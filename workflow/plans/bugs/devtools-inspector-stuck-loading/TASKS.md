# DevTools Inspector Stuck Loading — Task Index

## Overview

Five-phase fix for the Inspector "Loading widget tree" hang. Phase 1 adds diagnostic instrumentation so root cause can be confirmed from logs; Phases 2-4 apply targeted fixes (debounce-clear, isolate resolution, readiness poll refactor); Phase 5 verifies and updates docs.

**Total Tasks:** 8
**Estimated Hours:** 12-16 hours

## Task Dependency Graph

```
01-add-diagnostic-instrumentation
         │
         ├──────────────────┐
         ▼                  ▼
02-clear-fetch-debounce  03-promote-channel-drop-log
         │                  │
         └──────┬───────────┘
                ▼
04-resolve-flutter-ui-isolate
                │
                ▼
05-shrink-readiness-poll-budget
         │                  │
         ▼                  ▼
06-bypass-poll-on-refresh   │
         │                  │
         └──────┬───────────┘
                ▼
07-tests-inspector-handlers
                │
                ▼
08-update-architecture-doc (doc_maintainer)
```

## Tasks

| # | Task | Status | Depends On | Est. Hours | Modules |
|---|------|--------|------------|------------|---------|
| 1 | [01-add-diagnostic-instrumentation](tasks/01-add-diagnostic-instrumentation.md) | Done | — | 1-2h | `actions/inspector/`, `vm_service/client.rs`, `process.rs` |
| 2 | [02-clear-fetch-debounce-on-failure](tasks/02-clear-fetch-debounce-on-failure.md) | Done | 1 | 1-2h | `state.rs`, `handler/devtools/inspector.rs` |
| 3 | [03-promote-channel-drop-to-error-log](tasks/03-promote-channel-drop-to-error-log.md) | Done | 1 | 0.5-1h | `process.rs` |
| 4 | [04-resolve-flutter-ui-isolate](tasks/04-resolve-flutter-ui-isolate.md) | Done | 2, 3 | 3-4h | `vm_service/client.rs`, `actions/inspector/mod.rs` |
| 5 | [05-shrink-readiness-poll-budget](tasks/05-shrink-readiness-poll-budget.md) | Done | 4 | 1-2h | `actions/inspector/widget_tree.rs`, `config/settings.rs` |
| 6 | [06-bypass-readiness-poll-on-refresh](tasks/06-bypass-readiness-poll-on-refresh.md) | Done | 4 | 1-2h | `actions/inspector/mod.rs`, `state.rs` |
| 7 | [07-tests-inspector-handlers](tasks/07-tests-inspector-handlers.md) | Done | 5, 6 | 2-3h | `handler/devtools/inspector.rs` tests, `actions/inspector/` tests |
| 8 | [08-update-architecture-doc](tasks/08-update-architecture-doc.md) | Done | 7 | 0.5h | `docs/ARCHITECTURE.md` |

## File Overlap Analysis

| Task | Files Modified (Write) | Files Read (Dependencies) |
|------|----------------------|--------------------------|
| 01-add-diagnostic-instrumentation | `actions/inspector/mod.rs`, `actions/inspector/widget_tree.rs`, `vm_service/client.rs`, `process.rs`, `handler/update.rs` | — |
| 02-clear-fetch-debounce-on-failure | `state.rs`, `handler/devtools/inspector.rs` | — |
| 03-promote-channel-drop-to-error-log | `process.rs` | — |
| 04-resolve-flutter-ui-isolate | `vm_service/client.rs`, `actions/inspector/mod.rs`, `vm_service/protocol.rs` | `state.rs` |
| 05-shrink-readiness-poll-budget | `actions/inspector/widget_tree.rs`, `config/settings.rs` | — |
| 06-bypass-readiness-poll-on-refresh | `actions/inspector/mod.rs`, `state.rs` | `handler/devtools/inspector.rs` |
| 07-tests-inspector-handlers | `handler/devtools/inspector.rs` (tests), `actions/inspector/widget_tree.rs` (tests) | All above |
| 08-update-architecture-doc | `docs/ARCHITECTURE.md` | All above |

### Overlap Matrix

| Task Pair | Shared Write Files | Isolation Strategy |
|-----------|-------------------|-------------------|
| 02 + 03 | None (state.rs/inspector.rs vs process.rs) | Parallel (worktree) |
| 05 + 06 | `actions/inspector/` (widget_tree.rs vs mod.rs) + both touch `state.rs` indirectly | **Sequential (same branch)** — both touch the inspector module and `state.rs` via 06 |
| 02 + 04 | `state.rs` is touched by 02 (clear_fetch_debounce helper) and 06; 04 does not touch state.rs | Sequential by dependency (04 depends on 02) |
| 01 + others | 01 touches almost every file in the module; all others depend on it | Sequential by dependency |

### Wave Plan

- **Wave 1**: 01 (alone — instrumentation foundation; everything else depends on it).
- **Wave 2**: 02 + 03 in parallel (worktree).
- **Wave 3**: 04 alone (touches both files modified in 02 indirectly via cache hooks).
- **Wave 4**: 05 → 06 sequential (both touch the inspector module).
- **Wave 5**: 07 (tests touch handlers + action helpers).
- **Wave 6**: 08 (doc, depends on 07).

## Success Criteria

- [ ] Inspector renders widget tree within 1.5 s on warm Flutter session.
- [ ] `r` refresh fires RPC within ~100 ms; no silent debounce after a failure.
- [ ] Multi-isolate apps select the Flutter UI isolate correctly.
- [ ] Diagnostic log shows isolate selection, poll attempts, RPC call/response for every fetch.
- [ ] `docs/ARCHITECTURE.md` updated.
- [ ] All four CI quality gates pass.

## Keyboard Shortcuts

No new bindings; existing `r` refresh becomes responsive after fixes.

## Notes

- All inspector instrumentation stays at `info!` for this bug-fix's life; downgrade to `debug!` in a later cleanup once the issue is verified fixed in production.
- Per CLAUDE.md, custom errors must use the `Error` enum from `fdemon-core/error.rs`.
- Cell-write sites for the resolved-isolate cache need `// EXCEPTION` annotations per `docs/CODE_STANDARDS.md`.
