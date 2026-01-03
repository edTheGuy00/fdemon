# Phase 2 Bug Fixes - Task Index

## Overview

Four bugs were discovered after completing Phase 2 of Flutter Demon development. This document provides an index of all tasks required to fix these bugs.

**Total Tasks:** 4  
**Estimated Duration:** 4-6 hours  
**Critical Path:** Task 1 → Task 2 (response routing must be fixed before shutdown sequence)

## Task Dependency Graph

```
┌─────────────────────────┐
│  fix-response-routing   │ ← CRITICAL (Bug #2)
│  (1-2 hours)            │
└───────────┬─────────────┘
            │
            ▼
┌─────────────────────────┐
│  fix-shutdown-sequence  │   (Bug #3)
│  (30 min)               │
└─────────────────────────┘

┌─────────────────────────┐
│ fix-process-exit-handle │   (Bug #4) - Independent
│  (15-30 min)            │
└─────────────────────────┘

┌─────────────────────────┐
│    fix-selector-ui      │   (Bug #1) - Independent
│  (2-3 hours)            │
└─────────────────────────┘
```

## Tasks

| # | Task | Status | Priority | Depends On | Modules | Est. Time |
|---|------|--------|----------|------------|---------|-----------|
| 1 | [fix-response-routing](tasks/fix-response-routing.md) | ✅ Done | **Critical** | - | `tui/mod.rs` | 1-2 hours |
| 2 | [fix-shutdown-sequence](tasks/fix-shutdown-sequence.md) | ✅ Done | High | Task 1 | `daemon/process.rs`, `tui/mod.rs` | 30 min |
| 3 | [fix-process-exit-handling](tasks/fix-process-exit-handling.md) | ✅ Done | Medium | - | `app/handler.rs` | 15-30 min |
| 4 | [fix-selector-ui](tasks/fix-selector-ui.md) | ✅ Done | Low | - | `tui/selector.rs` | 2-3 hours |

## Bug Summary

### Bug #1: Selector UI Layout (Low Priority)
**File:** `src/tui/selector.rs`  
**Symptom:** Project selector text appears on plain new lines without proper centering or borders.  
**Fix:** Refactor to use Ratatui layout system instead of raw crossterm output.

### Bug #2: Reload Timeout (Critical)
**File:** `src/app/handler.rs`, `src/tui/mod.rs`  
**Symptom:** Hot reload commands complete on Flutter side but Flutter Demon times out after 30 seconds.  
**Root Cause:** Daemon responses are parsed but never forwarded to `RequestTracker.handle_response()`.  
**Fix:** Route responses to RequestTracker in `run_loop()`.

### Bug #3: Quit Doesn't Kill App (High Priority)
**File:** `src/daemon/process.rs`, `src/tui/mod.rs`  
**Symptom:** Pressing 'q' closes Flutter Demon but Flutter app continues running.  
**Root Cause:** `daemon.shutdown` only disconnects protocol; `app.stop` is never sent.  
**Fix:** Send `app.stop` before `daemon.shutdown` in shutdown sequence.

### Bug #4: App Exit Not Handled (Medium Priority)
**File:** `src/app/handler.rs`  
**Symptom:** When Flutter app closes externally, Flutter Demon stays in Loading state.  
**Root Cause:** `DaemonEvent::Exited` sets phase to `Initializing` instead of `Quitting`.  
**Fix:** Change phase to `Quitting` when Flutter process exits.

## Recommended Execution Order

1. **Task 1: fix-response-routing** - Must be done first; unblocks Task 2
2. **Task 3: fix-process-exit-handling** - Quick win, can be done in parallel with Task 1
3. **Task 2: fix-shutdown-sequence** - Depends on Task 1
4. **Task 4: fix-selector-ui** - Lowest priority, cosmetic improvement

## Testing Checklist

After all tasks complete:

- [ ] Press 'r' → reload completes without timeout
- [ ] Press 'R' → restart completes without timeout
- [ ] Modify .dart file → auto-reload works
- [ ] Press 'q' → both Flutter Demon and Flutter app close
- [ ] Close app externally → Flutter Demon exits
- [ ] Multiple Flutter projects → selector shows centered modal with borders