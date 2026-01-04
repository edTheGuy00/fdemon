# Phase 4 Multi-Session Refinements - Task Index

## Overview

This phase addresses 5 refinements to improve the multi-session TUI experience:
1. Persistent session header with device name for single sessions
2. Status bar showing build config (Debug/Profile/Release + flavor) instead of device info
3. Removal of all legacy single-session backward compatibility code
4. Shutdown optimization to reduce 5+ second delay to near-instant
5. File watcher hot reload for ALL running sessions (not just selected)

Total: **5 tasks** with an estimated **15 hours** of effort

---

## Task Dependency Graph

```
Phase 4: Multi-Session Refinements
──────────────────────────────────

Independent Tasks (can be done in parallel):
┌─────────────────────────────────────────────────────────────┐
│                                                             │
│  01-persistent-session-header ──┐                           │
│                                 │                           │
│  02-status-bar-config-info ─────┼──► 04-legacy-code-removal │
│                                 │                           │
│  03-shutdown-optimization ──────┤                           │
│                                 │                           │
│  05-watcher-all-sessions-reload ┘                           │
│                                                             │
└─────────────────────────────────────────────────────────────┘

Note: Task 04 should be done LAST as it is the highest risk
      and touches code modified by tasks 01-03, 05.
```

---

## Tasks

| # | Task | Status | Depends On | Effort | Modules |
|---|------|--------|------------|--------|---------|
| 01 | [persistent-session-header](tasks/01-persistent-session-header.md) | ✅ Done | - | 2 hours | `layout.rs`, `tabs.rs`, `render.rs` |
| 02 | [status-bar-config-info](tasks/02-status-bar-config-info.md) | ✅ Done | - | 2 hours | `status_bar.rs`, `state.rs` |
| 03 | [shutdown-optimization](tasks/03-shutdown-optimization.md) | ✅ Done | - | 3 hours | `process.rs`, `actions.rs`, `startup.rs` |
| 04 | [legacy-code-removal](tasks/04-legacy-code-removal.md) | Not Started | 01, 02, 03, 05 | 6 hours | Multiple (see task) |
| 05 | [watcher-all-sessions-reload](tasks/05-watcher-all-sessions-reload.md) | Not Started | - | 2 hours | `update.rs`, `actions.rs`, `session_manager.rs` |

---

## Task Summaries

### Task 01: Persistent Session Header
Make the session header row always visible when at least one session exists. Currently, the tabs/header row only shows when there are 2+ sessions. With this change, a single session will display the device name with status icon in the subheader row.

**Key Changes:**
- `layout.rs`: Change `show_tabs = session_count > 1` to `session_count >= 1`
- `tabs.rs`: Add single-session rendering mode (device name + status icon)
- Tests: Update layout and tabs tests for new behavior

### Task 02: Status Bar Config Info
Replace device info in the status bar with build configuration information. Since the device name will now appear in the session header (Task 01), the status bar should show more useful per-session info: the build mode (Debug/Profile/Release) and optional flavor.

**Key Changes:**
- `status_bar.rs`: Remove `device_info()`, add `config_info()` showing FlutterMode + flavor
- Color coding: Debug=Green, Profile=Yellow, Release=Magenta
- Handle sessions without launch_config (default to "Debug")

### Task 03: Shutdown Optimization
Reduce Flutter Demon shutdown time from 5+ seconds to near-instant. The current delay is caused by stacked 5-second timeouts waiting for processes that may have already exited.

**Key Changes:**
- `process.rs`: Add `has_exited()` check, reduce timeouts from 5s to 1-2s
- `actions.rs`: Track when `DaemonEvent::Exited` received, skip shutdown if already exited
- `startup.rs`: Reduce per-session wait timeout from 5s to 2s

### Task 04: Legacy Code Removal
Remove all backward compatibility code for single-session mode. This is a significant refactor that should be done after tasks 01-03 and 05 are complete.

**Key Changes:**
- Remove `Message::Daemon` variant (keep only `SessionDaemon`)
- Remove `handle_daemon_event()` (keep only `handle_session_daemon_event()`)
- Remove legacy fields from `AppState` (phase, logs, current_app_id, device_name, etc.)
- Remove legacy fallback paths in update handlers
- Refactor auto-start to use session-based flow
- Estimated ~450+ lines of code removed

### Task 05: File Watcher Multi-Session Reload
Make the file watcher hot reload ALL running sessions on file saves, not just the selected session. Keyboard shortcuts `r` and `R` remain per-session for granular control.

**Key Changes:**
- `update.rs`: Modify `AutoReloadTriggered` handler to reload all running sessions
- `handler/mod.rs`: Add new `UpdateAction::ReloadAllSessions` variant
- `actions.rs`: Handle new action to spawn reload tasks for all sessions
- `session_manager.rs`: Add `reloadable_sessions()` helper method

---

## Implementation Order

### Week 1: Independent Tasks (Tasks 01-03, 05)
These can be done in parallel or sequence as preferred.

1. **Task 01: Persistent Session Header** (2 hours)
   - Quick UI polish
   - Low risk
   - Immediately visible improvement

2. **Task 02: Status Bar Config Info** (2 hours)
   - Depends on understanding how launch_config flows to sessions
   - Low risk
   - Provides useful build mode visibility

3. **Task 03: Shutdown Optimization** (3 hours)
   - Performance improvement
   - Medium risk (timeout changes)
   - Significant UX improvement

4. **Task 05: File Watcher Multi-Session Reload** (2 hours)
   - Behavior change for multi-session workflow
   - Low-medium risk
   - Critical for multi-device development

**Checkpoint:** UI shows device in header, config in status bar, shutdown is fast, file saves reload all devices

### Week 2: Legacy Removal (Task 04)
5. **Task 04: Legacy Code Removal** (6 hours)
   - High risk - touches many files
   - Do incrementally with compile checks after each step
   - Run full test suite frequently
   - Consider feature branch

**Checkpoint:** No legacy code, all tests pass, clippy clean

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
| 04 - Legacy Removal | High | Do last, incremental steps, feature branch |
| 05 - Watcher Multi-Reload | Low-Medium | Skip if any session busy, maintains sync |

---

## Success Metrics

1. **Header Visibility:** Device name visible in subheader when 1+ sessions exist
2. **Config Display:** Status bar shows Debug/Profile/Release + flavor correctly
3. **Shutdown Speed:** <2 seconds for normal shutdown, <0.5s when process already exited
4. **Multi-Session Reload:** File save triggers reload on ALL running sessions
5. **Code Reduction:** ~450 lines of legacy code removed
6. **Test Coverage:** All tests pass, no new clippy warnings
7. **No Regressions:** All multi-session functionality preserved

---

## Completion Status

| Task | Status | Notes |
|------|--------|-------|
| 01 - Persistent Session Header | ✅ Done | Implemented single-session subheader display |
| 02 - Status Bar Config Info | ✅ Done | Replaced device info with build config |
| 03 - Shutdown Optimization | ✅ Done | Reduced timeouts, added fast exit path |
| 04 - Legacy Code Removal | Not Started | Do last |
| 05 - Watcher Multi-Session Reload | Not Started | |
| **Overall Progress** | **60%** | 3/5 tasks complete |