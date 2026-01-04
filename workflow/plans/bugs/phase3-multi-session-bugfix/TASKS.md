# Phase 3 Multi-Session Bugfix - Task Index

## Overview

This bugfix addresses 4 issues identified after Phase 3 completion:
1. Device selector missing animated LineGauge progress indicator
2. New session replaces existing session instead of multi-session mode
3. Quit doesn't terminate all running sessions properly
4. `tui/mod.rs` is too large and needs refactoring

Additional fixes added:
5. Device selector footer not visible (color issue)
6. Esc keybinding shown when it does nothing (no sessions running)
7. Device discovery is slow, needs caching for instant display on subsequent opens
8. `app/handler.rs` is over 3000 lines and needs refactoring

Total: **14 tasks** across 4 phases

---

## Task Dependency Graph

```
Phase 1: Multi-Session Architecture (Bug 2 - Critical)
─────────────────────────────────────────────────────
01-spawn-session-with-id ─────────────────────────┐
                                                  │
02-create-session-on-device-select ◄──────────────┘
           │
           ▼
03-per-session-task-tracking
           │
           ▼
04-session-cmd-sender-storage
           │
           ▼
05-event-routing-to-sessions
           │
           ▼
06-session-started-handler


Phase 2: Quit & Close Session Behavior (Bug 3)
───────────────────────────────────────────────
07-x-key-close-session ─────────────────────────┐
                                                │
08-q-key-request-quit ◄─────────────────────────┤
           │                                    │
           ▼                                    │
09-confirm-dialog-ui                            │
           │                                    │
           ▼                                    │
10-multi-session-shutdown ◄─────────────────────┘
                │
                └── depends on Phase 1 completion


Phase 3: UI Polish (Bugs 1, 5, 6, 7) - Independent
──────────────────────────────────────────────────
11-linegauge-progress (standalone - also fixes footer & Esc display)
           │
           ▼
11a-device-cache (depends on 11 for LineGauge)


Phase 4: Code Quality Refactoring (Bugs 4, 8) - Independent
───────────────────────────────────────────────────────────
12-refactor-tui-mod (standalone)

13-refactor-handler (standalone)
```

---

## Tasks

| # | Task | Status | Depends On | Modules |
|---|------|--------|------------|---------|
| 01 | [spawn-session-with-id](tasks/01-spawn-session-with-id.md) | ✅ Done | - | `handler.rs`, `mod.rs` |
| 02 | [create-session-on-device-select](tasks/02-create-session-on-device-select.md) | ✅ Done | 01 | `handler.rs` |
| 03 | [per-session-task-tracking](tasks/03-per-session-task-tracking.md) | ✅ Done | 02 | `mod.rs` |
| 04 | [session-cmd-sender-storage](tasks/04-session-cmd-sender-storage.md) | ✅ Done | 02, 03 | `mod.rs`, `session.rs` |
| 05 | [event-routing-to-sessions](tasks/05-event-routing-to-sessions.md) | ✅ Done | 04 | `handler.rs`, `mod.rs` |
| 06 | [session-started-handler](tasks/06-session-started-handler.md) | ✅ Done | 05 | `handler.rs` |
| 07 | [x-key-close-session](tasks/07-x-key-close-session.md) | ✅ Done | - | `handler.rs` |
| 08 | [q-key-request-quit](tasks/08-q-key-request-quit.md) | ✅ Done | - | `handler.rs` |
| 09 | [confirm-dialog-ui](tasks/09-confirm-dialog-ui.md) | ✅ Done | 08 | `render.rs`, `widgets/` |
| 10 | [multi-session-shutdown](tasks/10-multi-session-shutdown.md) | ✅ Done | Phase 1, 09 | `mod.rs`, `handler.rs` |
| 11 | [linegauge-progress](tasks/11-linegauge-progress.md) | ✅ Done | - | `widgets/device_selector.rs`, `render.rs` |
| 11a | [device-cache](tasks/11a-device-cache.md) | ✅ Done | 11 | `widgets/device_selector.rs`, `handler.rs` |
| 12 | [refactor-tui-mod](tasks/12-refactor-tui-mod.md) | ✅ Done | - | `tui/mod.rs` → multiple files |
| 13 | [refactor-handler](tasks/13-refactor-handler.md) | Not Started | - | `app/handler.rs` → multiple files |

