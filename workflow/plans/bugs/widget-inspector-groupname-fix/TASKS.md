# Widget Inspector `groupName` Fix - Task Index

## Overview

Fix the `getRootWidgetTree` VM Service call that crashes with "Null check operator used on a null value" because we send `objectGroup` instead of the expected `groupName` parameter key.

**Total Tasks:** 3
**Estimated Time:** 30-60 minutes

## Task Dependency Graph

```
┌──────────────────────────────┐     ┌──────────────────────────────┐
│  01-fix-actions-param-key    │     │  02-fix-inspector-param-key  │
│  (actions.rs — primary fix)  │     │  (inspector.rs — consistency)│
└──────────────┬───────────────┘     └──────────────┬───────────────┘
               │                                    │
               └──────────────┬─────────────────────┘
                              ▼
               ┌──────────────────────────────┐
               │  03-verify-dispose-params    │
               │  (audit + regression tests)  │
               └──────────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Est. | Modules |
|---|------|--------|------------|------|---------|
| 1 | [01-fix-actions-param-key](tasks/01-fix-actions-param-key.md) | Not Started | - | 15m | `fdemon-app/src/actions.rs` |
| 2 | [02-fix-inspector-param-key](tasks/02-fix-inspector-param-key.md) | Not Started | - | 10m | `fdemon-daemon/src/vm_service/extensions/inspector.rs` |
| 3 | [03-verify-dispose-params](tasks/03-verify-dispose-params.md) | Not Started | 1, 2 | 15m | Audit + tests |

## Success Criteria

Bug fix is complete when:

- [ ] `getRootWidgetTree` called with `groupName` parameter key (not `objectGroup`)
- [ ] `withPreviews` set to `"true"` for `getRootWidgetTree`
- [ ] Legacy `getRootWidgetSummaryTree` fallback still uses `objectGroup`
- [ ] `disposeGroup` still uses `objectGroup` (unchanged)
- [ ] `cargo build --workspace` compiles cleanly
- [ ] `cargo test --workspace` passes
- [ ] `cargo clippy --workspace` no new warnings

## Notes

- Tasks 1 and 2 are independent and can be worked in parallel
- Task 3 depends on both 1 and 2 being complete (verification pass)
- The `getRootWidgetTree` API was added in Flutter 3.22+ (June 2024) and uses raw `registerServiceExtension` which expects `groupName`
- The older `getRootWidgetSummaryTree` uses `_registerObjectGroupServiceExtension` helper which wraps the callback and extracts `objectGroup` — so the fallback must keep using `objectGroup`
