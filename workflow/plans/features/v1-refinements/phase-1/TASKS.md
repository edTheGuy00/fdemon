# Phase 1: Log View Word Wrap - Task Index

## Overview

Add word wrap mode to the log view, eliminating horizontal scrolling by default. Users can toggle between wrap and horizontal scroll modes with the `w` key.

**Total Tasks:** 3
**Crates Affected:** `fdemon-app`, `fdemon-tui`

## Task Dependency Graph

```
┌─────────────────────────┐
│  01-wrap-state          │
│  (app layer: state,     │
│   message, keybinding,  │
│   handler, scroll guard)│
└───────────┬─────────────┘
            │
            ▼
┌─────────────────────────┐
│  02-wrap-rendering      │
│  (TUI layer: LogView    │
│   widget, conditional   │
│   wrap, line height,    │
│   status indicator,     │
│   render wiring)        │
└───────────┬─────────────┘
            │
            ▼
┌─────────────────────────┐
│  03-wrap-tests          │
│  (both layers: unit     │
│   tests for state,      │
│   rendering, scroll)    │
└─────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Crates | Key Files |
|---|------|--------|------------|--------|-----------|
| 1 | [01-wrap-state](tasks/01-wrap-state.md) | Done | - | `fdemon-app` | `log_view_state.rs`, `message.rs`, `keys.rs`, `update.rs`, `scroll.rs` |
| 2 | [02-wrap-rendering](tasks/02-wrap-rendering.md) | Done | 1 | `fdemon-tui` | `log_view/mod.rs`, `render/mod.rs` |
| 3 | [03-wrap-tests](tasks/03-wrap-tests.md) | Done | 2 | `fdemon-app`, `fdemon-tui` | `log_view_state.rs` tests, `log_view/tests.rs` |

## Success Criteria

Phase 1 is complete when:

- [ ] Logs wrap at window width by default (wrap mode defaults to `true`)
- [ ] `w` key toggles between wrap and horizontal scroll modes
- [ ] Scroll position remains correct with wrapped lines
- [ ] Horizontal scroll keys (`h/l/0/$`) are no-ops when wrap is on
- [ ] `[wrap]` / `[nowrap]` indicator appears in the log view metadata bar
- [ ] All existing log view tests pass + new wrap mode tests added
- [ ] `cargo test --workspace` passes
- [ ] `cargo clippy --workspace -- -D warnings` passes

## Keyboard Shortcuts

| Key | Mode | Action |
|-----|------|--------|
| `w` | Normal | Toggle wrap mode for log view |

## Notes

- `w` is unused in normal mode; `Ctrl+w` is taken for `CloseCurrentSession` but bare `w` is free
- `wrap_mode` lives on `LogViewState` (per-session) rather than `UiSettings` (global) — this allows different sessions to have independent wrap preferences
- Default is `true` (wrap on) — most users want full log visibility without horizontal scrolling
- When wrap is enabled, `h_offset` is reset to 0 and horizontal scroll keys become no-ops
- Ratatui's `Paragraph::wrap(Wrap { trim: false })` handles word wrapping at widget boundary
- The main complexity is `calculate_entry_lines()` — it must account for wrapped line heights to keep scroll position and scrollbar accurate
