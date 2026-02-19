# Phase 3: Performance & Memory Monitoring — Task Index

## Overview

Add real-time performance metrics and memory usage monitoring by extending the VM Service client with memory/GC/timeline RPCs, introducing a shareable request handle for on-demand calls from outside the event forwarding loop, and integrating periodic data collection into the TEA architecture. This phase creates the data pipeline that Phase 4 (TUI DevTools Mode) will visualize.

**Total Tasks:** 6
**Estimated Hours:** 20-28 hours

## Task Dependency Graph

```
┌──────────────────────────┐     ┌──────────────────────────┐
│ 01-performance-data-     │     │ 02-vm-request-handle     │
│ models (fdemon-core)     │     │ (fdemon-daemon,          │
│                          │     │  fdemon-app)             │
└───────────┬──────────────┘     └──────────┬───────────────┘
            │                               │
    ┌───────┼───────────┐                   │
    │       │           │                   │
    ▼       ▼           │                   │
┌────────┐ ┌──────────┐ │                   │
│03-mem  │ │04-frame  │ │                   │
│-gc-rpcs│ │-timing   │ │                   │
│        │ │-rpcs     │ │                   │
└───┬────┘ └────┬─────┘ │                   │
    │           │       │                   │
    │     ┌─────┘       │                   │
    │     │     ┌───────┘                   │
    ▼     │     ▼                           │
┌──────────────────────┐                    │
│05-memory-monitoring  │◄───────────────────┘
│   integration        │
└──────────┬───────────┘
           │
           ▼
┌──────────────────────┐
│06-frame-timing       │
│   -aggregation       │
└──────────────────────┘
```

## Waves (Parallelizable Groups)

### Wave 1 (Foundation)
- **01-performance-data-models** — Domain types for memory, GC, frame timing, ring buffer (pure types, fdemon-core)
- **02-vm-request-handle** — Shareable request handle extracted from VmServiceClient (fdemon-daemon + fdemon-app)

### Wave 2 (VM RPCs)
- **03-memory-gc-rpcs** — `getMemoryUsage`, `getAllocationProfile`, GC stream parsing (fdemon-daemon)
- **04-frame-timing-rpcs** — Timeline stream subscription, frame timing event parsing (fdemon-daemon)

### Wave 3 (TEA Integration)
- **05-memory-monitoring-integration** — Periodic polling, session state, Message variants, handlers (fdemon-app)

### Wave 4 (Aggregation)
- **06-frame-timing-aggregation** — FPS calculation, jank detection, rolling stats, frame timing handlers (fdemon-app)

## Tasks

| # | Task | Status | Depends On | Est. Hours | Crate | Key Modules |
|---|------|--------|------------|------------|-------|-------------|
| 1 | [01-performance-data-models](tasks/01-performance-data-models.md) | Not Started | - | 3-4h | `fdemon-core` | `performance.rs` |
| 2 | [02-vm-request-handle](tasks/02-vm-request-handle.md) | Not Started | - | 3-4h | `fdemon-daemon`, `fdemon-app` | `vm_service/client.rs`, `session.rs`, `actions.rs` |
| 3 | [03-memory-gc-rpcs](tasks/03-memory-gc-rpcs.md) | Not Started | 1 | 3-4h | `fdemon-daemon` | `vm_service/client.rs`, `vm_service/performance.rs` |
| 4 | [04-frame-timing-rpcs](tasks/04-frame-timing-rpcs.md) | Not Started | 1 | 3-4h | `fdemon-daemon` | `vm_service/timeline.rs` |
| 5 | [05-memory-monitoring-integration](tasks/05-memory-monitoring-integration.md) | Not Started | 1, 2, 3 | 4-6h | `fdemon-app` | `session.rs`, `message.rs`, `handler/`, `actions.rs` |
| 6 | [06-frame-timing-aggregation](tasks/06-frame-timing-aggregation.md) | Not Started | 1, 2, 4, 5 | 4-6h | `fdemon-app` | `session.rs`, `message.rs`, `handler/`, `actions.rs` |

## Success Criteria

Phase 3 is complete when:

- [ ] `MemoryUsage` data retrieved via `getMemoryUsage()` RPC
- [ ] `AllocationProfile` with class-level heap stats retrieved via `getAllocationProfile()`
- [ ] GC stream events captured and forwarded as Messages
- [ ] Frame timing data parsed from Timeline stream events
- [ ] `VmRequestHandle` allows on-demand RPC calls from outside the forwarding loop
- [ ] Periodic memory polling runs at configurable interval (default 2s)
- [ ] Memory and GC history stored in rolling ring buffers per session
- [ ] FPS calculated from frame timing data
- [ ] Janky frames (>16.67ms budget) detected and counted
- [ ] Performance data aggregated (avg, min, max, p95)
- [ ] Data collection does not impact TUI rendering performance
- [ ] Graceful degradation when VM Service unavailable or RPCs fail
- [ ] All new code has unit tests with mock JSON responses
- [ ] No regressions in existing functionality (`cargo test --workspace`)
- [ ] `cargo clippy --workspace -- -D warnings` passes

## New Module Structure

```
crates/fdemon-core/src/
├── ...existing files...
└── performance.rs              # NEW: MemoryUsage, GcEvent, FrameTiming, RingBuffer, etc.

crates/fdemon-daemon/src/vm_service/
├── mod.rs                      # MODIFIED: add performance, timeline module exports
├── client.rs                   # MODIFIED: add VmRequestHandle, memory/timeline RPCs
├── protocol.rs                 # existing
├── errors.rs                   # existing
├── logging.rs                  # existing
├── extensions/                 # existing (Phase 2)
├── performance.rs              # NEW: getMemoryUsage, getAllocationProfile, GC event parsing
└── timeline.rs                 # NEW: Timeline stream parsing, frame timing extraction

crates/fdemon-app/src/
├── session.rs                  # MODIFIED: add PerformanceState, vm_request_handle
├── message.rs                  # MODIFIED: add VmServiceMemorySnapshot, VmServiceGcEvent, etc.
├── handler/
│   ├── update.rs               # MODIFIED: handle new perf Message variants
│   └── ...
└── actions.rs                  # MODIFIED: store VmRequestHandle, spawn polling task
```

## Notes

- **No TUI changes in Phase 3.** New widgets and panels for displaying performance data belong to Phase 4.
- **The `VmRequestHandle` is the key architectural enabler.** It allows on-demand RPC calls without modifying the existing event forwarding loop's contract. The handle wraps the `cmd_tx` sender (which is `Clone`), so it can be shared freely.
- **Periodic polling uses a dedicated background task** per session that runs alongside the event forwarding task. It sends `Message` variants back through the TEA channel.
- **Ring buffers** are used for history storage to bound memory usage. Sizes are configurable.
- **GC stream subscription** is additive — it's added to `RESUBSCRIBE_STREAMS` alongside the existing `Extension` and `Logging` streams.
- **Timeline stream** requires `setVMTimelineFlags` to enable the `Dart` recording category before events will appear. This must be called after connection.
- **Frame timing calculation** follows Flutter DevTools' approach: parsing `Flutter.Frame` events from the Extension stream (posted via `developer.postEvent`) rather than raw Chrome Trace Format timeline events. This is more reliable and well-documented.
- Phase 1 established the VM Service client with `request()` — Phase 3 adds performance-specific RPCs and the sharing mechanism for on-demand calls.
