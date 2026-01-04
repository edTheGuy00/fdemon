# Phase 4 Multi-Session Refinements - Task Index

## Overview

This phase addresses 5 refinements to improve the multi-session TUI experience:
1. Persistent session header with device name for single sessions
2. Status bar showing build config (Debug/Profile/Release + flavor) instead of device info
3. Removal of all legacy single-session backward compatibility code
4. Shutdown optimization to reduce 5+ second delay to near-instant
5. File watcher hot reload for ALL running sessions (not just selected)

Total: **5 main tasks** (Task 4 expanded to 7 subtasks) with an estimated **17 hours** of effort

---

## Task Dependency Graph

```
Phase 4: Multi-Session Refinements
──────────────────────────────────────

Independent Tasks (can be done in parallel):
┌─────────────────────────────────────────────────────────────┐
│                                                             │
│  01-persistent-session-header ──┐                           │
│                                 │                           │
│  02-status-bar-config-info ─────┼──► Task 04 (Legacy Removal)
│                                 │                           │
│  03-shutdown-optimization ──────┤                           │
│                                 │                           │
│  05-watcher-all-sessions-reload ┘                           │
│                                                             │
└─────────────────────────────────────────────────────────────┘

Task 04 Subtask Dependencies (MUST be done in order):
┌─────────────────────────────────────────────────────────────┐
│                                                             │
│  04a-autostart-session-refactor                             │
│           │                                                 │
│           ▼                                                 │
│  04b-remove-message-daemon                                  │
│           │                                                 │
│           ▼                                                 │
│  04c-remove-fallback-paths                                  │
│           │                                                 │
│           ▼                                                 │
│  04d-remove-global-state-updates                            │
│           │                                                 │
│           ▼                                                 │
│  04e-remove-appstate-fields                                 │
│           │                                                 │
│           ▼                                                 │
│  04f-cleanup-actions-legacy                                 │
│           │                                                 │
│           ▼                                                 │
│  04g-update-tests                                           │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

---

## Tasks

| # | Task | Status | Depends On | Effort | Modules |
|---|------|--------|------------|--------|---------|
| 01 | [persistent-session-header](tasks/01-persistent-session-header.md) | ✅ Done | - | 2 hours | `layout.rs`, `tabs.rs`, `render.rs` |
| 02 | [status-bar-config-info](tasks/02-status-bar-config-info.md) | ✅ Done | - | 2 hours | `status_bar.rs`, `state.rs` |
| 03 | [shutdown-optimization](tasks/03-shutdown-optimization.md) | ✅ Done | - | 3 hours | `process.rs`, `actions.rs`, `startup.rs` |
| 04 | [legacy-code-removal](tasks/04-legacy-code-removal.md) | ✅ Done | 01, 02, 03, 05 | 8.5 hours | Multiple (see subtasks) |
| 05 | [watcher-all-sessions-reload](tasks/05-watcher-all-sessions-reload.md) | ✅ Done | - | 2 hours | `update.rs`, `actions.rs`, `session_manager.rs` |

---

## Task 4 Subtasks (Legacy Code Removal)

Task 4 is a significant refactor broken into 7 incremental subtasks that **MUST be done in order** to avoid breaking changes.

| # | Subtask | Status | Depends On | Effort | Key Changes |
|---|---------|--------|------------|--------|-------------|
| 4a | [autostart-session-refactor](tasks/04a-autostart-session-refactor.md) | ✅ Done | Tasks 01-03, 05 | 2 hours | Refactor auto-start to use sessions instead of owning FlutterProcess directly |
| 4b | [remove-message-daemon](tasks/04b-remove-message-daemon.md) | ✅ Done | 4a | 1 hour | Remove `Message::Daemon` variant and `handle_daemon_event()` |
| 4c | [remove-fallback-paths](tasks/04c-remove-fallback-paths.md) | ✅ Done | 4b | 1 hour | Remove legacy fallbacks to `current_app_id` in handlers |
| 4d | [remove-global-state-updates](tasks/04d-remove-global-state-updates.md) | ✅ Done | 4c | 0.5 hours | Stop updating global AppState fields from session events |
| 4e | [remove-appstate-fields](tasks/04e-remove-appstate-fields.md) | ✅ Done | 4d | 2 hours | Remove legacy fields and methods from AppState |
| 4f | [cleanup-actions-legacy](tasks/04f-cleanup-actions-legacy.md) | ✅ Done | 4e | 0.5 hours | Remove session_id checks and global cmd_sender updates |
| 4g | [update-tests](tasks/04g-update-tests.md) | ✅ Done | 4f | 1.5 hours | Update/remove tests that rely on legacy behavior |

---

## Task 4 Subtask Details

### Subtask 4a: Refactor Auto-Start to Use Sessions (2 hours)
**Objective**: Change auto-start mode to create sessions through SessionManager instead of owning FlutterProcess directly.

**Key Changes:**
- `startup.rs`: Return `Option<UpdateAction>` instead of `Option<FlutterProcess>`
- `runner.rs`: Remove `daemon_rx` channel, handle startup action
- `cleanup_sessions()`: Simplify to only handle session_tasks

**Why first**: Eliminates the dual code path where auto-start bypasses the session system. Required before removing Message::Daemon.

---

### Subtask 4b: Remove Message::Daemon Variant (1 hour)
**Objective**: Eliminate the legacy `Message::Daemon(DaemonEvent)` variant and all associated handling code.

**Key Changes:**
- `message.rs`: Remove `Daemon(DaemonEvent)` variant
- `daemon.rs`: Remove `handle_daemon_event()` (~97 lines), `handle_daemon_message_state()` (~17 lines)
- `update.rs`: Remove `Message::Daemon` match arm
- `process.rs`: Remove `route_legacy_daemon_response()`

**Lines removed**: ~142 lines

---

### Subtask 4c: Remove Legacy Fallback Paths (1 hour)
**Objective**: Remove all fallback code paths that use `state.current_app_id` when no session is selected.

**Key Changes:**
- `update.rs`: Remove fallback blocks in HotReload, HotRestart, StopApp, AutoReloadTriggered handlers
- Errors logged to session instead of global state
- No more `session_id: 0` task spawns

**Lines removed**: ~50 lines

---

### Subtask 4d: Remove Legacy Global State Updates (30 min)
**Objective**: Stop updating global AppState fields when session events occur.

**Key Changes:**
- `session.rs`: Remove `state.current_app_id = ...` updates
- `update.rs`: Remove `state.device_name = ...` and `state.platform = ...` in SessionStarted handler

**Lines removed**: ~10 lines

---

### Subtask 4e: Remove Legacy Fields from AppState (2 hours)
**Objective**: Remove all unused legacy single-session fields and methods from AppState.

**Fields to REMOVE:**
- `current_app_id`, `device_name`, `platform`, `flutter_version`
- `session_start`, `reload_start_time`, `last_reload_time`, `reload_count`
- `logs`, `log_view_state`, `max_logs`

**Keep**: `phase` (used for app-level quitting state)

**Methods to REMOVE:**
- `add_log()`, `log_info()`, `log_error()`
- `start_reload()`, `record_reload_complete()`, `reload_elapsed()`, `last_reload_display()`
- `session_duration()`, `session_duration_display()`, `start_session()`, `set_device_info()`, `is_busy()`

**Also update:**
- `render.rs`: Remove fallback to global logs

**Lines removed**: ~140 lines

---

### Subtask 4f: Clean Up actions.rs Legacy Code (30 min)
**Objective**: Remove session_id checks and global cmd_sender updates.

**Key Changes:**
- Remove `session_id > 0` checks in `execute_task()`
- Remove global `cmd_sender` updates in `spawn_session()`
- Remove `cmd_sender` parameter from `handle_action()` and `spawn_session()`

**Lines removed**: ~40 lines

---

### Subtask 4g: Update and Remove Obsolete Tests (1.5 hours)
**Objective**: Fix all tests that rely on legacy single-session behavior.

**Tests to REMOVE (5):**
- `test_daemon_exited_event_logs_message`
- `test_daemon_exited_sets_quitting_phase`
- `test_daemon_exited_with_error_code_sets_quitting`
- `test_session_started_updates_legacy_global_state`
- `test_auto_reload_falls_back_to_legacy`

**Tests to UPDATE (~15+):**
- All tests using `state.current_app_id` → use sessions
- All tests using `state.logs` → use session logs
- All tests using `Message::Daemon` → remove or update

---

## Implementation Order

### Week 1: Independent Tasks (Tasks 01-03, 05) - ✅ COMPLETE

### Week 2: Legacy Removal (Task 04)

**Day 1: Subtasks 4a + 4b (3 hours)**
1. 4a: Refactor auto-start to use sessions
2. 4b: Remove Message::Daemon variant
3. Checkpoint: Compile passes, auto-start still works

**Day 2: Subtasks 4c + 4d (1.5 hours)**
1. 4c: Remove legacy fallback paths
2. 4d: Remove legacy global state updates
3. Checkpoint: Control actions work via sessions only

**Day 3: Subtask 4e (2 hours)**
1. Remove fields incrementally with compile checks
2. Update render.rs
3. Replace log_info/log_error calls
4. Checkpoint: AppState is clean, no legacy fields

**Day 4: Subtasks 4f + 4g (2 hours)**
1. 4f: Clean up actions.rs
2. 4g: Update/remove tests
3. Final checkpoint: All tests pass, clippy clean

---

## Testing Strategy

### Unit Tests
- Layout tests for single-session header visibility
- Tab rendering tests for 1-session case
- Status bar tests for config_info display
- Shutdown timing tests (where possible)
- Multi-session reload tests for file watcher
- Handler tests updated to remove legacy paths

### Integration Tests
- Start single session → verify header shows device
- Start multiple sessions → verify tabs appear
- Check config display in status bar per session
- Quit timing verification
- File save → all sessions reload
- Auto-start mode with sessions

### Manual Testing
- Visual inspection of header/tabs for 0, 1, 2+ sessions
- Status bar shows correct mode/flavor
- Shutdown timing with stopwatch (target: <2s)
- Process cleanup verification: `ps aux | grep flutter`
- File save with 2 devices: both reload
- `r` key with 2 devices: only selected reloads

---

## Risk Assessment

| Task | Risk Level | Mitigation |
|------|------------|------------|
| 01 - Persistent Header | Low | Pure UI change, no business logic |
| 02 - Status Bar Config | Low | Additive change, falls back to "Debug" |
| 03 - Shutdown Optimization | Medium | Keep force_kill as fallback, log timing |
| 04a - Auto-start Refactor | Medium | Test thoroughly before and after |
| 04b - Remove Message::Daemon | Low-Medium | Compiler catches all usages |
| 04c - Remove Fallback Paths | Low | Just removes optional code paths |
| 04d - Remove Global Updates | Low | Minimal code removal |
| 04e - Remove AppState Fields | High | Do incrementally with compile checks |
| 04f - Clean Up actions.rs | Low-Medium | Mostly code removal |
| 04g - Update Tests | Medium | Update incrementally |
| 05 - Watcher Multi-Reload | Low-Medium | Skip if any session busy |

---

## Success Metrics

1. **Header Visibility:** Device name visible in subheader when 1+ sessions exist
2. **Config Display:** Status bar shows Debug/Profile/Release + flavor correctly
3. **Shutdown Speed:** <2 seconds for normal shutdown, <0.5s when process already exited
4. **Multi-Session Reload:** File save triggers reload on ALL running sessions
5. **Code Reduction:** ~450+ lines of legacy code removed
6. **Test Coverage:** All tests pass, no new clippy warnings
7. **No Regressions:** All multi-session functionality preserved

---

## Code Removal Summary (Task 4)

| Subtask | Lines Removed | Lines Changed |
|---------|---------------|---------------|
| 4a - Auto-start Refactor | ~30 | ~50 |
| 4b - Remove Message::Daemon | ~142 | ~5 |
| 4c - Remove Fallback Paths | ~50 | ~15 |
| 4d - Remove Global Updates | ~10 | ~2 |
| 4e - Remove AppState Fields | ~140 | ~35 |
| 4f - Clean Up actions.rs | ~40 | ~25 |
| 4g - Update Tests | ~100+ | ~200 |
| **Total** | **~510+** | **~330** |

---

## Completion Status

| Task | Status | Notes |
|------|--------|-------|
| 01 - Persistent Session Header | ✅ Done | Implemented single-session subheader display |
| 02 - Status Bar Config Info | ✅ Done | Replaced device info with build config |
| 03 - Shutdown Optimization | ✅ Done | Reduced timeouts, added fast exit path |
| 04 - Legacy Code Removal | ✅ Done | 7/7 subtasks complete, ~500+ lines removed |
| 04a - Auto-start Refactor | ✅ Done | Refactored to use sessions, removed daemon_rx |
| 04b - Remove Message::Daemon | ✅ Done | Removed ~130 lines of legacy code |
| 04c - Remove Fallback Paths | ✅ Done | Removed ~50 lines, 11 tests commented |
| 04d - Remove Global Updates | ✅ Done | Removed ~21 lines from session.rs and update.rs |
| 04e - Remove AppState Fields | ✅ Done | Removed 11 fields, 9 methods, 6 message variants |
| 04f - Clean Up actions.rs | ✅ Done | Removed cmd_sender params, ~40 lines of legacy code |
| 04g - Update Tests | ✅ Done | Removed ~200 lines of commented-out legacy test code |
| 05 - Watcher Multi-Session Reload | ✅ Done | Auto-reload triggers all sessions |
| **Overall Progress** | **100%** | 5/5 main tasks complete, Task 4: 7/7 subtasks done |