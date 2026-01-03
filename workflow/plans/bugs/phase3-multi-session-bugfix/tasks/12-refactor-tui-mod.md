## Task: Refactor tui/mod.rs into Smaller Modules

**Objective**: Split the large `src/tui/mod.rs` file (~740 lines) into smaller, focused modules for better maintainability and readability.

**Depends on**: None (standalone refactoring task)

---

### Scope

- `src/tui/mod.rs`: Split into multiple files
- `src/tui/runner.rs`: New file for main run functions
- `src/tui/actions.rs`: New file for action handlers
- `src/tui/spawn.rs`: New file for background task spawning

---

### Current State

```rust
// src/tui/mod.rs - ~740 lines containing:

pub mod event;
pub mod layout;
pub mod render;
pub mod selector;
pub mod terminal;
pub mod widgets;

// Main entry points (~230 lines)
pub async fn run_with_project(project_path: &Path) -> Result<()> { ... }
pub async fn run() -> Result<()> { ... }

// Background spawning (~80 lines)
fn spawn_device_discovery(msg_tx: mpsc::Sender<Message>) { ... }
fn spawn_emulator_discovery(msg_tx: mpsc::Sender<Message>) { ... }
fn spawn_emulator_launch(msg_tx: mpsc::Sender<Message>, emulator_id: String) { ... }
fn spawn_ios_simulator_launch(msg_tx: mpsc::Sender<Message>) { ... }

// Main loop and message processing (~130 lines)
fn run_loop(...) -> Result<()> { ... }
fn process_message(...) { ... }

// Action handling (~240 lines)
fn handle_action(...) { ... }
async fn execute_task(...) { ... }
```

**Problem:** Large file is difficult to navigate, understand, and maintain.

---

### Implementation Details

#### 1. New Module Structure

```
src/tui/
├── mod.rs              # Re-exports only (~50 lines)
├── runner.rs           # run_with_project, run, run_loop, process_message
├── actions.rs          # handle_action, execute_task
├── spawn.rs            # spawn_device_discovery, spawn_emulator_*, etc.
├── event.rs            # (existing)
├── layout.rs           # (existing)
├── render.rs           # (existing)
├── selector.rs         # (existing)
├── terminal.rs         # (existing)
└── widgets/            # (existing)
```

#### 2. Create src/tui/spawn.rs

```rust
//! Background task spawning for async operations

use tokio::sync::mpsc;

use crate::app::message::Message;
use crate::daemon::{devices, emulators};

/// Spawn device discovery in background
pub fn spawn_device_discovery(msg_tx: mpsc::Sender<Message>) {
    tokio::spawn(async move {
        match devices::discover_devices().await {
            Ok(result) => {
                let _ = msg_tx
                    .send(Message::DevicesDiscovered {
                        devices: result.devices,
                    })
                    .await;
            }
            Err(e) => {
                let _ = msg_tx
                    .send(Message::DeviceDiscoveryFailed {
                        error: e.to_string(),
                    })
                    .await;
            }
        }
    });
}

/// Spawn emulator discovery in background
pub fn spawn_emulator_discovery(msg_tx: mpsc::Sender<Message>) {
    tokio::spawn(async move {
        match emulators::discover_emulators().await {
            Ok(result) => {
                let _ = msg_tx
                    .send(Message::EmulatorsDiscovered {
                        emulators: result.emulators,
                    })
                    .await;
            }
            Err(e) => {
                let _ = msg_tx
                    .send(Message::EmulatorDiscoveryFailed {
                        error: e.to_string(),
                    })
                    .await;
            }
        }
    });
}

/// Spawn emulator launch in background
pub fn spawn_emulator_launch(msg_tx: mpsc::Sender<Message>, emulator_id: String) {
    tokio::spawn(async move {
        match emulators::launch_emulator(&emulator_id).await {
            Ok(result) => {
                let _ = msg_tx.send(Message::EmulatorLaunched { result }).await;
            }
            Err(e) => {
                let result = emulators::EmulatorLaunchResult {
                    success: false,
                    emulator_id,
                    message: Some(e.to_string()),
                    elapsed: std::time::Duration::from_secs(0),
                };
                let _ = msg_tx.send(Message::EmulatorLaunched { result }).await;
            }
        }
    });
}

/// Spawn iOS Simulator launch in background (macOS only)
pub fn spawn_ios_simulator_launch(msg_tx: mpsc::Sender<Message>) {
    tokio::spawn(async move {
        match emulators::launch_ios_simulator().await {
            Ok(result) => {
                let _ = msg_tx.send(Message::EmulatorLaunched { result }).await;
            }
            Err(e) => {
                let result = emulators::EmulatorLaunchResult {
                    success: false,
                    emulator_id: "apple_ios_simulator".to_string(),
                    message: Some(e.to_string()),
                    elapsed: std::time::Duration::from_secs(0),
                };
                let _ = msg_tx.send(Message::EmulatorLaunched { result }).await;
            }
        }
    });
}
```

