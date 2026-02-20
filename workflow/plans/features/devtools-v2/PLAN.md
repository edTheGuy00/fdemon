# Plan: DevTools V2 — Inspector Merge, Performance Overhaul, Network Monitor

## TL;DR

Significantly upgrade the DevTools TUI to approach browser DevTools parity. Three major changes: (1) merge Inspector and Layout tabs into a unified Inspector tab with widget tree + layout explorer side-by-side, (2) replace the Performance tab's sparkline/gauge with a proper frame bar chart and time-series memory chart with class allocation table, removing the stats section, and (3) add a new Network Monitor tab for fine-grained HTTP/WebSocket traffic inspection. All large widgets will be decomposed into smaller, testable sub-components.

---

## Background

The initial DevTools integration (Phase 4–5 of the original plan) delivered functional Inspector, Layout, and Performance panels. However, compared to the browser-based Flutter DevTools, several areas are under-featured:

### Current Limitations

**Inspector + Layout are separate tabs with weak details:**
- The Inspector tab shows a widget tree (left) and a "details" panel (right) that only displays widget name, a properties list, and source location — minimal value
- The Layout tab is a separate panel requiring manual switching, showing constraints/size/flex but disconnected from the tree
- In the browser DevTools, the Inspector tab combines the widget tree with an integrated Widget Explorer that shows a box-model visualization (width, height, padding), properties, render object details, and a flex explorer — all in one view

**Performance tab uses basic visualizations:**
- Frame timing uses a `Sparkline` chart — no individual frame selection, no UI/Raster thread breakdown, no jank/shader markers
- Memory uses a simple `Gauge` — no time-series tracking, no allocation breakdown, no class-level consumption
- A "Stats" section shows aggregated numbers redundant with what frame timing and memory should display
- The browser DevTools shows a bar chart with selectable frames (UI vs Raster bars, jank highlighting in red, shader compilation in dark red), a time-series memory chart with RSS/Allocated/Dart/Native/Raster layers, and a class allocation profile table

**No network monitoring:**
- The browser DevTools has a full Network tab showing HTTP/HTTPS/WebSocket requests with method, URI, status, duration, size, timing, headers, and body inspection
- Flutter exposes this data via `ext.dart.io.getHttpProfile` / `getSocketProfile` VM Service extensions
- fdemon currently has zero network monitoring capability

### Widget File Size Concerns

Current widget files are already approaching or exceeding the 500-line threshold:
- `inspector.rs`: 1,003 lines
- `layout_explorer.rs`: 853 lines
- `performance.rs`: 833 lines
- `mod.rs`: 611 lines
- `handler/devtools.rs`: 1,516 lines

Adding the planned features to these files would push them well past maintainable sizes. This plan front-loads a component decomposition phase.

---

## Research: Official Flutter DevTools Reference

### Inspector (Widget Explorer)

The browser DevTools Inspector combines:
- **Widget tree** (left panel): expandable tree with user-code vs framework-code distinction
- **Widget Explorer** (right panel) with three tabs:
  1. **Widget Properties**: box-model mini-view (width, height, padding) + property list with default-value indicators
  2. **Render Object**: all properties on the render object
  3. **Flex Explorer** (for Row/Column/Flex widgets): interactive visualization of main/cross axis alignment, flex factor, flex fit, with modifiable dropdown values

### Performance — Frame Chart

- **Bar chart**: each pair of bars = one frame (UI thread + Raster thread)
- **Color coding**: blue/standard = UI, green/standard = Raster, RED = jank (>16ms), DARK RED = shader compilation
- **Frame selection**: clicking a frame shows timeline events, build/layout/paint phase breakdown
- **Timeline events tab**: shows all traced events including build, layout, paint phases, HTTP requests, GC events

### Performance — Memory View

- **Time-series chart** (y-axis: memory, x-axis: time, polled every 500ms):
  - **Dart/Flutter Heap**: objects in the Dart heap
  - **Dart/Flutter Native**: memory outside Dart heap (decoded images, file reads, etc.)
  - **Raster Cache**: Flutter engine raster cache layers/pictures
  - **Allocated**: total Dart heap capacity
  - **RSS**: resident set size (heap + stack + shared libraries)
- **Events overlay**: GC events, snapshots, custom memory events
- **Profile Memory tab**: allocation by class, CSV export, refresh-on-GC toggle
- **Diff Snapshots tab**: before/after snapshot comparison for leak detection

### Network View

- **Request table** (left): method, URI, status, type, duration, size
- **Details panel** (right): general info, timing info, request headers/body, response headers/body
- **Filter syntax**: `method:GET`, `status:200`, `type:json`, or free text
- **VM Service API**: `ext.dart.io.getHttpProfile(isolateId, updatedSince?)` → `HttpProfile { requests, timestamp }`
- **Per-request details**: `ext.dart.io.getHttpProfileRequest(isolateId, id)` → full `HttpProfileRequest` with bodies
- **Socket profiling**: `ext.dart.io.getSocketProfile(isolateId)` for WebSocket traffic
- **Controls**: pause/resume recording, clear history

---

## Affected Modules

### Widget Decomposition (Phase 1 — Refactor)

