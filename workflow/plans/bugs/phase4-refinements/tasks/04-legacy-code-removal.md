## Task 04: Legacy Single-Session Code Removal (Overview)

**Objective**: Remove all backward compatibility code for single-session mode, fully committing to the multi-session architecture. This is a significant refactor that has been broken into 7 incremental subtasks.

**Depends on**: Tasks 01, 02, 03, 05 (all must be complete)

**Status**: Not Started

---

### Why This Refactor?

The codebase currently maintains two parallel code paths:
1. **Legacy single-session mode**: Uses global `AppState` fields (`current_app_id`, `logs`, `phase`, etc.) and `Message::Daemon` events
2. **Multi-session mode**: Uses `SessionManager` with per-session state and `Message::SessionDaemon` events

This dual architecture:
- Adds ~500+ lines of redundant code
- Creates confusion about which state to use
- Makes bug fixes harder (changes needed in two places)
- Increases cognitive load for maintenance

After this refactor, there will be ONE way to run Flutter sessions: through `SessionManager`.

---

### Subtask Breakdown

This task has been split into 7 subtasks that **MUST be done in order**:

| # | Subtask | Effort | Risk | Key Changes |
|---|---------|--------|------|-------------|
| [4a](04a-autostart-session-refactor.md) | Auto-start Session Refactor | 2 hrs | Medium | Refactor auto-start to use sessions |
| [4b](04b-remove-message-daemon.md) | Remove Message::Daemon | 1 hr | Low-Med | Remove legacy daemon event path |
| [4c](04c-remove-fallback-paths.md) | Remove Fallback Paths | 1 hr | Low | Remove `current_app_id` fallbacks |
| [4d](04d-remove-global-state-updates.md) | Remove Global State Updates | 0.5 hrs | Low | Stop updating global state |
| [4e](04e-remove-appstate-fields.md) | Remove AppState Fields | 2 hrs | High | Remove legacy fields/methods |
| [4f](04f-cleanup-actions-legacy.md) | Clean Up actions.rs | 0.5 hrs | Low-Med | Remove session_id checks |
| [4g](04g-update-tests.md) | Update Tests | 1.5 hrs | Medium | Fix/remove legacy tests |

**Total Estimated Effort: 8.5 hours**

---

### Dependency Chain

```
Tasks 01-03, 05 (Prerequisites)
         │
         ▼
   ┌─────────────────────────────────────┐
   │  4a: Auto-start Session Refactor   │
   │  - Removes direct FlutterProcess   │
   │    ownership in startup            │
   └─────────────┬───────────────────────┘
                 │
                 ▼
   ┌─────────────────────────────────────┐
   │  4b: Remove Message::Daemon        │
   │  - Removes legacy event variant    │
   │  - Removes handle_daemon_event()   │
   └─────────────┬───────────────────────┘
                 │
                 ▼
   ┌─────────────────────────────────────┐
   │  4c: Remove Fallback Paths         │
   │  - Removes current_app_id fallbacks│
   │  - Removes session_id: 0 pattern   │
   └─────────────┬───────────────────────┘
                 │
                 ▼
   ┌─────────────────────────────────────┐
   │  4d: Remove Global State Updates   │
   │  - Stops updating global fields    │
   │    from session events             │
   └─────────────┬───────────────────────┘
                 │
                 ▼
   ┌─────────────────────────────────────┐
   │  4e: Remove AppState Fields        │
   │  - Removes ~12 legacy fields       │
   │  - Removes ~10 legacy methods      │
   │  - Highest risk - many compile errs│
   └─────────────┬───────────────────────┘
                 │
                 ▼
   ┌─────────────────────────────────────┐
   │  4f: Clean Up actions.rs           │
   │  - Removes session_id > 0 checks   │
   │  - Removes global cmd_sender usage │
   └─────────────┬───────────────────────┘
                 │
                 ▼
   ┌─────────────────────────────────────┐
   │  4g: Update Tests                  │
   │  - Removes 5 obsolete tests        │
   │  - Updates ~15+ tests              │
   └─────────────────────────────────────┘
```

---

