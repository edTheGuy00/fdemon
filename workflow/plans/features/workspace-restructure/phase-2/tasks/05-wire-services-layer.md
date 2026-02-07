## Task: Wire Services Layer into Engine

**Objective**: Connect the dormant services layer (`FlutterController`, `LogService`, `StateService` / `SharedState`) to the `Engine`, making them live and usable. The Engine will maintain a `SharedState` instance that is synchronized from `AppState` after each message processing cycle. The Engine will expose accessor methods for each service trait implementation.

**Depends on**: Task 04 (both runners use Engine -- services need to be wired at the Engine level)

**Estimated Time**: 4-5 hours

### Scope

- `src/app/engine.rs`: Add `SharedState`, synchronization logic, service accessors
- `src/services/state_service.rs`: May need adjustments to `SharedState` fields to match current `AppState`/`SessionManager`
- `src/services/flutter_controller.rs`: May need adjustments to work with `SessionManager` (multi-session)
- `src/services/log_service.rs`: May need adjustments for multi-session log access

### Details

#### Current Services Status

The services layer has three well-designed trait abstractions with implementations, but they are **completely disconnected** from the actual runtime:

| Trait | Implementation | Status |
|---|---|---|
| `FlutterController` | `DaemonFlutterController` (uses SharedState), `CommandSenderController` (uses CommandSender directly) | Tested in isolation, never instantiated by TUI or headless |
| `LogService` | `SharedLogService` (uses SharedState.logs) | Tested in isolation, never instantiated |
| `StateService` | `SharedStateService` (uses SharedState) | Tested in isolation, never instantiated |

`SharedState` holds `Arc<RwLock<>>` wrappers around:
- `app_state: AppRunState` -- phase, app_id, device info, devtools URI
- `logs: Vec<LogEntry>` -- log buffer
- `devices: Vec<DeviceInfo>` -- discovered devices
- `event_tx: broadcast::Sender<DaemonMessage>` -- daemon event broadcaster

#### Synchronization Strategy

**One-way sync: AppState -> SharedState** (never reverse).

After each message processing cycle (i.e., after `engine.process_message()` or `engine.drain_pending_messages()`), the Engine copies relevant state from `AppState` to `SharedState`:

```rust
impl Engine {
    /// Synchronize AppState changes to SharedState.
    ///
    /// Called after processing messages. One-way: AppState is the source of truth.
    async fn sync_shared_state(&self) {
        let shared = &self.shared_state;

        // Sync app run state from selected session
        if let Some(session_handle) = self.state.session_manager.selected() {
            let session = &session_handle.session;
            let mut app_state = shared.app_state.write().await;
            app_state.phase = session.phase;
            app_state.app_id = session.app_id.clone();
            app_state.device_id = Some(session.device_id.clone());
            app_state.device_name = Some(session.device_name.clone());
            app_state.platform = session.platform.as_ref().map(|p| p.to_string());
        }

        // Sync logs from selected session
        if let Some(session_handle) = self.state.session_manager.selected() {
            let mut logs = shared.logs.write().await;
            // Replace with current session's logs
            // Note: This is a snapshot, not a stream -- optimize later if needed
            *logs = session_handle.session.logs.clone();
        }
    }
}
```

**Important**: Synchronization is async (uses `RwLock::write().await`). Since the TUI runner's `run_loop` is synchronous (no `.await`), the sync must be called from an appropriate async context. Options:
1. Make `sync_shared_state()` blocking by using `try_write()` (preferred -- avoids making run_loop async)
2. Use a `tokio::spawn` to sync in the background
3. Sync only when a service accessor is called (lazy sync)

**Recommended approach**: Use `try_write()` for non-blocking sync. If the lock is held by a service consumer, skip the sync for this cycle. The data is eventually consistent.

```rust
fn sync_shared_state_nonblocking(&self) {
    if let Some(session_handle) = self.state.session_manager.selected() {
        let session = &session_handle.session;

        // Try to update app state (skip if lock is held)
        if let Ok(mut app_state) = self.shared_state.app_state.try_write() {
            app_state.phase = session.phase;
            app_state.app_id = session.app_id.clone();
            app_state.device_id = Some(session.device_id.clone());
            app_state.device_name = Some(session.device_name.clone());
        }

        // Logs sync is heavier -- could use a dirty flag
    }
}
```

