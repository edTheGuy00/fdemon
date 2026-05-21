# Plan: DevTools Performance / Memory Tab Split + Performance Tab Detail Expansion

**Status:** Draft — awaiting approval
**Author:** Planner
**Owner Crates:** `fdemon-core`, `fdemon-daemon`, `fdemon-app`, `fdemon-tui`

---

## TL;DR

Split the current single Performance tab into two top-level DevTools tabs: **Performance** (frame timing + per-frame analysis) and **Memory** (memory chart + allocation profile). The current 45/55 vertical split is unreadable on short-wide terminals — frame timing disappears below the fold and the allocation table only shows a couple of rows. After the split, each tab gets the full panel height. Then expand the Performance tab to mirror the official DevTools structure: a Flutter Frames bar chart on top with a tabbed details pane below (Frame Analysis / Rebuild Stats / Timeline Events), using the same conditional-tabbed-details pattern as the Inspector parity work.

---

## 1. Problem

The DevTools Performance panel currently bundles two distinct concerns into one vertically-stacked layout (`crates/fdemon-tui/src/widgets/devtools/performance/mod.rs:194–305`):

1. **Frame Timing** (top 45%) — bar chart + detail panel
2. **Memory** (bottom 55%) — time-series chart + allocation class table

In tall terminals this works. In horizontal / short-wide terminals (the most common dock-bottom layout — wide but only ~16–22 rows), the split produces two unreadable panes:

| Symptom | Cause |
|---|---|
| Frame timing **chart is invisible** below the bottom of the screen on layouts with `height < DUAL_SECTION_MIN_HEIGHT (16)` — but ALSO at 17–22 rows where the budget is tight, the frame detail panel and the legend get pushed off | The 45% allocation for Frame Timing combined with mandatory `Borders::ALL` (-2 rows) and a 1-row footer reserve leaves ≤ 6 inner rows at 16 rows; below 16 rows the panel silently drops the memory section entirely (`DUAL_SECTION_MIN_HEIGHT` short-circuit) |
| Memory **class allocation table only shows 2-3 rows** | Memory section's inner area is split *again* into chart-on-top + table-on-bottom in `memory_chart/mod.rs`, leaving the table with whatever rows remain after the chart consumed its share |

Beyond the layout bug, the fdemon Performance panel is feature-thin compared to official DevTools (`tmp/devtools/packages/devtools_app/lib/src/screens/performance/tabbed_performance_view.dart`). DevTools provides three tabs below the frames bar:

- **Frame Analysis** — phase breakdown (build / layout / paint / raster) for the selected frame, jank hints, refresh-rate-aware diagnostics
- **Rebuild Stats** — `countWidgetBuilds` data showing per-location rebuild counts per frame
- **Timeline Events** — UI/Raster thread timeline events for the selected frame

fdemon currently has the data plumbing for *some* of this (FrameTiming already carries optional `FramePhases { build, layout, paint, raster, shader_compilation }` — see `crates/fdemon-core/src/performance.rs:208–234`) but the UI only renders a single small detail box.

## 2. Goals

1. **Split memory into its own DevTools tab.** Add `DevToolsPanel::Memory` between Performance and Network in the sub-tab bar. The memory chart, allocation profile table, allocation sort toggle, GC markers, and rich memory samples move there. Performance tab no longer has a Memory section.
2. **Performance tab uses the Inspector parity layout pattern**: a primary always-visible chart on top + a tabbed details pane below. Top = Flutter Frames bar chart (existing `FrameChart`, full inner height). Bottom = tabbed details with **Frame Analysis** (always), **Rebuild Stats** (Flutter app + extension toggle), **Timeline Events** (always).
3. **Frame Analysis tab is functional in Phase 1** using existing `FramePhases` data — show phase percentage bars, total UI vs raster split, shader-compilation indicator, jank-budget hints, refresh-rate-aware diagnostics. No new VM Service calls required.
4. **Rebuild Stats and Timeline Events tabs are populated in Phase 3** behind new VM Service extension calls (`ext.flutter.inspector.profileWidgetBuilds` + `Flutter.Rebuilt` event stream; `getVMTimeline` + filter to UI/Raster thread tracks).
5. **Responsive layout** — Performance details pane uses the same `area.height < MIN_*` decision pattern documented in `docs/CODE_STANDARDS.md` Principle 1. In very short terminals (height < `MIN_DETAILS_HEIGHT`), the details pane collapses to a single-line summary of the active tab; the chart always wins.

## 3. Non-Goals

- We are NOT replicating DevTools' Perfetto-based timeline view (that's a full WebView). Timeline Events tab will show a flat scrollable list of `(event_name, thread, duration)` entries for the selected frame.
- We are NOT adding the "Enhance Tracing" controls (track-image-sizes, track-platform-channels, etc.). These are out of scope; the Frame Analysis tab will note when extended tracing is missing.
- We are NOT introducing a "Performance Settings" overlay. The existing `Ctrl+p` overlay toggle stays as-is.
- We are NOT changing the existing 30-second / 1800-sample frame history budget (`DEFAULT_FRAME_HISTORY_SIZE = 1800`).

## 4. Background Research

### 4.1 Current fdemon layout (relevant files)

