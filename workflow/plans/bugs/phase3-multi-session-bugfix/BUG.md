# Bugfix Plan: Phase 3 Multi-Session Issues

## TL;DR

Phase 3 implementation has four critical bugs: (1) device selector shows spinner instead of animated LineGauge, (2) starting a new session replaces the existing one instead of enabling multi-session mode with tabs, (3) quit doesn't terminate all running sessions properly, and (4) `tui/mod.rs` is too large. The root cause of bugs 2 and 3 is that `SessionManager` exists but isn't actually used to manage multiple concurrent Flutter processes - the architecture only supports a single shared command sender and task handle.

## Bug Reports

### Bug 1: Device Selector Missing Progress Indicator
**Symptom:** Device selector popup shows a text spinner ("⠋ Discovering devices...") instead of an animated LineGauge as specified in task 09-refined-layout.md.

**Expected:** Animated LineGauge widget showing indeterminate progress during device discovery.

**Affected Files:** `src/tui/widgets/device_selector.rs`

---

### Bug 2: New Session Replaces Existing Session (Critical)
**Symptom:** When a session is running on device A and user presses 'n' to start a new session on device B, the new session starts but in the current window - the previous session appears to be discarded.

**Expected:** Starting a new session should enter multi-session mode with tabs showing all running sessions, each with their own respective logs.

**Root Cause Analysis:**
1. `DeviceSelected` handler returns `UpdateAction::SpawnSession` but **never calls `session_manager.create_session()`**
2. `handle_action` for `SpawnSession` stores the new process handle in `session_task: Arc<Mutex<Option<JoinHandle>>>` - this is a SINGLE slot that overwrites the previous handle
3. `cmd_sender: Arc<Mutex<Option<CommandSender>>>` is also a single slot that gets replaced
4. `Message::SessionStarted` updates legacy global state fields (`device_name`, `platform`, `phase`) instead of session-specific state
5. The `SessionManager` and `SessionHandle` exist with proper fields (`process`, `cmd_sender`) but are never populated

**Affected Files:**
- `src/tui/mod.rs` - handle_action, run_with_project
- `src/app/handler.rs` - DeviceSelected handler, SessionStarted handler
- `src/app/state.rs` - legacy single-session fields

---

### Bug 3: Quit Doesn't Terminate All Sessions
**Symptom:** Pressing 'q' exits Flutter Demon but leaves Flutter apps running on all devices.

**Expected:** 
- Pressing 'x' should close only the currently selected session and switch to another available session
- Pressing 'q' should prompt for confirmation if sessions are running, then terminate ALL sessions across all devices

**Root Cause Analysis:**
1. 'q' key maps directly to `Message::Quit` which immediately sets `phase = AppPhase::Quitting`
2. No call to `state.request_quit()` which would check for running sessions and show confirmation
3. 'x' key is not mapped to `Message::CloseCurrentSession`
4. Cleanup code in `run_with_project` only handles the single `flutter` variable or single `session_task` - not multiple sessions
5. Confirmation dialog rendering (`UiMode::ConfirmDialog`) is stubbed with TODO comment

**Affected Files:**
- `src/app/handler.rs` - handle_key_normal, update
- `src/tui/mod.rs` - cleanup path
- `src/tui/render.rs` - ConfirmDialog rendering

---

### Bug 4: tui/mod.rs Too Large
**Symptom:** `src/tui/mod.rs` is ~740 lines and difficult to navigate.

**Expected:** Code should be split into logical modules for maintainability.

**Affected Files:** `src/tui/mod.rs`

---

## Affected Modules

- `src/tui/mod.rs`: Main TUI orchestration (~740 lines, needs split)
- `src/tui/render.rs`: View function (already has ConfirmDialog TODO)
- `src/tui/widgets/device_selector.rs`: Loading indicator rendering
- `src/app/handler.rs`: Message handlers for device selection, session lifecycle, keyboard
- `src/app/state.rs`: AppState with legacy single-session fields
- `src/app/session_manager.rs`: SessionManager (exists but underutilized)
- `src/app/session.rs`: Session and SessionHandle structs

---

## Phases

### Phase 1: Multi-Session Architecture Fix (Bug 2) - Critical
Fix the core architectural issue preventing multiple concurrent sessions. This is the foundation for proper quit behavior.

**Steps:**
1. Update `SpawnSession` action to include `SessionId` 
2. Modify `DeviceSelected` handler to create session in manager first, then spawn
3. Replace single `session_task` with per-session task tracking: `HashMap<SessionId, JoinHandle>`
4. Update `handle_action::SpawnSession` to store `cmd_sender` in `SessionHandle` instead of shared mutex
5. Route daemon events to correct session based on `app_id` or `device_id`
6. Update `SessionStarted` to modify session state, not global state
7. Ensure session tabs and logs display correctly with multiple sessions

**Measurable Outcomes:**
- Starting second session shows both in tabs
- Each session's logs are independent
- Switching tabs (1-9, Tab, Shift+Tab) shows correct session's logs
- Session status indicators update per-session

---

### Phase 2: Quit & Close Session Behavior (Bug 3)
Implement proper session closing and application quit with confirmation.