- `crates/fdemon-tui/src/widgets/devtools/inspector.rs` — **SPLIT** into `inspector/mod.rs`, `inspector/tree_panel.rs`, `inspector/layout_panel.rs`
- `crates/fdemon-tui/src/widgets/devtools/layout_explorer.rs` — **ABSORBED** into `inspector/layout_panel.rs` (merged)
- `crates/fdemon-tui/src/widgets/devtools/performance.rs` — **SPLIT** into `performance/mod.rs`, `performance/frame_chart.rs`, `performance/memory_chart.rs`
- `crates/fdemon-tui/src/widgets/devtools/mod.rs` — Update sub-tab bar (remove Layout tab, add Network tab)
- `crates/fdemon-app/src/handler/devtools.rs` — **SPLIT** into `handler/devtools/mod.rs`, `handler/devtools/inspector.rs`, `handler/devtools/performance.rs` (proactive split before adding network handlers)

### Merged Inspector Tab (Phase 2)

- `crates/fdemon-tui/src/widgets/devtools/inspector/mod.rs` — **NEW** Top-level inspector: 50/50 split (horizontal/vertical responsive)
- `crates/fdemon-tui/src/widgets/devtools/inspector/tree_panel.rs` — **NEW** Extracted widget tree rendering
- `crates/fdemon-tui/src/widgets/devtools/inspector/layout_panel.rs` — **NEW** Enhanced layout explorer with box model, padding, dimensions
- `crates/fdemon-app/src/state.rs` — Remove `DevToolsPanel::Layout` variant, merge `LayoutExplorerState` into `InspectorState`
- `crates/fdemon-app/src/handler/devtools.rs` — Update panel switching, auto-fetch layout on tree node selection
- `crates/fdemon-app/src/handler/keys.rs` — Remove `'l'` keybinding, update hints

### Performance Overhaul (Phase 3)

- `crates/fdemon-tui/src/widgets/devtools/performance/mod.rs` — **NEW** Two-section layout (frame timing + memory)
- `crates/fdemon-tui/src/widgets/devtools/performance/frame_chart.rs` — **NEW** Bar chart with selectable frames, jank/shader markers, phase breakdown
- `crates/fdemon-tui/src/widgets/devtools/performance/memory_chart.rs` — **NEW** Time-series line chart, legend, events overlay, class table
- `crates/fdemon-core/src/performance.rs` — Add `FramePhases` struct (build/paint/raster), `MemorySample` with RSS/Native/Raster fields, `ClassAllocation` for table
- `crates/fdemon-app/src/session/performance.rs` — Extend `PerformanceState` with memory breakdown history, selected frame, class allocations
- `crates/fdemon-app/src/message.rs` — Add `SelectFrame`, `AllocationProfileReceived`, `MemoryBreakdownReceived` messages
- `crates/fdemon-app/src/handler/devtools.rs` — Handle frame selection, allocation profile fetch
- `crates/fdemon-daemon/src/vm_service/performance.rs` — Add `get_allocation_profile()` parsing for class-level data, memory breakdown parsing

### Network Monitor Tab (Phase 4)

- `crates/fdemon-tui/src/widgets/devtools/network/mod.rs` — **NEW** Network monitor top-level: request table + details panel
- `crates/fdemon-tui/src/widgets/devtools/network/request_table.rs` — **NEW** Scrollable request list with columns
- `crates/fdemon-tui/src/widgets/devtools/network/request_details.rs` — **NEW** Headers, body, timing detail view
- `crates/fdemon-core/src/network.rs` — **NEW** Domain types: `HttpRequest`, `HttpResponse`, `NetworkEntry`, `SocketEntry`
- `crates/fdemon-app/src/state.rs` — Add `DevToolsPanel::Network` variant, `NetworkMonitorState`
- `crates/fdemon-app/src/session.rs` — Add `NetworkState` to `SessionHandle` or `PerformanceState`
- `crates/fdemon-app/src/message.rs` — Add `HttpProfileReceived`, `HttpRequestDetailReceived`, `ClearNetworkProfile`, `ToggleNetworkRecording` messages
- `crates/fdemon-app/src/handler/devtools.rs` — Handle network messages, polling, request selection
- `crates/fdemon-app/src/handler/keys.rs` — Add `'n'` keybinding for Network tab
- `crates/fdemon-daemon/src/vm_service/network.rs` — **NEW** `getHttpProfile`, `getHttpProfileRequest`, `clearHttpProfile`, `getSocketProfile` wrappers
- `crates/fdemon-daemon/src/vm_service/extensions/mod.rs` — Add `ext.dart.io.*` constants

### Documentation Updates (Phase 5)

- `docs/KEYBINDINGS.md` — Update sub-tab keys (remove `l`, add `n`)
- `docs/ARCHITECTURE.md` — Update widget directory structure

---

## Development Phases

### Phase 1: Widget Component Decomposition

**Goal**: Break existing oversized widget files into smaller, modular sub-components without changing any visible behavior. This is a pure refactor phase.

#### Steps

1. **Split `inspector.rs` into directory module**
   - Create `crates/fdemon-tui/src/widgets/devtools/inspector/` directory
   - `mod.rs` — `WidgetInspector` struct and main `render()` dispatch (disconnected/loading/error/content/empty states)
   - `tree_panel.rs` — `TreePanel` widget: tree row rendering, expand icons, viewport scrolling, scroll thumb, selection highlighting, node styling
   - `details_panel.rs` — `DetailsPanel` widget: widget name, properties list, creation location (this will be replaced in Phase 2, but extracting it first keeps the refactor clean)
   - Move constants (`WIDE_TERMINAL_THRESHOLD`, `TREE_WIDTH_PCT`, etc.) and helpers (`expand_icon`, `visible_viewport_range`, `node_style`, `short_path`) into appropriate sub-files
   - All existing 26 tests must continue passing with no changes to test assertions

