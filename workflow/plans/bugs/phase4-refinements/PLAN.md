# Plan: Phase 4 Multi-Session Refinements

## TL;DR

Five refinements to improve the multi-session TUI experience:
1. **Persistent Session Header**: Always show device name in session header row when sessions exist (not just when >1 session)
2. **Status Bar Config Info**: Replace device info with build mode (Debug/Profile/Release) and flavor display per-session
3. **Legacy Code Removal**: Remove all single-session backward compatibility code, fully commit to multi-session architecture
4. **Shutdown Optimization**: Reduce 5+ second shutdown delay to near-instant by optimizing timeout handling and parallel shutdown
5. **File Watcher Multi-Session Reload**: Hot reload ALL running sessions on file saves (not just selected); `r`/`R` keys remain per-session

---

## Affected Modules

### Refinement 1: Persistent Session Header
- `src/tui/layout.rs` — Change tabs area visibility logic from `session_count > 1` to `session_count >= 1`
- `src/tui/widgets/tabs.rs` — Render device info row for single session (already has `render_single_session_header`, needs integration with tabs area)
- `src/tui/render.rs` — Ensure tabs area renders for single session

### Refinement 2: Status Bar Config Info
- `src/tui/widgets/status_bar.rs` — Remove device_info(), add config_info() showing FlutterMode + flavor
- `src/app/state.rs` — Add helper to get selected session's config info
- `src/app/session.rs` — Ensure launch_config is properly populated

### Refinement 3: Shutdown Optimization
- `src/daemon/process.rs` — Reduce timeouts, add process state checks, skip commands for dead processes
- `src/tui/actions.rs` — Check if process already exited before initiating shutdown
- `src/tui/startup.rs` — Parallel shutdown for multiple sessions, shorter timeouts

### Refinement 4: Legacy Code Removal
- `src/app/state.rs` — Remove legacy single-session fields (phase, logs, log_view_state, current_app_id, device_name, platform, etc.)
- `src/app/message.rs` — Remove `Message::Daemon(DaemonEvent)` variant
- `src/app/handler/daemon.rs` — Remove `handle_daemon_event()`, keep only `handle_session_daemon_event()`
- `src/app/handler/update.rs` — Remove legacy fallback paths for reload/restart/stop
- `src/app/handler/session.rs` — Remove "legacy compatibility" global state updates
- `src/tui/runner.rs` — Remove legacy daemon_rx channel, simplify startup
- `src/tui/startup.rs` — Remove auto-start legacy process path, use session-based approach
- `src/tui/actions.rs` — Remove global cmd_sender legacy updates
- `src/tui/process.rs` — Remove Message::Daemon handling
- `src/tui/render.rs` — Remove fallback to global logs

### Refinement 5: File Watcher Multi-Session Reload
- `src/app/handler/update.rs` — Modify `AutoReloadTriggered` handler to reload all running sessions
- `src/app/handler/mod.rs` — Add `UpdateAction::ReloadAllSessions` variant
- `src/tui/actions.rs` — Handle new action to spawn reload tasks for each running session
- `src/app/session_manager.rs` — Add `reloadable_sessions()` helper method

---

## Phases

### Phase 1: Persistent Session Header (Low Risk)
Make the session header row always visible when at least one session exists, showing device name with status icon.

**Steps:**
1. Modify `layout.rs`: Change `show_tabs = session_count > 1` to `show_tabs = session_count >= 1`
2. Modify `tabs.rs`: Update `SessionTabs` widget to render a simplified device info row when only 1 session exists
3. Update tests in layout.rs and tabs.rs to reflect new behavior
4. Visual verification: single session shows device name in header row

**Acceptance Criteria:**
- Single session displays device name with status icon in subheader row
- Multiple sessions show tabs as before
- No sessions = no subheader row

### Phase 2: Status Bar Config Info (Low Risk)
Replace device info in status bar with build configuration info (mode + flavor).

**Steps:**
1. Add `config_info()` method to `StatusBar` that displays FlutterMode and optional flavor
2. Remove `device_info()` method from StatusBar (device now shown in header)
3. Ensure session's `launch_config` is populated during session creation
4. Handle case where no launch_config exists (show "Debug" as default)
5. Update status bar tests

**Acceptance Criteria:**
- Status bar shows "Debug", "Profile", or "Release" based on session config
- If flavor exists, shows "Debug (production)" format
- Multiple sessions show config of currently selected session
- When switching sessions, status bar updates to show that session's config

### Phase 3: Legacy Code Removal (High Risk, Careful)
Remove all backward compatibility code for single-session mode. This is a significant refactor.

**Steps:**
1. Remove legacy fields from `AppState`:
   - `phase`, `logs`, `log_view_state`, `max_logs`
   - `current_app_id`, `device_name`, `platform`
   - `flutter_version`, `session_start`, `reload_start_time`, `last_reload_time`, `reload_count`
2. Remove `Message::Daemon(DaemonEvent)` from message.rs
3. Remove `handle_daemon_event()` from handler/daemon.rs
4. Update handler/update.rs:
   - Remove legacy fallback paths in HotReload, HotRestart, StopApp
   - Remove legacy fallback in FilesChanged auto-reload
   - Remove legacy global state updates in SessionStarted