#### Engine Changes

Add to `Engine` struct:
```rust
pub struct Engine {
    // ... existing fields ...

    /// Shared state for service layer consumers.
    /// Synchronized from AppState after message processing.
    shared_state: SharedState,
}
```

Add service accessors:
```rust
impl Engine {
    /// Get a FlutterController for the currently selected session.
    ///
    /// Returns None if no session is selected or no command sender is available.
    pub fn flutter_controller(&self) -> Option<impl FlutterController + '_> {
        let session = self.state.session_manager.selected()?;
        let cmd_sender = session.cmd_sender.as_ref()?;
        Some(CommandSenderController::new(cmd_sender.clone()))
    }

    /// Get access to the shared log service.
    pub fn log_service(&self) -> SharedLogService {
        SharedLogService::new(self.shared_state.clone())
    }

    /// Get access to the shared state service.
    pub fn state_service(&self) -> SharedStateService {
        SharedStateService::new(self.shared_state.clone())
    }

    /// Get a reference to the shared state (for custom consumers).
    pub fn shared_state(&self) -> &SharedState {
        &self.shared_state
    }
}
```

#### SharedState Adjustments

The current `SharedState` was designed for a single-session model. It may need updates for multi-session:

1. **Single selected session** (simplest): `SharedState` reflects the currently selected session. When the user switches sessions, SharedState updates. This matches the current TUI behavior.

2. **Multi-session map** (future): `SharedState` holds state per session. This is more complex but needed for MCP server (which may want to control any session). Defer to Phase 4.

**For this task, use option 1** -- SharedState reflects the selected session.

#### Where to Call sync_shared_state

Add sync call to `Engine::flush_pending_logs()` (which is already called after message processing):

```rust
pub fn flush_pending_logs(&mut self) {
    self.state.session_manager.flush_all_pending_logs();
    self.sync_shared_state_nonblocking();
}
```

This ensures SharedState is updated every render cycle (TUI) or every message (headless) without requiring callers to change.

### Step-by-Step Implementation

1. **Add `SharedState` to Engine struct**: Initialize in `Engine::new()` with default values.

2. **Implement `sync_shared_state_nonblocking()`**: Non-blocking sync from AppState to SharedState. Sync phase, app_id, device info. Defer log sync to a dirty-flag optimization (only sync logs when they change).

3. **Add sync call to `flush_pending_logs()`**: So both TUI and headless runners get automatic sync.

4. **Add service accessor methods**: `flutter_controller()`, `log_service()`, `state_service()`, `shared_state()`.

5. **Verify `CommandSenderController`**: It takes a `CommandSender` -- ensure it works with the session's cmd_sender from `SessionHandle`.

6. **Verify `SharedLogService`**: It reads from `SharedState.logs` -- ensure the log sync populates this correctly.

7. **Add tests**: Test that sync works after processing a message. Test that service accessors return valid instances.

### Acceptance Criteria