### What Gets Removed

#### Message Enum
- `Message::Daemon(DaemonEvent)` variant

#### Functions
- `handle_daemon_event()` (~97 lines)
- `handle_daemon_message_state()` (~17 lines)
- `route_legacy_daemon_response()` (~15 lines)
- `route_daemon_response()` (~18 lines)

#### AppState Fields (11 fields)
- `current_app_id: Option<String>`
- `device_name: Option<String>`
- `platform: Option<String>`
- `flutter_version: Option<String>`
- `session_start: Option<DateTime<Local>>`
- `reload_start_time: Option<Instant>`
- `last_reload_time: Option<DateTime<Local>>`
- `reload_count: u32`
- `logs: Vec<LogEntry>`
- `log_view_state: LogViewState`
- `max_logs: usize`

**Note**: `phase: AppPhase` is KEPT for app-level quitting state.

#### AppState Methods (11 methods)
- `add_log()`, `log_info()`, `log_error()`
- `start_reload()`, `record_reload_complete()`
- `reload_elapsed()`, `last_reload_display()`
- `session_duration()`, `session_duration_display()`
- `start_session()`, `set_device_info()`, `is_busy()`

#### Legacy Code Patterns
- Fallback to `current_app_id` in HotReload/HotRestart/StopApp/AutoReloadTriggered
- `session_id: 0` for legacy mode tasks
- Global `cmd_sender` updates for "backward compatibility"
- Global state updates from session events

---

### Code Impact Summary

| Category | Lines Removed | Lines Changed |
|----------|---------------|---------------|
| Functions | ~150 | 0 |
| Fields & Methods | ~140 | 0 |
| Fallback Paths | ~50 | ~15 |
| Legacy Patterns | ~70 | ~80 |
| Tests | ~100 | ~200 |
| **Total** | **~510** | **~295** |

---

### Acceptance Criteria

1. ✅ No `Message::Daemon` variant exists
2. ✅ No `handle_daemon_event()` function exists
3. ✅ No legacy fields in `AppState` (except `phase`)
4. ✅ No legacy methods in `AppState`
5. ✅ No fallback to `current_app_id` anywhere
6. ✅ No `session_id: 0` patterns
7. ✅ No "legacy" or "backward compat" comments
8. ✅ Auto-start creates sessions like manual start
9. ✅ All tests pass
10. ✅ `cargo clippy` clean
11. ✅ All multi-session functionality preserved

---

### Testing Strategy

#### After Each Subtask
- `cargo check` passes
- `cargo clippy` shows no new warnings
- Run affected tests

#### After All Subtasks
- Full `cargo test` passes
- Manual testing checklist:
  - [ ] Manual device selection works
  - [ ] Auto-start with launch.toml works
  - [ ] Hot reload works (r key)
  - [ ] Hot restart works (R key)
  - [ ] File watcher auto-reload works
  - [ ] Session switching works (Tab, 1-9)
  - [ ] Session closing works (x)
  - [ ] Quit with confirmation works (q)
  - [ ] Force quit works (Ctrl+C)
  - [ ] Multiple sessions run independently
  - [ ] Logs display correctly per session

---

### Risk Mitigation

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Breaking auto-start | Medium | High | Test thoroughly after 4a |
| Missing code paths | Low | Medium | Compiler catches most |
| Test failures | High | Low | Expected, fixed in 4g |
| Regression in reload | Medium | High | Manual testing after 4c |
| Orphaned processes | Low | Medium | Shutdown logic unchanged |

---

### Rollback Plan

Each subtask should be a separate commit. If issues arise:
1. Revert to previous commit
2. Identify the problem
3. Fix and re-apply

Consider using a feature branch for the entire Task 4 to enable easy rollback if needed.

---

### Progress Tracking

| Subtask | Status | Date Completed | Notes |
|---------|--------|----------------|-------|
| 4a | Not Started | - | - |
| 4b | Not Started | - | - |
| 4c | Not Started | - | - |
| 4d | Not Started | - | - |
| 4e | Not Started | - | - |
| 4f | Not Started | - | - |
| 4g | Not Started | - | - |