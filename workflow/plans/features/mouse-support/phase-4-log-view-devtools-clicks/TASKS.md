# Phase 4: Log View & DevTools Panel-Internal Clicks — Task Index

## Overview

Phase 4 expands the per-frame mouse region registry from Phase 3 — currently used by the header and session tabs — to cover the **log view** and the **DevTools panels** (Inspector, Performance, Network). Concretely, after Phase 4:

1. **Log view** — left-clicking a row records a `Message::ClickLogRow { entry_id, frame_index }`. A second click on the same entry within 400 ms produces a follow-up `Message::ToggleStackTraceForEntry { entry_id }` so that double-click expands / collapses the clicked entry's stack trace. Single click is otherwise non-mutating in v1 (no scroll, no visible focus indicator) — the click is only retained in `AppState::last_log_click` for double-click detection.
2. **DevTools sub-tab bar** — left-clicking `[i] Inspector` / `[p] Performance` / `[n] Network` switches the active panel via the existing `Message::SwitchDevToolsPanel(DevToolsPanel)`.
3. **Inspector tree** — left-clicking a tree row sets `inspector.selected_index` and triggers an auto-fetch (mirroring `InspectorNav::Up/Down` semantics from Phase 5/Task 06 of the original DevTools work). Left-clicking the leading expansion glyph (`▶` / `▼` / `●`) in addition toggles the node's expanded state.
4. **Performance frame chart** — left-clicking a frame's bar pair (UI + Raster) selects that frame via the existing `Message::SelectPerformanceFrame { index: Some(i) }`.
5. **Network table** — left-clicking a request row selects it via the existing `Message::NetworkSelectRequest { index: Some(i) }`. Detail sub-tabs (`[g] [h] [q] [s] [t]`) become clickable via `Message::NetworkSwitchDetailTab(tab)`.

The dispatcher arm for `UiMode::DevTools` in `handler/mouse/mod.rs::handle_press` is wired to a new `handler/mouse/devtools::handle_press` that performs a registry hit-test and returns the matched `Emit` message. Right-click and Drag/Release remain no-ops in this phase. Modal precedence (z-index) is unchanged — Phase 5 dialogs/overlays will be the first consumers of `z_index = 1`.

When Phase 4 is done, every panel surface that has a keyboard activator also responds to clicks. Mouse is now usable for the most-trafficked DevTools workflows: pick a frame, expand a widget, focus a log entry, switch detail tabs.

**Total Tasks:** 10
**Estimated Hours:** ~12 hours

## Prerequisites

- Phases 1–3 plus Phase 3.5 (RAII guard, dispatcher gate lift) must be merged on `feat/mouse-support`. The registry, `MouseRegionGuard`, `MouseCtx` plumbing pattern, and the `tag_filter_visible` short-circuit must already be in place.
- No new external dependencies. `fdemon-app` continues not to depend on `ratatui`; the registry uses `MouseRect` and the TUI converts `ratatui::layout::Rect` at the boundary.

## Task Dependency Graph

