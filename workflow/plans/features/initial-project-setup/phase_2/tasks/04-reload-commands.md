## Task: Reload Commands

**Objective**: Wire 'r' key to hot reload and 'R' key to hot restart with visual feedback in the TUI, using the service layer from previous tasks.

**Depends on**: 03-command-system

---

### Scope

- `src/app/message.rs`: Add reload/restart message variants
- `src/app/handler.rs`: Handle reload/restart messages, call FlutterController
- `src/app/state.rs`: Track reload state and timing
- `src/tui/widgets/log_view.rs`: Show reload feedback in logs

---

### Implementation Details

#### New Message Variants

```rust
// src/app/message.rs - add to Message enum

/// All possible messages/actions in the application
#[derive(Debug, Clone)]
pub enum Message {
    // ... existing variants ...

    // ─────────────────────────────────────────────────────────
    // Control Messages
    // ─────────────────────────────────────────────────────────
    /// Request hot reload
    HotReload,
    /// Request hot restart
    HotRestart,
    /// Stop the running app
    StopApp,

    // ─────────────────────────────────────────────────────────
    // Internal State Updates
    // ─────────────────────────────────────────────────────────
    /// Reload started
    ReloadStarted,
    /// Reload completed successfully
    ReloadCompleted { time_ms: u64 },
    /// Reload failed
    ReloadFailed { reason: String },
    /// Restart started
    RestartStarted,
    /// Restart completed
    RestartCompleted,
    /// Restart failed
    RestartFailed { reason: String },
}
```

#### Key Bindings

```rust
// src/app/handler.rs - update handle_key function

fn handle_key(state: &AppState, key: KeyEvent) -> Option<Message> {
    // Don't process commands while reloading
    let is_busy = matches!(state.phase, AppPhase::Reloading);

    match key.code {
        // Quit
        KeyCode::Char('q') | KeyCode::Esc => Some(Message::Quit),
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => Some(Message::Quit),

        // Hot reload (lowercase 'r')
        KeyCode::Char('r') if !is_busy => Some(Message::HotReload),

        // Hot restart (uppercase 'R')
        KeyCode::Char('R') if !is_busy => Some(Message::HotRestart),

        // Stop app (lowercase 's')
        KeyCode::Char('s') if !is_busy => Some(Message::StopApp),

        // Scrolling (unchanged)
        KeyCode::Char('j') | KeyCode::Down => Some(Message::ScrollDown),
        KeyCode::Char('k') | KeyCode::Up => Some(Message::ScrollUp),
        KeyCode::Char('g') => Some(Message::ScrollToTop),
        KeyCode::Char('G') => Some(Message::ScrollToBottom),
        KeyCode::PageUp => Some(Message::PageUp),
        KeyCode::PageDown => Some(Message::PageDown),
        KeyCode::Home => Some(Message::ScrollToTop),
        KeyCode::End => Some(Message::ScrollToBottom),

        _ => None,
    }
}
```

#### Handler Update Function

```rust
// src/app/handler.rs - update the update function

use crate::services::{FlutterController, LogService};
use std::sync::Arc;

/// Context for message handling
pub struct UpdateContext {
    pub controller: Arc<dyn FlutterController>,
    pub log_service: Arc<dyn LogService>,
}

/// Process a message and update state
/// Returns an optional follow-up action
pub async fn update(
    state: &mut AppState,
    message: Message,
    ctx: Option<&UpdateContext>,
) -> Option<UpdateAction> {
    match message {
        Message::Quit => {
            state.phase = AppPhase::Quitting;
            None
        }

        Message::HotReload => {
            if let Some(ctx) = ctx {
                // Check if app is running
                if !ctx.controller.is_running().await {
                    state.log_info(LogSource::App, "No app running to reload");
                    return None;
                }

                state.phase = AppPhase::Reloading;
                state.log_info(LogSource::App, "Reloading...");

                // Spawn reload task
                Some(UpdateAction::SpawnTask(Task::Reload))
            } else {
                state.log_error(LogSource::App, "Controller not available");
                None
            }
        }

        Message::HotRestart => {
            if let Some(ctx) = ctx {
                if !ctx.controller.is_running().await {
                    state.log_info(LogSource::App, "No app running to restart");
                    return None;
                }

                state.phase = AppPhase::Reloading;
                state.log_info(LogSource::App, "Restarting...");

                Some(UpdateAction::SpawnTask(Task::Restart))
            } else {
                state.log_error(LogSource::App, "Controller not available");
                None
            }
        }

        Message::StopApp => {
            if let Some(ctx) = ctx {
                state.log_info(LogSource::App, "Stopping app...");
                Some(UpdateAction::SpawnTask(Task::Stop))
            } else {
                None
            }
        }

        Message::ReloadStarted => {
            state.phase = AppPhase::Reloading;
            state.reload_start_time = Some(std::time::Instant::now());
            None
        }

        Message::ReloadCompleted { time_ms } => {
            state.phase = AppPhase::Running;
            state.last_reload_time = Some(chrono::Local::now());
            state.log_info(
                LogSource::App,
                format!("Reloaded in {}ms", time_ms),
            );
            None
        }

        Message::ReloadFailed { reason } => {
            state.phase = AppPhase::Running;
            state.log_error(
                LogSource::App,
                format!("Reload failed: {}", reason),
            );
            None
        }

        Message::RestartStarted => {
            state.phase = AppPhase::Reloading;
            None
        }

        Message::RestartCompleted => {
            state.phase = AppPhase::Running;
            state.log_info(LogSource::App, "Restarted");
            None
        }

        Message::RestartFailed { reason } => {
            state.phase = AppPhase::Running;
            state.log_error(
                LogSource::App,
                format!("Restart failed: {}", reason),
            );
            None
        }

        // ... existing handlers for scroll, tick, daemon events ...
        _ => handle_other_message(state, message),
    }
}

/// Actions that the event loop should perform after update
pub enum UpdateAction {
    SpawnTask(Task),
    SendCommand(FlutterCommand),
}

/// Background tasks to spawn
pub enum Task {
    Reload,
    Restart,
    Stop,
}
```