2. **Split `performance.rs` into directory module**
   - Create `crates/fdemon-tui/src/widgets/devtools/performance/` directory
   - `mod.rs` — `PerformancePanel` struct and main layout dispatch (disconnected/compact/full)
   - `frame_section.rs` — `FrameSection` widget: FPS header line, sparkline chart (to be replaced in Phase 3)
   - `memory_section.rs` — `MemorySection` widget: heap gauge display (to be replaced in Phase 3)
   - `stats_section.rs` — `StatsSection` widget: frames/jank/GC stats (to be removed in Phase 3)
   - Move style helpers (`fps_style`, `gauge_style_for_utilization`, `jank_style`, `format_number`) into shared `styles.rs`
   - All existing 18 tests must continue passing

3. **Split `handler/devtools.rs` into directory module**
   - Create `crates/fdemon-app/src/handler/devtools/` directory
   - `mod.rs` — Top-level dispatch: `handle_devtools_message()`, shared helpers (`map_rpc_error`, `parse_default_panel`), enter/exit mode handlers
   - `inspector.rs` — Inspector-specific handlers: `handle_widget_tree_fetched`, `handle_widget_tree_fetch_failed`, `handle_widget_tree_fetch_timeout`, `handle_inspector_navigate`, `handle_layout_data_fetched`, `handle_layout_data_fetch_failed`, `handle_layout_data_fetch_timeout`, `handle_open_browser_devtools`
   - `performance.rs` — Performance-specific handlers: `handle_debug_overlay_toggled`, plus future frame selection and allocation profile handlers
   - Move tests alongside their handler functions into each sub-file
   - All existing 57 handler tests must continue passing

4. **Verify no regressions**
   - All 80+ devtools widget tests pass
   - All 57 handler tests pass
   - `cargo clippy --workspace` clean
   - Visual spot-check: all three panels render identically to pre-refactor

**Milestone**: Same visual output, but code organized into ~200–400 line files instead of 800–1500 line monoliths. Both widget and handler layers ready for feature additions.

---

### Phase 2: Merged Inspector + Layout Tab

**Goal**: Combine the Inspector and Layout tabs into a single unified Inspector tab. The widget tree occupies one half, the Layout Explorer occupies the other half. The separate Layout tab is removed.

#### Steps

1. **Merge `LayoutExplorerState` into `InspectorState`**
   - Move all fields from `LayoutExplorerState` into `InspectorState`: `layout`, `layout_loading`, `layout_error`, `has_layout_object_group`, `last_fetched_node_id`, `pending_node_id`
   - Remove `LayoutExplorerState` struct
   - Update `DevToolsViewState` to remove `layout_explorer` field
   - Update `DevToolsViewState::reset()` accordingly
   - Update all handler code referencing `state.devtools.layout_explorer` → `state.devtools.inspector`

2. **Remove `DevToolsPanel::Layout` variant**
   - Remove `Layout` from `DevToolsPanel` enum (leaving `Inspector`, `Performance`, and later `Network`)
   - Remove `SwitchDevToolsPanel(Layout)` handling
   - Remove `'l'` keybinding from `handle_key_devtools()` in `handler/keys.rs`
   - Update sub-tab bar rendering in `devtools/mod.rs` to show only `[i] Inspector  [p] Performance`
   - Update footer hints to remove layout-specific hints

3. **Auto-fetch layout on tree node selection**
   - When the user navigates to a different node in the Inspector tree (Up/Down), automatically trigger a layout data fetch for the newly selected node
   - Add debounce: only fetch if selected node changed AND 500ms have passed since last fetch
   - Use the selected node's `value_id` (already used by the current Layout tab's auto-fetch logic)
   - Clear stale layout data when selection changes (show loading state)

4. **Create `inspector/layout_panel.rs` — Enhanced Layout Explorer**
   - Replace the current `details_panel.rs` with a proper Layout Explorer that shows:
     - **Widget name and source location** (file:line) at the top — preserves the useful part of the old details panel
     - **Box model visualization** (inspired by browser DevTools):
       ```
       ┌─ padding ────────────────────────┐
       │  top: 8.0                        │
       │  ┌─ widget ──────────────────┐   │
       │  │                           │   │
       │  │   width: 200.0            │   │
       │  │   height: 48.0            │   │
       │  │                           │   │
       │  └───────────────────────────┘   │
       │  bottom: 8.0                     │
       └──────────────────────────────────┘
       ```
     - **Dimensions row**: `W: 200.0  H: 48.0` prominently displayed
     - **Padding details**: `top: 8.0  right: 0.0  bottom: 8.0  left: 0.0`
     - **Constraints**: `min: 0.0 x 0.0  max: 414.0 x 896.0` with `(tight)` indicator when applicable
     - **Flex properties** (when applicable): flex factor, flex fit, main/cross axis alignment
   - Render using ratatui `Block`, `Paragraph`, and manual ASCII drawing for the box model
   - When no layout data is loaded yet: show "Select a widget to see layout details" or loading spinner

5. **Responsive 50/50 split layout**
   - In `inspector/mod.rs`, implement responsive layout:
     - **Wide terminals** (width >= 100 cols): horizontal split — tree panel (50%) | layout panel (50%)
     - **Narrow terminals** (width < 100 cols): vertical split — tree panel (50%) | layout panel (50%)
   - Use `ratatui::layout::Layout` with `Constraint::Percentage(50)` for even split
   - Update `WIDE_TERMINAL_THRESHOLD` from 80 to 100 (wider threshold since both panels need more room)