#### 3. Create src/tui/actions.rs

```rust
//! Action handlers for the TUI event loop

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use tokio::sync::{mpsc, watch, Mutex};
use tracing::{error, info, warn};

use crate::app::message::Message;
use crate::app::session::SessionId;
use crate::app::{Task, UpdateAction};
use crate::daemon::{protocol, CommandSender, DaemonCommand, DaemonMessage, FlutterProcess, RequestTracker};
use crate::core::DaemonEvent;

use super::spawn;

/// Execute an action by spawning a background task
pub fn handle_action(
    action: UpdateAction,
    msg_tx: mpsc::Sender<Message>,
    cmd_sender: Arc<Mutex<Option<CommandSender>>>,
    session_tasks: Arc<Mutex<HashMap<SessionId, tokio::task::JoinHandle<()>>>>,
    shutdown_rx: watch::Receiver<bool>,
    project_path: &Path,
) {
    match action {
        UpdateAction::SpawnTask(task) => {
            let cmd_sender_clone = cmd_sender.clone();
            tokio::spawn(async move {
                let sender = cmd_sender_clone.lock().await.clone();
                execute_task(task, msg_tx, sender).await;
            });
        }

        UpdateAction::DiscoverDevices => {
            spawn::spawn_device_discovery(msg_tx);
        }

        UpdateAction::SpawnSession { session_id, device, config } => {
            spawn_session(
                session_id,
                device,
                config,
                project_path,
                msg_tx,
                cmd_sender,
                session_tasks,
                shutdown_rx,
            );
        }

        UpdateAction::DiscoverEmulators => {
            spawn::spawn_emulator_discovery(msg_tx);
        }

        UpdateAction::LaunchEmulator { emulator_id } => {
            spawn::spawn_emulator_launch(msg_tx, emulator_id);
        }

        UpdateAction::LaunchIOSSimulator => {
            spawn::spawn_ios_simulator_launch(msg_tx);
        }
    }
}

/// Spawn a Flutter session for a device
fn spawn_session(
    session_id: SessionId,
    device: crate::daemon::Device,
    config: Option<Box<crate::config::LaunchConfig>>,
    project_path: &Path,
    msg_tx: mpsc::Sender<Message>,
    cmd_sender: Arc<Mutex<Option<CommandSender>>>,
    session_tasks: Arc<Mutex<HashMap<SessionId, tokio::task::JoinHandle<()>>>>,
    shutdown_rx: watch::Receiver<bool>,
) {
    let project_path = project_path.to_path_buf();
    let msg_tx_clone = msg_tx.clone();
    let cmd_sender_clone = cmd_sender.clone();
    let session_tasks_clone = session_tasks.clone();
    let mut shutdown_rx_clone = shutdown_rx.clone();
    let device_id = device.id.clone();
    let device_name = device.name.clone();
    let device_platform = device.platform.clone();

    let handle = tokio::spawn(async move {
        info!(
            "Spawning Flutter session {} on device: {} ({})",
            session_id, device_name, device_id
        );

        let (daemon_tx, mut daemon_rx) = mpsc::channel::<DaemonEvent>(256);

        let spawn_result = if let Some(cfg) = config {
            FlutterProcess::spawn_with_config(&project_path, &device_id, &cfg, daemon_tx).await
        } else {
            FlutterProcess::spawn_with_device(&project_path, &device_id, daemon_tx).await
        };

        match spawn_result {
            Ok(mut process) => {
                info!("Flutter process started (PID: {:?})", process.id());

                let request_tracker = Arc::new(RequestTracker::default());
                let sender = process.command_sender(request_tracker);
                *cmd_sender_clone.lock().await = Some(sender);

                let _ = msg_tx_clone
                    .send(Message::SessionStarted {
                        session_id,
                        device_id: device_id.clone(),
                        device_name: device_name.clone(),
                        platform: device_platform.clone(),
                        pid: process.id(),
                    })
                    .await;

                let mut app_id: Option<String> = None;

                loop {
                    tokio::select! {
                        event = daemon_rx.recv() => {
                            match event {
                                Some(event) => {
                                    if let DaemonEvent::Stdout(ref line) = event {
                                        if let Some(json) = protocol::strip_brackets(line) {
                                            if let Some(DaemonMessage::AppStart(app_start)) =
                                                DaemonMessage::parse(json)
                                            {
                                                app_id = Some(app_start.app_id.clone());
                                            }
                                        }
                                    }

                                    if msg_tx_clone.send(Message::Daemon(event)).await.is_err() {
                                        break;
                                    }
                                }
                                None => break,
                            }
                        }
                        _ = shutdown_rx_clone.changed() => {
                            if *shutdown_rx_clone.borrow() {
                                info!("Shutdown signal received for session {}", session_id);
                                break;
                            }
                        }
                    }
                }

                info!("Session {} ending, initiating shutdown...", session_id);
                let sender_guard = cmd_sender_clone.lock().await;
                if let Err(e) = process
                    .shutdown(app_id.as_deref(), sender_guard.as_ref())
                    .await
                {
                    warn!("Shutdown error for session {}: {}", session_id, e);
                }
                drop(sender_guard);

                *cmd_sender_clone.lock().await = None;
            }
            Err(e) => {
                error!("Failed to spawn Flutter process: {}", e);
                let _ = msg_tx_clone
                    .send(Message::SessionSpawnFailed {
                        session_id,
                        device_id,
                        error: e.to_string(),
                    })
                    .await;
            }
        }

        session_tasks_clone.lock().await.remove(&session_id);
    });

    if let Ok(mut guard) = session_tasks.try_lock() {
        guard.insert(session_id, handle);
    }
}

/// Execute a task and send completion message
pub async fn execute_task(
    task: Task,
    msg_tx: mpsc::Sender<Message>,
    cmd_sender: Option<CommandSender>,
) {
    let Some(sender) = cmd_sender else {
        let msg = match task {
            Task::Reload { .. } => Message::ReloadFailed {
                reason: "Flutter not running".to_string(),
            },
            Task::Restart { .. } => Message::RestartFailed {
                reason: "Flutter not running".to_string(),
            },
            Task::Stop { .. } => return,
        };
        let _ = msg_tx.send(msg).await;
        return;
    };

    match task {
        Task::Reload { app_id } => {
            let start = std::time::Instant::now();
            match sender.send(DaemonCommand::Reload { app_id }).await {
                Ok(response) => {
                    if response.success {
                        let time_ms = start.elapsed().as_millis() as u64;
                        let _ = msg_tx.send(Message::ReloadCompleted { time_ms }).await;
                    } else {
                        let _ = msg_tx
                            .send(Message::ReloadFailed {
                                reason: response
                                    .error
                                    .unwrap_or_else(|| "Unknown error".to_string()),
                            })
                            .await;
                    }
                }
                Err(e) => {
                    let _ = msg_tx
                        .send(Message::ReloadFailed {
                            reason: e.to_string(),
                        })
                        .await;
                }
            }
        }
        Task::Restart { app_id } => {
            match sender.send(DaemonCommand::Restart { app_id }).await {
                Ok(response) => {
                    if response.success {
                        let _ = msg_tx.send(Message::RestartCompleted).await;
                    } else {
                        let _ = msg_tx
                            .send(Message::RestartFailed {
                                reason: response
                                    .error
                                    .unwrap_or_else(|| "Unknown error".to_string()),
                            })
                            .await;
                    }
                }
                Err(e) => {
                    let _ = msg_tx
                        .send(Message::RestartFailed {
                            reason: e.to_string(),
                        })
                        .await;
                }
            }
        }
        Task::Stop { app_id } => {
            if let Err(e) = sender.send(DaemonCommand::Stop { app_id }).await {
                error!("Failed to stop app: {}", e);
            }
        }
    }
}
```