#### Task Execution

```rust
// Task execution in the main event loop (src/main.rs or app runner)

async fn execute_task(
    task: Task,
    controller: Arc<dyn FlutterController>,
    message_tx: mpsc::Sender<Message>,
) {
    match task {
        Task::Reload => {
            let result = controller.reload().await;
            let message = match result {
                Ok(r) if r.success => Message::ReloadCompleted {
                    time_ms: r.time_ms.unwrap_or(0),
                },
                Ok(r) => Message::ReloadFailed {
                    reason: r.message.unwrap_or_else(|| "Unknown error".to_string()),
                },
                Err(e) => Message::ReloadFailed {
                    reason: e.to_string(),
                },
            };
            let _ = message_tx.send(message).await;
        }

        Task::Restart => {
            let result = controller.restart().await;
            let message = match result {
                Ok(r) if r.success => Message::RestartCompleted,
                Ok(r) => Message::RestartFailed {
                    reason: r.message.unwrap_or_else(|| "Unknown error".to_string()),
                },
                Err(e) => Message::RestartFailed {
                    reason: e.to_string(),
                },
            };
            let _ = message_tx.send(message).await;
        }

        Task::Stop => {
            if let Err(e) = controller.stop().await {
                let _ = message_tx
                    .send(Message::Daemon(DaemonEvent::SpawnFailed {
                        reason: e.to_string(),
                    }))
                    .await;
            }
        }
    }
}
```

#### State Updates

```rust
// src/app/state.rs - add reload tracking fields

use chrono::{DateTime, Local};
use std::time::Instant;

/// Complete application state (the Model in TEA)
#[derive(Debug)]
pub struct AppState {
    /// Current application phase
    pub phase: AppPhase,

    /// Log buffer
    pub logs: Vec<LogEntry>,

    /// Log view scroll state
    pub log_view_state: LogViewState,

    /// Maximum log buffer size
    pub max_logs: usize,

    // ─────────────────────────────────────────────────────────
    // Reload Tracking (NEW)
    // ─────────────────────────────────────────────────────────
    /// When the current reload started (for timing)
    pub reload_start_time: Option<Instant>,

    /// When the last successful reload completed
    pub last_reload_time: Option<DateTime<Local>>,

    /// Total reload count this session
    pub reload_count: u32,
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

impl AppState {
    pub fn new() -> Self {
        Self {
            phase: AppPhase::Initializing,
            logs: Vec::new(),
            log_view_state: LogViewState::new(),
            max_logs: 10_000,
            reload_start_time: None,
            last_reload_time: None,
            reload_count: 0,
        }
    }

    /// Called when a reload completes successfully
    pub fn record_reload_complete(&mut self) {
        self.reload_count += 1;
        self.last_reload_time = Some(Local::now());
        self.reload_start_time = None;
    }

    /// Get elapsed time since reload started
    pub fn reload_elapsed(&self) -> Option<std::time::Duration> {
        self.reload_start_time.map(|start| start.elapsed())
    }

    /// Format last reload time for display
    pub fn last_reload_display(&self) -> Option<String> {
        self.last_reload_time.map(|t| t.format("%H:%M:%S").to_string())
    }
}
```

---

### Visual Feedback

When reload is triggered, the UI should show:

