# Phase 3 — Rebuild Stats + Timeline Events Populated — Task Index

## Overview

Phase 3 turns the two Phase-2 stub tabs (Rebuild Stats, Timeline Events) into functional DevTools panels. Two independent VM Service flows are added:

- **Rebuild Stats** — subscribe to `Flutter.RebuiltWidgets` Extension events, fetch the widget location map via `ext.flutter.inspector.widgetLocationIdMap`, aggregate per-frame rebuild counts, render a sortable widget-rebuild table. Gated behind the `ext.flutter.profileWidgetBuilds` toggle. Tab is **hidden** when the extension is disabled.
- **Timeline Events** — periodically call `getVMTimeline` (1 Hz when Performance panel is the active DevTools panel), filter to UI / Raster threads, render a vertical scrollable list with thread badges + per-frame correlation. `f` cycles filter through `[All] [UI] [Raster]`.

The Phase-2 anchors are already in place: `PerfDetailsTab::{FrameAnalysis, RebuildStats, TimelineEvents}` enum (`crates/fdemon-app/src/state.rs:185–193`), `details_pane_visible_height: Cell<usize>` render-hint on `PerformanceState` reserved for Phase 3 scrolling, stub tab files at `crates/fdemon-tui/src/widgets/devtools/performance/details/{rebuild_stats_tab.rs, timeline_events_tab.rs}`, and handler directory `crates/fdemon-app/src/handler/devtools/performance/{mod.rs, frame.rs, details.rs}` ready for two new sub-modules.

> **Important correction from PLAN.md** — the upstream DevTools event is `Flutter.RebuiltWidgets` (NOT `Flutter.Rebuilt`). The event payload is `{ "startTime", "frameNumber", "events": [id, count, id, count, …], "locations": { "<file_uri>": { "ids", "lines", "columns", "names" } } }` — parallel arrays per file URI, not a flat id→object map. Task 01 + Task 02 use the verbatim DevTools shape.

**Total Tasks:** 7
**Estimated Hours:** 22–32 hours

## Task Dependency Graph

