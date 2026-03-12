# Phase 3 Fixes: Native Platform Log Capture — Task Index

## Overview

Address 11 issues (1 critical, 4 major, 6 minor) found during code review of the Phase 3 native-platform-logs implementation. These are all bug fixes and quality improvements — no new features.

**Total Tasks:** 8
**Source:** [REVIEW.md](../../../reviews/features/native-platform-logs-phase-3/REVIEW.md), [ACTION_ITEMS.md](../../../reviews/features/native-platform-logs-phase-3/ACTION_ITEMS.md)

## Task Dependency Graph

```
Wave 1 (parallel — no dependencies):
┌──────────────────────┐  ┌──────────────────────┐  ┌──────────────────────┐  ┌──────────────────────┐
│ 01-fix-macos-min     │  │ 02-fix-hot-restart   │  │ 04-abort-custom      │  │ 05-remove-debug      │
│    -level            │  │    -guard             │  │    -source-tasks     │  │    -scaffolding      │
│ Issue: #1 (CRITICAL) │  │ Issue: #2 (MAJOR)    │  │ Issue: #4 (MAJOR)    │  │ Issue: #5 (MAJOR)    │
│ fdemon-daemon/       │  │ fdemon-app/handler/   │  │ fdemon-app/session/  │  │ fdemon-app/actions/  │
│   macos.rs           │  │   session.rs          │  │   handle.rs          │  │   native_logs.rs     │
└──────────────────────┘  └──────────┬───────────┘  └──────────────────────┘  │ fdemon-app/handler/  │
                                     │                                        │   session.rs         │
                          ┌──────────▼───────────┐                            └──────────────────────┘
                          │ 03-fix-tag-state      │
                          │    -reset             │
                          │ Issue: #3 (MAJOR)     │
                          │ fdemon-app/handler/   │
                          │   update.rs           │
                          └──────────────────────┘

Wave 2 (parallel — no dependencies on wave 1):
┌──────────────────────┐  ┌──────────────────────┐  ┌──────────────────────┐
│ 06-move-parse-min    │  │ 07-fix-tag-case      │  │ 08-minor-cleanups    │
│    -level-to-core    │  │    -sensitivity       │  │                      │
│ Issue: #6 (MINOR)    │  │ Issues: #8, #11      │  │ Issues: #7, #9, #10  │
│ fdemon-core/types.rs │  │   (MINOR)            │  │   (MINOR)            │
│ fdemon-daemon/       │  │ fdemon-app/config/   │  │ fdemon-app/config/   │
│   native_logs/       │  │   types.rs           │  │   types.rs           │
│ fdemon-app/handler/  │  │ fdemon-app/session/  │  │ fdemon-app/actions/  │
│   update.rs          │  │   native_tags.rs     │  │   native_logs.rs     │
└──────────────────────┘  │ fdemon-app/handler/  │  │ fdemon-tui/widgets/  │
                          │   update.rs          │  │   tag_filter.rs      │
                          └──────────────────────┘  │ fdemon-daemon/       │
                                                    │   native_logs/       │
                                                    │     formats.rs       │
                                                    └──────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Issues | Modules |
|---|------|--------|------------|--------|---------|
| 1 | [01-fix-macos-min-level](tasks/01-fix-macos-min-level.md) | Done | — | #1 (CRITICAL) | `fdemon-daemon/src/native_logs/macos.rs` |
| 2 | [02-fix-hot-restart-guard](tasks/02-fix-hot-restart-guard.md) | Done | — | #2 (MAJOR) | `fdemon-app/src/handler/session.rs` |
| 3 | [03-fix-tag-state-reset](tasks/03-fix-tag-state-reset.md) | Done | 2 | #3 (MAJOR) | `fdemon-app/src/handler/update.rs` |
| 4 | [04-abort-custom-source-tasks](tasks/04-abort-custom-source-tasks.md) | Done | — | #4 (MAJOR) | `fdemon-app/src/session/handle.rs` |
| 5 | [05-remove-debug-scaffolding](tasks/05-remove-debug-scaffolding.md) | Done | — | #5 (MAJOR) | `fdemon-app/src/actions/native_logs.rs`, `fdemon-app/src/handler/session.rs` |
| 6 | [06-move-parse-min-level-to-core](tasks/06-move-parse-min-level-to-core.md) | Done | — | #6 (MINOR) | `fdemon-core/src/types.rs`, `fdemon-daemon/src/native_logs/mod.rs`, `fdemon-app/src/handler/update.rs` |
| 7 | [07-fix-tag-case-sensitivity](tasks/07-fix-tag-case-sensitivity.md) | Done | — | #8, #11 (MINOR) | `fdemon-app/src/config/types.rs`, `fdemon-app/src/session/native_tags.rs`, `fdemon-app/src/handler/update.rs` |
| 8 | [08-minor-cleanups](tasks/08-minor-cleanups.md) | Done | — | #7, #9, #10 (MINOR) | `fdemon-app/src/config/types.rs`, `fdemon-app/src/actions/native_logs.rs`, `fdemon-tui/src/widgets/tag_filter.rs`, `fdemon-daemon/src/native_logs/formats.rs` |

## Execution Plan

- **Wave 1** (tasks 01, 02, 04, 05): All independent — dispatch in parallel. Task 03 depends on 02 (same guard area).
- **Wave 2** (tasks 06, 07, 08): Minor fixes, no dependencies on wave 1 — dispatch in parallel with wave 1 or after.

**Critical path:** 02 → 03

## Success Criteria

Phase 3 fixes are complete when:

- [ ] macOS `min_level = "error"` produces only error-level logs (not info/warning)
- [ ] Hot-restart does not spawn duplicate custom source processes
- [ ] When `adb logcat` exits while custom sources are running, tag filter selections are preserved
- [ ] No detached Tokio tasks after session close (custom sources aborted on shutdown)
- [ ] No `[native-logs-debug]` strings remain in `tracing::info!` calls
- [ ] `parse_min_level` lives in `fdemon-core` alongside `LogLevel`
- [ ] Per-tag config lookup and tag visibility are case-insensitive
- [ ] Duplicate custom source names are rejected at config parse time
- [ ] `CustomSourceConfig::validate()` is called from the spawn path
- [ ] Tag column width is a named constant
- [ ] Syslog format on non-macOS emits a warning
- [ ] `cargo fmt --all && cargo check --workspace && cargo clippy --workspace -- -D warnings && cargo test --workspace --lib` passes

## Notes

- Task 01 is the **critical/blocking** fix — macOS min_level filtering is completely non-functional.
- Task 03 depends on 02 because both touch the native log guard/lifecycle code in overlapping areas — applying 03 first could conflict with the guard change in 02.
- Task 06 (move `parse_min_level`) should be done before or concurrently with task 01 (macOS fix) since the macOS fix needs to call `parse_min_level`. If done first, the macOS fix can call `LogLevel::from_level_str()` directly.
- Tasks 07 and 08 each bundle related minor issues to reduce task count.