6. **Extract padding and dimension data from VM Service**
   - The current `LayoutInfo` struct has `constraints`, `size`, `flex_factor`, `flex_fit`, `description`
   - Extend `LayoutInfo` in `fdemon-core/src/widget_tree.rs` to include:
     - `padding: Option<EdgeInsets>` — `{ top, right, bottom, left }`
     - `margin: Option<EdgeInsets>`
   - In `fdemon-daemon/src/vm_service/extensions/layout.rs`, extract padding from the `DiagnosticsNode` properties:
     - Look for `padding` property in the node's properties list
     - Parse `EdgeInsets` values from the diagnostic property format (e.g., `"EdgeInsets(8.0, 0.0, 8.0, 0.0)"` or individual `EdgeInsets.top`, `.right`, `.bottom`, `.left` properties)
   - Fallback: if padding data is not available in the diagnostics node, the box model view shows only dimensions and constraints (no padding box)

7. **Update tests**
   - Update all inspector tests for new 50/50 split layout
   - Add tests for layout panel rendering (box model, dimensions, padding)
   - Add tests for auto-fetch on node selection change
   - Add tests for responsive layout switching
   - Remove all Layout tab-specific tests from `layout_explorer.rs`
   - Remove `layout_explorer.rs` file entirely

**Milestone**: A single Inspector tab shows widget tree alongside an informative layout explorer with box model visualization, padding, dimensions, and constraints — matching the browser DevTools' integrated Widget Explorer experience.

---

### Phase 3: Performance Tab Overhaul

**Goal**: Replace the sparkline and gauge with a proper frame bar chart and time-series memory chart. Remove the stats section. Add frame selection with phase breakdown and class allocation table.

#### Steps

1. **Extend core performance types**
   - In `fdemon-core/src/performance.rs`, add:
     ```rust
     pub struct FramePhases {
         pub build_micros: u64,
         pub layout_micros: u64,
         pub paint_micros: u64,
         pub raster_micros: u64,
         pub shader_compilation: bool,
     }
     ```
   - Extend `FrameTiming` to include `phases: Option<FramePhases>` and `shader_compilation: bool`
   - Add `MemorySample` struct for time-series tracking:
     ```rust
     pub struct MemorySample {
         pub dart_heap: u64,        // Dart/Flutter heap objects
         pub dart_native: u64,      // Native memory (images, files)
         pub raster_cache: u64,     // Raster layer/picture cache
         pub allocated: u64,        // Total heap capacity
         pub rss: u64,              // Resident set size
         pub timestamp: Instant,
     }
     ```
   - Add `ClassAllocation` struct:
     ```rust
     pub struct ClassAllocation {
         pub class_name: String,
         pub library: String,
         pub instance_count: u64,
         pub shallow_size: u64,
         pub retained_size: u64,
     }
     ```

2. **Extend VM Service performance data collection**
   - In `fdemon-daemon/src/vm_service/performance.rs`:
     - Extend `get_memory_usage()` to parse `externalUsage` and return richer data (currently only returns `heapUsage`, `heapCapacity`, `externalUsage`)
     - Add RSS collection: call `getIsolate()` periodically to extract RSS from isolate data, or use platform-specific approach
     - Enhance `get_allocation_profile()` to return per-class breakdown with `ClassHeapStats` including class name, instance count, and size
     - Parse frame timeline events for build/layout/paint/raster phase breakdown when available
   - Add `MemorySample` construction from combined VM service data
   - Add detection of shader compilation from frame timeline events

3. **Extend `PerformanceState` for new data**
   - In `crates/fdemon-app/src/session/performance.rs`:
     - Add `memory_samples: RingBuffer<MemorySample>` (size: 120 = 60 seconds at 500ms polling)
     - Add `selected_frame: Option<usize>` — index into `frame_history` for the currently selected frame
     - Add `class_allocations: Vec<ClassAllocation>` — latest allocation profile snapshot
     - Add `allocation_sort_column: AllocationSortColumn` enum (`BySize`, `ByInstances`, `ByRetained`)
     - Keep existing `frame_history`, `gc_history` ring buffers
   - Add new messages in `message.rs`:
     - `SelectPerformanceFrame { index: Option<usize> }` — select/deselect a frame in the bar chart
     - `AllocationProfileReceived { session_id, profile: AllocationProfile }` — allocation data arrived
     - `MemorySampleReceived { session_id, sample: MemorySample }` — rich memory sample arrived

4. **Implement frame bar chart (`performance/frame_chart.rs`)**
   - **NEW** `FrameChart` widget replacing the sparkline:
     - Each frame rendered as a pair of vertical bars: UI thread (left, cyan) + Raster thread (right, green)
     - Bar height proportional to frame time, scaled to fit available height
     - **Jank frames**: bars colored RED when total frame time exceeds 16ms budget
     - **Shader compilation**: bars colored DARK RED (magenta in TUI) when shader compilation detected
     - **Frame budget line**: horizontal dashed line at 16ms mark across the chart
     - **Scrollable**: show last N frames that fit in the available width (2 chars per frame + 1 gap = 3 chars per frame)
     - **Selectable**: left/right arrow keys move selection highlight; selected frame has a distinct background or border
   - **Frame detail panel** (below the chart, 3–4 lines):
     - When a frame is selected, show:
       ```
       Frame #1234  Total: 18.2ms (JANK)
       UI:  Build: 6.1ms  Layout: 2.3ms  Paint: 3.8ms  = 12.2ms
       Raster: 6.0ms
       ```
     - When no frame selected: show summary line: `FPS: 60  Avg: 8.2ms  Jank: 2 (1.3%)  Shader: 0`
   - Color scheme:
     - UI bars: Cyan
     - Raster bars: Green
     - Jank: Red
     - Shader compilation: Magenta
     - Budget line: DarkGray dashed
     - Selected frame: White border/highlight