| Concern | Path | Notes |
|---|---|---|
| Performance panel render | `crates/fdemon-tui/src/widgets/devtools/performance/mod.rs` | 396 lines. `render_impl` decides between `compact_summary` (< 7 rows), frame-chart-only (< 16 rows), and dual section (≥ 16 rows). Will lose its Memory branch and shrink. |
| Frame chart widget | `crates/fdemon-tui/src/widgets/devtools/performance/frame_chart/` | `bars.rs`, `detail.rs`, `mod.rs`, `tests.rs`. Stays in Performance; the existing per-frame `detail.rs` becomes one source for the new Frame Analysis tab. |
| Memory chart widget | `crates/fdemon-tui/src/widgets/devtools/performance/memory_chart/` | `chart.rs`, `table.rs`, `braille_canvas.rs`, `mod.rs`, `tests.rs`. **Moves wholesale to a new `widgets/devtools/memory/` directory.** |
| Performance state | `crates/fdemon-app/src/session/performance.rs` | Holds frame_history, memory_history, gc_history, memory_samples, allocation_profile, etc. Currently a single struct mixing both concerns; will be split. |
| Performance handler | `crates/fdemon-app/src/handler/devtools/performance.rs` | 1907 lines. Will be split: `performance.rs` keeps frame-selection / scroll / Tab cycling for FrameChart + FrameAnalysis sections; new `memory.rs` handles allocation-sort, alloc-row selection, memory-chart scroll. |
| `PerfSection` enum | `crates/fdemon-app/src/session/performance.rs:37–65` | Currently `FrameChart \| MemoryChart \| MemoryList`. Will be split into `PerfSection { FrameChart, DetailsTab }` for Performance, and a new `MemorySection { Chart, AllocationList }` for Memory. |
| DevTools view dispatcher | `crates/fdemon-tui/src/widgets/devtools/mod.rs:120–170` | Match on `DevToolsPanel`. Will get a new `Memory` arm and the existing `Performance` arm shrinks. |
| Key bindings | `crates/fdemon-app/src/handler/keys.rs:486–523, 559–565` | Performance-specific bindings (Tab cycle, j/k, PageUp/Down, Home/End) are routed via `in_performance` guard. Will be re-evaluated for both `in_performance` AND a new `in_memory` flag. Letter shortcuts: `i`/`p`/`n` → new `m` for Memory. |
| Footer hints | `crates/fdemon-tui/src/widgets/devtools/mod.rs:347–367` | Adds a `Memory` arm; Performance arm updates for Tab-cycle between details tabs. |

### 4.2 DevTools reference (from `tmp/devtools/packages/devtools_app/lib/src/screens/performance/`)

| Concern | Location | Notes |
|---|---|---|
| Tab strip layout | `tabbed_performance_view.dart:78–164` | Three tabs: Frame Analysis (offline-aware), Rebuild Stats (Flutter-only, extension-gated), Timeline Events (always). Tab visibility is conditional. |
| Frame Analysis panes structure | `panes/frame_analysis/frame_analysis.dart` + `frame_analysis_model.dart` | `FrameAnalysis` model derives `buildPhase`, `layoutPhase`, `paintPhase`, `rasterPhase` from timeline events. We don't need timeline events for the basic phase breakdown — Flutter's `Flutter.Frame` extension event already provides `vsyncOverhead`, `build`, `raster` aggregates, and our existing `FramePhases` covers the split. |
| Frame Hints | `panes/frame_analysis/frame_hints.dart` | "Build was the longest phase…", "Raster was the longest phase…", "Shader compilation jank detected…", refresh-rate-aware budget warnings. All derivable from existing `FramePhases` + a `display_refresh_rate` field. |
| Frame Time Visualizer | `panes/frame_analysis/frame_time_visualizer.dart` | Proportional flex-based bar showing phase durations. We render an ASCII proportional bar (4 horizontal segments) keyed to phase percentages. |
| Rebuild Stats | `panes/rebuild_stats/rebuild_stats_controller.dart` | Subscribes to `Flutter.Rebuilt` Extension events; aggregates by `(file, line, column)` via `LocationMap` returned from `ext.flutter.inspector.widgetLocationMap`. Behind the `ext.flutter.profileWidgetBuilds` toggle. |
| Timeline Events | `panes/timeline_events/timeline_events_controller.dart` | Calls `getVMTimeline` periodically, filters by thread (`UI`, `Raster`), maintains rolling buffer. Each event has `(name, ph, ts, dur, tid, args)`. |

### 4.3 Current data already available in fdemon-core

- `FramePhases { build_micros, layout_micros, paint_micros, raster_micros, shader_compilation }` exists (`crates/fdemon-core/src/performance.rs:208`).
- `FrameTiming.phases: Option<FramePhases>` is populated when the timeline data is available (`timeline.rs`). When `None`, only the aggregated `build_micros + raster_micros` split is known.
- `PerformanceStats { fps, jank_count, avg_frame_ms, p95_frame_ms, p99_frame_ms }` already aggregates rolling-window stats (`performance.rs:289–325`).

This means **Phase 2 Frame Analysis tab is data-complete with no new VM Service work**.

### 4.4 New VM Service calls needed for Phase 3 (Rebuild Stats + Timeline)

| Call | Purpose | Source in DevTools |
|---|---|---|
| `ext.flutter.profileWidgetBuilds` (toggle) + `ext.flutter.profileUserWidgetBuilds` | Enable rebuild counting. Returns Extension events `Flutter.Rebuilt`. | `service_extensions.dart` |
| `ext.flutter.inspector.widgetLocationIdMap` | Returns `LocationMap` — id → (file, line, column, name). | `rebuild_stats_controller.dart` |
| `getVMTimeline` (VM RPC, not flutter extension) | Returns timeline events since last call; we filter to UI + Raster threads. | `timeline_events_controller.dart` |
| `getVMTimelineFlags` / `setVMTimelineFlags` | Ensure `Dart` + `Embedder` streams are enabled. We already call `setVMTimelineFlags(["Dart"])` indirectly via `profileWidgetBuilds`. | `timeline.rs:147` already calls `profileWidgetBuilds`. |