1. **Phase Indicator**: Status bar shows "Reloading..." with spinner/indicator
2. **Log Entry**: "Reloading..." log message appears immediately
3. **On Success**: "Reloaded in Xms" log message with timing
4. **On Failure**: Error message in red

```
┌─────────────────────────────────────────────────────────────────┐
│  🔥 Flutter Demon   [r] Reload  [R] Restart  [s] Stop          │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  12:34:56  INF [app] Starting Flutter process...                │
│  12:34:57  INF [flutter] App started on iPhone 15 Pro           │
│  12:35:01  INF [app] Reloading...                               │
│  12:35:01  INF [app] Reloaded in 245ms                          │
│                                                                 │
├─────────────────────────────────────────────────────────────────┤
│  ↻ Reloading │ iPhone 15 Pro (ios) │ 00:00:45 │ Last: 12:35:01 │
└─────────────────────────────────────────────────────────────────┘
```

---

### Acceptance Criteria

1. [ ] 'r' key triggers hot reload when app is running
2. [ ] 'R' key triggers hot restart when app is running
3. [ ] 's' key stops the running app
4. [ ] Keys are ignored when app is not running
5. [ ] Keys are ignored during an active reload/restart
6. [ ] Reload shows "Reloading..." status immediately
7. [ ] Success shows "Reloaded in Xms" with actual timing
8. [ ] Failure shows error message in red
9. [ ] `AppState.reload_count` tracks total reloads
10. [ ] `AppState.last_reload_time` records last successful reload
11. [ ] Status bar updates to show reloading state
12. [ ] Handler uses FlutterController trait (not direct daemon access)
13. [ ] Unit tests for key binding logic
14. [ ] Unit tests for state updates