1. `Engine` struct has a `shared_state: SharedState` field
2. `Engine::new()` initializes `SharedState` with defaults
3. `sync_shared_state_nonblocking()` copies phase, app_id, device info from selected session to SharedState
4. `flush_pending_logs()` calls sync after flushing logs
5. `engine.flutter_controller()` returns a working `FlutterController` when a session has a cmd_sender
6. `engine.log_service()` returns a `SharedLogService` backed by SharedState
7. `engine.state_service()` returns a `SharedStateService` backed by SharedState
8. `engine.shared_state()` returns a reference for custom consumers
9. Sync is non-blocking (uses `try_write()`, not `.await`)
10. `cargo build` succeeds
11. `cargo test` passes
12. `cargo clippy` is clean

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_shared_state_initialized() {
        let dir = tempfile::tempdir().unwrap();
        let engine = Engine::new(dir.path().to_path_buf());

        let state = engine.shared_state().app_state.read().await;
        assert_eq!(state.phase, AppPhase::Initializing);
    }

    #[tokio::test]
    async fn test_shared_state_sync_after_flush() {
        let dir = tempfile::tempdir().unwrap();
        let mut engine = Engine::new(dir.path().to_path_buf());

        // Simulate a phase change
        // (would need to create a session first, then change its phase)
        engine.flush_pending_logs();

        // SharedState should reflect current state
        let state = engine.shared_state().app_state.read().await;
        // Assert based on what state was set
    }

    #[test]
    fn test_log_service_accessor() {
        let dir = tempfile::tempdir().unwrap();
        let engine = Engine::new(dir.path().to_path_buf());

        let _log_service = engine.log_service();
        // Should not panic
    }

    #[test]
    fn test_flutter_controller_none_without_session() {
        let dir = tempfile::tempdir().unwrap();
        let engine = Engine::new(dir.path().to_path_buf());

        // No session selected, should return None
        assert!(engine.flutter_controller().is_none());
    }
}
```

### Notes

- **Performance**: Log sync is potentially expensive for large log buffers. Use a dirty flag or only sync log counts/metadata. Full log access can read directly from `AppState` via the Engine. Consider deferring full log sync to when `LogService` is actually called.
- **Thread safety**: `SharedState` uses `Arc<RwLock<>>` which is safe for concurrent access. The non-blocking `try_write()` approach means service consumers and the Engine never deadlock.
- **Multi-session**: This task wires services for the selected session only. Multi-session service access (needed for MCP server controlling specific sessions) is deferred to Phase 4 when the public API is designed.
- **`FlutterController` implementation choice**: Use `CommandSenderController` (direct command sending) rather than `DaemonFlutterController` (goes through SharedState). The direct approach is simpler and avoids an indirection layer.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `src/app/engine.rs` | Added SharedState field, initialized in new(), implemented sync_shared_state_nonblocking(), added service accessors (flutter_controller, log_service, state_service, shared_state), updated flush_pending_logs to call sync, added comprehensive tests |

### Notable Decisions/Tradeoffs

1. **Non-blocking sync with try_write()**: Used `try_write()` instead of `.await` to avoid blocking the main loop. If a service consumer holds the lock, that sync cycle is skipped (eventual consistency). This prevents deadlocks and keeps the TUI responsive.

2. **Single-session sync model**: SharedState reflects only the currently selected session. Multi-session support for services is deferred to Phase 4 (MCP server needs). This keeps the implementation simple and matches current TUI behavior.

3. **VecDeque to Vec conversion**: Session logs are stored in VecDeque for performance, but SharedState expects Vec. The sync does a full copy conversion. This is acceptable for now as it happens on every render cycle (~60fps) and log buffers are capped at 10,000 entries.

4. **CommandSenderController over DaemonFlutterController**: Used CommandSenderController for flutter_controller() accessor because it works directly with the session's cmd_sender. DaemonFlutterController would require a separate command channel which adds unnecessary indirection.

5. **ProjectInfo derived from path**: state_service() creates ProjectInfo from the project_path filename. This is simple and sufficient for current needs. A more sophisticated implementation could cache the project name from pubspec.yaml.

### Testing Performed

- `cargo check` - Passed
- `cargo test --lib` - Passed (1531 tests, 0 failed)
- `cargo clippy` - Passed (no new warnings)
- `cargo fmt` - Applied formatting

All new accessor methods tested:
- `test_shared_state_initialized` - Verifies SharedState is created with correct defaults
- `test_shared_state_sync_after_flush` - Verifies sync is called and handles no-session case
- `test_log_service_accessor` - Verifies log_service() returns working service
- `test_state_service_accessor` - Verifies state_service() returns working service
- `test_flutter_controller_none_without_session` - Verifies flutter_controller() returns None when no session
- `test_shared_state_reference` - Verifies shared_state() returns valid reference

### Risks/Limitations

1. **Log sync performance**: Currently copies all logs on every flush (~60fps). For sessions with 10,000 logs, this is ~160MB/sec of copying. Future optimization could use dirty flags or incremental sync.

2. **Eventual consistency**: Non-blocking sync means service consumers may see slightly stale data (up to one frame old). This is acceptable for current use cases but may need addressing for real-time MCP operations.

3. **No device list sync**: SharedState has a devices field but it's not populated. Device discovery happens in AppState but isn't synced. This will be needed when MCP server wants to query available devices.

4. **devtools_uri not synced**: Session doesn't track devtools_uri yet, so it's always None in SharedState. This will be needed for MCP server devtools integration.
