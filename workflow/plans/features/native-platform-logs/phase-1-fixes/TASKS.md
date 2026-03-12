# Phase 1 Fixes: Native Platform Log Capture — Task Index

## Overview

Address 11 issues (2 critical, 3 major, 6 minor) found during code review of the Phase 1 native-platform-logs implementation. These are all bug fixes and quality improvements — no new features.

**Total Tasks:** 6
**Source:** [REVIEW.md](../../reviews/features/native-platform-logs/REVIEW.md), [ACTION_ITEMS.md](../../reviews/features/native-platform-logs/ACTION_ITEMS.md)

## Task Dependency Graph

```
Wave 1 (parallel — no dependencies):
┌────────────────────────┐  ┌────────────────────────┐  ┌────────────────────────┐
│ 01-fix-macos-log-check │  │ 03-fix-macos-log-level │  │ 04-deduplicate-native  │
│ Issues: #1             │  │ Issues: #3, #7         │  │         -infra         │
│ fdemon-daemon          │  │ fdemon-daemon           │  │ Issues: #9, #10        │
│ tool_availability.rs   │  │ native_logs/            │  │ fdemon-daemon          │
└───────────┬────────────┘  └────────────────────────┘  │ native_logs/           │
            │                                           └────────────────────────┘
            │
Wave 2 (depends on 01):
            ▼
┌────────────────────────┐  ┌────────────────────────┐
│ 02-wire-tool-guard-and │  │ 05-fix-daemon-triple   │
│   -session-safety      │  │       -parse           │
│ Issues: #2, #4, #5, #6│  │ Issues: #8             │
│ fdemon-app handler/    │  │ fdemon-app             │
└────────────────────────┘  │ handler/daemon.rs      │
                            └────────────────────────┘

Wave 3 (depends on 02):
┌────────────────────────┐
│ 06-add-handler-tests   │
│ Issues: #11            │
│ fdemon-app             │
│ handler/tests.rs       │
└────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Issues | Modules |
|---|------|--------|------------|--------|---------|
| 1 | [01-fix-macos-log-check](tasks/01-fix-macos-log-check.md) | Done | — | #1 | `fdemon-daemon/src/tool_availability.rs` |
| 2 | [02-wire-tool-guard-and-session-safety](tasks/02-wire-tool-guard-and-session-safety.md) | Done | 1 | #2, #4, #5, #6 | `fdemon-app/src/handler/session.rs`, `update.rs` |
| 3 | [03-fix-macos-log-level](tasks/03-fix-macos-log-level.md) | Done | — | #3, #7 | `fdemon-daemon/src/native_logs/macos.rs`, `android.rs`, `mod.rs` |
| 4 | [04-deduplicate-native-infra](tasks/04-deduplicate-native-infra.md) | Done | — | #9, #10 | `fdemon-daemon/src/native_logs/mod.rs`, `android.rs`, `macos.rs` |
| 5 | [05-fix-daemon-triple-parse](tasks/05-fix-daemon-triple-parse.md) | Done | — | #8 | `fdemon-app/src/handler/daemon.rs` |
| 6 | [06-add-handler-tests](tasks/06-add-handler-tests.md) | Done | 2 | #11 | `fdemon-app/src/handler/tests.rs` |

## Execution Plan

- **Wave 1** (tasks 01, 03, 04, 05): All independent — dispatch in parallel
- **Wave 2** (task 02): Depends on 01 (macOS log check must return `true` before wiring the guard) — dispatch when 01 completes
- **Wave 3** (task 06): Depends on 02 (tests should cover the new guards) — dispatch when 02 completes

**Critical path:** 01 → 02 → 06

## Success Criteria

Phase 1 fixes are complete when:

- [ ] `check_macos_log()` returns `true` on macOS systems
- [ ] `native_logs_available()` is called before spawning native log capture
- [ ] `log stream` is not invoked with `--level error` (uses `--level default` instead)
- [ ] Double-start of native log capture is prevented
- [ ] Late `NativeLogCaptureStarted` for closed sessions sends shutdown signal
- [ ] `needs_capture` expression has explicit parentheses
- [ ] `should_include_tag` logic is deduplicated in `native_logs/mod.rs`
- [ ] `EVENT_CHANNEL_CAPACITY` constant is shared across android/macos
- [ ] `AndroidLogConfig` and `MacOsLogConfig` derive `Clone`
- [ ] `parse_daemon_message` is called once per Stdout line (not 3 times)
- [ ] TEA handler tests exist for all native log message variants
- [ ] `cargo fmt --all && cargo check --workspace && cargo clippy --workspace -- -D warnings && cargo test --workspace --lib` passes

## Notes

- Tasks 01 and 02 are the blocking issues identified in the review. All other tasks are quality improvements.
- Task 02 bundles 4 issues that all touch the same `handler/session.rs` and `handler/update.rs` files, keeping changes atomic.
- Task 06 depends on task 02 because the tests should validate the new guards and leak fixes.