5. Update handler/session.rs:
   - Remove "legacy compatibility" updates to global state
6. Update tui/runner.rs:
   - Remove legacy `daemon_rx` channel
   - Remove `route_daemon_response()` legacy function
7. Update tui/startup.rs:
   - Refactor auto-start to use session-based flow
   - Remove direct FlutterProcess ownership path
8. Update tui/actions.rs:
   - Remove global cmd_sender backward compatibility updates
9. Update tui/process.rs:
   - Remove Message::Daemon handling
10. Update tui/render.rs:
    - Remove fallback to global logs
11. Update all affected tests
12. Full test suite verification

**Acceptance Criteria:**
- All legacy fields removed from AppState
- Only SessionDaemon message variant for daemon events
- No fallback to global state in handlers
- All tests pass
- Multi-session functionality preserved

### Phase 5: File Watcher Multi-Session Reload (Low-Medium Risk)
Make the file watcher hot reload ALL running sessions on file saves, not just the selected session. Keyboard shortcuts `r` and `R` remain per-session for granular control.

**Steps:**
1. Add `UpdateAction::ReloadAllSessions { sessions: Vec<(SessionId, String)> }` variant
2. Modify `AutoReloadTriggered` handler to collect all running sessions and return new action
3. Add `reloadable_sessions()` helper to SessionManager
4. Handle `ReloadAllSessions` action in `actions.rs` to spawn reload tasks for each session
5. Skip all reloads if any session is busy (keeps devices in sync)
6. Update tests for multi-session reload behavior

**Acceptance Criteria:**
- File save triggers hot reload on ALL running sessions
- `r` key still reloads only selected session
- `R` key still restarts only selected session
- Sessions that are already reloading are skipped
- Log message shows count of sessions being reloaded

### Phase 4: Shutdown Optimization (Medium Risk)
Reduce shutdown time from 5+ seconds to near-instant.

**Steps:**
1. In `daemon/process.rs` `shutdown()`:
   - Check if child process already exited before sending commands
   - Reduce app.stop timeout from 5s to 1s
   - Reduce graceful exit timeout from 5s to 2s
   - Use non-blocking check for process status
2. In `tui/actions.rs` `spawn_session()`:
   - Track process exit state in loop
   - Skip shutdown commands if process already exited
   - Set flag when DaemonEvent::Exited received
3. In `tui/startup.rs` `cleanup_sessions()`:
   - Send shutdown signal first (already done)
   - Reduce per-session wait timeout from 5s to 2s
   - Consider parallel awaiting with `futures::future::join_all`
4. Add early exit detection in shutdown flow

**Acceptance Criteria:**
- Shutdown completes in <1 second when processes terminate quickly
- Shutdown completes in <3 seconds worst case
- No orphaned processes after quit
- Clean shutdown logs indicate fast path when process already exited

---

## Edge Cases & Risks

| Risk | Impact | Mitigation |
|------|--------|------------|
| Legacy removal breaks existing tests | High | Run test suite after each removal step, fix incrementally |
| Auto-start mode broken after refactor | Medium | Create dedicated session for auto-start, test thoroughly |
| Shutdown too aggressive causes data loss | Low | Keep 2s timeout as safety net, force kill only as last resort |
| Config info missing for quick device select | Low | Default to "Debug" when no launch_config provided |
| Race condition in process exit detection | Medium | Use atomic flag for exit state, lock properly |
| Status bar shows stale config on session switch | Low | Refresh status bar on session navigation |

---

## Further Considerations

1. **Should we keep any global state for compatibility?** — Recommend no, fully commit to multi-session model
2. **What happens to sessions started without launch config?** — Show "Debug" (Flutter default)
3. **Should shutdown be cancellable?** — Not in scope, but could add interrupt handling later
4. **Do we need to migrate auto-start to multi-session?** — Yes, auto-start should create a session like manual start
5. **Should tabs always show even with zero sessions?** — No, only show when at least one session exists

---

## Implementation Order

| Order | Task | Effort | Dependencies |
|-------|------|--------|--------------|
| 1 | Persistent session header | 2 hours | None |
| 2 | Status bar config info | 2 hours | None |
| 3 | Shutdown optimization | 3 hours | None |
| 4 | File watcher multi-session reload | 2 hours | None |
| 5 | Legacy code removal | 6 hours | 1, 2, 3, 4 (do last as it touches everything) |

**Recommended approach:** Complete 1, 2, 3, and 4 first (independent), then tackle legacy removal last since it's the highest risk and touches all the code paths.

---

## Success Metrics

1. **Header Visibility:** Session header visible with device name for any number of sessions >= 1
2. **Config Display:** Status bar correctly shows Debug/Profile/Release + flavor for selected session
3. **Code Reduction:** Remove ~500+ lines of legacy code, no module contains backward-compat comments
4. **Shutdown Speed:** Application exits in <2 seconds under normal conditions
5. **Multi-Session Reload:** File save triggers reload on ALL running sessions simultaneously
6. **Test Coverage:** All existing tests pass, new tests cover single-session header, config display, and multi-session reload