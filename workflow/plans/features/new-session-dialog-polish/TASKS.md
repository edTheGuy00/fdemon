# NewSessionDialog Polish - Task Index

## Overview

Fix four issues identified after NewSessionDialog implementation: responsive layout, scrollable sections, emulator boot bug, and device caching.

**Total Tasks:** 8
**Priority Order:** Bug fix first, then UX improvements, then larger refactors

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
```

## Tasks

| # | Task | Status | Depends On | Est. | Priority | Issue |
|---|------|--------|------------|------|----------|-------|
| 1 | [01-fix-boot-platform-mismatch](tasks/01-fix-boot-platform-mismatch.md) | Not Started | - | 20m | Critical | #3 |
| 2 | [02-add-scroll-state](tasks/02-add-scroll-state.md) | Not Started | - | 25m | High | #2 |
| 3 | [03-implement-device-list-scroll](tasks/03-implement-device-list-scroll.md) | Not Started | 2 | 45m | High | #2 |
| 4 | [04-implement-device-cache-usage](tasks/04-implement-device-cache-usage.md) | Not Started | - | 35m | High | #4 |
| 5 | [05-add-layout-mode-detection](tasks/05-add-layout-mode-detection.md) | Not Started | - | 25m | Medium | #1 |
| 6 | [06-implement-vertical-layout](tasks/06-implement-vertical-layout.md) | Not Started | 5 | 45m | Medium | #1 |
| 7 | [07-adapt-widgets-responsive](tasks/07-adapt-widgets-responsive.md) | Not Started | 6 | 40m | Medium | #1 |
| 8 | [08-update-tests](tasks/08-update-tests.md) | Not Started | 1-7 | 30m | Low | All |

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

## Success Criteria

- [ ] iOS simulator boots successfully from Bootable tab
- [ ] Android AVD boots successfully from Bootable tab
- [ ] Device list scrolls when items exceed visible height
- [ ] Selected item always visible when navigating
- [ ] Dialog opens instantly with cached devices
- [ ] Dialog renders in vertical mode at 60x30 terminal
- [ ] Dialog renders in horizontal mode at 100x40 terminal
- [ ] All existing tests pass
- [ ] `cargo fmt && cargo check && cargo test && cargo clippy -- -D warnings` passes

## Issue Reference

| Issue | Description | Tasks |
|-------|-------------|-------|
| #1 | Responsive Layout | 5, 6, 7 |
| #2 | Scrollable Sections | 2, 3 |
| #3 | Emulator/Simulator Boot | 1 |
| #4 | Device Caching | 4 |