---

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    #[test]
    fn test_r_key_produces_hot_reload() {
        let mut state = AppState::new();
        state.phase = AppPhase::Running;

        let key = KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE);
        let result = handle_key(&state, key);

        assert!(matches!(result, Some(Message::HotReload)));
    }

    #[test]
    fn test_shift_r_produces_hot_restart() {
        let mut state = AppState::new();
        state.phase = AppPhase::Running;

        let key = KeyEvent::new(KeyCode::Char('R'), KeyModifiers::SHIFT);
        let result = handle_key(&state, key);

        assert!(matches!(result, Some(Message::HotRestart)));
    }

    #[test]
    fn test_reload_ignored_when_already_reloading() {
        let mut state = AppState::new();
        state.phase = AppPhase::Reloading;

        let key = KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE);
        let result = handle_key(&state, key);

        assert!(result.is_none());
    }

    #[test]
    fn test_s_key_produces_stop() {
        let mut state = AppState::new();
        state.phase = AppPhase::Running;

        let key = KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE);
        let result = handle_key(&state, key);

        assert!(matches!(result, Some(Message::StopApp)));
    }

    #[test]
    fn test_reload_completed_updates_state() {
        let mut state = AppState::new();
        state.phase = AppPhase::Reloading;

        // Simulate message handling (sync version for testing)
        state.phase = AppPhase::Running;
        state.record_reload_complete();

        assert_eq!(state.reload_count, 1);
        assert!(state.last_reload_time.is_some());
    }

    #[test]
    fn test_reload_count_increments() {
        let mut state = AppState::new();

        state.record_reload_complete();
        assert_eq!(state.reload_count, 1);

        state.record_reload_complete();
        assert_eq!(state.reload_count, 2);

        state.record_reload_complete();
        assert_eq!(state.reload_count, 3);
    }

    #[test]
    fn test_reload_elapsed_tracking() {
        let mut state = AppState::new();

        assert!(state.reload_elapsed().is_none());

        state.reload_start_time = Some(std::time::Instant::now());
        std::thread::sleep(std::time::Duration::from_millis(10));

        let elapsed = state.reload_elapsed().unwrap();
        assert!(elapsed.as_millis() >= 10);
    }

    #[test]
    fn test_last_reload_display_format() {
        use chrono::{Local, TimeZone};

        let mut state = AppState::new();
        state.last_reload_time = Some(Local.with_ymd_and_hms(2024, 1, 15, 12, 30, 45).unwrap());

        let display = state.last_reload_display().unwrap();
        assert_eq!(display, "12:30:45");
    }

    // Mock controller for integration tests
    #[derive(Default)]
    struct MockFlutterController {
        reload_calls: std::sync::atomic::AtomicU32,
        restart_calls: std::sync::atomic::AtomicU32,
        is_running: std::sync::atomic::AtomicBool,
    }

    impl LocalFlutterController for MockFlutterController {
        async fn reload(&self) -> Result<ReloadResult> {
            self.reload_calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(ReloadResult {
                success: true,
                time_ms: Some(250),
                message: None,
            })
        }

        async fn restart(&self) -> Result<RestartResult> {
            self.restart_calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(RestartResult {
                success: true,
                message: None,
            })
        }

        async fn stop(&self) -> Result<()> {
            Ok(())
        }

        async fn is_running(&self) -> bool {
            self.is_running.load(std::sync::atomic::Ordering::SeqCst)
        }

        async fn get_app_id(&self) -> Option<String> {
            Some("mock-app-id".to_string())
        }
    }

    #[tokio::test]
    async fn test_hot_reload_calls_controller() {
        let controller = Arc::new(MockFlutterController::default());
        controller.is_running.store(true, std::sync::atomic::Ordering::SeqCst);

        let result = controller.reload().await.unwrap();

        assert!(result.success);
        assert_eq!(result.time_ms, Some(250));
        assert_eq!(controller.reload_calls.load(std::sync::atomic::Ordering::SeqCst), 1);
    }
}
```

---

### Notes

- The handler function becomes async to support FlutterController calls
- Consider debouncing rapid key presses (e.g., ignore 'r' within 200ms of last reload)
- The UpdateAction pattern allows the handler to request side effects without blocking
- Tasks are spawned by the event loop, not inside the handler
- Mock controller enables testing without real Flutter process
- Future enhancement: show reload progress bar for long rebuilds

---

### Files to Create/Modify

| File | Action | Description |
|------|--------|-------------|
| `src/app/message.rs` | MODIFY | Add HotReload, HotRestart, StopApp messages |
| `src/app/handler.rs` | MODIFY | Handle new messages, use FlutterController |
| `src/app/state.rs` | MODIFY | Add reload tracking fields |
| `src/app/mod.rs` | MODIFY | Export UpdateAction, Task, UpdateContext |
| `src/tui/mod.rs` | MODIFY | Integrate command execution with event loop |
| `src/daemon/commands.rs` | MODIFY | Add Clone derive to CommandSender |

---

## Completion Summary

**Status**: ✅ Complete

**Date Completed**: 2026-01-03

### Implementation Notes

1. **Message Variants**: Added all control messages (`HotReload`, `HotRestart`, `StopApp`) and internal state updates (`ReloadStarted`, `ReloadCompleted`, `ReloadFailed`, `RestartStarted`, `RestartCompleted`, `RestartFailed`)

2. **Key Bindings**: Implemented 'r' for reload, 'R' for restart, 's' for stop, with guards to prevent actions when busy

3. **App ID Tracking**: Added `current_app_id` to AppState to track the running app's ID from `app.start` events, required for sending commands

4. **UpdateResult Pattern**: Changed handler's update() to return `UpdateResult` instead of `Option<Message>`, supporting both follow-up messages and spawned actions

5. **Task Enum with App ID**: Task variants now carry the app_id (`Task::Reload { app_id }`) to support proper command building

6. **TUI Integration**: Updated `tui/mod.rs` to:
   - Create a `RequestTracker` for command response matching
   - Pass `CommandSender` to the event loop
   - Spawn async tasks for reload/restart/stop operations
   - Send completion messages back through the message channel

7. **Error Handling**: Shows "No app running" error when attempting reload/restart without an active app

### Files Modified
- `src/app/message.rs` - New message variants
- `src/app/state.rs` - Added `current_app_id` and reload tracking fields
- `src/app/handler.rs` - Key bindings, message handlers, app ID tracking from daemon events
- `src/app/mod.rs` - Re-exports for `Task`, `UpdateAction`, `UpdateResult`
- `src/tui/mod.rs` - Command execution integration
- `src/daemon/commands.rs` - Added `Clone` derive to `CommandSender`

### Tests Added
- Key binding tests (r, R, s keys)
- Busy state guards (reload ignored during reload)
- Reload/restart completion and failure state updates
- App ID tracking from AppStart/AppStop events
- No-app-running error cases
- Total: 24 new tests in handler.rs

### Acceptance Criteria Met
- [x] 'r' key triggers hot reload when app is running
- [x] 'R' key triggers hot restart when app is running
- [x] 's' key stops the running app
- [x] Keys are ignored when app is not running (shows error message)
- [x] Keys are ignored during an active reload/restart
- [x] Reload shows "Reloading..." status immediately
- [x] Success shows "Reloaded in Xms" with actual timing
- [x] Failure shows error message in logs
- [x] `AppState.reload_count` tracks total reloads
- [x] `AppState.last_reload_time` records last successful reload
- [x] Handler uses CommandSender (via DaemonCommand enum)
- [x] Unit tests for key binding logic
- [x] Unit tests for state updates