5. **Implement memory chart (`performance/memory_chart.rs`)**
   - **NEW** `MemoryChart` widget replacing the gauge:
     - **Time-series line/area chart** showing memory over time (last 60 seconds)
     - **Y-axis**: auto-scaled memory in MB, with gridlines
     - **X-axis**: time (relative, e.g., "30s ago", "now")
     - **Stacked area layers** (bottom to top):
       - Dart/Flutter Heap (cyan)
       - Dart/Flutter Native (blue)
       - Raster Cache (magenta)
     - **Line overlays**:
       - Allocated capacity (yellow dashed line)
       - RSS (white/gray line)
     - **Event markers**: GC events shown as small triangles/dots on the x-axis
     - **Legend row** at top: colored indicators for each series
     - **Implementation**: Use Unicode braille characters (`⡀⡄⡆⡇⣇⣧⣷⣿` etc.) for high-resolution plotting (~4x pixel resolution vs block characters). Each braille cell is a 2x4 dot grid, providing sub-character precision. Implement a custom `BrailleCanvas` helper that maps data points to braille dot patterns, supporting multiple overlapping series with distinct colors
   - **Class allocation table** (below the chart):
     - Table with columns: `Class`, `Instances`, `Shallow Size`, `Retained Size`
     - Sorted by shallow size descending (configurable)
     - Show top 10 classes (scrollable if more)
     - Format sizes as human-readable (KB/MB)
     - Update on periodic allocation profile fetch (every 5 seconds)