`fdemon-daemon/src/vm_service/timeline.rs` already enables `profileWidgetBuilds`. Phase 3 adds the event-subscription handling and the location-map fetch.

### 4.5 Constraints from CODE_STANDARDS / ARCHITECTURE

- Layer rules: data parsing in `fdemon-core`, RPC in `fdemon-daemon`, state + handlers in `fdemon-app`, render in `fdemon-tui`.
- Files > 500 lines should be split (`docs/CODE_STANDARDS.md`). The existing `handler/devtools/performance.rs` is already 1907 lines — splitting it into per-tab sub-modules under `handler/devtools/performance/` and a new `handler/devtools/memory.rs` is overdue and aligns with the existing pattern (cf. `handler/devtools/inspector.rs` + `inspector/` sub-tree).
- Layout thresholds: every numeric layout threshold gets a named constant with derivation comment (`docs/CODE_STANDARDS.md` Principle 4).
- TEA: render-hint `Cell<usize>` fields move with their respective widgets (e.g. `alloc_table_visible_height` moves to `MemoryState`).

## 5. High-Level Solution

### 5.1 Tab split (Phase 1)

Three structural changes happen in one wave:

**A. New `DevToolsPanel::Memory` enum variant.**

```rust
// crates/fdemon-app/src/state.rs
pub enum DevToolsPanel {
    Inspector,
    Performance,
    Memory,    // NEW
    Network,
}
```

Tab bar order in `widgets/devtools/mod.rs` becomes `[Inspector, Performance, Memory, Network]`. Letter shortcuts: `i`, `p`, `m`, `n`. The `m` key is currently unused in DevTools mode (verified by reading `keys.rs:559–565`).

**B. `PerformanceState` split into `PerformanceState` + new `MemoryState`.**

Today's monolithic `PerformanceState` carries both frame-timing and memory fields. Split:

```rust
// crates/fdemon-app/src/session/performance.rs (slim)
pub struct PerformanceState {
    pub frame_history: RingBuffer<FrameTiming>,
    pub stats: PerformanceStats,
    pub monitoring_active: bool,
    pub selected_frame: Option<usize>,
    pub focused_section: PerfSection,           // FrameChart | DetailsTab
    pub details_tab: PerfDetailsTab,            // FrameAnalysis | RebuildStats | TimelineEvents
    pub frame_chart_scroll_offset: usize,
    pub frame_chart_visible_width: Cell<usize>,
    // Phase 3 additions:
    pub rebuild_stats: Option<RebuildStatsSnapshot>,
    pub timeline_events: Vec<TimelineEvent>,
    pub timeline_events_scroll_offset: usize,
    pub details_pane_visible_height: Cell<usize>,
}

// crates/fdemon-app/src/session/memory.rs (NEW)
pub struct MemoryState {
    pub memory_history: RingBuffer<MemoryUsage>,
    pub gc_history: RingBuffer<GcEvent>,
    pub memory_samples: RingBuffer<MemorySample>,
    pub allocation_profile: Option<AllocationProfile>,
    pub allocation_sort: AllocationSortColumn,
    pub monitoring_active: bool,
    pub focused_section: MemorySection,         // Chart | AllocationList
    pub memory_chart_scroll_offset: usize,
    pub memory_chart_visible_width: Cell<usize>,
    pub alloc_table_selected_row: Option<usize>,
    pub alloc_table_scroll_offset: usize,
    pub alloc_table_visible_height: Cell<usize>,
}
```

`Session` gains a `memory: MemoryState` field next to the slimmed `performance: PerformanceState`. Both keep their own `monitoring_active` flag because frame timing and memory polling are independently controllable (the daemon already polls them independently — see `vm_service/client.rs` polling loop).

**C. New `MemoryPanel` widget under `widgets/devtools/memory/`.**

Move `widgets/devtools/performance/memory_chart/` to `widgets/devtools/memory/` and rename it `MemoryPanel`. The widget is structurally unchanged — it just gets the full panel inner area now, so the allocation table can render 15+ rows instead of 3.

**D. Routing + dispatch.**

- `widgets/devtools/mod.rs` `render_impl` match adds a `Memory` arm calling `memory::render_with_regions`.
- Key handler `handle_key_devtools` adds an `in_memory` guard mirroring `in_performance` for Tab cycling between chart / alloc-list sections, j/k row navigation, alloc-sort `s` toggle.
- `Esc` semantics on Memory tab match Network: if a frame / row is selected, Esc deselects first; otherwise Esc returns to Logs.

### 5.2 Performance details pane (Phase 2)

Phase 2 transforms the now-frame-only Performance tab into the **chart + tabbed details** layout:

```
┌─ Performance ─────────────────────────────────────────────────────┐
│ Flutter Frames (bar chart, scrollable, full width)                │
│                                                                   │
│ ▮▮▮▮▮▮▮▮▮▮▮▮▮▮▮▮▮▮▮▮▮▮▮▮▮▮▮▮▮▮▮▮▮▮▮▮▮▮▮▮▮▮▮▮▮▮▮ FPS 58.7  Jank 2 │
│                                                                   │
├─ [Frame Analysis] [Rebuild Stats] [Timeline Events] ──────────────┤
│ Flutter frame: 142                                                │
│                                                                   │
│ Total: 18.2ms  Budget @ 60Hz: 16.7ms — JANK +1.5ms                │
│ ┌──── Build 6.1ms ─┬─ Layout 2.0ms ─┬─ Paint 3.4ms ─┬─Rast 6.7ms┐ │
│ │ ████████████████ │ █████          │ █████████      │██████████ │ │
│ └──────────────────┴────────────────┴────────────────┴──────────┘ │
│                                                                   │
│ Hints:                                                            │
│  • Raster was the longest phase — investigate GPU work / shaders. │
│  • Shader compilation detected — pre-warm shaders at startup.     │
└───────────────────────────────────────────────────────────────────┘
```

