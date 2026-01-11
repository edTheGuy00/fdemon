# Phase 5: Target Selector Widget - Task Index

## Overview

Create the Target Selector widget - the left pane of the NewSessionDialog. Features tabbed navigation between Connected and Bootable devices with platform grouping.

**Total Tasks:** 5
**Estimated Time:** 3 hours

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
└─────────────────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Est. | Modules |
|---|------|--------|------------|------|---------|
| 1 | [01-tab-bar-widget](tasks/01-tab-bar-widget.md) | Not Started | Phase 1 | 30m | `new_session_dialog/tab_bar.rs` |
| 2 | [02-device-grouping](tasks/02-device-grouping.md) | Not Started | 1 | 25m | `new_session_dialog/device_groups.rs` |
| 3 | [03-device-list-widget](tasks/03-device-list-widget.md) | Not Started | 2 | 40m | `new_session_dialog/device_list.rs` |
| 4 | [04-target-selector-widget](tasks/04-target-selector-widget.md) | Not Started | 3 | 30m | `new_session_dialog/target_selector.rs` |
| 5 | [05-target-selector-messages](tasks/05-target-selector-messages.md) | Not Started | 4 | 15m | `app/message.rs`, `app/handler/update.rs` |

## Success Criteria

Phase 5 is complete when:

- [ ] Tab bar widget renders with Connected/Bootable tabs
- [ ] Active tab is visually highlighted
- [ ] 1/2 keys switch between tabs
- [ ] Devices are grouped by platform with section headers
- [ ] Device list supports scrolling for long lists
- [ ] Selection indicator (▶) shows current selection
- [ ] Up/Down navigation works within and across groups
- [ ] Connected tab shows `flutter devices` results
- [ ] Bootable tab shows simulators/AVDs (or unavailable message)
- [ ] Loading state with spinner
- [ ] Empty state messages
- [ ] Enter on Bootable device triggers boot (not launch)
- [ ] Refresh key (r) triggers device re-discovery
- [ ] `cargo fmt && cargo check && cargo test && cargo clippy -- -D warnings` passes

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
