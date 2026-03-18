# Phase 2 Fixes: Review Remediation - Task Index

## Overview

Address all issues found during the Phase 2 code review. Three critical bugs (FVM cache path mismatch, .fvmrc overwrite, stale dart_version), two major UX issues (loading state flash, no deletion confirmation), and a batch of minor code quality fixes.

**Total Tasks:** 5
**Review:** `workflow/reviews/features/flutter-sdk-management-phase-2/REVIEW.md`

## Task Dependency Graph

```
┌──────────────────────────┐     ┌──────────────────────────┐     ┌──────────────────────────┐
│  01-fix-fvm-cache-path   │     │  02-fix-fvmrc-merge      │     │  03-fix-dart-version     │
│  (CRITICAL bug fix)      │     │  (CRITICAL bug fix)      │     │  (CRITICAL bug fix)      │
└──────────────────────────┘     └──────────────────────────┘     └──────────────────────────┘

┌──────────────────────────┐     ┌──────────────────────────┐
│  04-fix-loading-state    │     │  05-deletion-confirm     │
│  (MAJOR UX fix)          │     │  (MAJOR UX + minor fixes)│
└──────────────────────────┘     └──────────────────────────┘
```

### Parallelism

| Wave | Tasks | Can Run In Parallel |
|------|-------|-------------------|
| 1 | 01, 02, 03, 04, 05 | Yes (all touch different files/functions) |

## Tasks

| # | Task | Status | Depends On | Modules |
|---|------|--------|------------|---------|
| 1 | [01-fix-fvm-cache-path](tasks/01-fix-fvm-cache-path.md) | Not Started | - | `fdemon-daemon: flutter_sdk/cache_scanner.rs, mod.rs` `fdemon-app: actions/mod.rs` |
| 2 | [02-fix-fvmrc-merge](tasks/02-fix-fvmrc-merge.md) | Not Started | - | `fdemon-app: actions/mod.rs` |
| 3 | [03-fix-dart-version](tasks/03-fix-dart-version.md) | Not Started | - | `fdemon-app: flutter_version/state.rs, handler/flutter_version/actions.rs` |
| 4 | [04-fix-loading-state](tasks/04-fix-loading-state.md) | Not Started | - | `fdemon-app: flutter_version/state.rs` |
| 5 | [05-deletion-confirm-and-cleanup](tasks/05-deletion-confirm-and-cleanup.md) | Not Started | - | `fdemon-app: handler/flutter_version/actions.rs, handler/update.rs, flutter_version/state.rs, message.rs` `fdemon-tui: widgets/flutter_version_panel/mod.rs, sdk_info.rs, version_list.rs` |

## Success Criteria

Phase 2 fixes are complete when:

- [ ] Version removal works correctly when `FVM_CACHE_PATH` is set to a custom path
- [ ] Removal safety check uses the same cache resolution as the scanner (single source of truth)
- [ ] `dirs::home_dir()` failure returns a proper error instead of silently producing an empty path
- [ ] Switching versions preserves existing `.fvmrc` fields (`flavors`, `runPubGetOnSdkChanges`, etc.)
- [ ] After switching versions, the SDK info pane shows the correct Dart version
- [ ] Panel shows "Scanning..." immediately on open (not "No versions found" flash)
- [ ] Pressing `d` to delete a version requires confirmation before proceeding
- [ ] `#[allow(dead_code)]` on unused `icons` fields is removed
- [ ] `centered_rect` uses the shared `modal_overlay::centered_rect_percent` utility
- [ ] Stub handlers are routed through the handler module
- [ ] `cargo fmt --all && cargo check --workspace && cargo test --workspace && cargo clippy --workspace -- -D warnings` passes

## Notes

- All 5 tasks touch different functions/files and can be dispatched in parallel (Wave 1).
- Tasks 01-04 are focused single-function fixes. Task 05 bundles the deletion confirmation (major) with the minor code quality fixes since they touch overlapping files.
- All changes should be on the existing `feature/flutter-sdk-management` branch.
