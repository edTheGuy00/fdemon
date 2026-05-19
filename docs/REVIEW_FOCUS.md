# Project-Specific Review Focus

This document defines project-specific concerns that code reviewers should pay special attention to when reviewing changes to this codebase.

## Architectural Concerns

### TEA Pattern (The Elm Architecture)

This project uses the TEA pattern for state management. Watch for:

| Concern | What to Check |
|---------|---------------|
| **Side effects in update()** | The `update()` function should be pure; side effects return via `UpdateAction` |
| **Direct state mutation** | State should only change through `handler::update()`, never directly |
| **View function purity** | `tui::render()` should only read state, never mutate (see [Approved Exception](#approved-tea-exception-render-hint-feedback) below) |
| **Message routing** | All events must be routed through the `Message` enum |

### Approved TEA Exception: Render-Hint Feedback

`Cell<usize>` fields on state types are permitted as a narrow exception to view purity
when they carry **render-derived layout metrics** (e.g., visible row counts) back to the
handler layer. These fields:

- Must only carry numeric hints that improve scroll/layout accuracy
- Must not affect logical application state or business rules
- Must not participate in state equality comparisons or serialization
- Must default to a safe fallback (e.g., 0) so the handler works without any render

**Current usage:**
- `TargetSelectorState::last_known_visible_height` — the renderer writes the actual device list area height each frame; the handler reads it for scroll calculations.
- `AppState::mouse_regions: MouseRegionsCell` — the renderer populates a fresh `MouseRegions` registry each frame (header shortcuts, session tabs, device pill); `handler/mouse/normal.rs::handle_press` reads it for click hit-tests. Wrapped in a `MouseRegionsCell` newtype to satisfy `#[derive(Debug)]` on `AppState` (since `Cell<T>: Debug` requires `T: Copy`, which `MouseRegions` cannot be).
- `TagFilterUiState::last_known_scroll_offset` — the renderer writes ratatui's `ListState.offset()` each frame after `render_stateful_widget`; the region recorder reads it to convert screen-row numbers to absolute tag indices for click region registration. Default 0 (safe fallback when no render has happened yet).
- `MemoryState::memory_chart_visible_width` — the renderer writes the actual chart plot width (in columns) each frame; the chart-scroll handler reads it to clamp `memory_chart_scroll_offset` against the latest geometry. Default 0 (safe fallback when no render has happened yet).
- `MemoryState::alloc_table_visible_height` — the renderer writes the visible data-row count (excluding header) each frame; the alloc-table page and jump handlers read it to size page-step and end-of-list navigation. Default 0 (safe fallback when no render has happened yet).
- `PerformanceState::details_pane_visible_height` — the renderer writes the inner details-pane height (excluding borders) each frame; Phase 3 Rebuild Stats and Timeline Events scroll handlers read it. Default 0 (safe fallback when no render has happened yet).
- `PerformanceState::frame_chart_visible_width` — the renderer writes the visible bar count each frame; the chart-scroll, page, and jump handlers read it to clamp `frame_chart_scroll_offset` and size page-step navigation. Default 0 (safe fallback when no render has happened yet).
- `PerformanceState::timeline_visible_row_count` — the Gantt renderer writes the actual visible thread-row count each frame; the `↑/↓` timeline thread-row scroll handler reads it to bound `timeline_thread_scroll_offset`. Default 0 (safe fallback when no render has happened yet). Write site annotated with the standard `// EXCEPTION:` comment in `timeline_events/mod.rs`.

New `Cell`-based render-hint fields require explicit review and documentation here.

### Layer Boundary Violations

Watch for imports that violate the layered architecture:

| Layer | Should NOT Import From |
|-------|----------------------|
| `core/` | Any other layer (pure domain types) |
| `tui/` | `daemon/`, `app/` (except via messages) |
| `daemon/` | `tui/`, `app/`, `services/` |
| `config/` | `daemon/`, `tui/`, `app/`, `services/` |

See `docs/ARCHITECTURE.md` for the complete dependency matrix.

## Concurrency Concerns

### Session State

- Race conditions between multiple device sessions
- Session manager operations should be thread-safe
- Check for potential deadlocks when accessing shared state

### File Watcher

- Debouncing logic for rapid file changes
- Missed events during high activity
- Watch path validation

### JSON-RPC Communication

- Response matching with correct request IDs
- Timeout handling for unresponsive daemon
- Request tracking cleanup on session close

## Terminal/TUI Concerns

### Terminal State Management

- Proper cleanup on panic/error paths
- Alternate screen restoration
- Raw mode exit handling
- Signal handlers (SIGINT/SIGTERM)

### Rendering

- No blocking operations in render loop
- Efficient redraws (only when state changes)
- Proper terminal resize handling

## Error Handling Concerns

### Common Anti-Patterns

| Pattern | Risk |
|---------|------|
| `unwrap()` without justification | Panic in production |
| Swallowed errors (`let _ = ...`) | Silent failures |
| String errors instead of typed | Poor error context |
| Missing `.context()` on errors | Hard to debug |

### Required Patterns

- Use `Error` enum from `common/error.rs`
- Use `Result<T>` type alias from prelude
- Classify errors as `fatal` vs `recoverable`
- Add context with `.context()` or `.with_context()`

## Approved Optimizations

### Forwarder Panel Gate (`forward_vm_events`)

`forward_vm_events` in `fdemon-app/src/actions/vm_service.rs` consults a `watch::Receiver<bool>` (`rebuilt_widgets_gate_rx`) before parsing any `Flutter.RebuiltWidgets` event. When the value is `false`, the branch calls `continue` without parsing or allocating. This is an intentional early-return optimization, not a logic error: `Flutter.RebuiltWidgets` events arrive at ~60 fps and are only meaningful when the Performance panel is visible. The gate is managed by `handle_switch_panel` and `handle_exit_devtools_mode`.

### `try_send` for `Flutter.RebuiltWidgets` (Backpressure)

`Flutter.RebuiltWidgets` events are forwarded to the TEA handler via `msg_tx.try_send(...)` rather than `.send().await`. This is the canonical backpressure strategy for high-frequency VM Service events: if the handler is slow and the channel is full, the current frame is dropped and logged at `debug` level, preventing head-of-line blocking of lower-volume events (`Flutter.Frame`, error events). `TrySendError::Closed` exits the loop. Do not change this to `.send().await` without understanding the throughput implications.

### `pub(super)` Module Boundary: `text_helpers`

`fdemon-tui/src/widgets/devtools/performance/details/text_helpers.rs` is declared `pub(super)` and all its exports (`truncate_with_ellipsis`, `pad_right`, `pad_left`, `PLACEHOLDER_LINE_COUNT`) are also `pub(super)`. This is intentional: the helpers are shared across sibling tab renderers within the `details` module but must not leak to the broader `widgets` hierarchy. Future helpers added to this module must keep the same visibility.

### Gantt Depth-Stacked Rendering

Phase 4: depth-stacked timeline event rendering follows DevTools' legacy `FlameChart` pattern — depth-N child events render at row `Y+N` within their parent's row band. This is an approved exception to "one widget = one rectangular region" because depth math is bounded by `MAX_DEPTH` and the renderer always honors `Layout::vertical` parent constraints. Reviewers should not flag this.

### Thread-Row Scroll Offset Semantics

Phase 4: `timeline_thread_scroll_offset` measures scroll position in **thread rows**, not event lines. The Gantt has no event-level selection in Phase 4, so the scroll target is the thread row itself. Phase 5 may add event-level selection within rows.

### Full-Column Frame-Chart Selection Overlay

Phase 4: the frame chart's selected bar is rendered with a full-column overlay (side-marker characters `▏`/`▕` across every chart row), not a single-character tip. This is an approved replacement for the Phase 1 single-`▔` highlight, which was visually invisible.

### Phase 5 Deferred Scope (Timeline Gantt)

Pan/zoom, minimap, event-level selection, search/filter by name, and CPU sample overlays in the Timeline Gantt view are deferred to Phase 5. Reviewers seeing PRs touching `timeline_events/` should expect a fixed-viewport rendering in Phase 4 and a configurable viewport in Phase 5. Absence of these features is intentional and should not be flagged.

## Performance Concerns

### Hot Paths

Pay extra attention to performance in:

- Log parsing and filtering (high volume)
- Terminal rendering loop
- File watcher event processing
- JSON-RPC message parsing

### Memory

- Log buffer size limits
- Cleanup of old sessions
- Stack trace storage

## Testing Concerns

### What Must Have Tests

- All new public functions
- State transition logic
- Message handlers
- Error paths

### Test Patterns

- Use `tempdir()` for file-based tests
- No shared mutable state between tests
- Descriptive test names: `test_<function>_<scenario>_<expected_result>`

## Common Red Flags

| Red Flag | Why It's Concerning |
|----------|---------------------|
| Index-based operations without bounds check | Panic on empty/short collections |
| Spawned tasks without error handling | Silent failures |
| String-based field matching | Typos cause silent failures |
| No concurrent access consideration | Race conditions |
| External file operations without locking | Data corruption |
| Early returns that skip cleanup | Resource leaks |
| Magic numbers without constants | Maintenance burden |

## Module-Specific Concerns

### `app/`

- TEA pattern compliance
- Message exhaustiveness
- State consistency after updates

### `daemon/`

- Process lifecycle management
- Stream handling (stdout/stderr)
- Graceful shutdown

### `tui/`

- Widget state isolation
- Render performance
- Input handling edge cases

### `config/`

- TOML parsing error messages
- Default value handling
- Migration from old configs

### `watcher/`

- Path resolution
- Event coalescing
- Error recovery