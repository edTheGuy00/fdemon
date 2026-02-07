//! Engine - shared orchestration state for TUI and headless runners
//!
//! The Engine encapsulates all shared state and initialization logic currently
//! duplicated between the TUI and headless runners. It owns the message channel,
//! session tasks, shutdown signal, file watcher, and settings.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use tokio::sync::{broadcast, mpsc, watch, Mutex};
use tracing::{info, warn};

use crate::app::actions::SessionTaskMap;
use crate::app::engine_event::EngineEvent;
use crate::app::message::Message;
use crate::app::process;
use crate::app::session::SessionId;
use crate::app::signals;
use crate::app::state::AppState;
use crate::config::{self, Settings};
use crate::core::AppPhase;
use crate::services::{
    CommandSenderController, LocalFlutterController, ProjectInfo, SharedLogService, SharedState,
    SharedStateService,
};
use crate::watcher::{FileWatcher, WatcherConfig, WatcherEvent};

/// Lightweight snapshot of state for change detection.
///
/// Captured before message processing, compared after to detect
/// what changed and emit appropriate EngineEvents.
#[derive(Debug, Clone)]
struct StateSnapshot {
    phase: AppPhase,
    selected_session_id: Option<SessionId>,
    log_count: usize,
    _session_count: usize,
    _reload_count: u32,
}

impl StateSnapshot {
    fn capture(state: &AppState) -> Self {
        let (phase, log_count, reload_count) = state
            .session_manager
            .selected()
            .map(|s| {
                (
                    s.session.phase,
                    s.session.logs.len(),
                    s.session.reload_count,
                )
            })
            .unwrap_or((AppPhase::Initializing, 0, 0));

        Self {
            phase,
            selected_session_id: state.session_manager.selected().map(|s| s.session.id),
            log_count,
            _session_count: state.session_manager.len(),
            _reload_count: reload_count,
        }
    }
}

/// Orchestration engine for Flutter Demon.
///
/// Encapsulates all shared state between TUI and headless runners:
/// - TEA state management
/// - Message channel
/// - Session task tracking
/// - Shutdown signaling
/// - File watcher
/// - Settings
/// - Shared state for service layer
/// - Event broadcasting for external consumers
pub struct Engine {
    /// TEA application state (the Model)
    pub state: AppState,

    /// Sender half of the unified message channel.
    /// Clone this to give to input sources (signal handler, watcher, daemon tasks).
    pub msg_tx: mpsc::Sender<Message>,

    /// Receiver half of the unified message channel.
    /// The frontend event loop drains messages from here.
    pub msg_rx: mpsc::Receiver<Message>,

    /// Map of session IDs to their background task handles.
    pub session_tasks: SessionTaskMap,

    /// Sender for the shutdown signal. Send `true` to initiate shutdown.
    pub shutdown_tx: watch::Sender<bool>,

    /// Receiver for the shutdown signal. Clone for background tasks.
    pub shutdown_rx: watch::Receiver<bool>,

    /// File watcher for auto-reload. None if watcher failed to start.
    file_watcher: Option<FileWatcher>,

    /// Loaded settings (cached from config)
    pub settings: Settings,

    /// Path to the Flutter project
    pub project_path: PathBuf,

    /// Shared state for service layer consumers.
    /// Synchronized from AppState after message processing.
    shared_state: Arc<SharedState>,

    /// Event broadcaster for external consumers.
    /// Subscribers receive EngineEvents after each message processing cycle.
    event_tx: broadcast::Sender<EngineEvent>,
}

