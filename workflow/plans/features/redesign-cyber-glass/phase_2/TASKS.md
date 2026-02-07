# Phase 2: Main Log Screen Redesign - Task Index

## Overview

Transform the main log screen (header + log panel + status bar) to match the Cyber-Glass design. This phase introduces visual depth with glass containers, metadata bars, styled log entries, a device pill in the header, and terminal background fill.

**Total Tasks:** 6
**Crate:** `fdemon-tui`
**Depends on:** Phase 1 (theme module must exist)

## Task Dependency Graph

```
┌───────────────────────────┐
│  01-terminal-background   │
│  (DEEPEST_BG fill)        │
└───────────────────────────┘

┌───────────────────────────┐     ┌──────────────────────────────┐
│  02-redesign-header       │     │  03-redesign-log-view        │
│  (glass, device pill,     │     │  (glass container, top meta  │
│   shortcuts, tabs)        │     │   bar, styled entries)       │
└───────────────────────────┘     └──────────────┬───────────────┘
                                                 │
                                  ┌──────────────▼───────────────┐
                                  │  04-merge-status-into-log    │
                                  │  (bottom metadata bar)       │
                                  └──────────────┬───────────────┘
                                                 │
         ┌───────────────────────────────────────┘
         ▼
┌───────────────────────────┐
│  05-update-layout         │
│  (proportions, gaps)      │
└───────────┬───────────────┘
            │
            ▼
┌───────────────────────────┐
│  06-update-tests          │
└───────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Modules |
|---|------|--------|------------|---------|
| 1 | [01-terminal-background](tasks/01-terminal-background.md) | Done | - | `render/mod.rs` |
| 2 | [02-redesign-header](tasks/02-redesign-header.md) | Done | - | `widgets/header.rs`, `widgets/tabs.rs` |
| 3 | [03-redesign-log-view](tasks/03-redesign-log-view.md) | Done | - | `widgets/log_view/mod.rs`, `widgets/log_view/styles.rs` |
| 4 | [04-merge-status-into-log](tasks/04-merge-status-into-log.md) | Done | 3 | `widgets/log_view/mod.rs`, `widgets/status_bar/mod.rs` |
| 5 | [05-update-layout](tasks/05-update-layout.md) | Done | 2, 3, 4 | `layout.rs`, `render/mod.rs` |
| 6 | [06-update-tests](tasks/06-update-tests.md) | Done | 1, 2, 3, 4, 5 | All test files |

## Execution Strategy

**Wave 1** (parallel): Tasks 01, 02, and 03 are independent — terminal background, header, and log view can be developed simultaneously.

**Wave 2** (after 03): Task 04 merges the status bar into the log view's bottom metadata bar. Requires the log view redesign to be in place.

**Wave 3** (after 02, 03, 04): Task 05 adjusts layout proportions to fit the redesigned widgets.

**Wave 4** (after all): Task 06 updates all broken tests.

## Success Criteria

Phase 2 is complete when:

- [ ] Terminal background uses `DEEPEST_BG` color
- [ ] Header shows pulsing dot, project name, shortcut hints, device pill
- [ ] Header uses glass container style (rounded borders, `CARD_BG` background)
- [ ] Session tabs are integrated below the header title line
- [ ] Log panel is a glass container with `CARD_BG` background and rounded borders
- [ ] Log panel has top metadata bar ("TERMINAL LOGS" + "LIVE FEED" badge)
- [ ] Log entries show colored timestamps, source tags, and messages matching design
- [ ] Status info is integrated into log panel's bottom metadata bar
- [ ] Bottom bar shows: running dot + phase + mode badge + uptime + error count
- [ ] Layout has visual breathing room between header and log panel
- [ ] All existing functionality preserved (scroll, search, filter, links, stack traces)
- [ ] `cargo test --workspace` passes
- [ ] `cargo clippy --workspace` passes with no warnings

## Notes

- **Phase 1 prerequisite**: All tasks assume the theme module from Phase 1 exists. Colors are referenced via `palette::` constants and `styles::` builders.
- **Functionality preservation is critical**: The log view has complex features (virtualized scroll, horizontal scroll, search highlighting, stack trace collapse, link highlight mode). None of these should regress.
- **StatusBar compact variant**: The compact status bar (< 60 cols) still needs to work. Task 04 handles this.
- **HeaderWithTabs legacy code**: The `HeaderWithTabs` widget in `tabs.rs` is not used by the main render pipeline. It can be cleaned up or left as-is. Task 02 focuses on `MainHeader` + `SessionTabs`.
