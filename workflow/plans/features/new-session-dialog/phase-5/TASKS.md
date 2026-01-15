# Phase 5: Target Selector Widget - Task Index

## Overview

Create the Target Selector widget - the left pane of the NewSessionDialog. Features tabbed navigation between Connected and Bootable devices with platform grouping.

**Total Tasks:** 12 (5 initial + 7 review fixes)
**Status:** Initial implementation complete, review fixes pending

## UI Design

```
┌── 🎯 Target Selector ─────────────────┐
│                                       │
│ ╭─────────────╮ ╭─────────────╮       │
│ │ 1 Connected │ │ 2 Bootable  │       │
│ ╰─────────────╯ ╰─────────────╯       │
│                                       │
│  iOS Devices                          │  ← Platform group header
│  ▶ iPhone 15 Pro (physical)           │
│    iPad Pro 12.9" (physical)          │
│                                       │
│  Android Devices                      │
│    Pixel 8 (physical)                 │
│    Galaxy S23 (physical)              │
│                                       │
│  Other                                │
│    Chrome (web)                       │
│    Linux (desktop)                    │
│                                       │
│  [Enter] Select  [r] Refresh          │
└───────────────────────────────────────┘
```

## Task Dependency Graph

```
┌─────────────────────────────────────┐
│  01-tab-bar-widget                  │
└────────────────┬────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────┐
│  02-device-grouping                 │
└────────────────┬────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────┐
│  03-device-list-widget              │
└────────────────┬────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────┐
│  04-target-selector-widget          │
└────────────────┬────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────┐
│  05-target-selector-messages        │
└────────────────┬────────────────────┘
                 │
                 ▼
   ┌─────────────┴─────────────┐
   │     REVIEW FIXES          │
   └─────────────┬─────────────┘
                 │
   ┌─────────────┼─────────────────────────────────┐
   │             │                                 │
   ▼             ▼                                 ▼
┌────────┐  ┌────────┐  ┌────────┐  ┌────────┐  ┌────────┐  ┌────────┐  ┌────────┐
│   06   │  │   07   │  │   08   │  │   09   │  │   10   │  │   11   │  │   12   │
│ Rename │  │ Index  │  │ Flags  │  │ Error  │  │Feedback│  │ Perf   │  │Cleanup │
│ Enum   │  │ Reset  │  │ Mgmt   │  │Clearing│  │        │  │        │  │        │
└────────┘  └────────┘  └────────┘  └────────┘  └────────┘  └────────┘  └────────┘
 CRITICAL    CRITICAL    MAJOR       MAJOR       MAJOR       MAJOR       MINOR
```

## Tasks

### Initial Implementation (Complete)

| # | Task | Status | Depends On | Est. | Modules |
|---|------|--------|------------|------|---------|
| 1 | [01-tab-bar-widget](tasks/01-tab-bar-widget.md) | Done | Phase 1 | 30m | `new_session_dialog/tab_bar.rs` |
| 2 | [02-device-grouping](tasks/02-device-grouping.md) | Done | 1 | 25m | `new_session_dialog/device_groups.rs` |
| 3 | [03-device-list-widget](tasks/03-device-list-widget.md) | Done | 2 | 40m | `new_session_dialog/device_list.rs` |
| 4 | [04-target-selector-widget](tasks/04-target-selector-widget.md) | Done | 3 | 30m | `new_session_dialog/target_selector.rs` |
| 5 | [05-target-selector-messages](tasks/05-target-selector-messages.md) | Done | 4 | 15m | `app/message.rs`, `app/handler/update.rs` |

### Review Fixes (From Code Review 2026-01-15)

| # | Task | Status | Priority | Depends On | Modules |
|---|------|--------|----------|------------|---------|
| 6 | [06-rename-bootable-device-enum](tasks/06-rename-bootable-device-enum.md) | Not Started | Critical | 5 | `device_groups.rs`, `device_list.rs`, `target_selector.rs`, `mod.rs` |
| 7 | [07-fix-selection-index-reset](tasks/07-fix-selection-index-reset.md) | Not Started | Critical | 5 | `state.rs` |
| 8 | [08-consolidate-loading-flags](tasks/08-consolidate-loading-flags.md) | Not Started | Major | 5 | `update.rs`, `state.rs` |
| 9 | [09-standardize-error-clearing](tasks/09-standardize-error-clearing.md) | Not Started | Major | 5 | `state.rs`, `target_selector.rs`, `update.rs`, `message.rs` |
| 10 | [10-add-error-feedback-empty-selection](tasks/10-add-error-feedback-empty-selection.md) | Not Started | Major | 5 | `update.rs` |
| 11 | [11-optimize-navigation-performance](tasks/11-optimize-navigation-performance.md) | Not Started | Major | 5 | `target_selector.rs`, `device_groups.rs` |
| 12 | [12-minor-cleanup](tasks/12-minor-cleanup.md) | Not Started | Minor | 5 | `target_selector.rs`, `device_list.rs`, `device_groups.rs` |

## Success Criteria

### Initial Implementation (Tasks 1-5)

- [x] Tab bar widget renders with Connected/Bootable tabs
- [x] Active tab is visually highlighted
- [x] 1/2 keys switch between tabs
- [x] Devices are grouped by platform with section headers
- [x] Device list supports scrolling for long lists
- [x] Selection indicator (▶) shows current selection
- [x] Up/Down navigation works within and across groups
- [x] Connected tab shows `flutter devices` results
- [x] Bootable tab shows simulators/AVDs (or unavailable message)
- [x] Loading state with spinner
- [x] Empty state messages
- [x] Enter on Bootable device triggers boot (not launch)
- [x] Refresh key (r) triggers device re-discovery
- [x] `cargo fmt && cargo check && cargo test && cargo clippy -- -D warnings` passes

### Review Fixes (Tasks 6-12)

Phase 5 is fully complete when review fixes are done:

- [ ] **Critical:** No type name conflict - TUI enum renamed to `GroupedBootableDevice`
- [ ] **Critical:** Tab switching selects first device, never a header
- [ ] **Major:** Single source of truth for loading flag management
- [ ] **Major:** Consistent error clearing across all state methods
- [ ] **Major:** Empty device selection logged/reported
- [ ] **Major:** Navigation performance optimized (cached flat list)
- [ ] **Minor:** No dead code warnings, public items documented
- [ ] `cargo fmt && cargo check && cargo test && cargo clippy -- -D warnings` passes
- [ ] Re-review passes with no blocking issues

## Platform Grouping

### Connected Tab Groups
1. **iOS Devices** - iPhones, iPads (physical)
2. **Android Devices** - Android phones/tablets (physical)
3. **iOS Simulators** - Running simulators
4. **Android Emulators** - Running emulators
5. **Other** - Chrome, Linux desktop, macOS, Windows

### Bootable Tab Groups
1. **iOS Simulators** - Available simulators (from xcrun simctl)
2. **Android AVDs** - Available AVDs (from emulator -list-avds)

## Navigation Behavior

- Up/Down moves selection within flat list (groups are visual only)
- Tab bar is not focusable (use 1/2 keys to switch)
- Enter on Connected device → select for launch
- Enter on Bootable device → boot device, then switch to Connected tab
- Esc → close dialog (if sessions running) or do nothing

## Notes

- Groups with no devices are hidden
- Group headers are not selectable
- Bootable tab shows tool unavailable messages when applicable
- Consider scroll offset to keep selection visible
