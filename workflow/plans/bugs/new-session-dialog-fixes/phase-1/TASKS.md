# Phase 1: Device Discovery & Caching Fixes - Task Index

## Overview

Fix critical device discovery and caching issues that impact first-run experience. Connected devices should load instantly from cache on subsequent dialog opens, and bootable devices should populate on first launch.

**Total Tasks:** 3
**Bugs Addressed:** Bug 1 (Connected devices not cached), Bug 2 (Bootable devices not populated)

## Task Dependency Graph

```
┌─────────────────────────────┐
│  01-cache-preload           │  (Bug 1)
│  Pre-populate from cache    │
└─────────────────────────────┘

┌─────────────────────────────┐
│  02-bootable-discovery      │  (Bug 2)
│  Trigger at startup         │
└─────────────────────────────┘

┌─────────────────────────────┐
│  03-bootable-cache          │  (Bug 2)
│  Cache bootable devices     │
└──────────────┬──────────────┘
               │
               └── depends on: 02
```

## Tasks

| # | Task | Status | Depends On | Modules |
|---|------|--------|------------|---------|
| 1 | [01-cache-preload](tasks/01-cache-preload.md) | Done | - | `state.rs` |
| 2 | [02-bootable-discovery-startup](tasks/02-bootable-discovery-startup.md) | Done | - | `update.rs`, `target_selector.rs` |
| 3 | [03-bootable-cache](tasks/03-bootable-cache.md) | Done | 02 | `state.rs`, `update.rs` |

## Success Criteria

Phase 1 is complete when:

- [x] Opening dialog second time shows connected devices instantly (from cache)
- [x] Bootable devices populate on first dialog open (after tool check completes)
- [x] Bootable devices are cached and show instantly on subsequent dialog opens
- [x] "r" key refreshes both connected and bootable tabs
- [x] No regression in device selection flow
- [x] All new code has unit tests
- [x] `cargo test` passes
- [x] `cargo clippy` passes

## Notes

- Connected device cache already exists (`device_cache` in `AppState`) but isn't used when dialog opens
- Bootable device discovery depends on `tool_availability` check completing first
- Currently bootable devices are NOT cached - Task 03 adds this capability
- Cache TTL is 5 seconds (`DEVICE_CACHE_TTL`) - devices refresh in background after TTL expires
