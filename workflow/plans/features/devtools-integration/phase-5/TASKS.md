# Phase 5: Polish, Configuration & Documentation — Task Index

## Overview

Production-ready polish for the DevTools integration: expand configuration options, improve connection resilience UX, add error handling and performance optimizations, fix minor Phase 4 review issues, update the project README, and build out the website documentation (new DevTools page, keybindings sync, configuration expansion).

**Total Tasks:** 9
**Estimated Hours:** 32-48 hours

## Task Dependency Graph

```
Wave 1 (Core Improvements — parallelizable)
┌─────────────────────────┐  ┌──────────────────────────┐  ┌────────────────────────────┐
│01-expand-devtools-config│  │02-connection-state-ui    │  │05-phase4-review-fixes      │
│(fdemon-app: config,     │  │(fdemon-tui, fdemon-app:  │  │(fdemon-app: handler,       │
│ handlers, actions)      │  │ state indicators, timeouts│  │ actions: minor bug fixes)  │
└───────────┬─────────────┘  └────────────┬─────────────┘  └────────────┬───────────────┘
            │                              │                             │
            │            ┌─────────────────┤                             │
            │            │                 │                             │
            ▼            ▼                 │                             │
┌──────────────────────────┐               │                             │
│03-error-ux-improvements  │               │                             │
│(fdemon-tui, fdemon-app:  │               │                             │
│ fallback UI, error msgs) │               │                             │
└──────────────────────────┘               │                             │
                                           │                             │
Wave 1.5                                   │                             │
┌──────────────────────────┐               │                             │
│04-performance-polish     │───────────────┘                             │
│(fdemon-app: debounce,    │  (depends on 02 for timeout infrastructure) │
│ lazy tree hints)         │                                             │
└──────────────────────────┘                                             │
                                                                         │
Wave 2 (Documentation & Website — parallelizable after Wave 1)           │
┌──────────────────────────┐  ┌────────────────────────────┐             │
│06-readme-devtools-section│  │07-website-devtools-page    │             │
│(README.md)               │  │(website: new /docs/devtools│             │
└──────────────────────────┘  │ page with Leptos component)│             │
                              └────────────────────────────┘             │
                                                                         │
┌──────────────────────────┐  ┌────────────────────────────┐             │
│08-website-keybindings    │  │09-website-config-update    │─────────────┘
│(website: data.rs, fix d  │  │(website: expand [devtools] │  (needs 01 done first)
│ key, add devtools section│  │ section with new fields)   │
└──────────────────────────┘  └────────────────────────────┘
```

## Waves (Parallelizable Groups)

### Wave 1 (Core Improvements)
- **01-expand-devtools-config** — Add all planned `[devtools]` config fields, wire into handlers/actions
- **02-connection-state-ui** — TUI indicators for VM reconnection state, request timeout handling
- **05-phase4-review-fixes** — Fix `percent_encode_uri` uppercase, overlay toggle debounce, layout `object_id` check

### Wave 1.5 (Quality — after 01 and 02)
- **03-error-ux-improvements** — User-friendly error messages in panels, fallback UI when features unavailable
- **04-performance-polish** — Debounce rapid refresh/toggle requests, lazy tree depth configuration

### Wave 2 (Documentation & Website — after Wave 1)
- **06-readme-devtools-section** — Add DevTools section to project README
- **07-website-devtools-page** — New `/docs/devtools` Leptos page covering all DevTools features
- **08-website-keybindings-update** — Fix `d` key in `data.rs`, add DevTools keybinding sections
- **09-website-config-update** — Update configuration page with all new `[devtools]` settings (depends on 01)

## Tasks