```
                        ┌──────────────────────────────────────┐
Wave 1                  │ 01 add-phase-3-core-types            │
                        │   (fdemon-core: rebuild_stats,       │
                        │    timeline modules + parsers)       │
                        └────────────────┬─────────────────────┘
                                         │
                       ┌─────────────────┴───────────────────┐
                       ▼                                     ▼
        ┌──────────────────────────────────┐ ┌──────────────────────────────────┐
Wave 2  │ 02 rebuild-stats-vm-service      │ │ 03 timeline-events-vm-service    │
        │   (fdemon-daemon: inspector RPC, │ │   (fdemon-daemon: getVMTimeline, │
        │    profileWidgetBuilds toggle)   │ │    getVMTimelineMicros, fetch)   │
        └────────────────┬─────────────────┘ └────────────────┬─────────────────┘
                         │                                    │
                         └──────────────────┬─────────────────┘
                                            ▼
                        ┌──────────────────────────────────────┐
Wave 3                  │ 04 app-state-handlers-and-spawn      │
                        │   (fdemon-app: PerformanceState      │
                        │    fields, Message variants, two new │
                        │    handler modules, timeline poll    │
                        │    spawn, SessionHandle wiring,      │
                        │    config additions, hot-restart re- │
                        │    enable)                           │
                        └────────────────┬─────────────────────┘
                                         ▼
                        ┌──────────────────────────────────────┐
Wave 4                  │ 05 tab-uis-and-dispatch-widening     │
                        │   (fdemon-tui: widen details/mod.rs  │
                        │    dispatch to pass &PerformanceState│
                        │    populate rebuild_stats_tab.rs +   │
                        │    timeline_events_tab.rs, tab       │
                        │    visibility gate)                  │
                        └────────────────┬─────────────────────┘
                                         │
                       ┌─────────────────┴───────────────────┐
                       ▼                                     ▼
        ┌──────────────────────────────────┐ ┌──────────────────────────────────┐
Wave 5  │ 06 keybindings-and-footer        │ │ 07 update-architecture-doc       │
        │   (keys.rs `f` + rebuild toggle, │ │   (docs/ARCHITECTURE.md,         │
        │    KEYBINDINGS.md, footer hints) │ │    doc_maintainer)                │
        └──────────────────────────────────┘ └──────────────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Est. Hours | Agent | Wave |
|---|------|--------|------------|------------|-------|------|
| 01 | [add-phase-3-core-types](tasks/01-add-phase-3-core-types.md) | Done | — | 3–4h | implementor | 1 |
| 02 | [rebuild-stats-vm-service](tasks/02-rebuild-stats-vm-service.md) | Done | 01 | 3–4h | implementor | 2 |
| 03 | [timeline-events-vm-service](tasks/03-timeline-events-vm-service.md) | Done | 01 | 3–4h | implementor | 2 |
| 04 | [app-state-handlers-and-spawn](tasks/04-app-state-handlers-and-spawn.md) | Done (CONCERN) | 01, 02, 03 | 6–8h | implementor | 3 |
| 05 | [tab-uis-and-dispatch-widening](tasks/05-tab-uis-and-dispatch-widening.md) | Done | 04 | 4–6h | implementor | 4 |
| 06 | [keybindings-and-footer](tasks/06-keybindings-and-footer.md) | Done (CONCERN) | 04, 05 | 1–2h | implementor | 5 |
| 07 | [update-architecture-doc](tasks/07-update-architecture-doc.md) | Done (CONCERN) | 04, 05 | 2–4h | doc_maintainer | 5 |

## File Overlap Analysis

> The orchestrator uses this section to decide isolation strategy per wave. Read-only overlap is fine — only **write-file** overlap forces sequential execution.

| Task | Files Modified (Write) | Files Read (Dependencies) |
|------|------------------------|---------------------------|
| **01** add-phase-3-core-types | `crates/fdemon-core/src/rebuild_stats.rs` (NEW), `crates/fdemon-core/src/timeline.rs` (NEW), `crates/fdemon-core/src/lib.rs` (add `pub mod rebuild_stats; pub mod timeline;`) | `crates/fdemon-core/src/performance.rs` (FrameTiming reference for frame_number type) |
| **02** rebuild-stats-vm-service | `crates/fdemon-daemon/src/vm_service/extensions/inspector.rs` (add `widget_location_id_map`), `crates/fdemon-daemon/src/vm_service/extensions/mod.rs` (add `PROFILE_WIDGET_BUILDS`, `WIDGET_LOCATION_ID_MAP` constants + `pub mod performance;`), `crates/fdemon-daemon/src/vm_service/extensions/performance.rs` (NEW — `set_profile_widget_builds` wrapper using existing `toggle_bool_extension`) | T01 outputs (`LocationMap`, `RebuildEventPayload`), `crates/fdemon-daemon/src/vm_service/extensions/overlays.rs` (reference pattern for `toggle_bool_extension`), `crates/fdemon-daemon/src/vm_service/client.rs` (`call_extension` signature) |
| **03** timeline-events-vm-service | `crates/fdemon-daemon/src/vm_service/timeline.rs` (add `fetch_timeline_chunk`, `get_vm_timeline_micros`, parse helpers) | T01 outputs (`TimelineEvent`, `TimelineThread`), `crates/fdemon-daemon/src/vm_service/client.rs` (raw `request` API) |
| **04** app-state-handlers-and-spawn | `crates/fdemon-app/src/session/performance.rs` (add 6 fields for rebuild stats + 4 for timeline + 1 enum `TimelineFilter`), `crates/fdemon-app/src/session/handle.rs` (add `timeline_shutdown_tx`, `timeline_pause_tx`, `timeline_task_handle`), `crates/fdemon-app/src/message.rs` (add 7 variants), `crates/fdemon-app/src/handler/update.rs` (add 7 dispatch arms), `crates/fdemon-app/src/handler/devtools/performance/mod.rs` (declare 2 new sub-modules), `crates/fdemon-app/src/handler/devtools/performance/rebuild_stats.rs` (NEW), `crates/fdemon-app/src/handler/devtools/performance/timeline.rs` (NEW), `crates/fdemon-app/src/actions/performance.rs` (`spawn_timeline_polling` helper + threading the new pause/shutdown txs through `spawn_performance_polling`'s return shape), `crates/fdemon-app/src/actions/vm_service.rs` (new `Flutter.RebuiltWidgets` branch in `forward_vm_events`), `crates/fdemon-app/src/session/session_lifecycle.rs` (start/stop timeline poll alongside perf poll, re-enable `profileWidgetBuilds` on `SessionRestartCompleted` if previously on), `crates/fdemon-app/src/config/types.rs` (add `auto_enable_rebuild_tracking: bool`, `rebuild_stats_frame_window: u32`, `timeline_event_buffer_size: usize` to `DevToolsSettings`) | T01–T03 outputs, `crates/fdemon-app/src/session/memory.rs` (reference pattern for ring-buffer state fields) |
| **05** tab-uis-and-dispatch-widening | `crates/fdemon-tui/src/widgets/devtools/performance/details/mod.rs` (widen dispatch signatures to pass `&PerformanceState`, conditional `RebuildStats` tab visibility when `rebuild_stats_enabled == false`, tab-strip render adjustment), `crates/fdemon-tui/src/widgets/devtools/performance/details/rebuild_stats_tab.rs` (replace stub: header `"Rebuild tracking: ON/OFF"`, sortable widget table `(file:line  Name  Count)`, scroll/selection driven by `rebuild_stats_scroll_offset`/`rebuild_stats_selected_row`, render-hint write-back `details_pane_visible_height.set(area.height as usize)`, no-data placeholder), `crates/fdemon-tui/src/widgets/devtools/performance/details/timeline_events_tab.rs` (replace stub: filter strip `[All] [UI] [Raster]` driven by `timeline_events_filter`, scrollable event list `(thread-color badge, name, dur, ts-relative)`, render-hint write-back, empty-state placeholder), `crates/fdemon-tui/src/widgets/devtools/performance/details/tests.rs` (NEW or extended — table fixtures for both tabs) | T04 outputs, `crates/fdemon-tui/src/widgets/devtools/performance/details/frame_analysis_tab.rs` (reference pattern for state-aware tab) |
| **06** keybindings-and-footer | `crates/fdemon-app/src/handler/keys.rs` (when `in_performance && focused_section == Details && details_tab == TimelineEvents`: route `f` → `Message::TimelineEventsCycleFilter`; when `in_performance && focused_section == Details && details_tab == RebuildStats`: route `R` → `Message::ToggleRebuildStats`; verify existing `j/k`/`PageUp/Down`/`Home/End` still route correctly to details-scroll messages added by T04), `crates/fdemon-tui/src/widgets/devtools/mod.rs` (Performance footer hint string — add `[f] Filter` when on TimelineEvents and `[R] Rebuild track` when on RebuildStats; only when `focused_section == Details`), `docs/KEYBINDINGS.md` (document `f` filter cycle and `R` rebuild toggle under Performance section) | T04 outputs (Message variant names), T05 outputs (footer rendering contract) |
| **07** update-architecture-doc | `docs/ARCHITECTURE.md` ("Performance Panel Interactivity" section — document: `PerformanceState` new fields (rebuild stats + timeline events + filter enum), 2 new sub-modules in `handler/devtools/performance/`, new `Flutter.RebuiltWidgets` event branch in `forward_vm_events`, new `spawn_timeline_polling` action, `SessionHandle` timeline shutdown/pause fields, hot-restart re-enable of `profileWidgetBuilds`, new VM Service constants `PROFILE_WIDGET_BUILDS` + `WIDGET_LOCATION_ID_MAP`, conditional `RebuildStats` tab visibility) | T04, T05, T06 task specs + completion summaries |

### Overlap Matrix (write-files only)

| Pair | Shared Write Files | Wave | Strategy |
|------|--------------------|------|----------|
| 02 + 03 | **None** | 2 | **Parallel (worktree)** — T02 writes `extensions/{inspector, mod, performance}.rs`; T03 writes `timeline.rs`. Distinct files within `vm_service/`. Both depend on T01 (which writes `fdemon-core`, no overlap with daemon). |
| 01 + 02 | None | — | **Sequential by dependency** — T02 uses `LocationMap` from T01. T01 writes `fdemon-core`, T02 writes `fdemon-daemon`. No write overlap. |
| 01 + 03 | None | — | **Sequential by dependency** — T03 uses `TimelineEvent` from T01. Same crate-disjoint pattern. |
| 02/03 + 04 | None | — | **Sequential by dependency** — T04 uses T02/T03 APIs. T04 writes `fdemon-app`, T02/T03 write `fdemon-daemon`. No write overlap. |
| 04 + 05 | None | — | **Sequential by dependency** — T05 reads `PerformanceState` fields added by T04. T04 writes `fdemon-app`, T05 writes `fdemon-tui`. No write overlap. |
| 05 + 06 | None | — | **Sequential by dependency** — T06 reads the new Message variants from T04 and footer-render contract from T05. T06 writes `keys.rs` + `widgets/devtools/mod.rs` + `docs/KEYBINDINGS.md`. T05 writes within `widgets/devtools/performance/details/`. No write overlap. |
| 05 + 07 | None | — | T07 writes `docs/ARCHITECTURE.md` only. |
| 06 + 07 | **None** | 5 | **Parallel (worktree)** — T06 writes `keys.rs` + `widgets/devtools/mod.rs` + `docs/KEYBINDINGS.md`; T07 writes `docs/ARCHITECTURE.md`. Disjoint. |

## Success Criteria

Phase 3 is complete when:

- [ ] Full quality gate green: `cargo fmt --all -- --check && cargo check --workspace --all-targets && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings`.
- [ ] Running fdemon against a Flutter app, opening DevTools → Performance → `]` to **Rebuild Stats**: tab is **hidden** when `profileWidgetBuilds` is OFF. Pressing `R` enables it; tab appears within ~1s with per-frame widget rebuild counts. Disabling clears stats and re-hides the tab.
- [ ] Opening DevTools → Performance → `]` twice to **Timeline Events**: list populates within ~1s with UI + Raster events from the most recent ~1s window. `f` cycles `[All] [UI] [Raster]` filter; list narrows accordingly.
- [ ] Switching away from the Performance panel (`i`/`m`/`n` or `Esc`) **stops** the 1-Hz timeline polling — verify via tracing log `timeline poll paused`. Switching back **resumes** within one tick.
- [ ] Closing the session (`Esc` from Logs / SIGTERM / hot-restart-with-app-stop) sends shutdown signals through `timeline_shutdown_tx` and the spawned task exits within 100ms.
- [ ] Hot restart preserves the `profileWidgetBuilds` toggle state: if enabled before restart, fdemon re-enables it on `SessionRestartCompleted` and the Rebuild Stats tab stays visible.
- [ ] `Flutter.RebuiltWidgets` events arriving before DevTools opens are buffered or replayed correctly — the first time the user opens Performance, the most recent ~30 frames of rebuild data is visible (subject to event-stream history per `onExtensionEventWithHistorySafe` analogue; if unavailable, an empty state with `"Waiting for first frame…"` is shown).
- [ ] Timeline event buffer is capped at `timeline_event_buffer_size` (default 1000); when full, oldest events are dropped. Switching away from Performance clears the buffer (per PLAN.md §7.5 mitigation).
- [ ] Rebuild stats buffer is capped at `rebuild_stats_frame_window` (default 30 frames); oldest frame snapshots are dropped.
- [ ] Tests added:
  - `fdemon-core/rebuild_stats`: parses verbatim DevTools `Flutter.RebuiltWidgets` payload fixture, merges LocationMap deltas across events, handles missing `locations` key in subsequent events, evicts oldest frame when over window.
  - `fdemon-core/timeline`: parses Chrome-trace JSON fixture from `getVMTimeline` response, classifies thread by track name (`UI`/`Raster`/Other), handles `ph=B`/`ph=E` paired with `ph=X` complete events.
  - `fdemon-daemon/extensions/performance`: `set_profile_widget_builds` round-trips via `toggle_bool_extension`.
  - `fdemon-daemon/extensions/inspector`: `widget_location_id_map` parses LocationMap response from `ext.flutter.inspector.widgetLocationIdMap`.
  - `fdemon-daemon/timeline`: `fetch_timeline_chunk` issues `getVMTimeline` with correct `timeOriginMicros`/`timeExtentMicros`, parses response into `Vec<TimelineEvent>`.
  - `fdemon-app/handler/devtools/performance/rebuild_stats`: aggregates per-frame events, drops oldest snapshot at frame_window boundary, clears on extension disable, restores on hot restart.
  - `fdemon-app/handler/devtools/performance/timeline`: ring-buffer append + filter-cycle + scroll-clamp.
  - `fdemon-app/actions/performance`: `spawn_timeline_polling` honors pause/shutdown signals.
  - `fdemon-tui/widgets/.../details/rebuild_stats_tab`: renders table, hides tab when extension off, surfaces "Rebuild tracking: OFF — press R to enable" when disabled but tab forced visible during transition.
  - `fdemon-tui/widgets/.../details/timeline_events_tab`: renders list with filter, thread badges, empty state.
- [ ] `docs/KEYBINDINGS.md` documents `f` (TimelineEvents filter cycle) and `R` (rebuild-tracking toggle).
- [ ] `docs/ARCHITECTURE.md` "Performance Panel Interactivity" section is updated for all Phase 3 additions.

## Phase Acceptance Test Plan

After all 7 tasks merge, run the manual smoke sequence:

1. `cargo run -- ~/Dev/some-flutter-app` in a 200×30 iTerm split. Wait for app to attach.
2. Press `d` → DevTools, `p` → Performance. Press `]` once to focus the Details pane.
3. **Hidden-by-default check.** Press `]` again. Verify it cycles `FrameAnalysis → TimelineEvents → FrameAnalysis` (RebuildStats is skipped because the extension is OFF by default; per `auto_enable_rebuild_tracking = false`).
4. **Toggle on.** Press `R`. Verify the Rebuild Stats tab appears and is auto-selected; header shows `"Rebuild tracking: ON"`. Within ~1s, table populates with widget locations and per-frame counts.
5. Trigger rebuilds in the app (interact with UI). Verify counts increment.
6. **Toggle off.** Press `R` again. Verify the tab disappears within ~200ms; cycling with `]` no longer lands on RebuildStats.
7. Press `]` to **Timeline Events**. Within ~1s, list populates. Press `f` → filter cycles to `[UI]`; list narrows to UI-thread events. Press `f` → `[Raster]`. Press `f` → back to `[All]`.
8. **Hot-restart preservation.** Toggle Rebuild Stats ON via `R`. Trigger hot restart (`R` from Logs panel — note the global `R` is hot-restart). Switch back to Performance → RebuildStats. Verify the tab is still visible (toggle was re-enabled on `SessionRestartCompleted`).
9. **Polling stop.** Switch to Logs (`Esc`). Tail the fdemon log file — verify `timeline poll paused` appears within one tick.
10. **Session stop.** Press `Q` to quit. Verify the process exits cleanly (no orphaned timeline-poll task).

## Keyboard Shortcuts Added in Phase 3

| Key | Context | Action |
|-----|---------|--------|
| `f` | Performance, Details focused, `details_tab == TimelineEvents` | Cycle filter `All → UI → Raster → All` |
| `R` | Performance, Details focused, `details_tab == RebuildStats` | Toggle `profileWidgetBuilds` ON/OFF (drives tab visibility) |
| `j`/`k`/`↑`/`↓` | Performance, Details focused, `details_tab == RebuildStats` or `TimelineEvents` | Scroll table/list (Phase-2 generic Details-scroll routing extended to consume the new ring buffers) |

> The choice of `R` (Shift+r) follows the project convention of capital letters for destructive/state-changing toggles. Verify no clash with the global `r` (reload) / `R` (hot restart) bindings — in DevTools mode, top-level reload bindings are still routed to the daemon, so `R` must be intercepted ONLY when `in_performance && focused_section == Details && details_tab == RebuildStats`. T06's acceptance criteria includes a regression test asserting `R` outside that exact context still routes to hot-restart.

## Notes

- **Event name correction:** PLAN.md uses `Flutter.Rebuilt` throughout; the upstream Dart source uses `Flutter.RebuiltWidgets` (constant `FlutterEvent.rebuiltWidgets` in `service_extensions.dart`). All Phase 3 tasks use `Flutter.RebuiltWidgets`.
- **LocationMap shape:** Parallel arrays per file URI, NOT a flat id→object map. T01 captures the verbatim shape `{ "<file_uri>": { "ids": [...], "lines": [...], "columns": [...], "names": [...] } }`. T02's `widget_location_id_map()` returns the same shape, used as a one-shot fallback if DevTools missed the location data in earlier `Flutter.RebuiltWidgets` events.
- **`getVMTimeline` vs `getPerfettoVMTimeline`:** Upstream DevTools migrated to the Perfetto protobuf binary format. fdemon stays on the legacy Chrome-trace JSON via `getVMTimeline` to avoid pulling in a protobuf decoder. The JSON form is still supported by the VM Service. The trade-off is captured in PLAN.md §7.5 (timeline event volume mitigation).
- **Polling cadence:** PLAN.md §5.4 says 1 Hz; upstream DevTools uses 10s + rate limit. We keep 1 Hz because fdemon's TUI updates are cheap and the smaller poll window keeps memory bounded. The poll is paused when the Performance panel is not the active DevTools panel (per PLAN.md §7.5).
- **Phase-3 stub-tab signature widening:** Phase-2 stubs declared `pub(super) fn render(area: Rect, buf: &mut Buffer)` — no state argument. T05 widens both signatures to `pub(super) fn render(area: Rect, buf: &mut Buffer, state: &PerformanceState)`. This is a within-crate refactor; the only caller is `details/mod.rs`'s tab dispatch.
- **Tab visibility behavior:** Per PLAN.md §5.2 "Conditional visibility": Rebuild Stats tab is hidden when `rebuild_stats_enabled == false`. If the user is currently on the RebuildStats tab and the extension is disabled, the tab snaps to the next visible tab (`TimelineEvents`). T05 owns this transition; T04 sets `rebuild_stats_enabled` and emits a follow-up `Message::PerfFocusDetailsTab(TimelineEvents)` when disabling.
- **Frame-to-events correlation (Timeline):** Upstream DevTools correlates by `flutterFrameNumber` from the event's `args` field. fdemon adopts the same approach — when a `TimelineEvent` carries a frame-number arg, it is associated with that frame's selection state. Events without a frame number remain unassigned but still appear in the unfiltered list.
- **Hot restart re-enable:** PLAN.md §7.6 requires re-enabling `profileWidgetBuilds` on `SessionRestartCompleted` if it was previously ON. T04 implements this in `session_lifecycle.rs`; the previous-state tracking lives on `PerformanceState::rebuild_stats_enabled` which persists across hot restart (state is keyed by session, and hot restart is a same-session event).
- **No new dependencies:** All new RPCs use existing `serde_json` parsing. No `prost` / protobuf needed (we use the JSON `getVMTimeline` form).
- **Config additions are wired in T04** (`crates/fdemon-app/src/config/types.rs`), not a separate task. The new keys are `auto_enable_rebuild_tracking`, `rebuild_stats_frame_window`, `timeline_event_buffer_size`, all under `[devtools]` (NOT a new `[devtools.performance]` block — Phase 3 follows the existing flat `DevToolsSettings` convention).
- **`spawn_performance_polling` return shape:** Phase-2's `Message::VmServicePerformanceMonitoringStarted` already carries `perf_shutdown_tx`, `alloc_pause_tx`, `perf_pause_tx`. T04 extends it (or adds a parallel `Message::VmServiceTimelineMonitoringStarted`) with `timeline_shutdown_tx`, `timeline_pause_tx`, `timeline_task_handle`. The implementor should pick the simpler of: (a) extend the existing message, (b) add a parallel message. The task brief recommends (b) for clearer separation of concerns.