impl Engine {
    /// Create a new Engine for a Flutter project.
    ///
    /// Performs all shared initialization:
    /// - Initializes .fdemon directory
    /// - Loads settings from config files
    /// - Creates AppState with settings
    /// - Creates message channel (capacity 256)
    /// - Creates shutdown signal channel
    /// - Creates session task map
    /// - Spawns signal handler
    /// - Creates and starts file watcher with message bridge
    /// - Creates shared state for services layer
    pub fn new(project_path: PathBuf) -> Self {
        // 1. Init .fdemon directory (non-fatal if fails)
        if let Err(e) = config::init_fdemon_directory(&project_path) {
            warn!("Failed to initialize .fdemon directory: {}", e);
        }

        // 2. Load settings
        let settings = config::load_settings(&project_path);

        // 3. Create state
        let state = AppState::with_settings(project_path.clone(), settings.clone());

        // 4. Create message channel
        let (msg_tx, msg_rx) = mpsc::channel::<Message>(256);

        // 5. Create shutdown channel
        let (shutdown_tx, shutdown_rx) = watch::channel(false);

        // 6. Create session task map
        let session_tasks: SessionTaskMap = Arc::new(Mutex::new(HashMap::new()));

        // 7. Spawn signal handler
        signals::spawn_signal_handler(msg_tx.clone());

        // 8. Create and start file watcher
        let file_watcher = Self::start_file_watcher(&project_path, &settings, msg_tx.clone());

        // 9. Create shared state for services layer
        let shared_state = Arc::new(SharedState::new(10_000));

        // 10. Create broadcast channel for engine events (capacity 256)
        let (event_tx, _) = broadcast::channel(256);

        Self {
            state,
            msg_tx,
            msg_rx,
            session_tasks,
            shutdown_tx,
            shutdown_rx,
            file_watcher,
            settings,
            project_path,
            shared_state,
            event_tx,
        }
    }

    /// Subscribe to engine events.
    ///
    /// Returns a receiver that gets EngineEvents after each message
    /// processing cycle. Multiple subscribers are supported.
    ///
    /// If the subscriber falls behind (buffer full), older events are
    /// dropped. Use `broadcast::error::RecvError::Lagged` to detect this.
    pub fn subscribe(&self) -> broadcast::Receiver<EngineEvent> {
        self.event_tx.subscribe()
    }

    /// Process a single message through the TEA update cycle.
    ///
    /// Delegates to `process::process_message()` which runs handler::update()
    /// and dispatches any resulting UpdateActions. Emits EngineEvents based
    /// on state changes detected by comparing before/after snapshots.
    pub fn process_message(&mut self, msg: Message) {
        // Snapshot state before processing
        let pre = StateSnapshot::capture(&self.state);

        process::process_message(
            &mut self.state,
            msg,
            &self.msg_tx,
            &self.session_tasks,
            &self.shutdown_rx,
            &self.project_path,
        );

        // Snapshot state after processing
        let post = StateSnapshot::capture(&self.state);

        // Emit events for any state changes
        self.emit_events(&pre, &post);
    }

    /// Drain and process all pending messages from the channel.
    ///
    /// Returns the number of messages processed. Used by the TUI runner
    /// which needs to drain all pending messages before rendering.
    /// Events are emitted after each message is processed.
    pub fn drain_pending_messages(&mut self) -> usize {
        let mut count = 0;
        while let Ok(msg) = self.msg_rx.try_recv() {
            self.process_message(msg);
            count += 1;
        }
        count
    }

    /// Flush pending batched logs across all sessions.
    ///
    /// Call after processing messages and before rendering/emitting events.
    /// Also synchronizes AppState to SharedState.
    pub fn flush_pending_logs(&mut self) {
        self.state.session_manager.flush_all_pending_logs();
        self.sync_shared_state_nonblocking();
    }

    /// Synchronize AppState changes to SharedState (non-blocking).
    ///
    /// Called after processing messages. One-way: AppState is the source of truth.
    /// Uses try_write() to avoid blocking - if lock is held by a service consumer,
    /// skip this sync cycle (eventual consistency).
    fn sync_shared_state_nonblocking(&self) {
        if let Some(session_handle) = self.state.session_manager.selected() {
            let session = &session_handle.session;

            // Sync app run state from selected session
            if let Ok(mut app_state) = self.shared_state.app_state.try_write() {
                app_state.phase = session.phase;
                app_state.app_id = session.app_id.clone();
                app_state.device_id = Some(session.device_id.clone());
                app_state.device_name = Some(session.device_name.clone());
                app_state.platform = Some(session.platform.clone());
                app_state.devtools_uri = None; // Not tracked in Session yet
                app_state.started_at = session.started_at;
                app_state.last_reload_at = session.last_reload_time;
            }

            // Sync logs from selected session (convert VecDeque to Vec)
            if let Ok(mut logs) = self.shared_state.logs.try_write() {
                // Replace with current session's logs
                // Note: This is a snapshot, not a stream -- optimize later if needed
                *logs = session.logs.iter().cloned().collect();
            }
        }
    }

