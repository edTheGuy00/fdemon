# Phase 4: TUI DevTools Mode & Panels — Task Index

## Overview

Build the terminal UI for DevTools integration: a new `UiMode::DevTools` that replaces the log view area with sub-panels for Widget Inspector, Layout Explorer, and Performance monitoring. Reassign the `d` key from NewSessionDialog to DevTools mode entry, add panel navigation keybindings, and integrate browser DevTools launching.

**Total Tasks:** 6
**Estimated Hours:** 28-40 hours

## Task Dependency Graph

```
┌──────────────────────────────┐
│ 01-devtools-state-foundation │
│ (fdemon-app: state, message) │
└──────────┬───────────────────┘
           │
    ┌──────┼──────────────┬──────────────────┐
    │      │              │                  │
    ▼      ▼              ▼                  ▼
┌────────┐ ┌────────────┐ ┌────────────────┐ ┌────────────────┐
│02-hdlrs│ │03-perf     │ │04-widget       │ │05-layout       │
│& keys  │ │panel       │ │inspector       │ │explorer        │
│        │ │widget      │ │panel           │ │panel           │
└───┬────┘ └─────┬──────┘ └───────┬────────┘ └───────┬────────┘
    │            │                │                   │
    │            │                │  05 also depends  │
    │            │                │  on 04 (shares    │
    │            │                │  tree selection)   │
    │            │                │                   │
    └────────────┴────────┬───────┴───────────────────┘
                          ▼
               ┌────────────────────┐
               │06-render-integration│
               │   & documentation  │
               └────────────────────┘
```

## Waves (Parallelizable Groups)

### Wave 1 (Foundation)
- **01-devtools-state-foundation** — `UiMode::DevTools`, `DevToolsPanel` enum, `DevToolsViewState`, new `Message` variants (pure state/type additions in fdemon-app)

### Wave 2 (Handlers + Widgets — all parallelizable)
- **02-devtools-handlers-key-reassignment** — Reassign `d` key, create `handle_key_devtools()`, enter/exit/switch handlers, browser opener, overlay toggles (fdemon-app)
- **03-performance-panel-widget** — FPS sparkline, memory gauge, jank indicator widget (fdemon-tui)
- **04-widget-inspector-panel** — Tree view with expand/collapse, details panel, on-demand RPC (fdemon-tui + fdemon-app)
- **05-layout-explorer-panel** — ASCII flex layout visualization, constraint display, on-demand RPC (fdemon-tui + fdemon-app)

### Wave 3 (Integration)
- **06-render-integration-docs** — Wire panels into `render/mod.rs`, sub-tab bar, contextual header hints, KEYBINDINGS.md (fdemon-tui + docs)

## Tasks

| # | Task | Status | Depends On | Est. Hours | Crates | Key Modules |
|---|------|--------|------------|------------|--------|-------------|
| 1 | [01-devtools-state-foundation](tasks/01-devtools-state-foundation.md) | Done | - | 3-4h | `fdemon-app` | `state.rs`, `message.rs` |
| 2 | [02-devtools-handlers-key-reassignment](tasks/02-devtools-handlers-key-reassignment.md) | Done | 1 | 5-7h | `fdemon-app` | `handler/keys.rs`, `handler/update.rs`, `handler/devtools.rs` |
| 3 | [03-performance-panel-widget](tasks/03-performance-panel-widget.md) | Done | 1 | 5-7h | `fdemon-tui` | `widgets/performance_panel.rs` |
| 4 | [04-widget-inspector-panel](tasks/04-widget-inspector-panel.md) | Done | 1 | 6-8h | `fdemon-tui`, `fdemon-app` | `widgets/widget_inspector.rs`, `handler/devtools.rs`, `actions.rs` |
| 5 | [05-layout-explorer-panel](tasks/05-layout-explorer-panel.md) | Done | 1, 4 | 5-7h | `fdemon-tui`, `fdemon-app` | `widgets/layout_explorer.rs`, `handler/devtools.rs` |
| 6 | [06-render-integration-docs](tasks/06-render-integration-docs.md) | Done | 2, 3, 4, 5 | 4-6h | `fdemon-tui`, docs | `render/mod.rs`, `widgets/header.rs`, `KEYBINDINGS.md` |

## Success Criteria

Phase 4 is complete when:

