# Phase 2 Fixes — Review Remediation Task Index

## Overview

Address all issues identified in the Phase 2 code review (`workflow/reviews/features/devtools-integration-phase-2/`). Two critical blocking issues, three major issues, and several minor fixes must be resolved before Phase 4 integration begins.

**Total Tasks:** 5
**Estimated Hours:** 8-12 hours

## Task Dependency Graph

```
┌──────────────────────────────────┐
│ 01-split-extensions-submodules   │
│ (critical, no deps)             │
└──────────────┬───────────────────┘
               │
               ▼
┌──────────────────────────────────┐     ┌──────────────────────────────────┐
│ 02-refactor-client-ownership     │     │ 03-error-handling-fixes          │
│ (critical, depends: 01)         │     │ (major, depends: 01)            │
└──────────────────────────────────┘     └──────────────────────────────────┘

┌──────────────────────────────────┐     ┌──────────────────────────────────┐
│ 04-client-rwlock-poison-safety   │     │ 05-minor-quality-fixes           │
│ (minor, no deps)                │     │ (minor, depends: 01)            │
└──────────────────────────────────┘     └──────────────────────────────────┘
```

## Waves (Parallelizable Groups)

### Wave 1 (Foundation)
- **01-split-extensions-submodules** — Split the 1955-line file into submodules (blocker for tasks 02, 03, 05)
- **04-client-rwlock-poison-safety** — Independent fix in `client.rs`

### Wave 2 (Depends on 01)
- **02-refactor-client-ownership** — Refactor ObjectGroupManager/WidgetInspector ownership model
- **03-error-handling-fixes** — Fix silent error swallowing and state loss in extension functions
- **05-minor-quality-fixes** — Magic numbers, doc fixes, serde derives, code cleanup

## Tasks

| # | Task | Status | Depends On | Est. Hours | Crate | Key Files |
|---|------|--------|------------|------------|-------|-----------|
| 1 | [01-split-extensions-submodules](tasks/01-split-extensions-submodules.md) | Done | - | 2-3h | `fdemon-daemon` | `vm_service/extensions/` |
| 2 | [02-refactor-client-ownership](tasks/02-refactor-client-ownership.md) | Done | 1 | 2-3h | `fdemon-daemon` | `vm_service/extensions/inspector.rs` |
| 3 | [03-error-handling-fixes](tasks/03-error-handling-fixes.md) | Done | 1 | 1-2h | `fdemon-daemon` | `vm_service/extensions/inspector.rs`, `layout.rs` |
| 4 | [04-client-rwlock-poison-safety](tasks/04-client-rwlock-poison-safety.md) | Done | - | 0.5-1h | `fdemon-daemon` | `vm_service/client.rs` |
| 5 | [05-minor-quality-fixes](tasks/05-minor-quality-fixes.md) | Done | 1 | 1-2h | `fdemon-daemon`, `fdemon-core` | Multiple |

## Success Criteria

Phase 2 fixes are complete when:

- [x] No single file exceeds 500 lines (per CODE_STANDARDS.md)
- [x] `ObjectGroupManager` does not own a `VmServiceClient`
- [x] `WidgetInspector` uses a single client reference consistently (no dual-client pattern)
- [x] `dispose_all` has no unused parameters
- [x] `get_root_widget_tree` fallback only triggers on "extension not available" errors
- [x] `create_group` succeeds even when old group dispose fails
- [x] `extract_layout_tree` logs warnings when children fail to parse
- [x] All `RwLock::unwrap()` replaced with poison-safe alternatives in `client.rs`
- [x] Magic number 113 replaced with named constant
- [x] `query_all_overlays` doc matches implementation (sequential, not concurrent)
- [x] `cargo fmt --all` clean
- [x] `cargo check --workspace` clean
- [x] `cargo test --lib` — all pass (no regressions)
- [x] `cargo clippy --workspace -- -D warnings` — zero warnings

## Notes

- **Task 01 must be done first** because tasks 02, 03, and 05 modify files that will be created by the split. Doing the split first avoids merge conflicts.
- **Task 04 is independent** — it only touches `client.rs` which is not affected by the split.
- **All tasks in Wave 2 touch different files** after the split, so they can run in parallel.
- **Review source:** `workflow/reviews/features/devtools-integration-phase-2/REVIEW.md` and `ACTION_ITEMS.md`