**Layout decision:**

```rust
const MIN_DETAILS_HEIGHT: u16 = 8;     // tab strip(1) + content(>= 6) + spacer(1)
const MIN_DUAL_PANE_HEIGHT: u16 = 18;  // frames chart(>= 10) + details(>= 8)

if area.height < MIN_DUAL_PANE_HEIGHT {
    // Frames chart only — details pane folds into a 1-line status row.
} else {
    // Chart top + details below; chart takes ~55% (10 rows min), details ~45%
}
```

**Tab strip rendering** mirrors the Inspector details tab strip pattern (`widgets/devtools/inspector/details/mod.rs`):
- One line of `[Frame Analysis] [Rebuild Stats] [Timeline Events]`.
- Active tab has reverse-video background.
- Conditional visibility (Phase 3): `Rebuild Stats` hidden when extension is not enabled; `Frame Analysis` always; `Timeline Events` always (it shows a "fetching…" stub when no data is loaded).

**Tab cycling:**
- `Tab` / `Shift+Tab` cycles `focused_section` between `FrameChart` and `DetailsTab` (so the user can scroll either pane).
- When `focused_section == DetailsTab`, `]`/`[` cycle the details tab. Reusing the existing Inspector pattern.
- Decision: `]`/`[` (instead of repurposing Tab) avoids conflict with the frame-bar-vs-detail-pane focus toggle and matches the new shortcut conventions for tabbed views.

**Frame Analysis tab content** (no new VM Service work — uses existing `FramePhases`):

| Element | Source | Rendering |
|---|---|---|
| `Flutter frame: <number>` header | `frame.number` | line 1 |
| Total + budget verdict | `frame.elapsed_ms()` vs `1000.0 / display_refresh_rate` | "Total: 18.2 ms  Budget @ 60 Hz: 16.7 ms — **JANK +1.5 ms**" |
| Proportional phase bar | `FramePhases { build, layout, paint, raster }` | Horizontal split: each phase's width = `(phase / total) * available_cols`. Label inside if wide enough; otherwise above. |
| Hints list | Derived in `fdemon-core` (`frame_hints(frame, refresh_rate) -> Vec<FrameHint>`) | Up to 5 hints, each ≤ 80 chars; e.g. "Build was the longest phase — check widget rebuilds.", "Shader compilation detected — pre-warm shaders." |
| No-data state | `frame.phases.is_none()` | "Phase data not available for this frame. Aggregate: build 4.2 ms, raster 6.7 ms." |
| No-selection state | `performance.selected_frame.is_none()` | "Select a frame above (←/→) to view analysis." |

**Rebuild Stats tab content** (Phase 1 stub, populated in Phase 3):

```
Phase 1 stub:
    [Rebuild stats]
    Coming soon — Phase 3 adds widget rebuild tracking.
    Requires ext.flutter.profileWidgetBuilds to be enabled.
```

**Timeline Events tab content** (Phase 1 stub, populated in Phase 3):

```
Phase 1 stub:
    [Timeline events]
    Coming soon — Phase 3 streams UI/Raster thread timeline events.
```

### 5.3 Rebuild Stats — Phase 3

Adds:

| File | Change |
|---|---|
| `crates/fdemon-core/src/performance.rs` (or new `rebuild_stats.rs` if it gets large) | `LocationMap { id_to_location: HashMap<u32, Location>, … }`, `Location { file, line, column, name }`, `RebuildLocation { location, build_count }`, `RebuildStatsSnapshot { frame_to_rebuilds: HashMap<u64, Vec<RebuildLocation>>, latest_frame_rebuilds: Vec<RebuildLocation> }`. |
| `crates/fdemon-daemon/src/vm_service/extensions/inspector.rs` | New method `widget_location_id_map() -> LocationMap`. Calls `ext.flutter.inspector.widgetLocationIdMap`. Cached per isolate. |
| `crates/fdemon-daemon/src/vm_service/extensions/mod.rs` | New constants `PROFILE_USER_WIDGET_BUILDS = "ext.flutter.profileUserWidgetBuilds"`, `WIDGET_LOCATION_ID_MAP = "ext.flutter.inspector.widgetLocationIdMap"`. |
| `crates/fdemon-daemon/src/vm_service/events.rs` (or timeline.rs) | Subscribe to `Flutter.Rebuilt` Extension events; parse the events list `[id, count, id, count, …]` against the cached LocationMap. Emit a `RebuildSnapshot` message. |
| `crates/fdemon-app/src/message.rs` | `RebuildStatsToggle { enabled }`, `RebuildStatsFrameReceived { session_id, frame_number, rebuilds }`, `RebuildStatsExtensionStateChanged { enabled }`. |
| `crates/fdemon-app/src/handler/devtools/performance/rebuild_stats.rs` (NEW) | Aggregates per-frame events into `RebuildStatsSnapshot`; clamps to ~30 most recent frames. |
| `crates/fdemon-tui/src/widgets/devtools/performance/details/rebuild_stats_tab.rs` (NEW) | Renders the rebuild table — columns `Widget (file:line)`, `Count`. Selectable rows (j/k); Enter copies the location to clipboard (deferred, optional). |

### 5.4 Timeline Events — Phase 3

Adds:

| File | Change |
|---|---|
| `crates/fdemon-core/src/performance.rs` | `TimelineEvent { name, thread, ph, ts, dur, args }`, `TimelineThread { Ui, Raster }`. Ring buffer of size 1000. |
| `crates/fdemon-daemon/src/vm_service/timeline.rs` | New `fetch_timeline_chunk(since_micros) -> Vec<TimelineEvent>`. Calls VM Service `getVMTimeline` with `timeOriginMicros` / `timeExtentMicros`. Filters to UI + Raster threads. |
| `crates/fdemon-daemon/src/vm_service/client.rs` | New periodic poll (1 Hz when Performance tab is visible) of `fetch_timeline_chunk`. Skipped when Performance tab is not the active panel. |
| `crates/fdemon-app/src/handler/devtools/performance/timeline.rs` (NEW) | Stores last 1000 events; filters per selected frame using `frame.timestamp ± 50ms`. |
| `crates/fdemon-tui/src/widgets/devtools/performance/details/timeline_tab.rs` (NEW) | Vertical scrollable list `(thread_color, name, dur)` rows; filter buttons `[All] [UI] [Raster]` (keyboard-only navigation `f` cycles filter). |

### 5.5 Per-tab content (where the existing detail.rs goes)

`widgets/devtools/performance/frame_chart/detail.rs` (the existing 193-line per-frame detail panel) is **reused** as the seed for `frame_analysis_tab.rs`. The proportional phase bar + hints are new; the existing UI/raster summary + jank verdict carries over.

## 6. Implementation Phases

The work delivers in **three phases**, each independently shippable.

---

### Phase 1 — Tab split (no new functionality)

**Goal:** Memory becomes its own DevTools tab. Performance tab keeps its current Frame Chart content only. Layout-bug symptoms disappear because each tab now gets full inner height.

This phase **must not regress** any existing Performance / Memory behaviour — every key, every mouse region, every rendered cell at every terminal size must round-trip through the new structure.

**Files modified / created:**

| Path | Change |
|---|---|
| `crates/fdemon-app/src/state.rs` | Add `DevToolsPanel::Memory` variant between Performance and Network. |
| `crates/fdemon-app/src/session/performance.rs` | Remove memory-related fields (`memory_history`, `gc_history`, `memory_samples`, `allocation_profile`, `allocation_sort`, `alloc_table_*`). Replace `PerfSection { FrameChart, MemoryChart, MemoryList }` with `PerfSection { FrameChart, DetailsTab }`. Note: `DetailsTab` becomes meaningful in Phase 2 — for Phase 1 it's just a placeholder (always `FrameChart` is focused). |
| `crates/fdemon-app/src/session/memory.rs` | **NEW.** `MemoryState` struct holding everything moved out of `PerformanceState`, plus `MemorySection { Chart, AllocationList }` and per-state defaults. |
| `crates/fdemon-app/src/session/session.rs` | Add `pub memory: MemoryState` to `Session`. Initialise in `Session::new`. |
| `crates/fdemon-app/src/session/mod.rs` | Re-export `MemoryState`, `MemorySection`. |
| `crates/fdemon-app/src/handler/devtools/performance.rs` | Drop allocation-profile / memory-sample / memory-chart / alloc-table handlers — they move to `memory.rs`. Keep frame-selection, frame-chart-scroll, frame-chart-Tab cycling. Trim to ~1000 lines. |
| `crates/fdemon-app/src/handler/devtools/memory.rs` | **NEW.** All memory-side handlers moved here (allocation sort toggle, alloc-row selection, memory-chart scroll, allocation profile fetched). |
| `crates/fdemon-app/src/handler/devtools/mod.rs` | Module declaration for `memory`; re-route memory-related `Message` arms to it. |
| `crates/fdemon-app/src/handler/keys.rs` | Add `in_memory` guard; route `j/k`, `PageUp/Down`, `Home/End`, `Tab/Shift+Tab` (cycles `MemorySection`), `s` (alloc sort toggle) to it. Letter shortcut `m` → `Message::SwitchDevToolsPanel(DevToolsPanel::Memory)`. |
| `crates/fdemon-app/src/message.rs` | No new variants for Phase 1 — `MemoryChartScroll*`, `AllocSortToggle`, `AllocRowSelect`, etc. already exist and only their dispatch shifts. Add `SwitchDevToolsPanel(Memory)` is automatic via enum. |
| `crates/fdemon-tui/src/widgets/devtools/memory/` | **NEW DIR.** Move `widgets/devtools/performance/memory_chart/{chart,table,braille_canvas,mod,tests}.rs` here as `widgets/devtools/memory/{chart,table,braille_canvas,mod,tests}.rs`. Top-level widget renamed `MemoryPanel`. Tests adjusted for the new full-height inner area. |
| `crates/fdemon-tui/src/widgets/devtools/performance/mod.rs` | Remove the `Memory` branch from `render_impl`. Performance now: if `total_h < COMPACT_THRESHOLD` → compact summary; else → frame chart fills inner area. Phase 2 will add the tabbed details pane in this same file. |
| `crates/fdemon-tui/src/widgets/devtools/mod.rs` | Add `Memory` arm to the dispatch match. Tab list becomes `[(Inspector, "[i] Inspector"), (Performance, "[p] Performance"), (Memory, "[m] Memory"), (Network, "[n] Network")]`. Footer-hint match also gets a `Memory` arm. |
| `docs/KEYBINDINGS.md` | Update DevTools section. |

**Acceptance:**