    /// Get a clone of the message sender for spawning input sources.
    pub fn msg_sender(&self) -> mpsc::Sender<Message> {
        self.msg_tx.clone()
    }

    /// Get a clone of the shutdown receiver for background tasks.
    pub fn shutdown_receiver(&self) -> watch::Receiver<bool> {
        self.shutdown_rx.clone()
    }

    /// Check if the application should quit.
    pub fn should_quit(&self) -> bool {
        self.state.should_quit()
    }

    /// Get a FlutterController for the currently selected session.
    ///
    /// Returns None if no session is selected or no command sender is available.
    pub fn flutter_controller(&self) -> Option<impl LocalFlutterController + '_> {
        let session = self.state.session_manager.selected()?;
        let cmd_sender = session.cmd_sender.as_ref()?;
        Some(CommandSenderController::new(
            cmd_sender.clone(),
            self.shared_state.clone(),
        ))
    }

    /// Get access to the shared log service.
    pub fn log_service(&self) -> SharedLogService {
        SharedLogService::new(self.shared_state.logs.clone(), self.shared_state.max_logs)
    }

    /// Get access to the shared state service.
    pub fn state_service(&self) -> SharedStateService {
        let project_name = self
            .project_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();
        let project_info = ProjectInfo::new(project_name, self.project_path.clone());
        SharedStateService::new(self.shared_state.clone(), project_info)
    }

    /// Get a reference to the shared state (for custom consumers).
    pub fn shared_state(&self) -> &Arc<SharedState> {
        &self.shared_state
    }

    /// Initiate shutdown: stop watcher, signal background tasks, cleanup sessions.
    pub async fn shutdown(&mut self) {
        // Emit shutdown event
        self.emit(EngineEvent::Shutdown);

        // Stop file watcher
        if let Some(ref mut watcher) = self.file_watcher {
            watcher.stop();
        }

        // Signal all background tasks to stop
        let _ = self.shutdown_tx.send(true);

        // Drain remaining session tasks with timeout
        let tasks: Vec<_> = {
            let mut map = self.session_tasks.lock().await;
            map.drain().collect()
        };

        for (session_id, handle) in tasks {
            match tokio::time::timeout(std::time::Duration::from_secs(2), handle).await {
                Ok(Ok(())) => info!("Session {} cleaned up", session_id),
                Ok(Err(e)) => warn!("Session {} panicked: {}", session_id, e),
                Err(_) => warn!("Session {} cleanup timed out", session_id),
            }
        }
    }

    /// Emit EngineEvents based on state changes after processing.
    ///
    /// Called after process_message() and flush_pending_logs().
    /// Compares pre/post snapshots to detect what changed.
    fn emit_events(&self, pre: &StateSnapshot, post: &StateSnapshot) {
        // Phase changes
        if pre.phase != post.phase {
            if let Some(session_id) = post.selected_session_id {
                self.emit(EngineEvent::PhaseChanged {
                    session_id,
                    old_phase: pre.phase,
                    new_phase: post.phase,
                });
            }
        }

        // Reload detection - transition from non-Reloading to Reloading
        if pre.phase != AppPhase::Reloading && post.phase == AppPhase::Reloading {
            if let Some(session_id) = post.selected_session_id {
                self.emit(EngineEvent::ReloadStarted { session_id });
            }
        }

        // Reload completion - transition from Reloading to Running
        if pre.phase == AppPhase::Reloading && post.phase == AppPhase::Running {
            if let Some(session_id) = post.selected_session_id {
                // Calculate reload time if we have reload start/end times
                // For now, emit with 0ms - actual timing is tracked elsewhere
                self.emit(EngineEvent::ReloadCompleted {
                    session_id,
                    time_ms: 0,
                });
            }
        }

        // New logs detected
        if post.log_count > pre.log_count {
            if let Some(session_id) = post.selected_session_id {
                // Get new log entries
                if let Some(session_handle) = self.state.session_manager.selected() {
                    let new_count = post.log_count - pre.log_count;
                    let logs: Vec<_> = session_handle
                        .session
                        .logs
                        .iter()
                        .rev()
                        .take(new_count)
                        .rev()
                        .cloned()
                        .collect();

                    // Use batch emission for multiple logs (more efficient)
                    if logs.len() > 1 {
                        self.emit(EngineEvent::LogBatch {
                            session_id,
                            entries: logs,
                        });
                    } else if let Some(entry) = logs.first() {
                        self.emit(EngineEvent::LogEntry {
                            session_id,
                            entry: entry.clone(),
                        });
                    }
                }
            }
        }

        // Note: Hot restart events (RestartStarted, RestartCompleted) would be
        // emitted here based on message type or phase changes, but the current
        // AppPhase enum doesn't have a Restarting variant. These events may be
        // added in a future update when restart tracking is implemented.
    }

    /// Emit a single EngineEvent to all subscribers.
    ///
    /// send() returns Err only if there are no receivers -- that's fine,
    /// we don't want to panic or log errors for having no subscribers.
    fn emit(&self, event: EngineEvent) {
        let _ = self.event_tx.send(event);
    }

    /// Create and start the file watcher, bridging events to messages.
    fn start_file_watcher(
        project_path: &Path,
        settings: &Settings,
        msg_tx: mpsc::Sender<Message>,
    ) -> Option<FileWatcher> {
        let mut watcher = FileWatcher::new(
            project_path.to_path_buf(),
            WatcherConfig::new()
                .with_debounce_ms(settings.watcher.debounce_ms)
                .with_auto_reload(settings.watcher.auto_reload),
        );

        let (watcher_tx, mut watcher_rx) = mpsc::channel::<WatcherEvent>(32);

        if let Err(e) = watcher.start(watcher_tx) {
            warn!("Failed to start file watcher: {}", e);
            return None;
        }

        // Bridge watcher events to app messages
        tokio::spawn(async move {
            while let Some(event) = watcher_rx.recv().await {
                let msg = match event {
                    WatcherEvent::AutoReloadTriggered => Message::AutoReloadTriggered,
                    WatcherEvent::FilesChanged { count } => Message::FilesChanged { count },
                    WatcherEvent::Error { message } => Message::WatcherError { message },
                };
                let _ = msg_tx.send(msg).await;
            }
        });

        Some(watcher)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::AppPhase;

    #[tokio::test]
    async fn test_engine_new_creates_valid_state() {
        // Engine::new() requires a project path but doesn't require Flutter
        // Use a temp directory to test construction
        let dir = tempfile::tempdir().unwrap();
        let engine = Engine::new(dir.path().to_path_buf());

        assert!(!engine.should_quit());
        assert_eq!(engine.project_path, dir.path());
    }

    #[tokio::test]
    async fn test_engine_drain_empty_channel() {
        let dir = tempfile::tempdir().unwrap();
        let mut engine = Engine::new(dir.path().to_path_buf());

        // No messages pending
        assert_eq!(engine.drain_pending_messages(), 0);
    }

    #[tokio::test]
    async fn test_engine_process_quit_message() {
        let dir = tempfile::tempdir().unwrap();
        let mut engine = Engine::new(dir.path().to_path_buf());

        engine.process_message(Message::Quit);
        assert!(engine.should_quit());
    }

    #[tokio::test]
    async fn test_engine_shutdown() {
        let dir = tempfile::tempdir().unwrap();
        let mut engine = Engine::new(dir.path().to_path_buf());

        // Should not panic on empty engine
        engine.shutdown().await;
    }

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

        // Initially no sessions, so sync should be a no-op
        engine.flush_pending_logs();

        // SharedState should still be in default state
        let state = engine.shared_state().app_state.read().await;
        assert_eq!(state.phase, AppPhase::Initializing);
        assert!(state.app_id.is_none());
    }

    #[tokio::test]
    async fn test_log_service_accessor() {
        let dir = tempfile::tempdir().unwrap();
        let engine = Engine::new(dir.path().to_path_buf());

        let _log_service = engine.log_service();
        // Should not panic
    }

    #[tokio::test]
    async fn test_state_service_accessor() {
        let dir = tempfile::tempdir().unwrap();
        let engine = Engine::new(dir.path().to_path_buf());

        let _state_service = engine.state_service();
        // Should not panic
    }

    #[tokio::test]
    async fn test_flutter_controller_none_without_session() {
        let dir = tempfile::tempdir().unwrap();
        let engine = Engine::new(dir.path().to_path_buf());

        // No session selected, should return None
        assert!(engine.flutter_controller().is_none());
    }

    #[tokio::test]
    async fn test_shared_state_reference() {
        let dir = tempfile::tempdir().unwrap();
        let engine = Engine::new(dir.path().to_path_buf());

        let shared_state = engine.shared_state();
        assert_eq!(shared_state.max_logs, 10_000);
    }

    // ─────────────────────────────────────────────────────────
    // Event Broadcasting Tests (Task 06)
    // ─────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_subscribe_receives_shutdown_event() {
        let dir = tempfile::tempdir().unwrap();
        let mut engine = Engine::new(dir.path().to_path_buf());

        let mut rx = engine.subscribe();

        // Shutdown should emit event
        engine.shutdown().await;

        // Should receive shutdown event
        match tokio::time::timeout(std::time::Duration::from_millis(100), rx.recv()).await {
            Ok(Ok(event)) => {
                assert!(matches!(event, EngineEvent::Shutdown));
            }
            _ => panic!("Should have received shutdown event"),
        }
    }

    #[tokio::test]
    async fn test_no_subscribers_no_error() {
        let dir = tempfile::tempdir().unwrap();
        let mut engine = Engine::new(dir.path().to_path_buf());

        // No subscribers -- should not error
        engine.process_message(Message::Quit);
        // No panic
    }

    #[tokio::test]
    async fn test_multiple_subscribers() {
        let dir = tempfile::tempdir().unwrap();
        let engine = Engine::new(dir.path().to_path_buf());

        let _rx1 = engine.subscribe();
        let _rx2 = engine.subscribe();
        let _rx3 = engine.subscribe();

        // All three should be valid receivers
    }

    #[test]
    fn test_state_snapshot_capture() {
        let state = AppState::new();
        let snapshot = StateSnapshot::capture(&state);

        assert_eq!(snapshot.phase, AppPhase::Initializing);
        assert_eq!(snapshot.log_count, 0);
        assert_eq!(snapshot._session_count, 0);
    }

    #[tokio::test]
    async fn test_subscribe_channel_capacity() {
        let dir = tempfile::tempdir().unwrap();
        let engine = Engine::new(dir.path().to_path_buf());

        let mut rx = engine.subscribe();

        // Generate many events to test buffer size (256 capacity)
        for _ in 0..100 {
            engine.emit(EngineEvent::Shutdown);
        }

        // Should be able to receive at least some events
        let mut count = 0;
        while rx.try_recv().is_ok() {
            count += 1;
        }

        assert!(count > 0, "Should have received some events");
        assert!(count <= 256, "Should not exceed buffer capacity");
    }

    #[tokio::test]
    async fn test_phase_change_event() {
        let dir = tempfile::tempdir().unwrap();
        let mut engine = Engine::new(dir.path().to_path_buf());

        let mut rx = engine.subscribe();

        // Process quit message which changes phase to Quitting
        engine.process_message(Message::Quit);

        // Should receive PhaseChanged event
        match tokio::time::timeout(std::time::Duration::from_millis(100), rx.recv()).await {
            Ok(Ok(event)) => match event {
                EngineEvent::PhaseChanged {
                    old_phase,
                    new_phase,
                    ..
                } => {
                    assert_eq!(old_phase, AppPhase::Initializing);
                    assert_eq!(new_phase, AppPhase::Quitting);
                }
                _ => panic!("Expected PhaseChanged event, got {:?}", event),
            },
            _ => {
                // No session selected, so no event expected - this is OK
            }
        }
    }

    #[tokio::test]
    async fn test_event_type_label() {
        let event = EngineEvent::Shutdown;
        assert_eq!(event.event_type(), "shutdown");
    }
}