#### 4. Create src/tui/runner.rs

```rust
//! Main TUI runner - entry points and event loop

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use tokio::sync::{mpsc, watch, Mutex};
use tracing::{error, info, warn};

use crate::app::state::UiMode;
use crate::app::{handler, message::Message, state::AppState};
use crate::common::{prelude::*, signals};
use crate::config;
use crate::core::{AppPhase, DaemonEvent, LogSource};
use crate::daemon::{devices, protocol, CommandSender, DaemonMessage, FlutterProcess, RequestTracker};
use crate::watcher::{FileWatcher, WatcherConfig};

use super::{actions, render, terminal};

/// Run the TUI application with a Flutter project
pub async fn run_with_project(project_path: &Path) -> Result<()> {
    terminal::install_panic_hook();

    let settings = config::load_settings(project_path);
    info!("Loaded settings: auto_start={}", settings.behavior.auto_start);

    let mut term = ratatui::init();
    let mut state = AppState::with_settings(project_path.to_path_buf(), settings.clone());
    state.log_info(LogSource::App, "Flutter Demon starting...");

    let (msg_tx, msg_rx) = mpsc::channel::<Message>(256);
    let (daemon_tx, daemon_rx) = mpsc::channel::<DaemonEvent>(256);

    signals::spawn_signal_handler(msg_tx.clone());

    let cmd_sender: Arc<Mutex<Option<CommandSender>>> = Arc::new(Mutex::new(None));
    let session_tasks: Arc<Mutex<HashMap<u64, tokio::task::JoinHandle<()>>>> = 
        Arc::new(Mutex::new(HashMap::new()));
    let (shutdown_tx, shutdown_rx) = watch::channel(false);

    // Startup logic (auto-start or device selector)
    let (flutter, initial_cmd_sender) = startup_flutter(
        &mut state,
        &settings,
        project_path,
        &daemon_tx,
        &msg_tx,
    ).await;

    if let Some(sender) = initial_cmd_sender {
        *cmd_sender.lock().await = Some(sender);
    }

    // Start file watcher
    let mut file_watcher = FileWatcher::new(
        project_path.to_path_buf(),
        WatcherConfig::new()
            .with_debounce_ms(settings.watcher.debounce_ms)
            .with_auto_reload(settings.watcher.auto_reload),
    );

    if let Err(e) = file_watcher.start(msg_tx.clone()) {
        warn!("Failed to start file watcher: {}", e);
        state.log_error(LogSource::Watcher, format!("Failed to start file watcher: {}", e));
    } else {
        state.log_info(LogSource::Watcher, "File watcher started (watching lib/)");
    }

    // Run main loop
    let result = run_loop(
        &mut term,
        &mut state,
        msg_rx,
        daemon_rx,
        msg_tx,
        cmd_sender.clone(),
        session_tasks.clone(),
        shutdown_rx,
        project_path,
    );

    // Cleanup
    file_watcher.stop();
    cleanup_sessions(&mut state, &mut term, flutter, cmd_sender, session_tasks, shutdown_tx).await;
    ratatui::restore();

    result
}

/// Run TUI without Flutter (for testing/demo)
pub async fn run() -> Result<()> {
    terminal::install_panic_hook();
    let mut term = ratatui::init();
    let mut state = AppState::new();

    let (msg_tx, msg_rx) = mpsc::channel::<Message>(1);
    let (_daemon_tx, daemon_rx) = mpsc::channel::<DaemonEvent>(1);
    let cmd_sender: Arc<Mutex<Option<CommandSender>>> = Arc::new(Mutex::new(None));
    let session_tasks: Arc<Mutex<HashMap<u64, tokio::task::JoinHandle<()>>>> = 
        Arc::new(Mutex::new(HashMap::new()));
    let (_shutdown_tx, shutdown_rx) = watch::channel(false);

    let dummy_path = Path::new(".");
    let result = run_loop(
        &mut term,
        &mut state,
        msg_rx,
        daemon_rx,
        msg_tx,
        cmd_sender,
        session_tasks,
        shutdown_rx,
        dummy_path,
    );
    ratatui::restore();
    result
}

// ... remaining helper functions: run_loop, process_message, startup_flutter, cleanup_sessions
```