- Switching to the Performance tab in a short terminal (e.g. `tput cols 200 lines 20`) now shows the **full Frame Chart visible at all times**.
- Switching to the Memory tab shows **15+ rows in the allocation table** at the same 200×20 terminal.
- Memory polling continues independently of Performance polling — switching between Performance and Memory does not stop the underlying RPC poll.
- All previous Tab / j/k / PageUp/Down / `s` / `Home/End` bindings still work, but Tab now cycles `MemorySection { Chart, AllocationList }` on the Memory tab and `PerfSection { FrameChart, DetailsTab }` on the Performance tab (with DetailsTab a no-op stub for Phase 1).
- Letter shortcut `m` switches to the Memory panel.
- `cargo test --workspace` passes — existing Performance tests adjusted for the split (move memory-only tests to a new `memory/` test module).

---

### Phase 2 — Performance tab details pane (Frame Analysis populated)

**Goal:** The Performance tab gains the chart-plus-tabbed-details layout. The Frame Analysis tab is fully functional using existing data. Rebuild Stats and Timeline Events render stubs.

**Files modified / created:**

| Path | Change |
|---|---|
| `crates/fdemon-core/src/performance.rs` (or new `crates/fdemon-core/src/frame_analysis.rs` if it gets large) | `frame_hints(frame: &FrameTiming, refresh_rate_hz: f64) -> Vec<FrameHint>`; `pub enum FrameHint { LongestPhase(FramePhaseKind), ShaderCompilation, RasterDominant, BuildDominant, OverBudget { excess_ms: f64 } }`. Unit-tested with table-driven cases. |
| `crates/fdemon-app/src/session/performance.rs` | Add `pub details_tab: PerfDetailsTab`, `pub details_pane_visible_height: Cell<usize>`, `pub display_refresh_rate: f64` (default 60.0; set from `Flutter.Frame` event metadata once available, otherwise stay at 60.0). |
| `crates/fdemon-app/src/state.rs` | New enum `PerfDetailsTab { FrameAnalysis, RebuildStats, TimelineEvents }` (alongside `DetailsTab` for inspector). |
| `crates/fdemon-app/src/message.rs` | `PerfCycleDetailsTab { forward: bool }`, `PerfFocusDetailsTab(PerfDetailsTab)`. |
| `crates/fdemon-app/src/handler/devtools/performance.rs` (split into a `performance/` directory) | Move into `performance/mod.rs` + `performance/frame.rs` (frame selection/scroll), `performance/details.rs` (tab cycling, focus). Keep handler logic for tab navigation. |
| `crates/fdemon-app/src/handler/keys.rs` | When `in_performance && focused_section == DetailsTab`: `]` → next details tab, `[` → previous. `Tab/Shift+Tab` still cycles `FrameChart` ↔ `DetailsTab`. |
| `crates/fdemon-tui/src/widgets/devtools/performance/mod.rs` | Restructure rendering: `MIN_DUAL_PANE_HEIGHT` decides chart-only vs chart+details. When dual-pane, top 55% (>= 10 rows) is `FrameChart`, bottom 45% (>= 8 rows) is a new `DetailsPane`. |
| `crates/fdemon-tui/src/widgets/devtools/performance/details/mod.rs` | **NEW.** Tab strip + dispatch. Borrows from `widgets/devtools/inspector/details/mod.rs` for layout structure. |
| `crates/fdemon-tui/src/widgets/devtools/performance/details/frame_analysis_tab.rs` | **NEW.** Renders the frame number header, total/budget line, proportional phase bar, hint list, no-data fallback, no-selection fallback. |
| `crates/fdemon-tui/src/widgets/devtools/performance/details/rebuild_stats_tab.rs` | **NEW (stub).** Returns "Coming soon" text. |
| `crates/fdemon-tui/src/widgets/devtools/performance/details/timeline_events_tab.rs` | **NEW (stub).** Returns "Coming soon" text. |
| `crates/fdemon-tui/src/widgets/devtools/performance/frame_chart/detail.rs` | Trimmed — the per-frame summary line moves to `frame_analysis_tab.rs`. The frame-bar legend stays. |
| `docs/KEYBINDINGS.md` | Document `]`/`[` tab cycling within Performance details pane. |

**Acceptance:**

- Performance tab at 200×30 shows: top 55% Frame Chart, bottom 45% tabbed Frame Analysis content.
- Selecting a frame (←/→) populates the Frame Analysis tab with frame-specific data; deselecting reverts to "Select a frame above…".
- Frames with `FramePhases` data show the proportional 4-segment phase bar with build/layout/paint/raster labels.
- Frames without `FramePhases` data fall back to the aggregate build+raster split.
- `]`/`[` cycle between the three tabs; the inactive Rebuild Stats / Timeline Events tabs show "Coming soon" stubs.
- At terminal heights `< MIN_DUAL_PANE_HEIGHT (18)`, the details pane collapses and the Frame Chart fills the area (matches current Phase 1 behaviour for short terminals).
- All existing Performance tests still pass; new tests cover hint generation, phase-bar proportions, tab cycling.

---

### Phase 3 — Rebuild Stats + Timeline Events populated

**Goal:** Both stub tabs become functional. Two independent VM Service flows are added; either may be feature-flagged off independently.

**Files modified / created:**

