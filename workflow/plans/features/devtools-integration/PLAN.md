# Plan: Full DevTools Integration via VM Service Protocol

## TL;DR

Integrate Flutter DevTools capabilities directly into Flutter Demon by connecting to the Dart VM Service Protocol over WebSocket. This enables access to the Widget Inspector, Widget Tree, Layout Explorer, Performance Overlay, and Memory Usage—all consumed and displayed within the terminal UI, eliminating the need to open a separate browser-based DevTools window.

---

## Background

When a Flutter app runs in debug mode, it exposes a VM Service endpoint via WebSocket (the `ws_uri` from the `app.debugPort` event). This service provides:

1. **VM Service Protocol** (v4.x) - Core Dart VM introspection
2. **Flutter Service Extensions** - Flutter-specific debugging features prefixed with `ext.flutter.*`

Currently, Flutter Demon captures the `ws_uri` but only uses it to open DevTools in a browser. This feature will establish a direct WebSocket connection to access these powerful debugging capabilities natively.

---

## Research Findings

### VM Service Protocol (Core)

The Dart VM Service Protocol provides JSON-RPC 2.0 over WebSocket. Key methods:

| Method | Description |
|--------|-------------|
| `getVM` | Get VM information (version, isolates) |
| `getIsolate` | Get isolate details (libraries, classes) |
| `getMemoryUsage` | Get heap memory statistics |
| `getAllocationProfile` | Get GC allocation profile |
| `getScripts` | List loaded scripts |
| `streamListen` / `streamCancel` | Subscribe to event streams |

### Flutter Service Extensions

Called via `callServiceExtension(method, isolateId, args)`:

| Extension | Description |
|-----------|-------------|
| `ext.flutter.inspector.structuredErrors` | Enable/get structured error info |
| `ext.flutter.inspector.show` | Toggle widget inspector overlay |
| `ext.flutter.inspector.getRootWidgetSummaryTree` | Get widget tree structure |
| `ext.flutter.inspector.getDetailsSubtree` | Get detailed subtree for a widget |
| `ext.flutter.inspector.getLayoutExplorerNode` | Get layout/flex info for a widget |
| `ext.flutter.inspector.getSelectedWidget` | Get currently selected widget |
| `ext.flutter.repaintRainbow` | Toggle repaint rainbow overlay |
| `ext.flutter.debugPaint` | Toggle debug paint overlay |
| `ext.flutter.showPerformanceOverlay` | Toggle performance overlay |
| `ext.flutter.debugDumpApp` | Dump widget tree to string |
| `ext.flutter.debugDumpRenderTree` | Dump render tree to string |
| `ext.flutter.debugDumpLayerTree` | Dump layer tree to string |

### WebSocket Connection Flow

```
1. Flutter app starts → emits app.debugPort event with ws_uri
2. Flutter Demon connects to ws_uri via WebSocket
3. Call getVM → get list of isolates
4. Call getIsolate → get main isolate ID
5. Call streamListen("Extension") → receive service extension events
6. Call Flutter service extensions as needed
```

### Crate Dependencies

| Crate | Purpose |
|-------|---------|
| `tokio-tungstenite` | Async WebSocket client |
| `futures-util` | Stream utilities for WebSocket |
| `serde_json` | JSON-RPC serialization |

---

## Affected Modules

- `src/daemon/events.rs` - Already has `AppDebugPort` with `ws_uri`
- `src/services/state_service.rs` - Already stores `devtools_uri`
- **NEW** `src/vmservice/mod.rs` - VM Service client module
- **NEW** `src/vmservice/client.rs` - WebSocket client implementation
- **NEW** `src/vmservice/protocol.rs` - VM Service JSON-RPC types
- **NEW** `src/vmservice/extensions.rs` - Flutter service extension wrappers
- **NEW** `src/core/widget_tree.rs` - Widget tree data models
- **NEW** `src/core/performance.rs` - Performance/memory data models
- **NEW** `src/tui/widgets/widget_inspector.rs` - Widget inspector TUI widget
- **NEW** `src/tui/widgets/layout_explorer.rs` - Layout explorer TUI widget
- **NEW** `src/tui/widgets/performance_panel.rs` - Performance panel TUI widget
- `src/app/state.rs` - Add DevTools panel state
- `src/app/message.rs` - Add DevTools-related messages
- `src/tui/render.rs` - Add DevTools panel rendering
- `src/tui/actions.rs` - Add DevTools keyboard actions
- `Cargo.toml` - Add WebSocket dependencies

---

## Development Phases

### Phase 1: VM Service Client Foundation

**Goal**: Establish WebSocket connection and basic VM Service communication.

**Duration**: 1-2 weeks

#### Steps

1. **Add WebSocket Dependencies**
   - Add `tokio-tungstenite` and `futures-util` to Cargo.toml
   - Verify async WebSocket works with tokio runtime

2. **Create VM Service Client Module**
   - `src/vmservice/mod.rs` - Module exports
   - `src/vmservice/client.rs` - `VmServiceClient` struct
   - Implement `connect(ws_uri)` async method
   - Implement `disconnect()` with graceful shutdown
   - Handle reconnection on connection loss