- [ ] `d` key reassigned from NewSessionDialog to DevTools mode entry
- [ ] `+` remains the sole keybinding for NewSessionDialog
- [ ] `UiMode::DevTools` replaces log view area with DevTools panels
- [ ] `Esc` returns to Normal mode (log view)
- [ ] Widget inspector tree renders in terminal with expand/collapse navigation
- [ ] Widget details shown for selected widget
- [ ] Layout explorer visualizes flex layouts with ASCII box models
- [ ] Performance panel shows FPS sparkline, memory gauge, jank indicator
- [ ] `i`/`l`/`p` switch between Inspector, Layout, Performance sub-panels
- [ ] `b` opens DevTools in system browser from DevTools mode
- [ ] `Ctrl+r`/`Ctrl+p`/`Ctrl+d` toggle debug overlays
- [ ] Header shows contextual key hints for DevTools mode
- [ ] `docs/KEYBINDINGS.md` updated with all new keybindings
- [ ] All new code has unit tests
- [ ] No regressions in existing functionality (`cargo test --workspace`)
- [ ] `cargo clippy --workspace -- -D warnings` passes

## Keyboard Shortcuts

### Normal Mode Changes

| Key | Before Phase 4 | After Phase 4 |
|-----|----------------|---------------|
| `d` | Open NewSessionDialog | Enter DevTools mode |
| `+` | Open NewSessionDialog | Open NewSessionDialog (sole binding) |

### DevTools Mode (New)

| Key | Action |
|-----|--------|
| `Esc` | Return to Normal mode (log view) |
| `i` | Switch to Inspector sub-panel |
| `l` | Switch to Layout Explorer sub-panel |
| `p` | Switch to Performance sub-panel |
| `b` | Open DevTools in system browser |
| `Ctrl+r` | Toggle repaint rainbow overlay |
| `Ctrl+p` | Toggle performance overlay on device |
| `Ctrl+d` | Toggle debug paint overlay |

### Inspector Sub-Panel Navigation

| Key | Action |
|-----|--------|
| `Up`/`k` | Move selection up in widget tree |
| `Down`/`j` | Move selection down in widget tree |
| `Enter`/`Right`/`l` | Expand tree node / show details |
| `Left`/`h` | Collapse tree node / go to parent |
| `r` | Refresh widget tree |

## New Module Structure

```
crates/fdemon-app/src/
├── state.rs                    # MODIFIED: add UiMode::DevTools, DevToolsPanel, DevToolsViewState
├── message.rs                  # MODIFIED: add DevTools Message variants
├── handler/
│   ├── mod.rs                  # MODIFIED: add UpdateAction variants for tree/layout fetch
│   ├── keys.rs                 # MODIFIED: reassign 'd', add handle_key_devtools()
│   ├── update.rs               # MODIFIED: handle DevTools messages
│   └── devtools.rs             # NEW: DevTools message handler functions
└── actions.rs                  # MODIFIED: spawn widget tree / layout fetch tasks

crates/fdemon-tui/src/
├── render/mod.rs               # MODIFIED: add UiMode::DevTools match arm
├── widgets/
│   ├── mod.rs                  # MODIFIED: add devtools module exports
│   ├── header.rs               # MODIFIED: contextual key hints for DevTools mode
│   ├── devtools/               # NEW: DevTools panel module
│   │   ├── mod.rs              # DevToolsPanel widget (sub-tab bar + dispatch)
│   │   ├── performance.rs      # Performance sub-panel widget
│   │   ├── inspector.rs        # Widget inspector sub-panel widget
│   │   └── layout_explorer.rs  # Layout explorer sub-panel widget
│   └── ...existing widgets...

docs/
└── KEYBINDINGS.md              # MODIFIED: update 'd' key, add DevTools section
```

## Notes

- **Phase 3 provides all data sources.** `session.performance` has ring buffers for memory, GC, and frame timing. `VmRequestHandle` enables on-demand RPC for inspector/layout. No new data plumbing is needed.
- **The SettingsPanel is the architectural template.** It's a full-screen overlay with sub-tabs and contextual key handling — the DevTools panel follows this exact pattern.
- **`DevToolsSettings` config already exists** in `config/types.rs:277` with `auto_open` and `browser` fields. Settings UI items are already wired in `settings_items.rs`.
- **`session.ws_uri`** is already captured from `app.debugPort` and propagated to `SharedState.devtools_uri` — this is used for the browser URL construction.
- **Widget tree fetching is async and on-demand.** The inspector doesn't pre-load trees; it fetches when the user enters the Inspector panel or presses `r` to refresh.