| Path | Change |
|---|---|
| `crates/fdemon-core/src/rebuild_stats.rs` (NEW) | `LocationMap`, `Location`, `RebuildLocation`, `RebuildStatsSnapshot`. |
| `crates/fdemon-core/src/timeline.rs` (NEW) | `TimelineEvent`, `TimelineThread { Ui, Raster, Other }`. |
| `crates/fdemon-daemon/src/vm_service/extensions/inspector.rs` | `widget_location_id_map()`. |
| `crates/fdemon-daemon/src/vm_service/extensions/mod.rs` | `PROFILE_USER_WIDGET_BUILDS`, `WIDGET_LOCATION_ID_MAP` constants. |
| `crates/fdemon-daemon/src/vm_service/timeline.rs` | `fetch_timeline_chunk(since_micros)`. Calls `getVMTimeline`. Filters by `thread name ∈ {"UI", "Raster"}`. |
| `crates/fdemon-daemon/src/vm_service/client.rs` | 1 Hz timeline poll loop, gated on `panel_active == Performance`. Listens for `Flutter.Rebuilt` Extension events. |
| `crates/fdemon-app/src/message.rs` | `RebuildStatsSnapshotReceived { session_id, snapshot }`, `TimelineEventsBatchReceived { session_id, events }`, `RebuildStatsExtensionStateChanged { enabled }`. |
| `crates/fdemon-app/src/handler/devtools/performance/rebuild_stats.rs` (NEW) | Aggregate handler; clamps to last 30 frames. |
| `crates/fdemon-app/src/handler/devtools/performance/timeline.rs` (NEW) | Append to ring buffer; filter by selected frame timestamp ± 50ms. |
| `crates/fdemon-tui/src/widgets/devtools/performance/details/rebuild_stats_tab.rs` | Replace stub with a sortable table (Widget / file:line / Count). |
| `crates/fdemon-tui/src/widgets/devtools/performance/details/timeline_events_tab.rs` | Replace stub with vertical event list, thread-colored rows, `f` to cycle filter `[All] [UI] [Raster]`. |
| `docs/KEYBINDINGS.md` | Document `f` filter cycle, `s` sort toggle on rebuild stats. |
| `docs/ARCHITECTURE.md` (doc_maintainer task) | Add Performance / Memory split + new VM Service calls + new state structures. |

**Acceptance:**

- With a running Flutter app, switching to Performance shows live Frame Analysis. Switching to the Rebuild Stats tab while the app is performing rebuilds shows per-frame widget rebuild counts within ~1s. Toggling the extension OFF hides the tab.
- Timeline Events tab populates within ~1s after switching; filters narrow to UI or Raster thread.
- Disabling Performance polling (by leaving the panel) stops both `getVMTimeline` and `Flutter.Rebuilt` subscriptions to avoid wasted work.
- `cargo test --workspace` adds: rebuild-stats aggregation, location-map parsing, timeline parsing, thread filtering.

---

## 7. Risks & Trade-offs

### 7.1 State migration risk

Splitting `PerformanceState` into `PerformanceState` + `MemoryState` touches dozens of call sites across `handler/devtools/`, `widgets/`, and tests. Risk: a missed migration leaves stale memory fields on `PerformanceState`, or a test still references `session.performance.memory_history`.

**Mitigation:** Phase 1 must do the migration in a single coherent commit, run `cargo check --workspace --all-targets` after each edit, and lean on the compiler to find missed references. The split is straightforward — every memory-named field on the old struct moves; everything else stays.

### 7.2 Mouse region map drift

Mouse regions are recorded by widget during render. The new dispatch (`Memory` arm) needs region forwarding from `widgets/devtools/mod.rs:render_impl`. Risk: clicks on the Memory chart fail to register because the region builder isn't threaded through.

**Mitigation:** the existing pattern (`memory_chart::render_with_regions`) already supports a `MouseCtx`; the new `widgets/devtools/memory/mod.rs:render_with_regions` is a copy-paste-and-rename. Add a regression test mirroring `devtools_tab_bar_registers_three_click_regions` that asserts four regions when Memory is included.

### 7.3 Phase 2 layout decision deviates from Phase 1

Phase 1 leaves Performance as a single-chart panel — straightforward. Phase 2 introduces `MIN_DUAL_PANE_HEIGHT`. Risk: the threshold is set too high and short-tall terminals (e.g. 50 rows tall but 80 cols wide — a vertical iTerm split) render the details pane too narrow for the proportional phase bar.

**Mitigation:** Two thresholds — `MIN_DUAL_PANE_HEIGHT (18)` AND `MIN_PHASE_BAR_WIDTH (40)`. When width < 40, the phase bar degenerates to a 1-line summary (`B 6.1ms | L 2.0ms | P 3.4ms | R 6.7ms`) and the proportional graphic is suppressed. Both thresholds get named constants with derivation comments per Principle 4.

### 7.4 Display refresh rate is not in current data

`Flutter.Frame` events in `vm_service/timeline.rs:50–95` do not currently extract `targetRasterTime` / display refresh rate. Phase 2 hint generation depends on `display_refresh_rate` to call out "over budget" frames.

**Mitigation:** Default to 60.0 Hz in Phase 2 (matches the current `FRAME_BUDGET_60FPS_MICROS` constant). If the user has a 90 Hz / 120 Hz device, the hints are slightly conservative but never wrong. Phase 3 can extend the Frame event parser to capture `Display.Refresh` events (a separate Extension stream event) — defer.

### 7.5 Timeline event volume

`getVMTimeline` can return thousands of events per second when timeline streams are enabled. Risk: 1 Hz polling × 1000-event buffer = healthy; but if a user idles on the Performance tab, the ring buffer fills up with idle events and overwhelms the selected-frame filter.

**Mitigation:** Cap the buffer at 1000 events globally and 200 events per frame. Drop oldest. Clear the buffer when the user switches away from the Performance tab.

### 7.6 Rebuild Stats requires extension to be enabled

The extension toggle (`ext.flutter.profileWidgetBuilds`) is per-isolate and per-session. Users who restart the app (hot restart) lose the toggle.

**Mitigation:** Re-enable the toggle on `SessionRestartCompleted` if it was previously on. Surface the state in the Rebuild Stats tab header ("Rebuild tracking: ON" / "OFF — press X to enable").

### 7.7 Per-frame deselection in Memory tab

