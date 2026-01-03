# Phase 3 Multi-Session Bugfix - Task Index

## Overview

This bugfix addresses 4 issues identified after Phase 3 completion:
1. Device selector missing animated LineGauge progress indicator
2. New session replaces existing session instead of multi-session mode
3. Quit doesn't terminate all running sessions properly
4. `tui/mod.rs` is too large and needs refactoring

Total: **12 tasks** across 3 phases

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


Phase 3: UI Polish (Bugs 1 & 4) - Independent
─────────────────────────────────────────────
11-linegauge-progress (standalone)

12-refactor-tui-mod (standalone)
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
| 11 | [linegauge-progress](tasks/11-linegauge-progress.md) | Not Started | - | `widgets/device_selector.rs` |
| 12 | [refactor-tui-mod](tasks/12-refactor-tui-mod.md) | Not Started | - | `tui/mod.rs` → multiple files |

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
| 11 | Replace text spinner with animated `LineGauge` widget using indeterminate bouncing animation |
| 12 | Split `tui/mod.rs` into `runner.rs`, `actions.rs`, `spawn.rs` modules |

---

## Implementation Order

### Week 1: Critical Path (Phase 1)
1. Task 01: Add SessionId to SpawnSession (1 hour)
2. Task 02: Create session before spawn (2 hours)
3. Task 03: Per-session task HashMap (3 hours)
4. Task 04: SessionHandle cmd_sender (3 hours)
5. Task 05: Event routing (4 hours)
6. Task 06: SessionStarted handler (2 hours)

**Checkpoint:** Multi-session mode works - multiple tabs visible, independent logs

### Week 2: Quit Behavior (Phase 2)
7. Task 07: 'x' key mapping (1 hour)
8. Task 08: 'q' key flow (2 hours)
9. Task 09: Confirm dialog UI (3 hours)
10. Task 10: Multi-session shutdown (4 hours)

**Checkpoint:** Clean shutdown of all sessions, no orphans

### Week 2-3: Polish (Phase 3)
11. Task 11: LineGauge animation (2 hours)
12. Task 12: Refactor tui/mod.rs (3 hours)

**Checkpoint:** All tests pass, clippy clean

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

### Manual Testing
- Visual inspection of LineGauge animation smoothness
- Process cleanup verification with `ps aux | grep flutter`
- Multi-device hot reload coordination

---

## New Module Structure After Refactoring (Task 12)

```
src/tui/
├── mod.rs              # Re-exports only (~50 lines)
├── runner.rs           # run_with_project, run, run_loop, process_message (~250 lines)
├── actions.rs          # handle_action, execute_task (~200 lines)
├── spawn.rs            # spawn_device_discovery, spawn_emulator_*, etc. (~100 lines)
├── event.rs            # (existing)
├── layout.rs           # (existing)
├── render.rs           # (existing)
├── selector.rs         # (existing)
├── terminal.rs         # (existing)
└── widgets/            # (existing)
```

---

## Risk Mitigation

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Race conditions with concurrent sessions | Medium | High | Use session_manager mutex for all state access |
| Orphaned processes on crash | Low | Medium | Add signal handlers, periodic process health check |
| Event misrouting | Medium | Medium | Add logging for event routing decisions |
| Refactoring breaks existing functionality | Low | High | Run full test suite after each task |

---

## Success Metrics

1. **Multi-session:** Can run 3 devices concurrently, switch between them, independent logs
2. **Clean shutdown:** `ps aux | grep flutter` returns 0 results after quit
3. **UI polish:** Animated progress bar visible, smooth animation
4. **Code quality:** No module over 300 lines, no new clippy warnings