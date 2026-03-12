# Phase 2 Fixes: Native Platform Log Capture — Task Index

## Overview

Address 10 issues (1 critical, 5 major, 4 minor) found during code review of the Phase 2 native-platform-logs implementation. These are all bug fixes and quality improvements — no new features.

**Total Tasks:** 8
**Source:** [REVIEW.md](../../../reviews/features/native-platform-logs-phase-2/REVIEW.md), [ACTION_ITEMS.md](../../../reviews/features/native-platform-logs-phase-2/ACTION_ITEMS.md)

## Task Dependency Graph

```
Wave 1 (parallel — no dependencies):
┌──────────────────────┐  ┌──────────────────────┐  ┌──────────────────────┐  ┌──────────────────────┐
│ 01-fix-ios-process   │  │ 03-fix-simctl-min    │  │ 04-fix-ctrl-c-quit   │  │ 05-fix-truncate-tag  │
│      -name           │  │      -level          │  │                      │  │      -utf8           │
│ Issue: #1 (CRITICAL) │  │ Issue: #3 (MAJOR)    │  │ Issue: #5 (MAJOR)    │  │ Issue: #6 (MAJOR)    │
│ fdemon-app/actions   │  │ fdemon-daemon/ios.rs  │  │ fdemon-app/keys.rs   │  │ fdemon-tui/          │
└──────────┬───────────┘  └──────────────────────┘  └──────────────────────┘  │  tag_filter.rs       │
           │                                                                  └──────────────────────┘
           │
Wave 2 (depends on 01):
           ▼
┌──────────────────────┐  ┌──────────────────────┐
│ 02-wire-effective    │  │ 06-fix-scroll-offset │
│   -min-level         │  │                      │
│ Issue: #2 (MAJOR)    │  │ Issue: #4 (MAJOR)    │
│ fdemon-app/update.rs │  │ fdemon-app/state.rs  │
│                      │  │ fdemon-tui/          │
│                      │  │  tag_filter.rs       │
└──────────────────────┘  └──────────────────────┘

Wave 3 (parallel — no dependencies on waves 1-2):
┌──────────────────────┐  ┌──────────────────────┐
│ 07-fix-idevicesyslog │  │ 08-minor-cleanups    │
│      -regex          │  │                      │
│ Issues: #8, #10      │  │ Issues: #7, #9       │
│ fdemon-daemon/ios.rs │  │ fdemon-app/state.rs  │
│ tool_availability.rs │  │ fdemon-daemon/ios.rs │
└──────────────────────┘  └──────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Issues | Modules |
|---|------|--------|------------|--------|---------|
| 1 | [01-fix-ios-process-name](tasks/01-fix-ios-process-name.md) | Done | — | #1 | `fdemon-app/src/actions/native_logs.rs` |
| 2 | [02-wire-effective-min-level](tasks/02-wire-effective-min-level.md) | Done | 1 | #2 | `fdemon-app/src/handler/update.rs`, `fdemon-daemon/src/native_logs/mod.rs` |
| 3 | [03-fix-simctl-min-level](tasks/03-fix-simctl-min-level.md) | Done | — | #3 | `fdemon-daemon/src/native_logs/ios.rs` |
| 4 | [04-fix-ctrl-c-quit](tasks/04-fix-ctrl-c-quit.md) | Done | — | #5 | `fdemon-app/src/handler/keys.rs` |
| 5 | [05-fix-truncate-tag-utf8](tasks/05-fix-truncate-tag-utf8.md) | Done | — | #6 | `fdemon-tui/src/widgets/tag_filter.rs` |
| 6 | [06-fix-scroll-offset](tasks/06-fix-scroll-offset.md) | Done | — | #4 | `fdemon-app/src/state.rs`, `fdemon-tui/src/widgets/tag_filter.rs`, `fdemon-app/src/handler/update.rs` |
| 7 | [07-fix-idevicesyslog-regex](tasks/07-fix-idevicesyslog-regex.md) | Done | — | #8, #10 | `fdemon-daemon/src/native_logs/ios.rs`, `fdemon-daemon/src/tool_availability.rs` |
| 8 | [08-minor-cleanups](tasks/08-minor-cleanups.md) | Done | — | #7, #9 | `fdemon-app/src/state.rs`, `fdemon-daemon/src/native_logs/ios.rs` |

## Execution Plan

- **Wave 1** (tasks 01, 03, 04, 05): All independent — dispatch in parallel
- **Wave 2** (tasks 02, 06): Task 02 depends on 01 (process name fix must land first so iOS logs actually flow, making effective_min_level testable end-to-end). Task 06 is independent but grouped here to reduce wave count.
- **Wave 3** (tasks 07, 08): Minor fixes, no dependencies — dispatch in parallel with wave 2 or after

**Critical path:** 01 → 02

## Success Criteria

Phase 2 fixes are complete when:

- [ ] `derive_ios_process_name` always returns `"Runner"` regardless of bundle ID
- [ ] `effective_min_level()` is called in the `NativeLog` handler, filtering events below the per-tag or global threshold
- [ ] Simulator capture applies per-event `min_level` severity filter (matching physical device path)
- [ ] `Ctrl+C` quits from the tag filter overlay (like every other overlay)
- [ ] `truncate_tag()` uses character-based slicing (no panics on multi-byte UTF-8)
- [ ] `scroll_offset` is either wired to rendering or removed as dead state
- [ ] `IDEVICESYSLOG_RE` handles device names with spaces
- [ ] `check_idevicesyslog` does not rely on `--help` exit code
- [ ] Malformed doc comments are fixed
- [ ] Unnecessary clones in `idevicesyslog_line_to_event` are removed
- [ ] `cargo fmt --all && cargo check --workspace && cargo clippy --workspace -- -D warnings && cargo test --workspace --lib` passes

## Notes

- Task 01 is the **critical/blocking** fix — it is the root cause of the reported issue (no native logs on iOS Simulator).
- Tasks 02 and 03 both address min_level filtering gaps but in different layers: 02 is in the app handler (fdemon-app), 03 is in the daemon capture (fdemon-daemon). They can be done in parallel.
- Task 06 has two valid approaches (wire scroll_offset or remove it). The task describes both options — the implementor should choose based on the Responsive Layout Guidelines in `docs/CODE_STANDARDS.md` (Principle 3: scrollable lists with `Cell<usize>` render-hint).
- Tasks 07 and 08 bundle related minor issues to reduce task count.