```
Wave 1 (parallel — different crates):
┌────────────────────────────────────┐  ┌────────────────────────────────────┐
│ 01 - phase4-messages-and-state     │  │ 02 - tui-region-plumbing-and-      │
│ (message.rs + state.rs +           │  │      devtools-tab-bar              │
│  update.rs delegate arms +         │  │ (render/mod.rs +                   │
│  handler/log_view.rs stubs +       │  │  widgets/log_view + devtools/*     │
│  handler/devtools/inspector.rs     │  │  sister render_with_regions fns +  │
│  stubs)                            │  │  DevTools sub-tab regions in       │
│                                    │  │  widgets/devtools/mod.rs)          │
└──────────────┬─────────────────────┘  └──────────────┬─────────────────────┘
               │                                       │
       ┌───────┴────────┬────────────────────┐         │
Wave 2:▼                ▼                    ▼         ▼
┌──────────────┐ ┌──────────────────┐ ┌──────────────┐ ┌──────────────────┐
│ 03 - log-    │ │ 04 - inspector-  │ │ 05 - mouse-  │ │ 06 - log-view-   │
│   view-      │ │   click-         │ │   press-     │ │   row-regions    │
│   click-     │ │   handlers       │ │   devtools-  │ │ (widgets/log_    │
│   handlers   │ │ (handler/        │ │   dispatcher │ │  view/mod.rs)    │
│ (handler/    │ │  devtools/       │ │ (handler/    │ │                  │
│  log_view.rs)│ │  inspector.rs)   │ │  mouse/      │ │                  │
│              │ │                  │ │  devtools.rs │ │                  │
│              │ │                  │ │  + mouse/    │ │                  │
│              │ │                  │ │  mod.rs)     │ │                  │
└──────────────┘ └──────────────────┘ └──────────────┘ └──────────────────┘

           ┌──────────────────┐ ┌──────────────────┐ ┌──────────────────┐
Wave 2 ►   │ 07 - inspector-  │ │ 08 - performance │ │ 09 - network-    │
(continued)│   tree-row-      │ │   -frame-chart-  │ │   table-and-     │
           │   regions        │ │   regions        │ │   detail-tab-    │
           │ (widgets/        │ │ (widgets/        │ │   regions        │
           │  devtools/       │ │  devtools/       │ │ (widgets/        │
           │  inspector/      │ │  performance/    │ │  devtools/       │
           │  mod.rs +        │ │  mod.rs +        │ │  network/        │
           │  tree_panel.rs)  │ │  frame_chart/    │ │  mod.rs +        │
           │                  │ │  bars.rs)        │ │  request_table   │
           │                  │ │                  │ │  + request_      │
           │                  │ │                  │ │  details.rs)     │
           └──────────────────┘ └──────────────────┘ └──────────────────┘
                                                                │
Wave 3:                                                         ▼
┌──────────────────────────────────────────────────────────────────┐
│ 10 - integration-and-snapshot-tests                              │
│ (handler/tests.rs + render/tests.rs + per-panel snapshot tests)  │
└──────────────────────────────────────────────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Est. Hours | Crate / Area |
|---|------|--------|------------|------------|--------------|
| 1 | [01-phase4-messages-and-state](tasks/01-phase4-messages-and-state.md) | Not Started | — | 1.5h | `fdemon-app` |
| 2 | [02-tui-region-plumbing-and-devtools-tab-bar](tasks/02-tui-region-plumbing-and-devtools-tab-bar.md) | Not Started | — | 2h | `fdemon-tui` |
| 3 | [03-log-view-click-handlers](tasks/03-log-view-click-handlers.md) | Not Started | 1 | 1h | `fdemon-app` |
| 4 | [04-inspector-click-handlers](tasks/04-inspector-click-handlers.md) | Not Started | 1 | 1h | `fdemon-app` |
| 5 | [05-mouse-press-devtools-dispatcher](tasks/05-mouse-press-devtools-dispatcher.md) | Not Started | — | 0.75h | `fdemon-app` |
| 6 | [06-log-view-row-regions](tasks/06-log-view-row-regions.md) | Not Started | 1, 2 | 1.25h | `fdemon-tui` |
| 7 | [07-inspector-tree-row-regions](tasks/07-inspector-tree-row-regions.md) | Not Started | 1, 2 | 1h | `fdemon-tui` |
| 8 | [08-performance-frame-chart-regions](tasks/08-performance-frame-chart-regions.md) | Not Started | 2 | 1h | `fdemon-tui` |
| 9 | [09-network-table-and-detail-tab-regions](tasks/09-network-table-and-detail-tab-regions.md) | Not Started | 2 | 1.25h | `fdemon-tui` |
| 10 | [10-integration-and-snapshot-tests](tasks/10-integration-and-snapshot-tests.md) | Not Started | 3, 4, 5, 6, 7, 8, 9 | 1.25h | `fdemon-app`, `fdemon-tui` |

## File Overlap Analysis

| Task | Files Modified (Write) | Files Read (Dependencies) |
|------|------------------------|---------------------------|
| 01-phase4-messages-and-state | `crates/fdemon-app/src/message.rs`, `crates/fdemon-app/src/state.rs`, `crates/fdemon-app/src/handler/update.rs`, `crates/fdemon-app/src/handler/log_view.rs`, `crates/fdemon-app/src/handler/devtools/inspector.rs` | `crates/fdemon-app/src/session/session.rs` (for `focused_entry_id`, `toggle_stack_trace`), `crates/fdemon-app/src/state.rs` (for `InspectorState::visible_nodes`) |
| 02-tui-region-plumbing-and-devtools-tab-bar | `crates/fdemon-tui/src/render/mod.rs`, `crates/fdemon-tui/src/widgets/log_view/mod.rs`, `crates/fdemon-tui/src/widgets/devtools/mod.rs`, `crates/fdemon-tui/src/widgets/devtools/inspector/mod.rs`, `crates/fdemon-tui/src/widgets/devtools/performance/mod.rs`, `crates/fdemon-tui/src/widgets/devtools/network/mod.rs` | `crates/fdemon-app/src/mouse_regions.rs` (for `MouseRect`, `MouseAction::Emit`), `crates/fdemon-app/src/message.rs` (for `Message::SwitchDevToolsPanel`) |
| 03-log-view-click-handlers | `crates/fdemon-app/src/handler/log_view.rs` | `crates/fdemon-app/src/handler/update.rs` (for `UpdateResult` chaining) |
| 04-inspector-click-handlers | `crates/fdemon-app/src/handler/devtools/inspector.rs` | `crates/fdemon-app/src/state.rs` (for `InspectorState::visible_nodes`, `InspectorState::is_layout_fetch_debounced`) |
| 05-mouse-press-devtools-dispatcher | `crates/fdemon-app/src/handler/mouse/devtools.rs`, `crates/fdemon-app/src/handler/mouse/mod.rs` | `crates/fdemon-app/src/mouse_regions.rs` (for `MouseRegionGuard::take_guard`), `crates/fdemon-app/src/handler/mouse/normal.rs` (template for the take-guard + hit-test pattern) |
| 06-log-view-row-regions | `crates/fdemon-tui/src/widgets/log_view/mod.rs` | `crates/fdemon-app/src/message.rs` (`ClickLogRow`), `crates/fdemon-tui/src/render/mod.rs` (`MouseCtx`) |
| 07-inspector-tree-row-regions | `crates/fdemon-tui/src/widgets/devtools/inspector/mod.rs`, `crates/fdemon-tui/src/widgets/devtools/inspector/tree_panel.rs` | `crates/fdemon-app/src/message.rs` (`DevToolsInspectorSelectRow`, `DevToolsInspectorToggleNode`) |
| 08-performance-frame-chart-regions | `crates/fdemon-tui/src/widgets/devtools/performance/mod.rs`, `crates/fdemon-tui/src/widgets/devtools/performance/frame_chart/bars.rs` | `crates/fdemon-app/src/message.rs` (`SelectPerformanceFrame`) |
| 09-network-table-and-detail-tab-regions | `crates/fdemon-tui/src/widgets/devtools/network/mod.rs`, `crates/fdemon-tui/src/widgets/devtools/network/request_table.rs`, `crates/fdemon-tui/src/widgets/devtools/network/request_details.rs` | `crates/fdemon-app/src/message.rs` (`NetworkSelectRequest`, `NetworkSwitchDetailTab`) |
| 10-integration-and-snapshot-tests | `crates/fdemon-app/src/handler/tests.rs`, `crates/fdemon-tui/src/render/tests.rs` | All Phase-4 production files |

### Overlap Matrix

Wave 1 (no Phase-4 internal predecessors): 01, 02
Wave 2 (depends on 01 and/or 02): 03, 04, 05, 06, 07, 08, 09
Wave 3 (depends on 03 + 04 + 05 + 06 + 07 + 08 + 09): 10

| Task Pair | Wave | Shared Write Files | Isolation Strategy |
|-----------|------|--------------------|---------------------|
| 01 + 02 | Wave 1 | None — disjoint crates (`fdemon-app` vs `fdemon-tui`) | **Parallel (worktree)** |
| 03 + 04 + 05 | Wave 2 (handlers) | None — `handler/log_view.rs`, `handler/devtools/inspector.rs`, `handler/mouse/{devtools.rs,mod.rs}` are disjoint | **Parallel (worktree)** |
| 06 + 07 + 08 + 09 | Wave 2 (widgets) | None — `widgets/log_view/mod.rs`, `widgets/devtools/inspector/*`, `widgets/devtools/performance/*`, `widgets/devtools/network/*` are disjoint | **Parallel (worktree)** |
| 03–09 (across handler/widget) | Wave 2 | None — handlers live in `fdemon-app/src/handler/`, widgets in `fdemon-tui/src/widgets/`; the only common file the two groups read is `message.rs`, which is read-only after Task 01 lands | **Parallel (worktree)** |
| 10 alone | Wave 3 | n/a — single task | **Single task on current branch** |

Notes on overlap analysis:

- **`handler/log_view.rs` overlap (01 ↔ 03)** is dependency-ordered: Task 01 inserts stub functions (`handle_click_log_row`, `handle_toggle_stack_trace_for_entry`) returning `UpdateResult::none()` so the dispatch arms in `update.rs` compile; Task 03 then fills in the bodies. Sequential by wave structure.
- **`handler/devtools/inspector.rs` overlap (01 ↔ 04)** is dependency-ordered for the same reason: Task 01 adds stubs (`handle_inspector_select_row`, `handle_inspector_toggle_node`); Task 04 fills them in.
- **`widgets/log_view/mod.rs` overlap (02 ↔ 06)** is dependency-ordered: Task 02 introduces a sister `render_with_regions(...)` function that delegates to the existing `Widget::render` impl with a no-op for region recording; Task 06 adds the actual `mouse_ctx.click(...)` calls inside that function.
- **`widgets/devtools/inspector/mod.rs` overlap (02 ↔ 07)**, **`widgets/devtools/performance/mod.rs` overlap (02 ↔ 08)**, and **`widgets/devtools/network/mod.rs` overlap (02 ↔ 09)** all follow the same pattern: Task 02 stubs the sister function; the per-panel task fills in the body.
- **`widgets/devtools/mod.rs` is written only by Task 02** — sub-tab regions for the `[i] Inspector` / `[p] Performance` / `[n] Network` labels are recorded inside `render_tab_bar` as part of the plumbing pass. No follow-up task needs to edit `mod.rs`. This is intentional: the sub-tab bar is the only DevTools surface where region recording is trivially co-located with the existing render code, so deferring it to Wave 2 only adds a sequential dependency without parallelism gain.
- **`render/mod.rs`** is written only by Task 02. Each widget retains its existing `Widget` / `StatefulWidget` impl untouched; the new sister function pattern follows precedent set by Phase 3 Task 04 (`render_main_header(area, buf, &header, Some(&mut mouse_ctx))`).
- **Cross-crate parallel safety (Wave 2 handlers ↔ widgets)**: Tasks 03/04/05 operate inside `fdemon-app`; Tasks 06/07/08/09 operate inside `fdemon-tui`. They communicate only via `Message` variants (defined in Task 01) and the registry API (defined in Phases 1–3). No write overlap; full parallelism.

## Success Criteria

Phase 4 is complete when:

- [ ] `cargo fmt --all -- --check` passes
- [ ] `cargo check --workspace --all-targets` passes
- [ ] `cargo test --workspace` passes (no regressions; existing baseline grows by ≥ 14 tests across the new handlers and snapshot suites)
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes
- [ ] **Messages exist:** `Message::ClickLogRow { entry_id: u64, frame_index: Option<usize> }`, `Message::ToggleStackTraceForEntry { entry_id: u64 }`, `Message::DevToolsInspectorSelectRow { index: usize }`, `Message::DevToolsInspectorToggleNode { index: usize }`. No new variants beyond these four.
- [ ] **Existing messages reused:** `Message::SwitchDevToolsPanel`, `Message::SelectPerformanceFrame`, `Message::NetworkSelectRequest`, `Message::NetworkSwitchDetailTab` are reused for clicks; no parallel variants are added.
- [ ] `AppState::last_log_click: Option<LogClickStamp>` exists (where `LogClickStamp { entry_id: u64, at: std::time::Instant }`), reset on session change and on `ClickLogRow` follow-up emission.
- [ ] **Log view single click → no state change** other than `last_log_click` update. (Visible focus indicator is deferred to a future enhancement.)
- [ ] **Log view double click on same entry within 400 ms** produces a follow-up `Message::ToggleStackTraceForEntry { entry_id }`. The handler delegates to `Session::toggle_stack_trace`. Verified by handler unit test.
- [ ] **DevTools sub-tab click** in the `[i]` / `[p]` / `[n]` rect emits `Message::SwitchDevToolsPanel(panel)`. Verified by snapshot test on the registry.
- [ ] **Inspector tree row click** sets `inspector.selected_index` and dispatches a layout-fetch action under the same debounce / cache-hit rules as `InspectorNav::Up` / `InspectorNav::Down`. Glyph click additionally toggles the node's `expanded` set.
- [ ] **Performance frame click** sets `performance.selected_frame = Some(global_idx)`. Click outside any bar (e.g., budget-line area without a bar) is a no-op.
- [ ] **Network row click** sets `network.selected_index = Some(entry_idx)` and triggers `fetch_selected_detail_action`. **Detail-tab click** ([g] / [h] / [q] / [s] / [t]) emits `Message::NetworkSwitchDetailTab(tab)`. Filter-input-active state suppresses both (mirroring `handle_network_scroll`).
- [ ] `handler/mouse/devtools::handle_press` exists, queries the registry under a `MouseRegionGuard`, returns the matched message, and is wired into the `UiMode::DevTools` arm of `handler/mouse/mod.rs::handle_press`. Right-click and middle-click in DevTools mode return `None` for v1.
- [ ] **z-index precedence unchanged** — Phase 4 never registers `z_index = 1`. Phase 5 dialogs/overlays remain the first consumers. Verified by snapshot test that all Phase-4 regions are at `z_index = 0`.
- [ ] **No widget renders unconditionally** — every region recording site checks the rect has non-zero area before pushing to the builder (consistent with Phase 3 invariant).
- [ ] Snapshot tests on the registry contents:
  - DevTools sub-tab bar at 80×24 with active panel = Inspector: 3 regions in left-to-right order matching `[i] Inspector` / `[p] Performance` / `[n] Network`.
  - Inspector tree at 80×24 with 5 visible nodes: 5 row regions + 5 glyph regions (10 total). Glyph regions are pushed after row regions so last-pushed-wins resolves the smaller area on a glyph cell.
  - Performance frame chart at 80×24 with 8 frames: 8 bar regions, each `CHARS_PER_FRAME` (3 cols) wide.
  - Network table at 80×24 with 10 visible rows: 10 row regions on the data rows + 5 detail-tab regions on the detail panel (when a request is selected).
  - Log view at 80×24 with 12 visible lines (mix of message + stack frames): 12 row regions; each carries the correct `entry_id` and `frame_index` (`None` for message lines, `Some(i)` for the i-th stack frame).
- [ ] Manual smoke test on macOS:
  - Run a Flutter session → click in the log area → no scroll, no crash → click again on same row within 400 ms → if the entry has a stack trace, it expands; if expanded, it collapses.
  - Open DevTools (`d` key) → click `[p] Performance` → Performance panel becomes active.
  - Inspector with a tree loaded: click a child row → row becomes selected; layout panel updates within ~500 ms.
  - Performance with frames recorded: click a bar in the middle of the chart → that frame is highlighted with `▔`; detail panel shows its timing.
  - Network with requests recorded: click a row → details appear in side panel; click `[h] Headers` → detail panel switches to headers tab.

## Notes

- **Why a new `Message::ToggleStackTraceForEntry { entry_id }` instead of reusing `ToggleStackTrace`.** The existing `ToggleStackTrace` operates on `session.focused_entry_id()`, which is the entry at the *scroll offset*, not the clicked entry. A double-click on a row five lines below the focus position must toggle that row's stack trace, not the scroll-focused one. The new variant carries the entry id explicitly. The keyboard `c` key continues to emit the existing `ToggleStackTrace` and operate on the scroll-focused entry — no behavioral change for keyboard users.

- **Why per-row registry entries instead of one coordinate-aware region.** The PLAN.md sketch suggested `MouseAction::EmitWithCoord(|x, y| Message::FocusLogEntryAtRow { row })` plus a row→entry map maintained by `LogViewState`. Per-row entries are cleaner: each row's `entry_id` and `frame_index` are known at render time, so the registry stores `Emit(ClickLogRow { entry_id, frame_index })` directly and the handler doesn't need to consult any auxiliary map. Wrap mode complicates pixel-row → entry mapping; per-row registration sidesteps this entirely. `EmitWithCoord` remains in the registry API for future use.

- **Why click handlers (`InspectorSelectRow`, `InspectorToggleNode`) live in `handler/devtools/inspector.rs` rather than as new variants of `InspectorNav`.** `InspectorNav::Up/Down/Expand/Collapse` is a relative-direction enum; introducing absolute-index variants (`Goto(i)`, `ToggleAt(i)`) would shoehorn unrelated semantics. Click is fundamentally absolute (the user pointed at a specific row), so a separate `Message` variant routed to a sibling handler is the cleaner factoring. The new handlers share the layout-fetch debounce + cache logic with `handle_inspector_navigate` via a small private helper extracted in Task 04.

- **Why `DevTools` mode dispatch lives in `handler/mouse/devtools.rs` rather than the existing `handle_scroll` module.** Phase 2 created `handler/mouse/devtools.rs` for scroll dispatch only; Phase 4 adds a peer `handle_press` function alongside the existing `handle_scroll`. This keeps `mouse/mod.rs` symmetric with the keyboard handler structure (per-mode submodule, multiple entry points per mode).

- **Why double-click detection lives in the `update()` chain rather than in `handle_press`.** `handle_press` receives `&AppState`; recording `last_log_click` requires `&mut AppState`. The cleanest factoring is: `handle_press` always returns `ClickLogRow`; the `update()` arm for `ClickLogRow` mutates `state.last_log_click`, compares against the previous value, and emits `ToggleStackTraceForEntry` as a follow-up `UpdateResult::message`. This avoids a `Cell<MouseClickState>` exception, reuses the existing chained-message mechanism (`UpdateResult::message`), and matches how `update()` already chains messages elsewhere (e.g., performance allocation profile fetch).

- **Why no scroll on single-click.** The PLAN.md interaction map says "left-click → focus the entry," but in v1 we deliberately make single-click visually inert: it only updates `last_log_click` for double-click detection. Adjusting `LogViewState::offset` to scroll the clicked entry into focus would be jarring (the click target moves under the cursor), and adding a separate visible focus indicator is a UI design call we want to validate before implementing. Future enhancement: a `selected_entry_id: Option<u64>` field on `LogViewState` that highlights the selected row without scrolling.

- **Why snapshot tests assert *registry contents*, not rendered pixel diffs.** The Phase 3 precedent (`view_renders_expected_*_regions` in `render/tests.rs`) tests the registry's `entries.len()` plus per-entry rect/action shapes. This catches drift in the rect math when widget copy changes (`[r] Run` → `[r] Reload`) without coupling to terminal rendering quirks. Phase 4 follows the same pattern.

- **Modal precedence is not exercised in Phase 4.** Every region this phase records uses the default `z_index = 0`. Phase 5 introduces overlay regions at `z_index = 1`. The hit-test machinery (highest-z wins) is already in place from Phase 3; Phase 4 simply doesn't rely on it.

- **Glyph vs row hit-test resolution.** Inspector tree rows register *two* regions per row: the wide row region first, then the narrow glyph region. The registry's `last_pushed_wins_at_same_z` invariant (verified by Phase 3 unit test) makes the glyph region take precedence on the glyph cell. No `z_index` bump is needed — same-z, last-pushed wins. This is the same pattern Phase 5 will use for inline modal buttons (e.g., NewSessionDialog Launch button on top of a wide row click region).

- **Filter-input gating in Network panel.** When `network.filter_input_active` is true, mouse clicks in the table area should not select rows (the user is typing). Task 05's dispatcher consults `filter_input_active` before consulting the registry — mirroring the pattern in `handler/mouse/devtools::handle_network_scroll`. Detail-tab clicks are similarly suppressed.

- **`DevToolsViewState` is read but not written in this phase.** Click messages route to the existing handler functions (`handle_select_performance_frame`, `handle_network_select_request`, etc.) which already mutate `DevToolsViewState`. Phase 4 only adds new entry points; no new state fields on `DevToolsViewState`.

- **Right-click reserved.** Phase 3 deferred right-click context menus indefinitely. Phase 4 maintains that — `mouse/devtools.rs::handle_press` returns `None` for `MouseButton::Right` for symmetry with the Normal-mode handler.

- **Dead-code freedom.** Wave 2 task 05 (`handler/mouse/devtools.rs`) does not depend on Task 01's new messages and could in principle land first. We schedule it in Wave 2 anyway so its tests can reference the new `ClickLogRow` / `DevToolsInspectorSelectRow` variants if useful — keeping the dependency graph linear is worth the small parallelism loss.

- **Manual smoke test deferred to Task 10.** Per-task manual smoke tests live in each task's "Acceptance Criteria"; the cross-cutting smoke test (mouse-only walk-through of Inspector → Performance → Network) lands in Task 10's completion summary as a single end-to-end check. This mirrors the Phase 3 / 3.5 pattern.
