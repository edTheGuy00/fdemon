## Task: Legacy Single-Session Code Removal

**Objective**: Remove all backward compatibility code for single-session mode, fully committing to the multi-session architecture. This is a significant refactor that touches many modules.

**Depends on**: Tasks 01, 02, 03 (complete these first as they're lower risk)

---

### Scope

#### `src/app/state.rs` (Major Changes)
Remove legacy single-session fields:
- `phase: AppPhase` — Sessions have their own phase
- `logs: Vec<LogEntry>` — Sessions have their own logs
- `log_view_state: LogViewState` — Sessions have their own scroll state
- `max_logs: usize` — Configured per-session
- `current_app_id: Option<String>` — Sessions track their own app_id
- `device_name: Option<String>` — Sessions have device_name
- `platform: Option<String>` — Sessions have platform
- `flutter_version: Option<String>` — Move to session or remove
- `session_start: Option<DateTime<Local>>` — Sessions have started_at
- `reload_start_time: Option<Instant>` — Sessions have reload_start_time
- `last_reload_time: Option<DateTime<Local>>` — Sessions have last_reload_time
- `reload_count: u32` — Sessions have reload_count

Remove legacy helper methods:
- `add_log()`, `log_info()`, `log_error()` — Use session's methods
- `start_reload()`, `record_reload_complete()` — Use session's methods
- `reload_elapsed()`, `last_reload_display()` — Use session's methods
- `session_duration()`, `session_duration_display()` — Use session's methods
- `start_session()`, `set_device_info()`, `is_busy()` — Use session's methods

Keep methods that still make sense at app level:
- `should_quit()` — Uses global quitting flag
- `request_quit()`, `force_quit()`, `confirm_quit()`, `cancel_quit()` — App-level quit handling
- `show_device_selector()`, `hide_device_selector()` — Modal control
- `has_running_sessions()` — Delegates to session_manager

#### `src/app/message.rs`
- Remove `Message::Daemon(DaemonEvent)` variant entirely
- Keep only `Message::SessionDaemon { session_id, event }` for daemon events

#### `src/app/handler/daemon.rs`
- Remove `handle_daemon_event()` function (legacy single-session handler)
- Keep only `handle_session_daemon_event()` for multi-session mode
- Update module documentation to remove "legacy" references

#### `src/app/handler/update.rs`
Remove legacy fallback paths in:
- `Message::HotReload` — Remove fallback to `state.current_app_id`
- `Message::HotRestart` — Remove fallback to `state.current_app_id`
- `Message::StopApp` — Remove fallback to `state.current_app_id`
- `Message::FilesChanged` — Remove fallback to `state.current_app_id`
- `Message::SessionStarted` — Remove updates to legacy global state fields

#### `src/app/handler/session.rs`
- Remove "legacy compatibility" updates in `handle_session_message_state()`:
  - Remove: `state.current_app_id = Some(app_start.app_id.clone())`
  - Remove: `state.current_app_id = None` (in stop handler)

#### `src/app/handler/mod.rs`
- Update module documentation to remove "legacy" references
- Potentially remove re-export of `handle_daemon_event` if removed

#### `src/tui/runner.rs`
- Remove legacy `daemon_rx` channel creation
- Remove `route_daemon_response()` function (legacy response routing)
- Simplify startup to only use session-based flow
- Remove passing of daemon_rx to run_loop

#### `src/tui/startup.rs`
- Refactor auto-start mode to create a session instead of owning FlutterProcess directly
- Remove the `flutter: Option<FlutterProcess>` return value path
- Always use session-based flow for both auto-start and manual start
- Update `cleanup_sessions()` to remove the `flutter` parameter path

#### `src/tui/actions.rs`
- Remove setting global `cmd_sender` for "backward compatibility"
- Remove: `*cmd_sender_clone.lock().await = Some(session_sender.clone());`
- Remove: `*guard = None;` cleanup of global sender

#### `src/tui/process.rs`
- Remove handling of `Message::Daemon(_)` in process_message
- Only handle `Message::SessionDaemon { session_id, event }`

#### `src/tui/render.rs`
- Remove fallback to global logs when no session selected
- Either show empty log view or a "No session" message

#### `src/app/handler/tests.rs`
- Remove test `test_session_started_updates_legacy_global_state`
- Remove tests that rely on legacy fallback behavior
- Update tests that checked legacy field updates

---

### Implementation Strategy

This is a large refactor. Recommended approach:

**Step 1: Create compatibility shim (temporary)**
Before removing anything, ensure all code paths go through session manager.

**Step 2: Remove Message::Daemon variant**
- Update message.rs
- Update process.rs to not handle this variant
- Update any code that sends Message::Daemon

**Step 3: Remove legacy handler**
- Remove handle_daemon_event from daemon.rs
- Update handler/mod.rs

**Step 4: Remove legacy fallbacks in update.rs**
- Remove current_app_id fallback paths
- Require selected session for reload/restart/stop

**Step 5: Remove legacy global state updates**
- Update session.rs handlers
- Update SessionStarted handler in update.rs

**Step 6: Remove legacy fields from AppState**
- Remove fields one at a time
- Fix all compile errors after each removal
- Run tests after each batch

**Step 7: Update startup.rs**
- Refactor auto-start to use sessions
- Remove FlutterProcess ownership path
- Update cleanup_sessions signature

**Step 8: Clean up actions.rs**
- Remove global cmd_sender updates
- Simplify spawn_session

**Step 9: Update render.rs**
- Remove fallback to global logs
- Handle no-session case gracefully

**Step 10: Update/remove tests**
- Remove obsolete tests
- Update remaining tests to use session-based approach

---

### Fields to Remove from AppState

```rust
// REMOVE all of these:

// Legacy single-session fields (maintained for backward compatibility)
/// Current application phase
pub phase: AppPhase,

/// Log buffer
pub logs: Vec<LogEntry>,

/// Log view scroll state
pub log_view_state: LogViewState,

/// Maximum log buffer size
pub max_logs: usize,

// App Tracking
/// Current app ID (from daemon's app.start event)
pub current_app_id: Option<String>,

/// Device name (e.g., "iPhone 15 Pro")
pub device_name: Option<String>,

/// Platform (e.g., "ios", "android", "macos")
pub platform: Option<String>,

/// Flutter SDK version (if detected)
pub flutter_version: Option<String>,

/// When the Flutter app started
pub session_start: Option<DateTime<Local>>,

// Reload Tracking
/// When the current reload started (for timing)
pub reload_start_time: Option<Instant>,

/// When the last successful reload completed
pub last_reload_time: Option<DateTime<Local>>,

/// Total reload count this session
pub reload_count: u32,
```

**Keep these fields:**
```rust
// UI and modal state
pub ui_mode: UiMode,
pub session_manager: SessionManager,
pub device_selector: DeviceSelectorState,
pub settings: Settings,
pub confirm_dialog_state: Option<ConfirmDialogState>,
pub project_path: PathBuf,
pub project_name: Option<String>,
```

---

### Acceptance Criteria

1. ✅ No "legacy" or "backward compat" comments remain in codebase
2. ✅ `Message::Daemon` variant removed from Message enum
3. ✅ `handle_daemon_event()` function removed
4. ✅ AppState has no single-session fields (phase, logs, current_app_id, etc.)
5. ✅ All handlers use session-based approach only
6. ✅ Auto-start creates a session like manual start
7. ✅ Global cmd_sender only used for initial setup, not backward compat
8. ✅ All tests pass after removal
9. ✅ No compile warnings about unused fields
10. ✅ Application works correctly for multi-session use cases

---

### Testing

#### Compile-Time Verification
After each step, ensure:
- `cargo check` passes
- `cargo clippy` shows no new warnings
- No "unused" warnings for removed fields

#### Unit Tests
- Remove tests that verify legacy behavior
- Update tests that created sessions without going through session_manager
- Ensure all handler tests use SessionDaemon, not Daemon

#### Integration Testing
1. Start fdemon → select device → verify session created correctly
2. Start multiple sessions → verify all work independently
3. Reload/Restart → verify works via session, not global state
4. Quit → verify all sessions shut down
5. Auto-start mode → verify creates session, not legacy process

#### Manual Testing Checklist
- [ ] Manual device selection works
- [ ] Auto-start with launch.toml works
- [ ] Hot reload works for selected session
- [ ] Hot restart works for selected session
- [ ] File watcher auto-reload works
- [ ] Session switching works
- [ ] Session closing works
- [ ] Quit with confirmation works
- [ ] Force quit (Ctrl+C) works
- [ ] Multiple sessions run independently
- [ ] Logs display correctly per session
- [ ] Status bar shows correct info

---

### Risk Mitigation

| Risk | Mitigation |
|------|------------|
| Breaking auto-start mode | Test thoroughly before and after refactor |
| Missing code path causes crash | Incremental removal with compile checks |
| Tests fail after removal | Fix or remove tests incrementally |
| Edge cases not covered | Keep comprehensive test suite running |
| Regression in reload/restart | Manual testing after each step |

---

### Estimated Impact

**Lines removed (approximately):**
- state.rs: ~80 lines of legacy fields and methods
- daemon.rs: ~100 lines (handle_daemon_event)
- update.rs: ~60 lines of fallback paths
- session.rs: ~20 lines of legacy updates
- runner.rs: ~30 lines (daemon_rx, route_daemon_response)
- startup.rs: ~30 lines (flutter ownership path)
- actions.rs: ~10 lines (global sender updates)
- process.rs: ~5 lines (Daemon handling)
- tests.rs: ~100+ lines of legacy tests

**Total: ~450+ lines removed**

---

### Notes

- This task should be done last in Phase 4 as it has the highest risk
- Consider creating a feature branch for this refactor
- Run the full test suite after each major step
- If any step causes too many issues, it can be reverted and done incrementally
- The `should_quit()` method should remain on AppState as it's genuinely app-level state
- Consider keeping a minimal `phase` for app-level quitting state, separate from session phases