**Steps:**
1. Map 'x' key to `Message::CloseCurrentSession` in normal mode
2. Change 'q' key handler to call `state.request_quit()` instead of direct `Message::Quit`
3. Implement confirmation dialog widget and rendering
4. Handle 'y'/'n' keys in confirm dialog mode
5. Implement multi-session shutdown: iterate all sessions, stop each app, wait for processes
6. Update cleanup path in `run_with_project` to use session manager

**Measurable Outcomes:**
- 'x' closes current session and switches to another (or shows device selector if last)
- 'q' shows "Quit all sessions? (y/n)" when sessions are running
- 'y' in dialog terminates all Flutter processes gracefully
- 'n' in dialog returns to normal mode
- No orphaned Flutter processes after quit

---

### Phase 3: UI Polish (Bugs 1 & 4)
Independent UI improvements for progress indicator and code organization.

**Steps:**
1. Replace text spinner with `LineGauge` widget in device selector loading state
2. Implement indeterminate animation using `animation_frame` to calculate bouncing ratio
3. Split `tui/mod.rs` into:
   - `tui/runner.rs` - `run_with_project`, `run`, `run_loop`, `process_message`
   - `tui/actions.rs` - `handle_action`, `execute_task`
   - `tui/spawn.rs` - `spawn_device_discovery`, `spawn_emulator_*`, etc.
4. Update `tui/mod.rs` to be thin re-export module

**Measurable Outcomes:**
- Device selector shows smooth animated progress bar during discovery
- Each new module is under 300 lines
- All existing tests pass
- No public API changes

---

## Edge Cases & Risks

### Multi-Session Complexity
- **Risk:** Race conditions when multiple sessions emit events simultaneously
- **Mitigation:** Route events by app_id which is unique per process; use session_manager mutex for state updates

### Session Cleanup on Crash
- **Risk:** If a session's Flutter process crashes, handle may not clean up
- **Mitigation:** Handle DaemonEvent::Exited to remove crashed sessions

### Device Reuse Prevention
- **Risk:** User might try to start second session on same device
- **Mitigation:** Check `session_manager.find_by_device_id()` before allowing new session

### Shutdown Timeout
- **Risk:** Graceful shutdown may hang if Flutter process is unresponsive
- **Mitigation:** Use tokio::timeout (already exists, 10s) and force-kill after

### LineGauge Animation Frame Rate
- **Risk:** Animation may be too fast/slow or jerky
- **Mitigation:** Use existing tick mechanism; tune animation speed via modulo on frame counter

---

## Further Considerations

1. **Should we limit max concurrent sessions?** Currently MAX_SESSIONS = 9, which aligns with 1-9 keyboard shortcuts.

2. **How should auto-reload work with multiple sessions?** Options:
   - Reload ALL sessions on file change (current design assumption)
   - Only reload the selected/focused session
   
3. **Should session ordering be preserved?** Currently uses Vec for order, but crashed sessions leave gaps.

4. **Memory management:** Each session accumulates logs independently. Need to ensure per-session log trimming works.

5. **What if user closes device selector with Esc when no sessions exist?** Currently stays on selector (correct behavior per HideDeviceSelector handler).

---

## Task Dependency Graph

```
Phase 1 (Multi-Session Architecture)
├── 01-spawnSession-with-sessionId
├── 02-create-session-on-device-select
│   └── depends on: 01
├── 03-per-session-task-tracking
│   └── depends on: 02
├── 04-session-cmd-sender-storage
│   └── depends on: 02, 03
├── 05-event-routing-to-sessions
│   └── depends on: 04
└── 06-session-started-handler
    └── depends on: 05

Phase 2 (Quit Behavior) - depends on Phase 1
├── 07-x-key-close-session
├── 08-q-key-request-quit
├── 09-confirm-dialog-ui
│   └── depends on: 08
├── 10-multi-session-shutdown
    └── depends on: 09

Phase 3 (UI Polish) - independent
├── 11-linegauge-progress
└── 12-refactor-tui-mod
```

---

## Success Criteria

### Phase 1 Complete When:
- [ ] Can start 3 sessions on different devices simultaneously
- [ ] Tab bar shows all 3 sessions with correct status icons
- [ ] Switching sessions (Tab, 1-2-3) shows correct logs
- [ ] Each session can hot-reload independently
- [ ] Session logs don't leak between sessions

### Phase 2 Complete When:
- [ ] 'x' closes current session without affecting others
- [ ] Closing last session returns to device selector
- [ ] 'q' shows confirmation dialog when sessions exist
- [ ] Confirming quit stops ALL Flutter processes
- [ ] `ps aux | grep flutter` shows no orphaned processes after quit

### Phase 3 Complete When:
- [ ] Device selector shows animated progress bar during discovery
- [ ] `tui/mod.rs` is under 100 lines (just module declarations and re-exports)
- [ ] All tests pass
- [ ] Cargo clippy has no new warnings

---

## Milestone Deliverable

A fully functional multi-session Flutter development environment where:
1. Users can run their app on multiple devices simultaneously
2. Each session has independent logging and controls
3. Clean shutdown ensures no orphaned processes
4. UI provides clear visual feedback during loading operations
5. Codebase is well-organized and maintainable