`Esc` semantics today on Performance: if a frame is selected, deselect; else exit DevTools. Memory tab equivalent: if an alloc row is selected, deselect; else exit. Risk: inconsistent precedence if a user has both a frame selected (on Performance tab) AND switches to Memory tab — but state is per-panel so this can't actually happen.

**Mitigation:** Document the per-panel Esc precedence in `KEYBINDINGS.md`. Add a regression test that switching panels does NOT auto-deselect.

## 8. Out-of-Scope (Future Work)

- Perfetto-style timeline visualization (would require a WebView or a custom flame-graph TUI renderer).
- "Enhance Tracing" controls (image sizes, platform channels, layouts) — these are extension toggles that produce more timeline events but the UI for the toggles is non-trivial.
- Refresh-rate-aware budget warnings (90 / 120 Hz devices) — depends on parsing `Display.Refresh` events.
- Per-frame screenshot capture / preview (DevTools' frame thumbnail feature).
- Memory leak detection via heap snapshot diffing.
- Export-to-JSON / save-session for offline analysis (DevTools' offline mode equivalent).

## 9. Configuration Additions

```toml
# .fdemon/config.toml — new [devtools.performance] block (defaults shown)
[devtools.performance]
# Phase 3: whether to enable widget rebuild tracking automatically on session start.
auto_enable_rebuild_tracking = false

# Phase 3: how many recent frames to keep in the rebuild stats ring buffer.
rebuild_stats_frame_window = 30

# Phase 3: max timeline events kept in memory.
timeline_event_buffer_size = 1000
```

The toggle `auto_enable_rebuild_tracking` defaults OFF because the underlying `profileWidgetBuilds` extension has non-trivial overhead in dev builds.

## 10. Keyboard Shortcuts Summary

After all three phases:

| Key | Context | Action |
|---|---|---|
| `i` | DevTools | Switch to Inspector |
| `p` | DevTools | Switch to Performance |
| `m` | DevTools | Switch to Memory (NEW) |
| `n` | DevTools | Switch to Network |
| `Tab` / `Shift+Tab` | Performance | Cycle focus FrameChart ↔ DetailsTab |
| `]` / `[` | Performance, DetailsTab focused | Cycle details tab forward / back |
| `←` / `→` | Performance, FrameChart focused | Select previous / next frame |
| `j` / `k` / `↑` / `↓` | Performance, DetailsTab focused | Scroll details list |
| `f` | Performance, TimelineEvents tab | Cycle filter All → UI → Raster |
| `Tab` / `Shift+Tab` | Memory | Cycle focus Chart ↔ AllocationList |
| `j` / `k` / `↑` / `↓` | Memory, AllocationList focused | Scroll allocation rows |
| `s` | Memory, AllocationList focused | Toggle sort By Size ↔ By Instances |
| `Esc` | Performance, frame selected | Deselect frame |
| `Esc` | Memory, alloc row selected | Deselect row |
| `Esc` | otherwise | Return to Logs |

## 11. Documentation Updates

| Doc | Owner | Trigger |
|---|---|---|
| `docs/ARCHITECTURE.md` — DevTools Subsystem section | `doc_maintainer` agent | Phase 1 (adds `DevToolsPanel::Memory`, splits `PerformanceState` and `MemoryState`) + Phase 3 (adds rebuild stats / timeline event RPCs) |
| `docs/CODE_STANDARDS.md` | unchanged | No new TEA exceptions; existing Cell-based render hints follow the established pattern. |
| `docs/KEYBINDINGS.md` — DevTools section | implementor (unmanaged doc) | Phase 1 (Memory tab `m`), Phase 2 (`]`/`[`), Phase 3 (`f`) |

## 12. Phased Checklist

- [ ] **Phase 1 — Tab split** (`phase-1/`)
  - 1A: `DevToolsPanel::Memory` enum variant + tab bar order + letter shortcut `m`.
  - 1B: `PerformanceState` slimmed + new `MemoryState`; per-session both populated.
  - 1C: Memory chart + alloc table widgets moved under `widgets/devtools/memory/`; new `MemoryPanel` widget.
  - 1D: Memory handler module + key routing (`in_memory` guard).
  - 1E: Footer + region tests + `KEYBINDINGS.md`.
  - 1F: ARCHITECTURE.md update (doc_maintainer task).

- [ ] **Phase 2 — Performance details pane + Frame Analysis populated** (`phase-2/`)
  - 2A: `frame_hints` core helper + tests.
  - 2B: `PerfDetailsTab` enum + tab-cycle messages + handler split into `performance/` directory.
  - 2C: `widgets/devtools/performance/details/{mod, frame_analysis_tab, rebuild_stats_tab, timeline_events_tab}.rs`.
  - 2D: `widgets/devtools/performance/mod.rs` restructured: dual-pane vs chart-only.
  - 2E: KEYBINDINGS.md update for `]`/`[`.

- [ ] **Phase 3 — Rebuild Stats + Timeline Events populated** (`phase-3/`)
  - 3A: VM Service additions (`widget_location_id_map`, `getVMTimeline`, `Flutter.Rebuilt` subscription).
  - 3B: Core types (`LocationMap`, `RebuildLocation`, `TimelineEvent`).
  - 3C: Handler modules (`performance/rebuild_stats.rs`, `performance/timeline.rs`).
  - 3D: Tab content populated.
  - 3E: ARCHITECTURE.md update (doc_maintainer task).

After this plan is approved, the task index for **Phase 1** will be created at `workflow/plans/features/devtools-performance-memory-split/phase-1/TASKS.md` with per-file tasks and the required File Overlap Analysis. Phases 2 and 3 task indexes will be created after their preceding phase completes so the breakdown can incorporate review feedback.
