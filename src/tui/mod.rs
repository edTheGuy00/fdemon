//! TUI presentation layer with signal handling

pub mod event;
pub mod layout;
pub mod render;
pub mod selector;
pub mod terminal;
pub mod widgets;

pub use selector::{select_project, SelectionResult};

use std::path::Path;
use tokio::sync::mpsc;

use std::sync::Arc;

use crate::app::{handler, message::Message, state::AppState, Task, UpdateAction};
use crate::common::{prelude::*, signals};
use crate::core::{AppPhase, DaemonEvent, LogSource};
use crate::daemon::{protocol, CommandSender, DaemonCommand, DaemonMessage, FlutterProcess, RequestTracker};
use crate::watcher::{FileWatcher, WatcherConfig};

/// Run the TUI application with a Flutter project
pub async fn run_with_project(project_path: &Path) -> Result<()> {
    // Install panic hook for terminal restoration
    terminal::install_panic_hook();

    // Initialize terminal
    let mut term = ratatui::init();

    // Create initial state
    let mut state = AppState::new();
    state.log_info(LogSource::App, "Flutter Demon starting...");

    // Create unified message channel (for signal handler, etc.)
    let (msg_tx, msg_rx) = mpsc::channel::<Message>(256);

    // Create channel for daemon events
    let (daemon_tx, daemon_rx) = mpsc::channel::<DaemonEvent>(256);

    // Spawn signal handler (sends Message::Quit on SIGINT/SIGTERM)
    signals::spawn_signal_handler(msg_tx.clone());

    // Create request tracker for command response matching
    let request_tracker = Arc::new(RequestTracker::default());

    // Spawn Flutter process
    let (flutter, cmd_sender) = match FlutterProcess::spawn(project_path, daemon_tx).await {
        Ok(p) => {
            state.log_info(
                LogSource::App,
                format!("Flutter process started (PID: {:?})", p.id()),
            );
            state.phase = AppPhase::Running;
            let sender = p.command_sender(request_tracker.clone());
            (Some(p), Some(sender))
        }
        Err(e) => {
            state.log_error(LogSource::App, format!("Failed to start Flutter: {}", e));
            (None, None)
        }
    };

    // Start file watcher for auto-reload
    let mut file_watcher = FileWatcher::new(
        project_path.to_path_buf(),
        WatcherConfig::new()
            .with_debounce_ms(500)
            .with_auto_reload(true),
    );

    if let Err(e) = file_watcher.start(msg_tx.clone()) {
        warn!("Failed to start file watcher: {}", e);
        state.log_error(
            LogSource::Watcher,
            format!("Failed to start file watcher: {}", e),
        );
    } else {
        state.log_info(LogSource::Watcher, "File watcher started (watching lib/)");
    }

    // Run the main loop
    let result = run_loop(&mut term, &mut state, msg_rx, daemon_rx, msg_tx, cmd_sender.clone());

    // Stop file watcher
    file_watcher.stop();

    // Cleanup Flutter process gracefully
    if let Some(mut p) = flutter {
        state.log_info(LogSource::App, "Shutting down Flutter process...");

        // Draw one more frame to show shutdown message
        let _ = term.draw(|frame| render::view(frame, &mut state));

        if let Err(e) = p
            .shutdown(state.current_app_id.as_deref(), cmd_sender.as_ref())
            .await
        {
            error!("Error during Flutter shutdown: {}", e);
        } else {
            info!("Flutter process shut down cleanly");
        }
    }

    // Restore terminal
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

    let result = run_loop(&mut term, &mut state, msg_rx, daemon_rx, msg_tx, None);
    ratatui::restore();
    result
}

fn run_loop(
    terminal: &mut ratatui::DefaultTerminal,
    state: &mut AppState,
    mut msg_rx: mpsc::Receiver<Message>,
    mut daemon_rx: mpsc::Receiver<DaemonEvent>,
    msg_tx: mpsc::Sender<Message>,
    cmd_sender: Option<CommandSender>,
) -> Result<()> {
    while !state.should_quit() {
        // Process external messages (from signal handler, etc.)
        while let Ok(msg) = msg_rx.try_recv() {
            process_message(state, msg, &msg_tx, &cmd_sender);
        }

        // Process daemon events (non-blocking)
        while let Ok(event) = daemon_rx.try_recv() {
            // Route JSON-RPC responses to RequestTracker before processing
            if let DaemonEvent::Stdout(ref line) = event {
                if let Some(json) = protocol::strip_brackets(line) {
                    if let Some(DaemonMessage::Response { id, result, error }) =
                        DaemonMessage::parse(json)
                    {
                        if let Some(ref sender) = cmd_sender {
                            if let Some(id_num) = id.as_u64() {
                                let tracker = sender.tracker().clone();
                                tokio::spawn(async move {
                                    tracker.handle_response(id_num, result, error).await;
                                });
                            }
                        }
                    }
                }
            }
            // Still pass to handler for logging/other processing
            process_message(state, Message::Daemon(event), &msg_tx, &cmd_sender);
        }

        // Render
        terminal.draw(|frame| render::view(frame, state))?;

        // Handle terminal events
        if let Some(message) = event::poll()? {
            process_message(state, message, &msg_tx, &cmd_sender);
        }
    }

    Ok(())
}

/// Process a message through the TEA update function
fn process_message(
    state: &mut AppState,
    message: Message,
    msg_tx: &mpsc::Sender<Message>,
    cmd_sender: &Option<CommandSender>,
) {
    let mut msg = Some(message);
    while let Some(m) = msg {
        let result = handler::update(state, m);

        // Handle any action
        if let Some(action) = result.action {
            handle_action(action, msg_tx.clone(), cmd_sender.clone());
        }

        // Continue with follow-up message
        msg = result.message;
    }
}

/// Execute an action by spawning a background task
fn handle_action(
    action: UpdateAction,
    msg_tx: mpsc::Sender<Message>,
    cmd_sender: Option<CommandSender>,
) {
    match action {
        UpdateAction::SpawnTask(task) => {
            // Spawn async task for command execution
            tokio::spawn(async move {
                execute_task(task, msg_tx, cmd_sender).await;
            });
        }
    }
}

/// Execute a task and send completion message
async fn execute_task(
    task: Task,
    msg_tx: mpsc::Sender<Message>,
    cmd_sender: Option<CommandSender>,
) {
    let Some(sender) = cmd_sender else {
        // No command sender available
        let msg = match task {
            Task::Reload { .. } => Message::ReloadFailed {
                reason: "Flutter not running".to_string(),
            },
            Task::Restart { .. } => Message::RestartFailed {
                reason: "Flutter not running".to_string(),
            },
            Task::Stop { .. } => return, // Nothing to do
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
        Task::Restart { app_id } => match sender.send(DaemonCommand::Restart { app_id }).await {
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
        },
        Task::Stop { app_id } => {
            if let Err(e) = sender.send(DaemonCommand::Stop { app_id }).await {
                error!("Failed to stop app: {}", e);
            }
        }
    }
}
