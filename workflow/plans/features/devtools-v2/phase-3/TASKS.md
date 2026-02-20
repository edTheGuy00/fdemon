# Phase 3: Performance Tab Overhaul - Task Index

## Overview

Replace the sparkline and gauge with a proper frame bar chart and time-series memory chart. Remove the stats section. Add frame selection with phase breakdown and class allocation table.

**Total Tasks:** 9
**Waves:** 5 (01 solo, then 02+03 parallel, then 04+05+06 parallel, then 07+08 parallel, then 09 solo)

## Task Dependency Graph

```
Wave 1
┌───────────────────────────────────────┐
│ 01-extend-core-performance-types      │
│ (fdemon-core)                         │
└──────────────────┬────────────────────┘
                   │
Wave 2 (parallel — different crates)
        ┌──────────┴──────────────────────────────┐
        ▼                                         ▼
┌──────────────────────────────────┐  ┌───────────────────────────────────────┐
│ 02-extend-performance-state-msgs │  │ 03-extend-vm-service-perf-collection  │
│ (fdemon-app)                     │  │ (fdemon-daemon)                       │
│ depends: 01                      │  │ depends: 01                           │
└──────────────┬───────────────────┘  └──────────────────┬────────────────────┘
               │                                         │
Wave 3 (parallel — different files/crates)               │
        ┌──────┼──────────────────────┐                  │
        ▼      ▼                      ▼                  │
┌────────────────────────┐ ┌────────────────────────┐    │
│ 05-frame-bar-chart     │ │ 06-memory-chart-widget │    │
│ (fdemon-tui, new file) │ │ (fdemon-tui, new file) │    │
│ depends: 01, 02        │ │ depends: 01, 02        │    │
└───────────┬────────────┘ └───────────┬────────────┘    │
            │                          │                  │
            │  ┌──────────────────────────────────────┐  │
            │  │ 04-perf-handler-and-key-bindings     │  │
            │  │ (fdemon-app handler/)                 │  │
            │  │ depends: 02                           │  │
            │  └──────────────┬───────────────────────┘  │
            │                 │                           │
Wave 4 (parallel — different crates/files)               │
            └────────┬────────┘                           │
                     ▼                                    │
     ┌───────────────────────────────────┐                │
     │ 07-rewire-perf-panel-remove-stats │                │
     │ (fdemon-tui performance/mod.rs)   │                │
     │ depends: 04, 05, 06              │                │
     └──────────────┬────────────────────┘                │
                    │                                     │
                    │  ┌──────────────────────────────────┘
                    │  ▼
                    │  ┌───────────────────────────────────┐
                    │  │ 08-wire-alloc-polling-memory-flow │
                    │  │ (fdemon-app engine/actions)       │
                    │  │ depends: 03, 04                   │
                    │  └──────────────┬────────────────────┘
                    │                 │
Wave 5              └────────┬────────┘
                             ▼
              ┌─────────────────────────────────┐
              │ 09-final-test-and-cleanup       │
              │ (workspace-wide)                │
              │ depends: 07, 08                 │
              └─────────────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Crate | Modules |
|---|------|--------|------------|-------|---------|
| 1 | [01-extend-core-performance-types](tasks/01-extend-core-performance-types.md) | Done | - | `fdemon-core` | `performance.rs`, `lib.rs` |
| 2 | [02-extend-performance-state-and-messages](tasks/02-extend-performance-state-and-messages.md) | Done | 1 | `fdemon-app` | `session/performance.rs`, `message.rs`, `state.rs` |
| 3 | [03-extend-vm-service-performance-collection](tasks/03-extend-vm-service-performance-collection.md) | Done | 1 | `fdemon-daemon` | `vm_service/performance.rs`, `vm_service/timeline.rs` |
| 4 | [04-add-performance-handler-and-key-bindings](tasks/04-add-performance-handler-and-key-bindings.md) | Done | 2 | `fdemon-app` | `handler/devtools/performance.rs`, `handler/devtools/mod.rs`, `handler/keys.rs` |
| 5 | [05-implement-frame-bar-chart-widget](tasks/05-implement-frame-bar-chart-widget.md) | Done | 1, 2 | `fdemon-tui` | `widgets/devtools/performance/frame_chart.rs` (NEW) |
| 6 | [06-implement-memory-chart-widget](tasks/06-implement-memory-chart-widget.md) | Done | 1, 2 | `fdemon-tui` | `widgets/devtools/performance/memory_chart.rs` (NEW) |
| 7 | [07-rewire-performance-panel-remove-stats](tasks/07-rewire-performance-panel-remove-stats.md) | Done | 4, 5, 6 | `fdemon-tui` | `widgets/devtools/performance/mod.rs`, `widgets/devtools/performance/stats_section.rs` |
| 8 | [08-wire-allocation-polling-and-memory-flow](tasks/08-wire-allocation-polling-and-memory-flow.md) | Done | 3, 4 | `fdemon-app` | `handler/update.rs`, engine/actions layer |
| 9 | [09-final-test-and-cleanup](tasks/09-final-test-and-cleanup.md) | Done | 7, 8 | workspace | All devtools modules |

## Dispatch Plan

**Wave 1** (solo — foundation types):
- Task 01: Extend core performance types (fdemon-core only)

**Wave 2** (parallel — different crates):
- Task 02: Extend PerformanceState and add messages (fdemon-app state/message)
- Task 03: Extend VM Service performance collection (fdemon-daemon)

**Wave 3** (parallel — different files/crates, no conflicts):
- Task 04: Add performance handler sub-module (fdemon-app handler/ — new file)
- Task 05: Implement frame bar chart widget (fdemon-tui — new file)
- Task 06: Implement memory chart widget (fdemon-tui — new file)

**Wave 4** (parallel — different crates):
- Task 07: Rewire performance panel layout and remove stats (fdemon-tui performance/mod.rs)
- Task 08: Wire allocation polling and memory sample flow (fdemon-app engine/actions)

**Wave 5** (solo — final verification):
- Task 09: Full test and cleanup pass

## Success Criteria

Phase 3 is complete when:

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
- [ ] All existing tests pass (with updates for new rendering)
- [ ] `cargo fmt && cargo check && cargo test && cargo clippy` clean

## Keyboard Shortcuts (New in Phase 3)

| Key | Action | Context |
|-----|--------|---------|
| `Left` | Select previous frame | Performance panel active |
| `Right` | Select next frame | Performance panel active |
| `Esc` | Deselect frame | Performance panel, frame selected |

## Notes

- **Phase 2 assumed complete**: Tasks reference the post-Phase-2 state where `DevToolsPanel::Layout` is removed and the Inspector uses a 50/50 split. The Performance panel is unaffected by Phase 2 changes.
- **`ClassHeapStats` reuse**: The plan mentions a `ClassAllocation` struct, but `ClassHeapStats` (already in fdemon-core) plus `AllocationProfile` already model the same data from `getAllocationProfile`. Tasks reuse these existing types instead of adding redundant ones.
- **`MemorySample` vs `MemoryUsage`**: `MemoryUsage` (heap_usage, heap_capacity, external_usage) is the existing type from `getMemoryUsage`. `MemorySample` is a richer type adding RSS and memory category breakdown for the time-series chart. Both ring buffers coexist during the transition; the existing `memory_history: RingBuffer<MemoryUsage>` continues to work for the simple gauge fallback, while `memory_samples: RingBuffer<MemorySample>` powers the new chart.
- **Braille canvas**: The memory chart uses Unicode braille characters for sub-character plotting resolution. This is a self-contained rendering utility within `memory_chart.rs` — not a separate file — to keep the module count down.
- **Shader compilation detection**: The `Flutter.Frame` Extension events don't directly report shader compilation. Detection relies on the `Flutter.ReportTimings` events or frame-level heuristics (first-render spike + subsequent recovery). The `shader_compilation` field defaults to `false` and detection is best-effort.
- **RSS collection**: RSS may not be available from all VM service versions. The `rss` field in `MemorySample` is `u64` with `0` as the "unavailable" sentinel. The chart omits the RSS line when all samples have `rss == 0`.