#### 5. Update src/tui/mod.rs (Thin Re-export)

```rust
//! TUI presentation layer with signal handling

pub mod actions;
pub mod event;
pub mod layout;
pub mod render;
pub mod runner;
pub mod selector;
pub mod spawn;
pub mod terminal;
pub mod widgets;

// Re-export main entry points
pub use runner::{run, run_with_project};
pub use selector::{select_project, SelectionResult};
```

---

### File Size Targets

| File | Target Lines | Contents |
|------|-------------|----------|
| `mod.rs` | ~20 | Module declarations and re-exports |
| `runner.rs` | ~250 | run_with_project, run, run_loop, process_message |
| `actions.rs` | ~200 | handle_action, spawn_session, execute_task |
| `spawn.rs` | ~80 | spawn_device_discovery, spawn_emulator_* |

---

### Acceptance Criteria

1. [ ] `tui/mod.rs` is under 50 lines (just re-exports)
2. [ ] Each new module is under 300 lines
3. [ ] All public functions are re-exported properly
4. [ ] `cargo build` succeeds with no errors
5. [ ] `cargo test` passes all existing tests
6. [ ] `cargo clippy` has no new warnings
7. [ ] No behavior changes - pure refactoring

---

### Testing

```bash
# Before refactoring - note current test count
cargo test --lib 2>&1 | grep -E "(test result|passed|failed)"

# After refactoring - same tests should pass
cargo test --lib 2>&1 | grep -E "(test result|passed|failed)"

# Verify no regressions
cargo run -- sample/
# Should work exactly as before
```

---

### Migration Steps

1. **Create spawn.rs**
   - Copy spawn_* functions
   - Update imports
   - Verify builds

2. **Create actions.rs**
   - Copy handle_action, execute_task
   - Copy spawn_session logic
   - Update imports
   - Verify builds

3. **Create runner.rs**
   - Copy run_with_project, run, run_loop, process_message
   - Extract helper functions (startup_flutter, cleanup_sessions)
   - Update imports
   - Verify builds

4. **Update mod.rs**
   - Remove moved code
   - Add module declarations
   - Add re-exports
   - Verify builds

5. **Final Verification**
   - Run full test suite
   - Manual testing of all features
   - Clippy check

---

### Notes

- This is a pure refactoring task - no behavior changes
- Keep function signatures identical for backward compatibility
- Internal helper functions can be made `pub(crate)` if needed
- Consider adding module-level doc comments
- Update any direct imports in other files to use re-exports
- The `session_tasks` type may need a type alias for clarity:
  ```rust
  pub type SessionTaskMap = Arc<Mutex<HashMap<SessionId, tokio::task::JoinHandle<()>>>>;
  ```
