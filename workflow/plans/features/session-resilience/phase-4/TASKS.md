# Phase 4: Stopped Session Device Reuse — Task Index

## Overview

Fix a UX bug where stopped sessions block new session creation on the same device. The duplicate-device guard in `handle_launch` uses `find_by_device_id`, which is phase-blind — it finds any session matching the device ID, including stopped ones. The fix adds phase-aware querying so only truly active sessions (Initializing, Running, Reloading) block device reuse.

**Total Tasks:** 4

## Root Cause

```
handle_launch()  →  session_manager.find_by_device_id(&device.id)
                         │
                         └── iterates ALL sessions in HashMap
                               └── matches device_id alone (no phase filter)
                                     └── finds Stopped session → blocks launch ← BUG
```

When a Flutter process exits normally, `handle_session_exited` sets `phase = AppPhase::Stopped` but intentionally does NOT remove the session (so the user can read the exit log). The duplicate-device check doesn't account for this.

## Task Dependency Graph

```
┌──────────────────────┐     ┌──────────────────────────┐
│ 01-is-active-method  │     │ 02-find-active-device-id │
└──────────┬───────────┘     └────────────┬─────────────┘
           │                              │
           └──────────┬───────────────────┘
                      ▼
           ┌──────────────────────────┐
           │ 03-update-launch-guard   │
           └──────────┬───────────────┘
                      │
                      ▼
           ┌──────────────────────────┐
           │ 04-device-reuse-tests    │
           └──────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Modules |
|---|------|--------|------------|---------|
| 1 | [01-is-active-method](tasks/01-is-active-method.md) | Done | - | `session/session.rs` |
| 2 | [02-find-active-device-id](tasks/02-find-active-device-id.md) | Done | 1 | `session_manager.rs` |
| 3 | [03-update-launch-guard](tasks/03-update-launch-guard.md) | Done | 2 | `handler/new_session/launch_context.rs` |
| 4 | [04-device-reuse-tests](tasks/04-device-reuse-tests.md) | Done | 3 | `handler/new_session/launch_context.rs`, `session_manager.rs` |

## Success Criteria

Phase 4 is complete when:

- [x] `Session::is_active()` returns `true` for `Initializing`, `Running`, `Reloading` and `false` for `Stopped`, `Quitting`
- [x] `SessionManager::find_active_by_device_id()` only returns sessions with active phases
- [x] `handle_launch` uses phase-aware query so stopped sessions don't block device reuse
- [x] User can start a new session on a device that has a stopped session
- [x] Initializing/Running/Reloading sessions still correctly block duplicate launches
- [x] All existing tests pass + new tests for device reuse scenarios
- [x] `cargo clippy --workspace -- -D warnings` clean

## Notes

- The stopped session remains in SessionManager (by design — preserves exit logs in the tab). Two sessions with the same device_id can coexist (one stopped, one active). This is safe because daemon events route by `session_id`, not `device_id`.
- `SpawnFailed` already calls `remove_session()`, so failed spawns don't cause the same problem.
- The `find_by_device_id` method is kept unchanged for backward compatibility — a new `find_active_by_device_id` is added alongside it.