6. **Remove stats section**
   - Delete `stats_section.rs` (or don't create it during Phase 1 split)
   - The stats data (FPS, jank count, GC count) is now embedded in the frame chart summary line and the memory chart legend/events
   - Remove `render_stats_section()` and related constants

7. **Update performance panel layout (`performance/mod.rs`)**
   - Two-section layout:
     - **Frame Timing** section: ~55% of height (bar chart + detail panel)
     - **Memory** section: ~45% of height (line chart + class table)
   - Responsive: if terminal height < 20 lines, show only frame chart with summary
   - If terminal height < 10 lines, show single-line compact summary (existing behavior)

8. **Performance tab key bindings**
   - Arrow Left/Right: navigate between frames in the bar chart
   - `Esc` (when frame selected): deselect frame
   - The existing `r` key still works to refresh data

9. **Update handler for frame selection**
   - In `handler/devtools.rs`, add handlers for:
     - `SelectPerformanceFrame`: update `selected_frame` in `PerformanceState`
     - `AllocationProfileReceived`: store class allocations
     - `MemorySampleReceived`: push to `memory_samples` ring buffer
   - Add periodic allocation profile fetch (every 5 seconds) when Performance panel is active
   - In `handler/keys.rs`, add Left/Right arrow handling when `active_panel == Performance`

**Milestone**: Performance tab shows a real bar chart with individual selectable frames (UI/Raster/Jank/Shader markers), a time-series memory chart with multiple layers and GC event markers, and a class allocation table — matching the core browser DevTools performance experience.

---

### Phase 4: Network Monitor Tab

**Goal**: Add a new Network tab showing HTTP/HTTPS/WebSocket traffic with request/response inspection.

#### Steps

1. **Add network domain types**
   - Create `crates/fdemon-core/src/network.rs` with:
     ```rust
     pub struct NetworkEntry {
         pub id: String,
         pub method: String,        // GET, POST, etc.
         pub uri: String,
         pub status: Option<u16>,   // None while pending
         pub content_type: Option<String>,
         pub start_time: Instant,
         pub end_time: Option<Instant>,
         pub request_size: Option<u64>,
         pub response_size: Option<u64>,
         pub is_websocket: bool,
     }

     pub struct NetworkEntryDetail {
         pub entry: NetworkEntry,
         pub request_headers: Vec<(String, String)>,
         pub response_headers: Vec<(String, String)>,
         pub request_body: Option<String>,  // truncated if large
         pub response_body: Option<String>, // truncated if large
         pub timing: NetworkTiming,
     }

     pub struct NetworkTiming {
         pub dns_ms: Option<f64>,
         pub connect_ms: Option<f64>,
         pub tls_ms: Option<f64>,
         pub send_ms: Option<f64>,
         pub wait_ms: Option<f64>,
         pub receive_ms: Option<f64>,
         pub total_ms: f64,
     }
     ```
   - Add `format_duration()`, `format_size()` helpers

2. **Implement VM Service network extensions**
   - Create `crates/fdemon-daemon/src/vm_service/network.rs`:
     - `get_http_profile(handle, isolate_id, updated_since?)` → `Vec<NetworkEntry>`
     - `get_http_profile_request(handle, isolate_id, request_id)` → `NetworkEntryDetail`
     - `clear_http_profile(handle, isolate_id)` → `Success`
     - `get_socket_profile(handle, isolate_id)` → `Vec<SocketEntry>`
     - `clear_socket_profile(handle, isolate_id)` → `Success`
     - `enable_http_timeline_logging(handle, isolate_id, enabled)` → toggle
   - Parse `HttpProfileRequest` JSON responses into `NetworkEntry` / `NetworkEntryDetail`
   - Handle `updatedSince` parameter for incremental polling (avoid re-fetching all requests)
   - Add `ext.dart.io.*` method constants to `extensions/mod.rs`

3. **Add network state**
   - In `crates/fdemon-app/src/state.rs`, add:
     ```rust
     pub struct NetworkMonitorState {
         pub entries: Vec<NetworkEntry>,
         pub selected_index: Option<usize>,
         pub selected_detail: Option<NetworkEntryDetail>,
         pub recording: bool,
         pub filter: String,
         pub detail_tab: NetworkDetailTab,  // General | Headers | Request | Response | Timing
         pub loading_detail: bool,
         pub last_poll_time: Option<Instant>,
         pub scroll_offset: usize,
     }

     pub enum NetworkDetailTab {
         General,
         Headers,
         RequestBody,
         ResponseBody,
         Timing,
     }
     ```
   - Add `DevToolsPanel::Network` variant
   - Add `NetworkMonitorState` to `DevToolsViewState`

4. **Add network messages**
   - In `message.rs`, add:
     - `HttpProfileReceived { session_id, entries: Vec<NetworkEntry> }`
     - `HttpRequestDetailReceived { session_id, detail: Box<NetworkEntryDetail> }`
     - `HttpRequestDetailFailed { session_id, error: String }`
     - `ClearNetworkProfile { session_id }`
     - `ToggleNetworkRecording`
     - `NetworkSelectRequest { index: Option<usize> }`
     - `NetworkSwitchDetailTab(NetworkDetailTab)`
     - `NetworkFilterChanged(String)`
     - `NetworkNavigate(NetworkNav)` — `Up`, `Down`, `PageUp`, `PageDown`

5. **Implement network handlers**
   - In `handler/devtools/network.rs` (new sub-module, since handler is already split in Phase 1):
     - `handle_http_profile_received()`: merge new entries, update timestamps
     - `handle_http_request_detail_received()`: store detail for selected request
     - `handle_network_select_request()`: update selection, trigger detail fetch
     - `handle_clear_network_profile()`: clear entries, return `ClearHttpProfile` action
     - `handle_toggle_network_recording()`: flip `recording` bool
     - Polling: when Network tab is active and recording, poll `getHttpProfile(updatedSince)` every 1 second
   - Add `UpdateAction::FetchHttpProfile`, `UpdateAction::FetchHttpRequestDetail`, `UpdateAction::ClearHttpProfile` variants

6. **Implement network request table (`network/request_table.rs`)**
   - **NEW** `RequestTable` widget:
     - Scrollable list/table with columns:
       - `Status` — HTTP status code (colored: 2xx green, 3xx cyan, 4xx yellow, 5xx red, pending gray)
       - `Method` — GET/POST/PUT/DELETE/etc. (colored by method)
       - `URI` — truncated to fit, showing path only for long URIs
       - `Type` — content type (json, html, image, etc.)
       - `Duration` — formatted as ms
       - `Size` — response size, human-readable
     - Selected row highlighted
     - Pending requests shown with spinner/animated dots
     - Filter applied: entries not matching filter are hidden
   - Column widths: Status (5), Method (7), Duration (8), Size (8), Type (8), URI (remaining)

7. **Implement request details panel (`network/request_details.rs`)**
   - **NEW** `RequestDetails` widget:
     - Sub-tab bar: `[g] General  [h] Headers  [q] Request  [s] Response  [t] Timing`
     - **General tab**: method, full URI, status, content-type, start/end time, duration, request/response sizes
     - **Headers tab**: request headers (left/top) + response headers (right/bottom), key-value pairs
     - **Request Body tab**: formatted body content (JSON pretty-printed if applicable), or "No request body"
     - **Response Body tab**: formatted body content (JSON pretty-printed, truncated if large)
     - **Timing tab**: waterfall-style breakdown: DNS → Connect → TLS → Send → Wait → Receive, with bar visualization
   - Handle loading state (fetching detail from VM service)
   - Handle "no selection" state

8. **Implement network monitor layout (`network/mod.rs`)**
   - **NEW** `NetworkMonitor` top-level widget:
     - **Wide terminals** (width >= 100): horizontal split — request table (55%) | details panel (45%)
     - **Narrow terminals**: full-width request table; press Enter to view details (replaces table), Esc to go back
     - Header bar: recording indicator (red dot when recording), request count, filter input hint
     - Footer: key hints

9. **Network tab key bindings**
   - `'n'` (from DevTools mode) → switch to Network panel
   - `Up/Down/j/k` → navigate request list
   - `Enter` → open request details (in narrow mode)
   - `Esc` → close details (in narrow mode) or deselect
   - `g/h/q/s/t` → switch detail sub-tabs
   - `Space` → toggle recording on/off
   - `Ctrl+x` → clear all recorded requests
   - `/` → enter filter mode (type filter text)

10. **Integration with Engine**
    - In `engine.rs`, when Network tab is active and recording:
      - Start polling timer (1 second interval)
      - On each poll, call `get_http_profile(updated_since)` via VM service
      - Convert results to `NetworkEntry` and send `HttpProfileReceived` message
    - When user selects a request:
      - Call `get_http_profile_request(id)` for full details including bodies
      - Send `HttpRequestDetailReceived` message
    - On session stop/switch: stop polling, clear network state

**Milestone**: Full Network Monitor tab showing real-time HTTP/WebSocket traffic with searchable request list, detailed inspection of headers/bodies/timing, and recording controls — bringing fdemon to parity with the browser DevTools network view.

---

### Phase 5: Polish, Documentation & Integration Testing

**Goal**: Refine UX edge cases, update documentation, and ensure everything works together.

#### Steps

1. **Cross-panel navigation polish**
   - Ensure smooth transitions between Inspector/Performance/Network tabs
   - Verify state preservation when switching tabs (selected frame, selected request, etc.)
   - Test with very small terminals (< 60 cols, < 15 rows)
   - Test with very large terminals (> 200 cols)

2. **Performance optimization**
   - Profile memory usage with all three panels collecting data simultaneously
   - Ensure network polling doesn't impact frame rate
   - Limit network entry history (configurable, default 500 entries)
   - Lazy-load request details only when selected

3. **Error handling consistency**
   - All three panels handle VM service disconnection gracefully
   - Show informative messages when features unavailable (e.g., no network data in release mode)
   - Reconnection recovers state for all panels

4. **Configuration additions**
   - Add to `.fdemon/config.toml`:
     ```toml
     [devtools.performance]
     # Memory sampling interval (ms)
     memory_sample_interval_ms = 500
     # Memory history duration (samples)
     memory_history_size = 120
     # Allocation profile refresh interval (ms)
     allocation_profile_interval_ms = 5000

     [devtools.network]
     # Max network entries to keep in memory
     max_entries = 500
     # Poll interval for HTTP profile (ms)
     poll_interval_ms = 1000
     # Auto-start recording when entering Network tab
     auto_record = true
     # Max body size to fetch (bytes, 0 = no limit)
     max_body_size = 102400
     ```

5. **Documentation updates**
   - Update `docs/KEYBINDINGS.md` with new DevTools sub-tab keys
   - Update `docs/ARCHITECTURE.md` with new widget directory structure
   - Update `CLAUDE.md` project structure section if needed

6. **Comprehensive testing**
   - Add tests for all new widgets (target: 20+ tests per widget file)
   - Add handler tests for all new message types
   - Add VM service parsing tests for network data
   - Integration test: verify full DevTools flow (enter mode → switch tabs → interact → exit)

**Milestone**: Production-ready DevTools V2 with polished UX, comprehensive test coverage, and up-to-date documentation.

---

## Edge Cases & Risks

### Inspector + Layout Merge

- **Risk**: Not all widgets have padding/margin data available in diagnostics
- **Mitigation**: Box model view gracefully degrades — shows only dimensions/constraints when padding unavailable; never shows empty padding section

- **Risk**: Auto-fetching layout on every tree navigation may cause excessive RPC calls
- **Mitigation**: 500ms debounce on layout fetch; cancel pending fetch if selection changes again; staleness check prevents re-fetching same node

### Performance Bar Chart

- **Risk**: Terminal character-based bar chart has limited resolution compared to pixel-based browser chart
- **Mitigation**: Use half-block characters (`▄▀`) and braille patterns for sub-character resolution; accept that TUI resolution is inherently lower but still clearly communicates jank/timing patterns

- **Risk**: Frame phase breakdown (build/layout/paint) requires timeline events that may not always be available
- **Mitigation**: Phase breakdown is `Option<FramePhases>` — show "N/A" when unavailable; still show total UI/Raster split which is always available

- **Risk**: Memory chart with multiple stacked layers in ASCII may be hard to read
- **Mitigation**: Use distinct Unicode block characters and colors; provide legend; consider simplified mode (single line per metric) for narrow terminals

### Network Monitor

- **Risk**: `ext.dart.io.getHttpProfile` is only available for `dart:io`-based HTTP clients; some Flutter web requests or third-party native HTTP clients won't be captured
- **Mitigation**: Document this limitation clearly; show "No network activity detected" with a hint about supported clients

- **Risk**: Response bodies can be very large, causing memory issues
- **Mitigation**: Configurable `max_body_size` (default 100KB); truncate with "[truncated]" indicator; only fetch body on demand (when user views Response tab)

- **Risk**: High-traffic apps may produce thousands of requests
- **Mitigation**: Configurable `max_entries` (default 500, FIFO eviction); efficient incremental polling with `updatedSince`; filter to reduce visible entries

- **Risk**: WebSocket profiling may not be available on all Dart versions
- **Mitigation**: Check `isSocketProfilingAvailable()` before attempting; gracefully omit WebSocket entries if unavailable

### General

- **Risk**: Handler file (`devtools.rs`) at 1,516 lines will grow further with network handlers
- **Mitigation**: Split into `handler/devtools/mod.rs`, `handler/devtools/inspector.rs`, `handler/devtools/performance.rs`, `handler/devtools/network.rs` during Phase 4 if it exceeds 2,000 lines

- **Risk**: Adding three new ring buffers (memory samples, network entries, frame phases) increases per-session memory footprint
- **Mitigation**: All buffers are bounded with configurable sizes; total additional memory ~50KB per session for default settings

---

## Keyboard Shortcuts Summary

### DevTools Mode (after pressing `d`)

| Key | Action | Phase |
|-----|--------|-------|
| `i` | Switch to Inspector panel | Existing |
| `p` | Switch to Performance panel | Existing |
| `n` | Switch to Network panel | Phase 4 |
| `Esc` | Return to log view | Existing |
| `b` | Open browser DevTools | Existing |
| `q` | Quit application | Existing |

### Inspector Panel

| Key | Action | Phase |
|-----|--------|-------|
| `Up/k` | Navigate tree up | Existing |
| `Down/j` | Navigate tree down | Existing |
| `Right/Enter` | Expand node | Existing |
| `Left/h` | Collapse node | Existing |
| `r` | Refresh widget tree | Existing |
| `Ctrl+r` | Toggle repaint rainbow | Existing |
| `Ctrl+p` | Toggle performance overlay | Existing |
| `Ctrl+d` | Toggle debug paint | Existing |

### Performance Panel

| Key | Action | Phase |
|-----|--------|-------|
| `Left` | Select previous frame | Phase 3 |
| `Right` | Select next frame | Phase 3 |
| `Esc` | Deselect frame | Phase 3 |

### Network Panel

| Key | Action | Phase |
|-----|--------|-------|
| `Up/Down/j/k` | Navigate request list | Phase 4 |
| `Enter` | View request details (narrow mode) | Phase 4 |
| `Esc` | Close details / deselect | Phase 4 |
| `g/h/q/s/t` | Switch detail sub-tabs | Phase 4 |
| `Space` | Toggle recording | Phase 4 |
| `Ctrl+x` | Clear recorded requests | Phase 4 |
| `/` | Filter requests | Phase 4 |

---

## Success Criteria

### Phase 1 Complete When:
- [ ] `inspector.rs` split into `inspector/{mod,tree_panel,details_panel}.rs` (each < 400 lines)
- [ ] `performance.rs` split into `performance/{mod,frame_section,memory_section,stats_section,styles}.rs`
- [ ] `handler/devtools.rs` split into `handler/devtools/{mod,inspector,performance}.rs`
- [ ] All 80+ existing devtools widget tests pass unchanged
- [ ] All 57 handler tests pass unchanged
- [ ] `cargo clippy --workspace` clean
- [ ] Visual output identical to pre-refactor

### Phase 2 Complete When:
- [ ] Inspector and Layout tabs merged into single Inspector tab
- [ ] `DevToolsPanel::Layout` variant removed
- [ ] `'l'` keybinding removed from DevTools mode
- [ ] Widget tree and Layout Explorer shown in 50/50 split
- [ ] Responsive: horizontal split (wide) / vertical split (narrow)
- [ ] Layout Explorer shows box model visualization with dimensions, padding
- [ ] Layout auto-fetches on tree node selection (with debounce)
- [ ] Source location (file:line) displayed in layout panel
- [ ] All new code has unit tests (20+ new tests)
- [ ] Old `layout_explorer.rs` file deleted

### Phase 3 Complete When:
- [ ] Frame timing uses bar chart (not sparkline)
- [ ] Each frame shows UI + Raster bars
- [ ] Jank frames highlighted in red, shader compilation in magenta
- [ ] Frame budget line (16ms) displayed
- [ ] Frames selectable with Left/Right keys
- [ ] Selected frame shows phase breakdown (build/layout/paint/raster)
- [ ] Memory uses time-series chart (not gauge)
- [ ] Memory chart shows Dart Heap, Native, Raster Cache, Allocated, RSS layers
- [ ] GC events marked on memory chart
- [ ] Class allocation table shown below memory chart
- [ ] Stats section removed
- [ ] All new code has unit tests (30+ new tests)

### Phase 4 Complete When:
- [ ] Network tab accessible via `'n'` key in DevTools mode
- [ ] HTTP requests displayed in scrollable table (method, URI, status, duration, size)
- [ ] Selecting a request shows detailed info (headers, body, timing)
- [ ] Recording can be toggled on/off
- [ ] Request history can be cleared
- [ ] Filter by method, status, or free text
- [ ] Pending requests shown with indicator
- [ ] WebSocket entries shown when available
- [ ] VM Service `ext.dart.io.*` extensions properly called
- [ ] Responsive layout (wide: table + details side-by-side; narrow: stacked)
- [ ] All new code has unit tests (30+ new tests)

### Phase 5 Complete When:
- [ ] All panels handle VM service disconnection gracefully
- [ ] Configuration options work for performance and network
- [ ] `docs/KEYBINDINGS.md` updated
- [ ] `docs/ARCHITECTURE.md` updated
- [ ] No regressions in existing functionality
- [ ] Full quality gate passes: `cargo fmt && cargo check && cargo test && cargo clippy`

---

## Future Enhancements

After DevTools V2 is complete, consider:

1. **CPU Profiler tab** — sampling profiler with flame chart visualization
2. **Widget selection sync** — select widget in fdemon, highlight on device
3. **Interactive flex editing** — modify flex properties from TUI (like browser DevTools dropdowns)
4. **Network request replay** — re-send a captured request with modifications
5. **Memory leak detection** — diff snapshots, retaining path analysis
6. **Timeline recording** — record and replay performance sessions
7. **Export/Import** — export performance data and network logs for sharing

---

## References

- [Flutter Inspector Documentation](https://docs.flutter.dev/tools/devtools/inspector)
- [Flutter Performance View](https://docs.flutter.dev/tools/devtools/performance)
- [Flutter Memory View](https://docs.flutter.dev/tools/devtools/memory)
- [Flutter Network View](https://docs.flutter.dev/tools/devtools/network)
- [DartIOExtension API](https://api.flutter.dev/flutter/vm_service/DartIOExtension.html)
- [HttpProfileRequest class](https://pub.dev/documentation/vm_service/latest/vm_service/HttpProfileRequest-class.html)
- [Dart VM Service Protocol](https://github.com/dart-lang/sdk/blob/main/runtime/vm/service/service.md)
- [Flutter Service Extensions](https://github.com/flutter/flutter/blob/main/packages/flutter/lib/src/widgets/service_extensions.dart)
- [DevTools Source Code](https://github.com/flutter/devtools)
- [Original DevTools Integration Plan](../devtools-integration/PLAN.md)