3. **Implement JSON-RPC Protocol**
   - `src/vmservice/protocol.rs` - Request/Response types
   - Implement request ID tracking (like daemon protocol)
   - Create typed response parsing
   - Handle streaming events

4. **Basic VM Introspection**
   - Implement `get_vm()` → `VmInfo`
   - Implement `get_isolate(id)` → `IsolateInfo`
   - Implement `stream_listen(stream)` → subscribe to events
   - Store main isolate ID for service extension calls

5. **Integration with Session**
   - Auto-connect when `app.debugPort` event received
   - Store `VmServiceClient` in session state
   - Disconnect on session stop
   - Handle connection errors gracefully

**Milestone**: Flutter Demon connects to VM Service and can query basic VM/isolate info.

---

### Phase 2: Flutter Service Extensions

**Goal**: Implement wrappers for Flutter-specific service extensions.

**Duration**: 1-2 weeks

#### Steps

1. **Service Extension Framework**
   - `src/vmservice/extensions.rs` - Extension method wrappers
   - Implement `call_service_extension(method, args)` generic method
   - Parse common response formats
   - Handle extension not available errors

2. **Debug Overlay Extensions**
   - `toggle_repaint_rainbow()` → bool
   - `toggle_debug_paint()` → bool
   - `toggle_performance_overlay()` → bool
   - `toggle_widget_inspector()` → bool
   - Track current state for UI indicators

3. **Widget Tree Extensions**
   - `get_root_widget_summary_tree()` → `WidgetTree`
   - `get_details_subtree(widget_id)` → `WidgetDetails`
   - `get_selected_widget()` → `SelectedWidget`
   - Create `src/core/widget_tree.rs` data models

4. **Layout Explorer Extensions**
   - `get_layout_explorer_node(widget_id)` → `LayoutNode`
   - Parse flex properties (mainAxis, crossAxis, etc.)
   - Parse constraint info (min/max width/height)
   - Parse actual size and position

5. **Debug Dump Extensions**
   - `debug_dump_app()` → String (widget tree dump)
   - `debug_dump_render_tree()` → String
   - `debug_dump_layer_tree()` → String
   - Useful for text-based debugging output

**Milestone**: All major Flutter service extensions accessible from Flutter Demon.

---

### Phase 3: Performance & Memory Monitoring

**Goal**: Real-time performance metrics and memory usage display.

**Duration**: 1.5-2 weeks

#### Steps

1. **Memory Usage Monitoring**
   - Implement `get_memory_usage(isolate_id)` → `MemoryUsage`
   - Create `src/core/performance.rs` data models
   - Track: heapUsage, heapCapacity, externalUsage
   - Periodic polling (configurable interval, default 1s)

2. **Allocation Profile**
   - Implement `get_allocation_profile(isolate_id)` → `AllocationProfile`
   - Track memory allocations by class
   - Calculate allocation rate
   - Identify potential memory leaks

3. **Performance Metrics**
   - Subscribe to timeline events for frame timing
   - Calculate FPS from frame timing data
   - Track UI thread vs raster thread time
   - Identify janky frames (>16ms budget)

4. **Data Aggregation Service**
   - Create background task for periodic data collection
   - Store rolling history (last N samples)
   - Calculate averages, min, max, percentiles
   - Emit data via message channel to TUI

**Milestone**: Real-time memory and performance data flowing to Flutter Demon.

---

### Phase 4: TUI Widget Inspector Panel

**Goal**: Display widget tree and layout info in terminal UI.

**Duration**: 2-3 weeks

#### Steps

1. **Widget Inspector Panel**
   - `src/tui/widgets/widget_inspector.rs`
   - Tree view widget using ratatui's `List` or custom tree
   - Expand/collapse tree nodes with Enter or arrow keys
   - Display widget type, key, and essential properties
   - Highlight selected widget

2. **Widget Details View**
   - Side panel or expandable section for widget details
   - Show all diagnostic properties
   - Show render object info
   - Show constraints and size

3. **Layout Explorer Panel**
   - `src/tui/widgets/layout_explorer.rs`
   - ASCII visualization of flex layouts
   - Show main axis, cross axis directions
   - Show flex factors and alignment
   - Visual representation of constraints

4. **Performance Panel**
   - `src/tui/widgets/performance_panel.rs`
   - FPS sparkline graph (ASCII art)
   - Memory usage bar/gauge
   - Frame timing histogram
   - Jank indicator with threshold

5. **UI Mode & Navigation**
   - Add `UiMode::DevTools` with sub-modes (Inspector, Layout, Performance)
   - Keyboard shortcuts: `i` Inspector, `l` Layout, `p` Performance
   - Tab switching between DevTools panels
   - `Escape` to return to normal mode

6. **Keyboard Shortcuts for Debug Overlays**
   - `Ctrl+r` - Toggle repaint rainbow
   - `Ctrl+p` - Toggle performance overlay on device
   - `Ctrl+d` - Toggle debug paint
   - Show current overlay state in status bar

**Milestone**: Full DevTools functionality accessible from terminal UI.