---

## Task Summaries

### Phase 1: Multi-Session Architecture

| Task | Summary |
|------|---------|
| 01 | Add `session_id: SessionId` to `UpdateAction::SpawnSession` for tracking which session is being spawned |
| 02 | Modify `DeviceSelected` handler to call `session_manager.create_session()` before returning SpawnSession action |
| 03 | Replace `session_task: Option<JoinHandle>` with `HashMap<SessionId, JoinHandle>` for multiple concurrent tasks |
| 04 | Store `CommandSender` in `SessionHandle.cmd_sender` instead of shared mutex; use session manager for lookups |
| 05 | Route incoming daemon events to correct session based on `app_id` or `device_id` |
| 06 | Update `SessionStarted` handler to modify session state in manager instead of global legacy fields |

### Phase 2: Quit & Close Session

| Task | Summary |
|------|---------|
| 07 | Map 'x' key and Ctrl+W to `Message::CloseCurrentSession` in normal mode |
| 08 | Change 'q' key to call `request_quit()` which shows confirmation dialog if sessions running |
| 09 | Implement `ConfirmDialog` widget rendering with yes/no prompt |
| 10 | Implement shutdown loop that stops all sessions, sends stop commands, waits for process exit |

### Phase 3: UI Polish

| Task | Summary |
|------|---------|
| 11 | Replace text spinner with animated `LineGauge` widget, fix footer visibility (DarkGray on DarkGray), conditionally show Esc keybinding only when sessions are running |
| 11a | Cache discovered devices for instant display on subsequent device selector opens; show header LineGauge refresh indicator while updating cached list |

### Phase 4: Code Quality Refactoring

| Task | Summary |
|------|---------|
| 12 | Split `tui/mod.rs` (872 lines) into `runner.rs`, `actions.rs`, `spawn.rs` modules |
| 13 | Split `app/handler.rs` (3318 lines) into focused modules: `update.rs`, `daemon.rs`, `session.rs`, `keys.rs` |

---

## Implementation Order

### Week 1: Critical Path (Phase 1) - ✅ COMPLETE
1. Task 01: Add SessionId to SpawnSession (1 hour) ✅
2. Task 02: Create session before spawn (2 hours) ✅
3. Task 03: Per-session task HashMap (3 hours) ✅
4. Task 04: SessionHandle cmd_sender (3 hours) ✅
5. Task 05: Event routing (4 hours) ✅
6. Task 06: SessionStarted handler (2 hours) ✅

**Checkpoint:** Multi-session mode works - multiple tabs visible, independent logs ✅

### Week 2: Quit Behavior (Phase 2) - ✅ COMPLETE
7. Task 07: 'x' key mapping (1 hour) ✅
8. Task 08: 'q' key flow (2 hours) ✅
9. Task 09: Confirm dialog UI (3 hours) ✅
10. Task 10: Multi-session shutdown (4 hours) ✅

**Checkpoint:** Clean shutdown of all sessions, no orphans ✅

### Week 2-3: Polish (Phase 3) - ✅ COMPLETE
11. Task 11: LineGauge animation + footer fixes (3 hours) ✅
11a. Task 11a: Device caching with refresh indicator (3 hours) ✅

**Checkpoint:** Polished UI with smooth animations ✅

### Week 3-4: Refactoring (Phase 4) - NOT STARTED
12. Task 12: Refactor tui/mod.rs (3 hours)
13. Task 13: Refactor app/handler.rs (4 hours)

**Checkpoint:** All modules under 400 lines, clippy clean

---

## Testing Strategy

### Integration Tests
- Start 2 sessions on different emulators
- Verify tab switching shows correct logs
- Verify 'x' closes one session, other continues
- Verify 'q' → 'y' terminates both processes

### Unit Tests
- Session creation and ID assignment
- Event routing by app_id/device_id
- Confirm dialog state transitions
- LineGauge animation frame calculation
- Device cache population and retrieval
- Footer text conditional on session state