| # | Task | Status | Depends On | Est. Hours | Crates/Areas | Key Modules |
|---|------|--------|------------|------------|--------------|-------------|
| 1 | [01-expand-devtools-config](tasks/01-expand-devtools-config.md) | Not Started | - | 4-6h | `fdemon-app` | `config/types.rs`, `handler/devtools.rs`, `actions.rs` |
| 2 | [02-connection-state-ui](tasks/02-connection-state-ui.md) | Not Started | - | 4-6h | `fdemon-tui`, `fdemon-app` | `widgets/devtools/mod.rs`, `state.rs`, `handler/devtools.rs` |
| 3 | [03-error-ux-improvements](tasks/03-error-ux-improvements.md) | Not Started | 1, 2 | 3-5h | `fdemon-tui`, `fdemon-app` | `widgets/devtools/*.rs`, `handler/devtools.rs` |
| 4 | [04-performance-polish](tasks/04-performance-polish.md) | Not Started | 2 | 3-4h | `fdemon-app` | `handler/devtools.rs`, `actions.rs` |
| 5 | [05-phase4-review-fixes](tasks/05-phase4-review-fixes.md) | Not Started | - | 2-3h | `fdemon-app` | `handler/devtools.rs`, `actions.rs` |
| 6 | [06-readme-devtools-section](tasks/06-readme-devtools-section.md) | Not Started | 1 | 1-2h | docs | `README.md` |
| 7 | [07-website-devtools-page](tasks/07-website-devtools-page.md) | Not Started | 1, 2 | 6-8h | website | `pages/docs/devtools.rs`, `lib.rs`, `pages/docs/mod.rs` |
| 8 | [08-website-keybindings-update](tasks/08-website-keybindings-update.md) | Not Started | - | 2-3h | website | `data.rs`, `pages/docs/keybindings.rs` |
| 9 | [09-website-config-update](tasks/09-website-config-update.md) | Not Started | 1 | 3-5h | website | `pages/docs/configuration.rs` |

## Success Criteria

Phase 5 is complete when:

- [ ] `DevToolsSettings` expanded with all planned fields (`default_panel`, `performance_refresh_ms`, `memory_history_size`, `tree_max_depth`, overlay defaults, logging sub-section)
- [ ] All new config fields are wired into actual behavior (refresh intervals, default panel, overlay defaults)
- [ ] TUI shows clear indicators when VM connection is reconnecting or lost
- [ ] Slow/timed-out VM responses handled gracefully with user-visible feedback
- [ ] Error messages in DevTools panels are user-friendly, not raw errors
- [ ] Fallback UI renders when features are unavailable (profile/release mode, no VM)
- [ ] Overlay toggle has debounce to prevent rapid RPC spam
- [ ] Widget tree refresh has debounce/cooldown
- [ ] `percent_encode_uri` uses uppercase hex per RFC 3986
- [ ] Layout panel uses correct `value_id` for `getLayoutExplorerNode`
- [ ] README.md has a DevTools section explaining the feature
- [ ] Website has a dedicated `/docs/devtools` page
- [ ] Website keybindings page includes DevTools mode sections
- [ ] Website keybindings data fixes `d` key (no longer "Start New Session")
- [ ] Website configuration page documents all expanded `[devtools]` settings
- [ ] All new code has unit tests
- [ ] No regressions (`cargo test --workspace` passes)
- [ ] `cargo clippy --workspace -- -D warnings` passes
- [ ] Website builds without errors (`cd website && trunk build`)

## Key Research Findings

### Connection Resilience (Already Partially Implemented)

The VM service client (`crates/fdemon-daemon/src/vm_service/client.rs`) already implements:
- Auto-reconnect with exponential backoff (1s → 2s → 4s → 8s → 16s → 30s cap)
- Max 10 reconnection attempts before giving up
- `ConnectionState::Reconnecting { attempt }` enum variant
- Isolate ID cache invalidation on reconnect
- Stream re-subscription after reconnect

**What's missing**: The TUI has no visual indicator of these states. `ConnectionState` is internal to the daemon crate and not surfaced to the app/TUI layer. Task 02 needs to bridge this gap.

### Website Technology Stack

The website is a **Rust + WASM SPA** built with **Leptos 0.8** + **Tailwind CSS v4**, NOT a Markdown-based static site. All content is authored as Leptos component code in `.rs` files. Adding a new page requires:
1. Create `pages/docs/<name>.rs` with a Leptos component
2. Register module in `pages/docs/mod.rs`
3. Add sidebar entry to `doc_items()` in `pages/docs/mod.rs`
4. Add route in `lib.rs` under `<ParentRoute path=path!("/docs") ...>`

### Existing Website DevTools Coverage

- **Keybindings page**: `data.rs` has NO DevTools section; still shows `+ / d` for "Start New Session"
- **Configuration page**: Only shows `auto_open` and `browser` (2 of the planned ~10+ fields)
- **No dedicated DevTools page** exists

## Notes

- **Phase 3/4 provide all infrastructure.** The VM connection, widget tree RPC, layout RPC, performance polling, and TUI panels are fully operational. Phase 5 is purely polish, configuration, and documentation.
- **The `[devtools.logging]` sub-section** from the PLAN.md (hybrid_enabled, prefer_vm_level, show_source_indicator, dedupe_threshold_ms) should be evaluated for actual need — some of these may already be hard-coded behaviors from Phase 1 that just need config knobs.
- **Website changes are substantial** because it's a Rust WASM app, not Markdown. Each page is a full Leptos component with local sub-components, tables, code blocks, and styled sections.