---

### Phase 5: Polish & Optimization

**Goal**: Refine UX, handle edge cases, optimize performance.

**Duration**: 1-2 weeks

#### Steps

1. **Connection Resilience**
   - Auto-reconnect on WebSocket disconnect
   - Graceful degradation when VM service unavailable
   - Clear UI indicators for connection state
   - Timeout handling for slow responses

2. **Performance Optimization**
   - Lazy loading of widget tree (load on demand)
   - Debounce rapid refresh requests
   - Efficient tree diffing to minimize redraws
   - Memory-efficient data structures

3. **Error Handling**
   - User-friendly error messages
   - Fallback UI when features unavailable
   - Log detailed errors for debugging

4. **Configuration**
   - Add `[devtools]` section to config
   - Configurable refresh intervals
   - Default panel preferences
   - Overlay toggle defaults

5. **Documentation**
   - README section on DevTools features
   - Keyboard shortcut reference
   - Troubleshooting guide

**Milestone**: Production-ready DevTools integration.

---

## Edge Cases & Risks

### Connection Management
- **Risk**: WebSocket connection drops during long session
- **Mitigation**: Implement auto-reconnect with exponential backoff; clear UI state on disconnect

### Isolate Handling
- **Risk**: Multiple isolates (e.g., Dart isolates for background work)
- **Mitigation**: Track main UI isolate; allow isolate selection in UI

### Extension Availability
- **Risk**: Some extensions not available in profile/release mode
- **Mitigation**: Gracefully disable features; show mode indicator in UI

### Large Widget Trees
- **Risk**: Apps with thousands of widgets cause slow tree fetches
- **Mitigation**: Fetch tree lazily; limit depth; add loading indicators

### Memory Usage
- **Risk**: Storing too much performance history causes memory bloat
- **Mitigation**: Configurable history size; use ring buffers

### Cross-Platform Compatibility
- **Risk**: WebSocket library behavior differences across platforms
- **Mitigation**: Test on macOS, Linux, Windows; use well-maintained crate

---

## Configuration

Add to `.fdemon/config.toml`:

```toml
[devtools]
# Auto-connect to VM service when app starts
auto_connect = true

# Default panel when opening DevTools mode
default_panel = "inspector"  # "inspector", "layout", "performance"

# Performance data refresh interval (milliseconds)
performance_refresh_ms = 1000

# Memory history size (number of samples to keep)
memory_history_size = 60

# Widget tree max depth (0 = unlimited)
tree_max_depth = 0

# Auto-enable debug overlays on connect
auto_repaint_rainbow = false
auto_performance_overlay = false
```

---

## New Dependencies

Add to `Cargo.toml`:

```toml
# WebSocket client
tokio-tungstenite = { version = "0.26", features = ["native-tls"] }
futures-util = "0.3"

# HTTP client for initial connection (if needed)
reqwest = { version = "0.12", features = ["json"], optional = true }
```

---

## Success Criteria

### Phase 1 Complete When:
- [ ] WebSocket connection established to VM service
- [ ] `getVM` and `getIsolate` calls return valid data
- [ ] Connection handles gracefully with session lifecycle
- [ ] Reconnection works after brief disconnects

### Phase 2 Complete When:
- [ ] All debug overlay toggles work from Flutter Demon
- [ ] Widget tree data retrieved successfully
- [ ] Layout explorer data parsed correctly
- [ ] Debug dump commands return valid output

### Phase 3 Complete When:
- [ ] Memory usage displayed in real-time
- [ ] FPS/frame timing calculated and tracked
- [ ] Performance data persisted for history view
- [ ] Data collection doesn't impact TUI performance

### Phase 4 Complete When:
- [ ] Widget inspector tree renders in terminal
- [ ] Widget details shown for selected widget
- [ ] Layout explorer visualizes flex layouts
- [ ] Performance panel shows graphs/gauges
- [ ] All keyboard shortcuts functional

### Phase 5 Complete When:
- [ ] Connection is resilient to network issues
- [ ] Large widget trees load efficiently
- [ ] All configuration options work
- [ ] Documentation is complete
- [ ] Works on macOS, Linux, and Windows

---

## Future Enhancements

After core DevTools integration is complete, consider:

1. **Widget Selection Sync** - Select widget in Flutter Demon, highlight on device
2. **Hot UI Editing** - Modify widget properties from Flutter Demon (experimental)
3. **Timeline Recording** - Record and analyze performance over time
4. **Network Inspector** - View HTTP requests (requires additional hooks)
5. **Logging Integration** - Enhanced log filtering based on widget hierarchy

---

## References

- [Dart VM Service Protocol](https://github.com/dart-lang/sdk/blob/main/runtime/vm/service/service.md)
- [Flutter Service Extensions](https://github.com/flutter/flutter/blob/main/packages/flutter/lib/src/widgets/service_extensions.dart)
- [DevTools Source Code](https://github.com/flutter/devtools)
- [tokio-tungstenite Documentation](https://docs.rs/tokio-tungstenite)
- [Ratatui Documentation](https://ratatui.rs/)