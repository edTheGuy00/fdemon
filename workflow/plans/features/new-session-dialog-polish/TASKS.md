# NewSessionDialog Polish - Task Index

## Overview

Fix four issues identified after NewSessionDialog implementation: responsive layout, scrollable sections, emulator boot bug, and device caching. Tasks 09-14 address issues found during code review.

**Total Tasks:** 14
**Priority Order:** Bug fix first, then UX improvements, then larger refactors, then review follow-ups

## Task Dependency Graph

```
┌─────────────────────────────────────┐
│  01-fix-boot-platform-mismatch      │  ← Critical bug fix (quick win)
└─────────────────────────────────────┘

┌─────────────────────────────────────┐
│  02-add-scroll-state                │  ← Add scroll_offset to state
└────────────────┬────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────┐
│  03-implement-device-list-scroll    │  ← Use scroll in rendering
└─────────────────────────────────────┘

┌─────────────────────────────────────┐
│  04-implement-device-cache-usage    │  ← Check cache before discovery
└─────────────────────────────────────┘

┌─────────────────────────────────────┐
│  05-add-layout-mode-detection       │  ← Detect horizontal vs vertical
└────────────────┬────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────┐
│  06-implement-vertical-layout       │  ← Render vertical layout
└────────────────┬────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────┐
│  07-adapt-widgets-responsive        │  ← Make child widgets responsive
└─────────────────────────────────────┘

┌─────────────────────────────────────┐
│  08-update-tests                    │  ← Update/add tests for all changes
└─────────────────────────────────────┘

═══════════════════════════════════════════════════════════════════════════════
                          CODE REVIEW FOLLOW-UP TASKS
═══════════════════════════════════════════════════════════════════════════════

┌─────────────────────────────────────┐     ┌─────────────────────────────────────┐
│  09-fix-utf8-truncation             │     │  10-enable-selection-preservation   │
│  (CRITICAL)                         │     │  (CRITICAL)                         │
└────────────────┬────────────────────┘     └─────────────────────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────┐
│  13-add-truncation-docs             │  ← Depends on 09 (document final impl)
│  (Minor)                            │
└─────────────────────────────────────┘

┌─────────────────────────────────────┐     ┌─────────────────────────────────────┐
│  11-extract-scroll-height-constant  │     │  12-fix-background-refresh-errors   │
│  (Major)                            │     │  (Major)                            │
└─────────────────────────────────────┘     └─────────────────────────────────────┘

┌─────────────────────────────────────┐
│  14-extract-indicator-width-const   │
│  (Minor)                            │
└─────────────────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Est. | Priority | Issue |
|---|------|--------|------------|------|----------|-------|
| 1 | [01-fix-boot-platform-mismatch](tasks/01-fix-boot-platform-mismatch.md) | Done | - | 20m | Critical | #3 |
| 2 | [02-add-scroll-state](tasks/02-add-scroll-state.md) | Done | - | 25m | High | #2 |
| 3 | [03-implement-device-list-scroll](tasks/03-implement-device-list-scroll.md) | Done | 2 | 45m | High | #2 |
| 4 | [04-implement-device-cache-usage](tasks/04-implement-device-cache-usage.md) | Done | - | 35m | High | #4 |
| 5 | [05-add-layout-mode-detection](tasks/05-add-layout-mode-detection.md) | Done | - | 25m | Medium | #1 |
| 6 | [06-implement-vertical-layout](tasks/06-implement-vertical-layout.md) | Done | 5 | 45m | Medium | #1 |
| 7 | [07-adapt-widgets-responsive](tasks/07-adapt-widgets-responsive.md) | Done | 6 | 40m | Medium | #1 |
| 8 | [08-update-tests](tasks/08-update-tests.md) | Done | 1-7 | 30m | Low | All |

### Code Review Follow-up Tasks

| # | Task | Status | Depends On | Est. | Priority | Issue |
|---|------|--------|------------|------|----------|-------|
| 9 | [09-fix-utf8-truncation](tasks/09-fix-utf8-truncation.md) | Not Started | - | 20m | Critical | Review #1 |
| 10 | [10-enable-selection-preservation](tasks/10-enable-selection-preservation.md) | Not Started | - | 15m | Critical | Review #2 |
| 11 | [11-extract-scroll-height-constant](tasks/11-extract-scroll-height-constant.md) | Not Started | - | 10m | Major | Review #3 |
| 12 | [12-fix-background-refresh-error-handling](tasks/12-fix-background-refresh-error-handling.md) | Not Started | - | 25m | Major | Review #4 |
| 13 | [13-add-truncation-docs](tasks/13-add-truncation-docs.md) | Not Started | 9 | 10m | Minor | Review #5 |
| 14 | [14-extract-indicator-width-constant](tasks/14-extract-indicator-width-constant.md) | Not Started | - | 10m | Minor | Review #6 |

## Parallel Execution Groups

Tasks can be executed in parallel within groups:

**Group A (Independent):**
- Task 01: Fix boot platform mismatch
- Task 02: Add scroll state
- Task 04: Implement device cache usage
- Task 05: Add layout mode detection

**Group B (After Group A):**
- Task 03: Implement device list scroll (requires Task 02)
- Task 06: Implement vertical layout (requires Task 05)

**Group C (After Group B):**
- Task 07: Adapt widgets responsive (requires Task 06)

**Group D (Final):**
- Task 08: Update tests (requires all above)

### Review Follow-up Execution Groups

**Group E (Critical - Independent):**
- Task 09: Fix UTF-8 truncation (CRITICAL)
- Task 10: Enable selection preservation (CRITICAL)
- Task 11: Extract scroll height constant
- Task 12: Fix background refresh error handling
- Task 14: Extract indicator width constant

**Group F (After Group E):**
- Task 13: Add truncation docs (requires Task 09)

## Success Criteria

### Original Tasks (1-8)

- [x] iOS simulator boots successfully from Bootable tab
- [x] Android AVD boots successfully from Bootable tab
- [x] Device list scrolls when items exceed visible height
- [x] Selected item always visible when navigating
- [x] Dialog opens instantly with cached devices
- [x] Dialog renders in vertical mode at 60x30 terminal
- [x] Dialog renders in horizontal mode at 100x40 terminal
- [x] All existing tests pass
- [x] `cargo fmt && cargo check && cargo test && cargo clippy -- -D warnings` passes

### Review Follow-up Tasks (9-14)

- [ ] No panic when truncating device names with emoji (e.g., "iPhone 🔥")
- [ ] No panic when truncating device names with CJK characters
- [ ] User's selected device preserved after background refresh
- [ ] Selection falls back to first device if selected device removed
- [ ] Magic numbers replaced with named constants
- [ ] Background refresh errors logged but don't show error UI
- [ ] Truncation functions documented with doc comments
- [ ] All new tests pass

## Issue Reference

### Original Issues

| Issue | Description | Tasks |
|-------|-------------|-------|
| #1 | Responsive Layout | 5, 6, 7 |
| #2 | Scrollable Sections | 2, 3 |
| #3 | Emulator/Simulator Boot | 1 |
| #4 | Device Caching | 4 |

### Code Review Issues

| Review # | Description | Severity | Task |
|----------|-------------|----------|------|
| Review #1 | UTF-8 truncation panic risk | Critical | 9 |
| Review #2 | Selection not preserved on refresh | Critical | 10 |
| Review #3 | Magic number for scroll height | Major | 11 |
| Review #4 | Background refresh error handling | Major | 12 |
| Review #5 | Missing docs on truncation utils | Minor | 13 |
| Review #6 | Magic number for indicator width | Minor | 14 |