### Manual Testing
- Visual inspection of LineGauge animation smoothness
- Process cleanup verification with `ps aux | grep flutter`
- Multi-device hot reload coordination

---

## Module Structure After Refactoring

### Task 12: tui/mod.rs Refactoring

```
src/tui/
├── mod.rs              # Re-exports only (~50 lines)
├── runner.rs           # run_with_project, run, run_loop (~300 lines)
├── actions.rs          # handle_action, execute_task (~200 lines)
├── spawn.rs            # spawn_device_discovery, spawn_emulator_*, spawn_session (~250 lines)
├── process.rs          # process_message (~100 lines)
├── event.rs            # (existing)
├── layout.rs           # (existing)
├── render.rs           # (existing)
├── selector.rs         # (existing)
├── terminal.rs         # (existing)
└── widgets/            # (existing)
```

### Task 13: app/handler.rs Refactoring

```
src/app/
├── handler.rs          # Re-exports, UpdateAction, UpdateResult, main update() (~200 lines)
├── handler/
│   ├── mod.rs          # Re-exports handler submodules
│   ├── update.rs       # Main update() dispatch logic (~200 lines)
│   ├── daemon.rs       # handle_daemon_event, handle_session_daemon_event (~200 lines)
│   ├── session.rs      # handle_session_stdout, handle_session_exited, handle_session_message_state (~150 lines)
│   ├── keys.rs         # handle_key_* functions for different UI modes (~200 lines)
│   └── helpers.rs      # detect_raw_line_level, handle_daemon_message_state (~100 lines)
├── message.rs          # (existing)
├── session.rs          # (existing)
└── state.rs            # (existing)

tests/
├── handler_tests.rs    # All tests from handler.rs moved here (~2000 lines)
```

**Current handler.rs breakdown:**
- Lines 1-90: Enums and structs (UpdateAction, Task, UpdateResult)
- Lines 91-756: Main `update()` function - dispatch logic
- Lines 759-856: `handle_daemon_event()` 
- Lines 859-917: `handle_session_daemon_event()`
- Lines 920-974: `handle_session_stdout()`
- Lines 977-999: `handle_session_exited()`
- Lines 1002-1035: `handle_session_message_state()`
- Lines 1038-1077: `detect_raw_line_level()`
- Lines 1080-1094: `handle_daemon_message_state()`
- Lines 1097-1257: `handle_key_*()` functions
- Lines 1260-3318: **Tests (2058 lines - 62% of file!)**

**Recommended approach for Task 13:**
1. Move all tests to a separate `tests/handler_tests.rs` file (biggest win, reduces to ~1260 lines)
2. Extract key handling to `keys.rs` (~160 lines saved)
3. Extract daemon event handling to `daemon.rs` (~100 lines saved)  
4. Extract session event handling to `session.rs` (~120 lines saved)
5. Result: Core handler.rs ~800 lines, well under target

---

## Risk Mitigation

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Race conditions with concurrent sessions | Medium | High | Use session_manager mutex for all state access |
| Orphaned processes on crash | Low | Medium | Add signal handlers, periodic process health check |
| Event misrouting | Medium | Medium | Add logging for event routing decisions |
| Refactoring breaks existing functionality | Low | High | Run full test suite after each task |
| Test isolation after extraction | Low | Medium | Ensure all test helpers are accessible |

---

## Success Metrics

1. **Multi-session:** Can run 3 devices concurrently, switch between them, independent logs ✅
2. **Clean shutdown:** `ps aux | grep flutter` returns 0 results after quit ✅
3. **UI polish:** Animated progress bar visible, smooth animation ✅
4. **Code quality:** No module over 400 lines (excluding tests), no new clippy warnings
5. **Test coverage:** All existing tests pass after refactoring

---

## Completion Status

| Phase | Tasks | Complete | Remaining |
|-------|-------|----------|-----------|
| Phase 1: Multi-Session | 6 | 6 | 0 |
| Phase 2: Quit Behavior | 4 | 4 | 0 |
| Phase 3: UI Polish | 2 | 2 | 0 |
| Phase 4: Refactoring | 2 | 1 | 1 |
| **Total** | **14** | **13** | **1** |

**Overall Progress: 93% Complete**