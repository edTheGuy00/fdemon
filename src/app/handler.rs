//! Update function - handles state transitions (TEA pattern)

use super::message::Message;
use super::session::SessionId;
use super::state::{AppState, UiMode};
use crate::config::LaunchConfig;
use crate::core::{AppPhase, DaemonEvent, LogEntry, LogLevel, LogSource};
use crate::daemon::{protocol, DaemonMessage, Device};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Actions that the event loop should perform after update
#[derive(Debug, Clone)]
pub enum UpdateAction {
    /// Spawn a background task
    SpawnTask(Task),

    /// Discover available devices
    DiscoverDevices,

    /// Discover available emulators
    DiscoverEmulators,

    /// Launch an emulator by ID
    LaunchEmulator { emulator_id: String },

    /// Launch iOS Simulator (macOS shortcut)
    LaunchIOSSimulator,

    /// Spawn a new session for a device
    SpawnSession {
        /// The session ID in SessionManager (already created)
        session_id: SessionId,
        /// The device to run on
        device: Device,
        /// Optional launch configuration
        config: Option<Box<LaunchConfig>>,
    },
}

/// Background tasks to spawn
#[derive(Debug, Clone)]
pub enum Task {
    /// Hot reload (with session context for cmd_sender lookup)
    Reload {
        session_id: SessionId,
        app_id: String,
    },
    /// Hot restart (with session context for cmd_sender lookup)
    Restart {
        session_id: SessionId,
        app_id: String,
    },
    /// Stop the app (with session context for cmd_sender lookup)
    Stop {
        session_id: SessionId,
        app_id: String,
    },
}

/// Result of processing a message
#[derive(Debug, Default)]
pub struct UpdateResult {
    /// Optional follow-up message to process
    pub message: Option<Message>,
    /// Optional action for the event loop to perform
    pub action: Option<UpdateAction>,
}

impl UpdateResult {
    pub fn none() -> Self {
        Self::default()
    }

    pub fn message(msg: Message) -> Self {
        Self {
            message: Some(msg),
            action: None,
        }
    }

    pub fn action(action: UpdateAction) -> Self {
        Self {
            message: None,
            action: Some(action),
        }
    }
}

/// Process a message and update state
/// Returns optional follow-up message and/or action
pub fn update(state: &mut AppState, message: Message) -> UpdateResult {
    match message {
        Message::RequestQuit => {
            state.request_quit();
            UpdateResult::none()
        }

        Message::Quit => {
            state.phase = AppPhase::Quitting;
            UpdateResult::none()
        }

        Message::ConfirmQuit => {
            state.confirm_quit();
            UpdateResult::none()
        }

        Message::CancelQuit => {
            state.cancel_quit();
            UpdateResult::none()
        }

        Message::Key(key) => {
            if let Some(msg) = handle_key(state, key) {
                UpdateResult::message(msg)
            } else {
                UpdateResult::none()
            }
        }

        Message::Daemon(event) => {
            handle_daemon_event(state, event);
            UpdateResult::none()
        }

        Message::SessionDaemon { session_id, event } => {
            handle_session_daemon_event(state, session_id, event);
            UpdateResult::none()
        }

        Message::ScrollUp => {
            state.log_view_state.scroll_up(1);
            UpdateResult::none()
        }

        Message::ScrollDown => {
            state.log_view_state.scroll_down(1);
            UpdateResult::none()
        }

        Message::ScrollToTop => {
            state.log_view_state.scroll_to_top();
            UpdateResult::none()
        }

        Message::ScrollToBottom => {
            state.log_view_state.scroll_to_bottom();
            UpdateResult::none()
        }

        Message::PageUp => {
            state.log_view_state.page_up();
            UpdateResult::none()
        }

        Message::PageDown => {
            state.log_view_state.page_down();
            UpdateResult::none()
        }

        Message::Tick => {
            // Advance device selector animation when visible and loading or refreshing
            if state.device_selector.visible
                && (state.device_selector.loading || state.device_selector.refreshing)
            {
                state.device_selector.tick();
            }
            UpdateResult::none()
        }

        // ─────────────────────────────────────────────────────────
        // Control Messages
        // ─────────────────────────────────────────────────────────
        Message::HotReload => {
            if state.is_busy() {
                return UpdateResult::none();
            }

            // Try to get session info from selected session
            if let Some(handle) = state.session_manager.selected() {
                if let Some(app_id) = handle.session.app_id.clone() {
                    if handle.cmd_sender.is_some() {
                        let session_id = handle.session.id;
                        state.start_reload();
                        state.log_info(LogSource::App, "Reloading...");
                        return UpdateResult::action(UpdateAction::SpawnTask(Task::Reload {
                            session_id,
                            app_id,
                        }));
                    }
                }
            }

            // Fall back to legacy global app_id
            if let Some(app_id) = state.current_app_id.clone() {
                // Use session_id 0 for legacy mode (will use global cmd_sender)
                state.start_reload();
                state.log_info(LogSource::App, "Reloading (legacy mode)...");
                UpdateResult::action(UpdateAction::SpawnTask(Task::Reload {
                    session_id: 0,
                    app_id,
                }))
            } else {
                state.log_error(LogSource::App, "No app running to reload");
                UpdateResult::none()
            }
        }

        Message::HotRestart => {
            if state.is_busy() {
                return UpdateResult::none();
            }

            // Try to get session info from selected session
            if let Some(handle) = state.session_manager.selected() {
                if let Some(app_id) = handle.session.app_id.clone() {
                    if handle.cmd_sender.is_some() {
                        let session_id = handle.session.id;
                        state.start_reload();
                        state.log_info(LogSource::App, "Restarting...");
                        return UpdateResult::action(UpdateAction::SpawnTask(Task::Restart {
                            session_id,
                            app_id,
                        }));
                    }
                }
            }

            // Fall back to legacy global app_id
            if let Some(app_id) = state.current_app_id.clone() {
                state.start_reload();
                state.log_info(LogSource::App, "Restarting (legacy mode)...");
                UpdateResult::action(UpdateAction::SpawnTask(Task::Restart {
                    session_id: 0,
                    app_id,
                }))
            } else {
                state.log_error(LogSource::App, "No app running to restart");
                UpdateResult::none()
            }
        }

        Message::StopApp => {
            if state.is_busy() {
                return UpdateResult::none();
            }

            // Try to get session info from selected session
            if let Some(handle) = state.session_manager.selected() {
                if let Some(app_id) = handle.session.app_id.clone() {
                    if handle.cmd_sender.is_some() {
                        let session_id = handle.session.id;
                        state.log_info(LogSource::App, "Stopping app...");
                        return UpdateResult::action(UpdateAction::SpawnTask(Task::Stop {
                            session_id,
                            app_id,
                        }));
                    }
                }
            }

            // Fall back to legacy global app_id
            if let Some(app_id) = state.current_app_id.clone() {
                state.log_info(LogSource::App, "Stopping app (legacy mode)...");
                UpdateResult::action(UpdateAction::SpawnTask(Task::Stop {
                    session_id: 0,
                    app_id,
                }))
            } else {
                state.log_error(LogSource::App, "No app running to stop");
                UpdateResult::none()
            }
        }

        // ─────────────────────────────────────────────────────────
        // Internal State Updates
        // ─────────────────────────────────────────────────────────
        Message::ReloadStarted => {
            state.start_reload();
            UpdateResult::none()
        }

        Message::ReloadCompleted { time_ms } => {
            state.record_reload_complete();
            state.log_info(LogSource::App, format!("Reloaded in {}ms", time_ms));
            UpdateResult::none()
        }

        Message::ReloadFailed { reason } => {
            state.phase = AppPhase::Running;
            state.reload_start_time = None;
            state.log_error(LogSource::App, format!("Reload failed: {}", reason));
            UpdateResult::none()
        }

        Message::RestartStarted => {
            state.start_reload();
            UpdateResult::none()
        }

        Message::RestartCompleted => {
            state.record_reload_complete();
            state.log_info(LogSource::App, "Restarted");
            UpdateResult::none()
        }

        Message::RestartFailed { reason } => {
            state.phase = AppPhase::Running;
            state.reload_start_time = None;
            state.log_error(LogSource::App, format!("Restart failed: {}", reason));
            UpdateResult::none()
        }

        // ─────────────────────────────────────────────────────────
        // File Watcher Messages
        // ─────────────────────────────────────────────────────────
        Message::AutoReloadTriggered => {
            // Only auto-reload if app is running and not already reloading
            if !state.is_busy() {
                // Try to get session info from selected session first
                if let Some(handle) = state.session_manager.selected() {
                    if let Some(app_id) = handle.session.app_id.clone() {
                        if handle.cmd_sender.is_some() {
                            let session_id = handle.session.id;
                            state
                                .log_info(LogSource::Watcher, "File change detected, reloading...");
                            state.start_reload();
                            return UpdateResult::action(UpdateAction::SpawnTask(Task::Reload {
                                session_id,
                                app_id,
                            }));
                        }
                    }
                }

                // Fall back to legacy global app_id
                if let Some(app_id) = state.current_app_id.clone() {
                    state.log_info(LogSource::Watcher, "File change detected, reloading...");
                    state.start_reload();
                    UpdateResult::action(UpdateAction::SpawnTask(Task::Reload {
                        session_id: 0,
                        app_id,
                    }))
                } else {
                    // App not running, just log it
                    tracing::debug!("Auto-reload skipped: no app running");
                    UpdateResult::none()
                }
            } else {
                // Already reloading, skip
                tracing::debug!("Auto-reload skipped: already reloading");
                UpdateResult::none()
            }
        }

        Message::FilesChanged { count } => {
            state.log_info(LogSource::Watcher, format!("{} file(s) changed", count));
            UpdateResult::none()
        }

        Message::WatcherError { message } => {
            state.log_error(LogSource::Watcher, format!("Watcher error: {}", message));
            UpdateResult::none()
        }

        // ─────────────────────────────────────────────────────────
        // Device Selector Messages
        // ─────────────────────────────────────────────────────────
        Message::ShowDeviceSelector => {
            state.ui_mode = UiMode::DeviceSelector;

            // Use cache if available for instant display, otherwise show loading
            if state.device_selector.has_cache() {
                state.device_selector.show_refreshing();
            } else {
                state.device_selector.show_loading();
            }

            // Always trigger discovery to get fresh data
            UpdateResult::action(UpdateAction::DiscoverDevices)
        }

        Message::HideDeviceSelector => {
            // Only hide if there are running sessions, otherwise stay on selector
            if state.session_manager.has_running_sessions() {
                state.device_selector.hide();
                state.ui_mode = UiMode::Normal;
            }
            UpdateResult::none()
        }

        Message::DeviceSelectorUp => {
            if state.ui_mode == UiMode::DeviceSelector {
                state.device_selector.select_previous();
            }
            UpdateResult::none()
        }

        Message::DeviceSelectorDown => {
            if state.ui_mode == UiMode::DeviceSelector {
                state.device_selector.select_next();
            }
            UpdateResult::none()
        }

        Message::DeviceSelected { device } => {
            // Check if device already has a running session
            if state
                .session_manager
                .find_by_device_id(&device.id)
                .is_some()
            {
                state.log_error(
                    LogSource::App,
                    format!("Device '{}' already has an active session", device.name),
                );
                // Stay in device selector to pick another device
                return UpdateResult::none();
            }

            // Create session in manager FIRST
            match state.session_manager.create_session(&device) {
                Ok(session_id) => {
                    state.log_info(
                        LogSource::App,
                        format!(
                            "Session created for {} (id: {}, device: {})",
                            device.name, session_id, device.id
                        ),
                    );

                    // Auto-switch to the newly created session
                    state.session_manager.select_by_id(session_id);

                    // Hide selector and switch to normal mode
                    state.device_selector.hide();
                    state.ui_mode = UiMode::Normal;

                    // Return action to spawn session WITH the session_id
                    UpdateResult::action(UpdateAction::SpawnSession {
                        session_id,
                        device,
                        config: None,
                    })
                }
                Err(e) => {
                    // Max sessions reached or other error
                    state.log_error(LogSource::App, format!("Failed to create session: {}", e));
                    UpdateResult::none()
                }
            }
        }

        Message::LaunchAndroidEmulator => {
            state.log_info(LogSource::App, "Discovering Android emulators...");
            state.ui_mode = UiMode::EmulatorSelector;
            UpdateResult::action(UpdateAction::DiscoverEmulators)
        }

        Message::LaunchIOSSimulator => {
            state.log_info(LogSource::App, "Launching iOS Simulator...");
            UpdateResult::action(UpdateAction::LaunchIOSSimulator)
        }

        Message::DevicesDiscovered { devices } => {
            let device_count = devices.len();
            state.device_selector.set_devices(devices);

            // If we were in Loading mode, transition to DeviceSelector
            if state.ui_mode == UiMode::Loading {
                state.ui_mode = UiMode::DeviceSelector;
            }

            if device_count > 0 {
                state.log_info(
                    LogSource::App,
                    format!("Discovered {} device(s)", device_count),
                );
            } else {
                state.log_info(LogSource::App, "No devices found");
            }

            UpdateResult::none()
        }

        Message::DeviceDiscoveryFailed { error } => {
            state.device_selector.set_error(error.clone());

            // If we were in Loading mode, transition to DeviceSelector to show error
            if state.ui_mode == UiMode::Loading {
                state.ui_mode = UiMode::DeviceSelector;
            }

            state.log_error(
                LogSource::App,
                format!("Device discovery failed: {}", error),
            );
            UpdateResult::none()
        }

        Message::RefreshDevices => {
            state.device_selector.show_loading();
            UpdateResult::action(UpdateAction::DiscoverDevices)
        }

        // ─────────────────────────────────────────────────────────
        // Emulator Messages
        // ─────────────────────────────────────────────────────────
        Message::DiscoverEmulators => {
            state.log_info(LogSource::App, "Discovering emulators...");
            UpdateResult::action(UpdateAction::DiscoverEmulators)
        }

        Message::EmulatorsDiscovered { emulators } => {
            let count = emulators.len();
            if count > 0 {
                state.log_info(LogSource::App, format!("Found {} emulator(s)", count));
                // TODO: Task 09 - Show emulator selector UI with the emulators
            } else {
                state.log_info(LogSource::App, "No emulators available");
            }
            // For now, go back to device selector - emulator selector UI is Task 09
            state.ui_mode = UiMode::DeviceSelector;
            UpdateResult::none()
        }

        Message::EmulatorDiscoveryFailed { error } => {
            state.log_error(
                LogSource::App,
                format!("Emulator discovery failed: {}", error),
            );
            // Go back to device selector on failure
            state.ui_mode = UiMode::DeviceSelector;
            UpdateResult::none()
        }

        Message::LaunchEmulator { emulator_id } => {
            state.log_info(
                LogSource::App,
                format!("Launching emulator: {}", emulator_id),
            );
            UpdateResult::action(UpdateAction::LaunchEmulator { emulator_id })
        }

        Message::EmulatorLaunched { result } => {
            if result.success {
                state.log_info(
                    LogSource::App,
                    format!(
                        "Emulator '{}' launched successfully ({:?})",
                        result.emulator_id, result.elapsed
                    ),
                );
                // After launching, refresh devices to pick up the new emulator
                // Go back to device selector to see the new device
                state.ui_mode = UiMode::DeviceSelector;
                state.device_selector.show_loading();
                UpdateResult::action(UpdateAction::DiscoverDevices)
            } else {
                let error_msg = result
                    .message
                    .unwrap_or_else(|| "Unknown error".to_string());
                state.log_error(
                    LogSource::App,
                    format!(
                        "Failed to launch emulator '{}': {}",
                        result.emulator_id, error_msg
                    ),
                );
                // Go back to device selector on failure
                state.ui_mode = UiMode::DeviceSelector;
                UpdateResult::none()
            }
        }

        // ─────────────────────────────────────────────────────────
        // Session Messages
        // ─────────────────────────────────────────────────────────
        Message::SessionStarted {
            session_id,
            device_id: _,
            device_name,
            platform,
            pid,
        } => {
            // Update session-specific state
            if let Some(handle) = state.session_manager.get_mut(session_id) {
                handle.session.phase = AppPhase::Running;
                handle.session.started_at = Some(chrono::Local::now());

                // Log to session-specific logs
                handle.session.log_info(
                    LogSource::App,
                    format!(
                        "Flutter process started (PID: {})",
                        pid.map_or("unknown".to_string(), |p| p.to_string())
                    ),
                );
            }

            // Also update legacy global state for backward compatibility
            state.device_name = Some(device_name.clone());
            state.platform = Some(platform.clone());
            state.phase = AppPhase::Running;
            state.session_start = Some(chrono::Local::now());

            // Log to global logs as well
            state.log_info(
                LogSource::App,
                format!(
                    "Flutter session {} started on {} (PID: {})",
                    session_id,
                    device_name,
                    pid.map_or("unknown".to_string(), |p| p.to_string())
                ),
            );
            UpdateResult::none()
        }

        Message::SessionSpawnFailed {
            session_id,
            device_id: _,
            error,
        } => {
            // Update session-specific state before removal
            if let Some(handle) = state.session_manager.get_mut(session_id) {
                handle.session.phase = AppPhase::Stopped;
                handle.session.log_error(
                    LogSource::App,
                    format!("Failed to start session: {}", error),
                );
            }

            // Log to global logs
            state.log_error(
                LogSource::App,
                format!("Failed to start session {}: {}", session_id, error),
            );

            // Remove the failed session from manager
            state.session_manager.remove_session(session_id);

            // Show device selector again so user can retry
            state.ui_mode = UiMode::DeviceSelector;
            UpdateResult::none()
        }

        Message::SessionProcessAttached {
            session_id,
            cmd_sender,
        } => {
            // Attach the command sender to the session
            if let Some(handle) = state.session_manager.get_mut(session_id) {
                handle.cmd_sender = Some(cmd_sender);
                state.log_info(
                    LogSource::App,
                    format!("Command sender attached to session {}", session_id),
                );
            } else {
                state.log_error(
                    LogSource::App,
                    format!("Cannot attach cmd_sender: session {} not found", session_id),
                );
            }
            UpdateResult::none()
        }

        // ─────────────────────────────────────────────────────────
        // Session Navigation (Task 10)
        // ─────────────────────────────────────────────────────────
        Message::SelectSessionByIndex(index) => {
            // Silently ignore if index is out of range
            state.session_manager.select_by_index(index);
            UpdateResult::none()
        }

        Message::NextSession => {
            state.session_manager.select_next();
            UpdateResult::none()
        }

        Message::PreviousSession => {
            state.session_manager.select_previous();
            UpdateResult::none()
        }

        Message::CloseCurrentSession => {
            // If there's only one session (or none), treat 'x' as quit request
            if state.session_manager.len() <= 1 {
                state.request_quit();
                return UpdateResult::none();
            }

            if let Some(current_session_id) = state.session_manager.selected_id() {
                // Check if session has a running app and cmd_sender
                let session_info = state.session_manager.get(current_session_id).and_then(|h| {
                    h.session
                        .app_id
                        .clone()
                        .map(|app_id| (app_id, h.cmd_sender.clone()))
                });

                if let Some((app_id, cmd_sender_opt)) = session_info {
                    state.log_info(
                        LogSource::App,
                        format!(
                            "Closing session {} (app: {})...",
                            current_session_id, app_id
                        ),
                    );

                    // Send stop command if we have a cmd_sender
                    if let Some(cmd_sender) = cmd_sender_opt {
                        // Spawn async task to stop the app
                        let app_id_clone = app_id.clone();
                        tokio::spawn(async move {
                            let _ = cmd_sender
                                .send(crate::daemon::DaemonCommand::Stop {
                                    app_id: app_id_clone,
                                })
                                .await;
                        });
                    }

                    // Remove the session from the manager
                    state.session_manager.remove_session(current_session_id);
                } else {
                    // No running app, just remove the session
                    state.session_manager.remove_session(current_session_id);
                }

                // If no sessions left after removal, show device selector
                if state.session_manager.is_empty() {
                    state.ui_mode = UiMode::DeviceSelector;
                    state.device_selector.show_loading();
                    return UpdateResult::action(UpdateAction::DiscoverDevices);
                }
            }
            UpdateResult::none()
        }

        // ─────────────────────────────────────────────────────────
        // Log Control (Task 10)
        // ─────────────────────────────────────────────────────────
        Message::ClearLogs => {
            if let Some(handle) = state.session_manager.selected_mut() {
                handle.session.clear_logs();
            } else {
                // Fallback to global logs
                state.logs.clear();
                state.log_view_state.offset = 0;
            }
            UpdateResult::none()
        }
    }
}

/// Handle daemon events - convert to log entries
fn handle_daemon_event(state: &mut AppState, event: DaemonEvent) {
    match event {
        DaemonEvent::Stdout(line) => {
            // Try to strip brackets and parse
            if let Some(json) = protocol::strip_brackets(&line) {
                if let Some(msg) = DaemonMessage::parse(json) {
                    // Handle responses separately (they don't create log entries)
                    if matches!(msg, DaemonMessage::Response { .. }) {
                        tracing::debug!("Response received: {}", msg.summary());
                        return;
                    }

                    // Convert to log entry if applicable
                    if let Some(entry_info) = msg.to_log_entry() {
                        // Add main log entry
                        state.add_log(LogEntry::new(
                            entry_info.level,
                            entry_info.source,
                            entry_info.message,
                        ));

                        // Add stack trace as separate entries if present
                        if let Some(trace) = entry_info.stack_trace {
                            for line in trace.lines().take(10) {
                                // Limit stack trace
                                state.add_log(LogEntry::new(
                                    LogLevel::Debug,
                                    LogSource::FlutterError,
                                    format!("    {}", line),
                                ));
                            }
                        }

                        // Update app state based on message type
                        handle_daemon_message_state(state, &msg);
                    } else {
                        // Unknown event type, log at debug level
                        tracing::debug!("Unhandled daemon message: {}", msg.summary());
                    }
                } else {
                    // Unparseable JSON - show raw
                    tracing::debug!("Unparseable daemon JSON: {}", json);
                }
            } else if !line.trim().is_empty() {
                // Non-JSON output (build progress, etc.)
                // Detect if it's an error or warning
                let (level, message) = detect_raw_line_level(&line);
                state.add_log(LogEntry::new(level, LogSource::Flutter, message));
            }
        }

        DaemonEvent::Stderr(line) => {
            if !line.trim().is_empty() {
                state.add_log(LogEntry::new(
                    LogLevel::Error,
                    LogSource::FlutterError,
                    line,
                ));
            }
        }

        DaemonEvent::Exited { code } => {
            let (level, message) = match code {
                Some(0) => (
                    LogLevel::Info,
                    "Flutter process exited normally".to_string(),
                ),
                Some(c) => (
                    LogLevel::Warning,
                    format!("Flutter process exited with code {}", c),
                ),
                None => (LogLevel::Warning, "Flutter process exited".to_string()),
            };
            state.add_log(LogEntry::new(level, LogSource::App, message));
            state.add_log(LogEntry::info(LogSource::App, "Exiting Flutter Demon..."));
            state.phase = AppPhase::Quitting;
        }

        DaemonEvent::SpawnFailed { reason } => {
            state.add_log(LogEntry::error(
                LogSource::App,
                format!("Failed to start Flutter: {}", reason),
            ));
        }

        DaemonEvent::Message(msg) => {
            // Legacy path - convert typed message
            if let Some(entry_info) = msg.to_log_entry() {
                state.add_log(LogEntry::new(
                    entry_info.level,
                    entry_info.source,
                    entry_info.message,
                ));
            }
            handle_daemon_message_state(state, &msg);
        }
    }
}

/// Handle daemon events for a specific session (multi-session mode)
fn handle_session_daemon_event(state: &mut AppState, session_id: SessionId, event: DaemonEvent) {
    // Check if session still exists (may have been closed)
    if state.session_manager.get(session_id).is_none() {
        tracing::debug!(
            "Discarding event for closed session {}: {:?}",
            session_id,
            match &event {
                DaemonEvent::Stdout(_) => "Stdout",
                DaemonEvent::Stderr(_) => "Stderr",
                DaemonEvent::Exited { .. } => "Exited",
                DaemonEvent::SpawnFailed { .. } => "SpawnFailed",
                DaemonEvent::Message(_) => "Message",
            }
        );
        return;
    }

    match event {
        DaemonEvent::Stdout(line) => {
            handle_session_stdout(state, session_id, &line);
        }
        DaemonEvent::Stderr(line) => {
            if !line.trim().is_empty() {
                if let Some(handle) = state.session_manager.get_mut(session_id) {
                    handle.session.add_log(LogEntry::new(
                        LogLevel::Error,
                        LogSource::FlutterError,
                        line,
                    ));
                }
            }
        }
        DaemonEvent::Exited { code } => {
            handle_session_exited(state, session_id, code);
        }
        DaemonEvent::SpawnFailed { reason } => {
            if let Some(handle) = state.session_manager.get_mut(session_id) {
                handle.session.add_log(LogEntry::error(
                    LogSource::App,
                    format!("Failed to start Flutter: {}", reason),
                ));
            }
        }
        DaemonEvent::Message(msg) => {
            // Legacy path - convert typed message
            if let Some(entry_info) = msg.to_log_entry() {
                if let Some(handle) = state.session_manager.get_mut(session_id) {
                    handle.session.add_log(LogEntry::new(
                        entry_info.level,
                        entry_info.source,
                        entry_info.message,
                    ));
                }
            }
            // Update session state based on message type
            handle_session_message_state(state, session_id, &msg);
        }
    }
}

/// Handle stdout events for a specific session
fn handle_session_stdout(state: &mut AppState, session_id: SessionId, line: &str) {
    // Try to parse as JSON daemon message
    if let Some(json) = protocol::strip_brackets(line) {
        if let Some(msg) = DaemonMessage::parse(json) {
            // Handle responses separately (they don't create log entries)
            if matches!(msg, DaemonMessage::Response { .. }) {
                tracing::debug!("Session {} response: {}", session_id, msg.summary());
                return;
            }

            // Convert to log entry if applicable
            if let Some(entry_info) = msg.to_log_entry() {
                if let Some(handle) = state.session_manager.get_mut(session_id) {
                    handle.session.add_log(LogEntry::new(
                        entry_info.level,
                        entry_info.source,
                        entry_info.message,
                    ));

                    // Add stack trace as separate entries if present
                    if let Some(trace) = entry_info.stack_trace {
                        for trace_line in trace.lines().take(10) {
                            handle.session.add_log(LogEntry::new(
                                LogLevel::Debug,
                                LogSource::FlutterError,
                                format!("    {}", trace_line),
                            ));
                        }
                    }
                }
            } else {
                // Unknown event type, log at debug level
                tracing::debug!(
                    "Session {} unhandled daemon message: {}",
                    session_id,
                    msg.summary()
                );
            }

            // Update session state based on message type
            handle_session_message_state(state, session_id, &msg);
        } else {
            // Unparseable JSON
            tracing::debug!("Session {} unparseable daemon JSON: {}", session_id, json);
        }
    } else if !line.trim().is_empty() {
        // Non-JSON output (build progress, etc.)
        let (level, message) = detect_raw_line_level(line);
        if let Some(handle) = state.session_manager.get_mut(session_id) {
            handle
                .session
                .add_log(LogEntry::new(level, LogSource::Flutter, message));
        }
    }
}

/// Handle session exit events
fn handle_session_exited(state: &mut AppState, session_id: SessionId, code: Option<i32>) {
    if let Some(handle) = state.session_manager.get_mut(session_id) {
        let (level, message) = match code {
            Some(0) => (
                LogLevel::Info,
                "Flutter process exited normally".to_string(),
            ),
            Some(c) => (
                LogLevel::Warning,
                format!("Flutter process exited with code {}", c),
            ),
            None => (LogLevel::Warning, "Flutter process exited".to_string()),
        };

        handle
            .session
            .add_log(LogEntry::new(level, LogSource::App, message));
        handle.session.phase = AppPhase::Stopped;

        // Don't auto-quit - let user decide what to do with the session
        // The session tab remains visible showing the exit log
    }
}

/// Update session state based on daemon message type
fn handle_session_message_state(state: &mut AppState, session_id: SessionId, msg: &DaemonMessage) {
    // Handle app.start event - capture app_id in session
    if let DaemonMessage::AppStart(app_start) = msg {
        if let Some(handle) = state.session_manager.get_mut(session_id) {
            handle.session.mark_started(app_start.app_id.clone());
            tracing::info!(
                "Session {} app started: app_id={}",
                session_id,
                app_start.app_id
            );
        }
        // Also update global state for legacy compatibility
        state.current_app_id = Some(app_start.app_id.clone());
    }

    // Handle app.stop event
    if let DaemonMessage::AppStop(app_stop) = msg {
        if let Some(handle) = state.session_manager.get_mut(session_id) {
            if handle.session.app_id.as_ref() == Some(&app_stop.app_id) {
                handle.session.app_id = None;
                handle.session.phase = AppPhase::Initializing;
                tracing::info!(
                    "Session {} app stopped: app_id={}",
                    session_id,
                    app_stop.app_id
                );
            }
        }
        // Also update global state for legacy compatibility
        if state.current_app_id.as_ref() == Some(&app_stop.app_id) {
            state.current_app_id = None;
        }
    }
}

/// Detect log level from raw (non-JSON) output line
fn detect_raw_line_level(line: &str) -> (LogLevel, String) {
    let trimmed = line.trim();

    // Android logcat format: E/, W/, I/, D/
    if trimmed.starts_with("E/") {
        return (LogLevel::Error, trimmed.to_string());
    }
    if trimmed.starts_with("W/") {
        return (LogLevel::Warning, trimmed.to_string());
    }

    // Gradle/build errors
    if trimmed.contains("FAILURE:")
        || trimmed.contains("BUILD FAILED")
        || trimmed.contains("error:")
    {
        return (LogLevel::Error, trimmed.to_string());
    }

    // Xcode errors
    if trimmed.contains("❌") {
        return (LogLevel::Error, trimmed.to_string());
    }

    // Warnings
    if trimmed.contains("warning:") || trimmed.contains("⚠") {
        return (LogLevel::Warning, trimmed.to_string());
    }

    // Build progress (often noise, show as debug)
    if trimmed.starts_with("Running ")
        || trimmed.starts_with("Building ")
        || trimmed.starts_with("Compiling ")
        || trimmed.contains("...")
    {
        return (LogLevel::Debug, trimmed.to_string());
    }

    (LogLevel::Info, trimmed.to_string())
}

/// Handle typed daemon messages - update app state (not logging)
fn handle_daemon_message_state(state: &mut AppState, msg: &DaemonMessage) {
    // Capture app_id from AppStart event
    if let DaemonMessage::AppStart(app_start) = msg {
        state.current_app_id = Some(app_start.app_id.clone());
        state.phase = AppPhase::Running;
    }

    // Clear app_id on AppStop
    if let DaemonMessage::AppStop(app_stop) = msg {
        if state.current_app_id.as_ref() == Some(&app_stop.app_id) {
            state.current_app_id = None;
            state.phase = AppPhase::Initializing;
        }
    }
}

/// Convert key events to messages based on current UI mode
fn handle_key(state: &AppState, key: KeyEvent) -> Option<Message> {
    match state.ui_mode {
        UiMode::DeviceSelector => handle_key_device_selector(state, key),
        UiMode::ConfirmDialog => handle_key_confirm_dialog(key),
        UiMode::EmulatorSelector => handle_key_emulator_selector(key),
        UiMode::Loading => handle_key_loading(key),
        UiMode::Normal => handle_key_normal(state, key),
    }
}

/// Handle key events in device selector mode
fn handle_key_device_selector(state: &AppState, key: KeyEvent) -> Option<Message> {
    match key.code {
        // Navigation
        KeyCode::Up | KeyCode::Char('k') => Some(Message::DeviceSelectorUp),
        KeyCode::Down | KeyCode::Char('j') => Some(Message::DeviceSelectorDown),

        // Selection
        KeyCode::Enter => {
            if state.device_selector.is_device_selected() {
                if let Some(device) = state.device_selector.selected_device() {
                    return Some(Message::DeviceSelected {
                        device: device.clone(),
                    });
                }
            } else if state.device_selector.is_android_emulator_selected() {
                return Some(Message::LaunchAndroidEmulator);
            } else if state.device_selector.is_ios_simulator_selected() {
                return Some(Message::LaunchIOSSimulator);
            }
            None
        }

        // Refresh
        KeyCode::Char('r') => Some(Message::RefreshDevices),

        // Cancel/close - only if there are running sessions
        KeyCode::Esc => Some(Message::HideDeviceSelector),

        // Quit with Ctrl+C
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => Some(Message::Quit),
        KeyCode::Char('q') => Some(Message::Quit),

        _ => None,
    }
}

/// Handle key events in confirm dialog mode
fn handle_key_confirm_dialog(key: KeyEvent) -> Option<Message> {
    match (key.code, key.modifiers) {
        // Confirm quit
        (KeyCode::Char('y'), _) | (KeyCode::Char('Y'), _) | (KeyCode::Enter, _) => {
            Some(Message::ConfirmQuit)
        }
        // Cancel
        (KeyCode::Char('n'), _) | (KeyCode::Char('N'), _) | (KeyCode::Esc, _) => {
            Some(Message::CancelQuit)
        }
        // Force quit with Ctrl+C even in dialog
        (KeyCode::Char('c'), m) if m.contains(KeyModifiers::CONTROL) => Some(Message::Quit),
        _ => None,
    }
}

/// Handle key events in emulator selector mode (placeholder)
fn handle_key_emulator_selector(key: KeyEvent) -> Option<Message> {
    match key.code {
        KeyCode::Esc => Some(Message::ShowDeviceSelector), // Go back to device selector
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => Some(Message::Quit),
        _ => None,
    }
}

/// Handle key events in loading mode
fn handle_key_loading(key: KeyEvent) -> Option<Message> {
    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => Some(Message::Quit),
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => Some(Message::Quit),
        _ => None,
    }
}

/// Handle key events in normal mode
fn handle_key_normal(state: &AppState, key: KeyEvent) -> Option<Message> {
    // Check if we're busy (reloading)
    let is_busy = state.is_busy();

    match (key.code, key.modifiers) {
        // Request quit (may show confirmation dialog if sessions running)
        (KeyCode::Char('q'), KeyModifiers::NONE) => Some(Message::RequestQuit),
        (KeyCode::Esc, _) => Some(Message::RequestQuit),

        // Force quit (bypass confirmation) - Ctrl+C for emergency exit
        (KeyCode::Char('c'), m) if m.contains(KeyModifiers::CONTROL) => Some(Message::Quit),

        // ─────────────────────────────────────────────────────────
        // Session Navigation (Task 10)
        // ─────────────────────────────────────────────────────────
        // Number keys 1-9 select session by index
        (KeyCode::Char('1'), KeyModifiers::NONE) => Some(Message::SelectSessionByIndex(0)),
        (KeyCode::Char('2'), KeyModifiers::NONE) => Some(Message::SelectSessionByIndex(1)),
        (KeyCode::Char('3'), KeyModifiers::NONE) => Some(Message::SelectSessionByIndex(2)),
        (KeyCode::Char('4'), KeyModifiers::NONE) => Some(Message::SelectSessionByIndex(3)),
        (KeyCode::Char('5'), KeyModifiers::NONE) => Some(Message::SelectSessionByIndex(4)),
        (KeyCode::Char('6'), KeyModifiers::NONE) => Some(Message::SelectSessionByIndex(5)),
        (KeyCode::Char('7'), KeyModifiers::NONE) => Some(Message::SelectSessionByIndex(6)),
        (KeyCode::Char('8'), KeyModifiers::NONE) => Some(Message::SelectSessionByIndex(7)),
        (KeyCode::Char('9'), KeyModifiers::NONE) => Some(Message::SelectSessionByIndex(8)),

        // Tab navigation
        (KeyCode::Tab, KeyModifiers::NONE) => Some(Message::NextSession),
        (KeyCode::BackTab, _) => Some(Message::PreviousSession),
        (KeyCode::Tab, m) if m.contains(KeyModifiers::SHIFT) => Some(Message::PreviousSession),

        // Close current session
        (KeyCode::Char('x'), KeyModifiers::NONE) => Some(Message::CloseCurrentSession),
        (KeyCode::Char('w'), m) if m.contains(KeyModifiers::CONTROL) => {
            Some(Message::CloseCurrentSession)
        }

        // Clear logs
        (KeyCode::Char('c'), KeyModifiers::NONE) => Some(Message::ClearLogs),

        // ─────────────────────────────────────────────────────────
        // App Control
        // ─────────────────────────────────────────────────────────
        // Hot reload (lowercase 'r') - only when not busy
        (KeyCode::Char('r'), KeyModifiers::NONE) if !is_busy => Some(Message::HotReload),

        // Hot restart (uppercase 'R') - only when not busy
        (KeyCode::Char('R'), KeyModifiers::NONE) if !is_busy => Some(Message::HotRestart),
        (KeyCode::Char('R'), m) if m.contains(KeyModifiers::SHIFT) && !is_busy => {
            Some(Message::HotRestart)
        }

        // Stop app (lowercase 's') - only when not busy
        (KeyCode::Char('s'), KeyModifiers::NONE) if !is_busy => Some(Message::StopApp),

        // New session (lowercase 'n') - show device selector
        (KeyCode::Char('n'), KeyModifiers::NONE) => Some(Message::ShowDeviceSelector),
        // Also allow 'd' for device selector (as shown in header)
        (KeyCode::Char('d'), KeyModifiers::NONE) => Some(Message::ShowDeviceSelector),

        // ─────────────────────────────────────────────────────────
        // Scrolling - always allowed
        // ─────────────────────────────────────────────────────────
        (KeyCode::Char('j'), KeyModifiers::NONE) => Some(Message::ScrollDown),
        (KeyCode::Down, _) => Some(Message::ScrollDown),
        (KeyCode::Char('k'), KeyModifiers::NONE) => Some(Message::ScrollUp),
        (KeyCode::Up, _) => Some(Message::ScrollUp),
        (KeyCode::Char('g'), KeyModifiers::NONE) => Some(Message::ScrollToTop),
        (KeyCode::Char('G'), KeyModifiers::NONE) => Some(Message::ScrollToBottom),
        (KeyCode::Char('G'), m) if m.contains(KeyModifiers::SHIFT) => Some(Message::ScrollToBottom),
        (KeyCode::PageUp, _) => Some(Message::PageUp),
        (KeyCode::PageDown, _) => Some(Message::PageDown),
        (KeyCode::Home, _) => Some(Message::ScrollToTop),
        (KeyCode::End, _) => Some(Message::ScrollToBottom),

        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::state::AppState;

    #[test]
    fn test_quit_message_sets_quitting_phase() {
        let mut state = AppState::new();
        assert_ne!(state.phase, AppPhase::Quitting);

        update(&mut state, Message::Quit);

        assert_eq!(state.phase, AppPhase::Quitting);
        assert!(state.should_quit());
    }

    #[test]
    fn test_should_quit_returns_true_when_quitting() {
        let mut state = AppState::new();
        state.phase = AppPhase::Quitting;
        assert!(state.should_quit());
    }

    #[test]
    fn test_should_quit_returns_false_when_running() {
        let mut state = AppState::new();
        state.phase = AppPhase::Running;
        assert!(!state.should_quit());
    }

    #[test]
    fn test_q_key_produces_request_quit_message() {
        let state = AppState::new();
        let key = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE);

        let result = handle_key(&state, key);

        assert!(matches!(result, Some(Message::RequestQuit)));
    }

    #[test]
    fn test_escape_key_produces_request_quit_message() {
        let state = AppState::new();
        let key = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);

        let result = handle_key(&state, key);

        assert!(matches!(result, Some(Message::RequestQuit)));
    }

    #[test]
    fn test_ctrl_c_produces_quit_message() {
        let state = AppState::new();
        let key = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);

        let result = handle_key(&state, key);

        assert!(matches!(result, Some(Message::Quit)));
    }

    #[test]
    fn test_daemon_exited_event_logs_message() {
        let mut state = AppState::new();
        let initial_logs = state.logs.len();

        update(
            &mut state,
            Message::Daemon(DaemonEvent::Exited { code: Some(0) }),
        );

        assert!(state.logs.len() > initial_logs);
    }

    #[test]
    fn test_daemon_exited_sets_quitting_phase() {
        let mut state = AppState::new();
        state.phase = AppPhase::Running;

        update(
            &mut state,
            Message::Daemon(DaemonEvent::Exited { code: Some(0) }),
        );

        assert_eq!(state.phase, AppPhase::Quitting);
        assert!(state.should_quit());
    }

    #[test]
    fn test_daemon_exited_with_error_code_sets_quitting() {
        let mut state = AppState::new();
        state.phase = AppPhase::Running;

        update(
            &mut state,
            Message::Daemon(DaemonEvent::Exited { code: Some(1) }),
        );

        assert_eq!(state.phase, AppPhase::Quitting);
        // Verify warning log for non-zero exit code
        assert!(state
            .logs
            .iter()
            .any(|log| log.message.contains("exited with code 1")));
    }

    #[test]
    fn test_scroll_messages_update_log_view_state() {
        let mut state = AppState::new();
        state.log_view_state.total_lines = 100;
        state.log_view_state.visible_lines = 20;
        state.log_view_state.offset = 50;

        update(&mut state, Message::ScrollUp);
        assert_eq!(state.log_view_state.offset, 49);

        update(&mut state, Message::ScrollDown);
        assert_eq!(state.log_view_state.offset, 50);

        update(&mut state, Message::ScrollToTop);
        assert_eq!(state.log_view_state.offset, 0);

        update(&mut state, Message::ScrollToBottom);
        assert_eq!(state.log_view_state.offset, 80);
    }

    // ─────────────────────────────────────────────────────────
    // Reload/Restart Key Tests
    // ─────────────────────────────────────────────────────────

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
    fn test_s_key_produces_stop() {
        let mut state = AppState::new();
        state.phase = AppPhase::Running;

        let key = KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE);
        let result = handle_key(&state, key);

        assert!(matches!(result, Some(Message::StopApp)));
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
    fn test_restart_ignored_when_already_reloading() {
        let mut state = AppState::new();
        state.phase = AppPhase::Reloading;

        let key = KeyEvent::new(KeyCode::Char('R'), KeyModifiers::SHIFT);
        let result = handle_key(&state, key);

        assert!(result.is_none());
    }

    #[test]
    fn test_stop_ignored_when_already_reloading() {
        let mut state = AppState::new();
        state.phase = AppPhase::Reloading;

        let key = KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE);
        let result = handle_key(&state, key);

        assert!(result.is_none());
    }

    // ─────────────────────────────────────────────────────────
    // Reload State Tests
    // ─────────────────────────────────────────────────────────

    #[test]
    fn test_hot_reload_message_starts_reload() {
        let mut state = AppState::new();
        state.phase = AppPhase::Running;
        state.current_app_id = Some("test-app".to_string());

        let result = update(&mut state, Message::HotReload);

        assert!(state.is_busy());
        assert!(state.reload_start_time.is_some());
        assert!(matches!(
            result.action,
            Some(UpdateAction::SpawnTask(Task::Reload { .. }))
        ));
    }

    #[test]
    fn test_hot_reload_without_app_id_shows_error() {
        let mut state = AppState::new();
        state.phase = AppPhase::Running;
        state.current_app_id = None;

        let result = update(&mut state, Message::HotReload);

        assert!(!state.is_busy());
        assert!(result.action.is_none());
        assert!(state
            .logs
            .last()
            .unwrap()
            .message
            .contains("No app running"));
    }

    #[test]
    fn test_hot_reload_ignored_when_busy() {
        let mut state = AppState::new();
        state.phase = AppPhase::Reloading;

        let result = update(&mut state, Message::HotReload);

        assert!(result.action.is_none());
    }

    #[test]
    fn test_reload_completed_updates_state() {
        let mut state = AppState::new();
        state.phase = AppPhase::Reloading;
        state.reload_start_time = Some(std::time::Instant::now());

        update(&mut state, Message::ReloadCompleted { time_ms: 250 });

        assert_eq!(state.phase, AppPhase::Running);
        assert_eq!(state.reload_count, 1);
        assert!(state.last_reload_time.is_some());
        assert!(state.reload_start_time.is_none());
    }

    #[test]
    fn test_reload_failed_updates_state() {
        let mut state = AppState::new();
        state.phase = AppPhase::Reloading;
        state.reload_start_time = Some(std::time::Instant::now());

        update(
            &mut state,
            Message::ReloadFailed {
                reason: "Compile error".to_string(),
            },
        );

        assert_eq!(state.phase, AppPhase::Running);
        assert!(state.reload_start_time.is_none());
        assert!(state.logs.last().unwrap().message.contains("Compile error"));
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

    #[test]
    fn test_is_busy_when_reloading() {
        let mut state = AppState::new();
        assert!(!state.is_busy());

        state.phase = AppPhase::Reloading;
        assert!(state.is_busy());

        state.phase = AppPhase::Running;
        assert!(!state.is_busy());
    }

    #[test]
    fn test_restart_completed_updates_state() {
        let mut state = AppState::new();
        state.phase = AppPhase::Reloading;

        update(&mut state, Message::RestartCompleted);

        assert_eq!(state.phase, AppPhase::Running);
        assert_eq!(state.reload_count, 1);
    }

    #[test]
    fn test_restart_failed_updates_state() {
        let mut state = AppState::new();
        state.phase = AppPhase::Reloading;

        update(
            &mut state,
            Message::RestartFailed {
                reason: "Failed to restart".to_string(),
            },
        );

        assert_eq!(state.phase, AppPhase::Running);
        assert!(state
            .logs
            .last()
            .unwrap()
            .message
            .contains("Failed to restart"));
    }

    #[test]
    fn test_stop_app_spawns_task() {
        let mut state = AppState::new();
        state.phase = AppPhase::Running;
        state.current_app_id = Some("test-app".to_string());

        let result = update(&mut state, Message::StopApp);

        assert!(matches!(
            result.action,
            Some(UpdateAction::SpawnTask(Task::Stop { .. }))
        ));
    }

    #[test]
    fn test_stop_app_without_app_id_shows_error() {
        let mut state = AppState::new();
        state.phase = AppPhase::Running;
        state.current_app_id = None;

        let result = update(&mut state, Message::StopApp);

        assert!(result.action.is_none());
        assert!(state
            .logs
            .last()
            .unwrap()
            .message
            .contains("No app running"));
    }

    // ─────────────────────────────────────────────────────────
    // File Watcher Tests
    // ─────────────────────────────────────────────────────────

    #[test]
    fn test_auto_reload_triggered_when_app_running() {
        let mut state = AppState::new();
        state.phase = AppPhase::Running;
        state.current_app_id = Some("test-app".to_string());

        let result = update(&mut state, Message::AutoReloadTriggered);

        assert!(state.is_busy());
        assert!(matches!(
            result.action,
            Some(UpdateAction::SpawnTask(Task::Reload { .. }))
        ));
        assert!(state
            .logs
            .last()
            .unwrap()
            .message
            .contains("File change detected"));
    }

    #[test]
    fn test_auto_reload_skipped_when_no_app() {
        let mut state = AppState::new();
        state.phase = AppPhase::Running;
        state.current_app_id = None;

        let result = update(&mut state, Message::AutoReloadTriggered);

        assert!(!state.is_busy());
        assert!(result.action.is_none());
    }

    #[test]
    fn test_auto_reload_skipped_when_busy() {
        let mut state = AppState::new();
        state.phase = AppPhase::Reloading;
        state.current_app_id = Some("test-app".to_string());

        let result = update(&mut state, Message::AutoReloadTriggered);

        assert!(result.action.is_none());
    }

    #[test]
    fn test_files_changed_logs_count() {
        let mut state = AppState::new();

        update(&mut state, Message::FilesChanged { count: 3 });

        assert!(state.logs.last().unwrap().message.contains("3 file(s)"));
    }

    #[test]
    fn test_watcher_error_logs_message() {
        let mut state = AppState::new();

        update(
            &mut state,
            Message::WatcherError {
                message: "Permission denied".to_string(),
            },
        );

        assert!(state
            .logs
            .last()
            .unwrap()
            .message
            .contains("Permission denied"));
        assert!(state.logs.last().unwrap().is_error());
    }

    // ─────────────────────────────────────────────────────────
    // App ID Tracking Tests
    // ─────────────────────────────────────────────────────────

    #[test]
    fn test_app_start_captures_app_id() {
        let mut state = AppState::new();
        assert!(state.current_app_id.is_none());

        let app_start = crate::daemon::events::AppStart {
            app_id: "my-app-123".to_string(),
            device_id: "device-1".to_string(),
            directory: "/path/to/app".to_string(),
            supports_restart: true,
            launch_mode: Some("run".to_string()),
        };

        handle_daemon_message_state(&mut state, &DaemonMessage::AppStart(app_start));

        assert_eq!(state.current_app_id, Some("my-app-123".to_string()));
        assert_eq!(state.phase, AppPhase::Running);
    }

    #[test]
    fn test_app_stop_clears_app_id() {
        let mut state = AppState::new();
        state.current_app_id = Some("my-app-123".to_string());
        state.phase = AppPhase::Running;

        let app_stop = crate::daemon::events::AppStop {
            app_id: "my-app-123".to_string(),
            error: None,
        };

        handle_daemon_message_state(&mut state, &DaemonMessage::AppStop(app_stop));

        assert!(state.current_app_id.is_none());
        assert_eq!(state.phase, AppPhase::Initializing);
    }

    // ─────────────────────────────────────────────────────────
    // Raw Line Level Detection Tests (Task 07)
    // ─────────────────────────────────────────────────────────

    #[test]
    fn test_detect_raw_line_level_android() {
        let (level, _) = detect_raw_line_level("E/flutter: Error in app");
        assert_eq!(level, LogLevel::Error);

        let (level, _) = detect_raw_line_level("W/flutter: Warning message");
        assert_eq!(level, LogLevel::Warning);
    }

    #[test]
    fn test_detect_raw_line_level_gradle() {
        let (level, _) = detect_raw_line_level("FAILURE: Build failed");
        assert_eq!(level, LogLevel::Error);

        let (level, _) = detect_raw_line_level("BUILD FAILED in 5s");
        assert_eq!(level, LogLevel::Error);
    }

    #[test]
    fn test_detect_raw_line_level_xcode() {
        let (level, _) = detect_raw_line_level("error: cannot find module");
        assert_eq!(level, LogLevel::Error);
    }

    #[test]
    fn test_detect_raw_line_level_build_progress() {
        let (level, _) = detect_raw_line_level("Running Gradle task 'assembleDebug'...");
        assert_eq!(level, LogLevel::Debug);

        let (level, _) = detect_raw_line_level("Building flutter assets...");
        assert_eq!(level, LogLevel::Debug);
    }

    #[test]
    fn test_detect_raw_line_level_default() {
        let (level, msg) = detect_raw_line_level("Normal log message");
        assert_eq!(level, LogLevel::Info);
        assert_eq!(msg, "Normal log message");
    }

    #[test]
    fn test_detect_raw_line_level_trims_whitespace() {
        let (_, msg) = detect_raw_line_level("  Some message with spaces  ");
        assert_eq!(msg, "Some message with spaces");
    }

    // ─────────────────────────────────────────────────────────
    // Task 10: Keyboard Shortcuts Tests
    // ─────────────────────────────────────────────────────────

    #[test]
    fn test_number_keys_select_session() {
        let state = AppState::new();

        let key1 = KeyEvent::new(KeyCode::Char('1'), KeyModifiers::NONE);
        let key5 = KeyEvent::new(KeyCode::Char('5'), KeyModifiers::NONE);
        let key9 = KeyEvent::new(KeyCode::Char('9'), KeyModifiers::NONE);

        assert!(matches!(
            handle_key(&state, key1),
            Some(Message::SelectSessionByIndex(0))
        ));
        assert!(matches!(
            handle_key(&state, key5),
            Some(Message::SelectSessionByIndex(4))
        ));
        assert!(matches!(
            handle_key(&state, key9),
            Some(Message::SelectSessionByIndex(8))
        ));
    }

    #[test]
    fn test_tab_cycles_sessions() {
        let state = AppState::new();

        let tab = KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE);
        let shift_tab = KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT);

        assert!(matches!(
            handle_key(&state, tab),
            Some(Message::NextSession)
        ));
        assert!(matches!(
            handle_key(&state, shift_tab),
            Some(Message::PreviousSession)
        ));
    }

    #[test]
    fn test_x_closes_session() {
        let state = AppState::new();

        let key = KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE);
        assert!(matches!(
            handle_key(&state, key),
            Some(Message::CloseCurrentSession)
        ));
    }

    #[test]
    fn test_ctrl_w_closes_session() {
        let state = AppState::new();

        let key = KeyEvent::new(KeyCode::Char('w'), KeyModifiers::CONTROL);
        assert!(matches!(
            handle_key(&state, key),
            Some(Message::CloseCurrentSession)
        ));
    }

    #[test]
    fn test_c_clears_logs() {
        let state = AppState::new();

        let key = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE);
        assert!(matches!(handle_key(&state, key), Some(Message::ClearLogs)));
    }

    #[test]
    fn test_d_shows_device_selector() {
        let state = AppState::new();

        let key = KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE);
        assert!(matches!(
            handle_key(&state, key),
            Some(Message::ShowDeviceSelector)
        ));
    }

    #[test]
    fn test_n_shows_device_selector() {
        let state = AppState::new();

        let key = KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE);
        assert!(matches!(
            handle_key(&state, key),
            Some(Message::ShowDeviceSelector)
        ));
    }

    #[test]
    fn test_select_session_by_index_message() {
        let mut state = AppState::new();

        // Create some sessions
        let device1 = Device {
            id: "d1".to_string(),
            name: "Device 1".to_string(),
            platform: "ios".to_string(),
            emulator: false,
            category: None,
            platform_type: None,
            ephemeral: false,
            emulator_id: None,
        };
        let device2 = Device {
            id: "d2".to_string(),
            name: "Device 2".to_string(),
            platform: "android".to_string(),
            emulator: false,
            category: None,
            platform_type: None,
            ephemeral: false,
            emulator_id: None,
        };

        state.session_manager.create_session(&device1).unwrap();
        state.session_manager.create_session(&device2).unwrap();

        assert_eq!(state.session_manager.selected_index(), 0);

        update(&mut state, Message::SelectSessionByIndex(1));
        assert_eq!(state.session_manager.selected_index(), 1);

        // Invalid index should be ignored
        update(&mut state, Message::SelectSessionByIndex(5));
        assert_eq!(state.session_manager.selected_index(), 1);
    }

    #[test]
    fn test_next_previous_session_messages() {
        let mut state = AppState::new();

        let device1 = Device {
            id: "d1".to_string(),
            name: "Device 1".to_string(),
            platform: "ios".to_string(),
            emulator: false,
            category: None,
            platform_type: None,
            ephemeral: false,
            emulator_id: None,
        };
        let device2 = Device {
            id: "d2".to_string(),
            name: "Device 2".to_string(),
            platform: "android".to_string(),
            emulator: false,
            category: None,
            platform_type: None,
            ephemeral: false,
            emulator_id: None,
        };

        state.session_manager.create_session(&device1).unwrap();
        state.session_manager.create_session(&device2).unwrap();

        assert_eq!(state.session_manager.selected_index(), 0);

        update(&mut state, Message::NextSession);
        assert_eq!(state.session_manager.selected_index(), 1);

        update(&mut state, Message::NextSession);
        assert_eq!(state.session_manager.selected_index(), 0); // Wraps

        update(&mut state, Message::PreviousSession);
        assert_eq!(state.session_manager.selected_index(), 1); // Wraps back
    }

    #[test]
    fn test_clear_logs_message() {
        let mut state = AppState::new();

        let device = Device {
            id: "d1".to_string(),
            name: "Device 1".to_string(),
            platform: "ios".to_string(),
            emulator: false,
            category: None,
            platform_type: None,
            ephemeral: false,
            emulator_id: None,
        };

        let id = state.session_manager.create_session(&device).unwrap();

        // Add some logs
        state
            .session_manager
            .get_mut(id)
            .unwrap()
            .session
            .log_info(LogSource::App, "Test log 1");
        state
            .session_manager
            .get_mut(id)
            .unwrap()
            .session
            .log_info(LogSource::App, "Test log 2");

        assert_eq!(state.session_manager.get(id).unwrap().session.logs.len(), 2);

        update(&mut state, Message::ClearLogs);

        assert_eq!(state.session_manager.get(id).unwrap().session.logs.len(), 0);
    }

    #[test]
    fn test_close_single_session_triggers_quit_confirmation() {
        let mut state = AppState::new();
        state.ui_mode = UiMode::Normal;
        state.settings.behavior.confirm_quit = true;

        let device = Device {
            id: "d1".to_string(),
            name: "Device 1".to_string(),
            platform: "ios".to_string(),
            emulator: false,
            category: None,
            platform_type: None,
            ephemeral: false,
            emulator_id: None,
        };

        let id = state.session_manager.create_session(&device).unwrap();
        // Mark session as running so quit confirmation is triggered
        state
            .session_manager
            .get_mut(id)
            .unwrap()
            .session
            .mark_started("app-1".to_string());

        let _result = update(&mut state, Message::CloseCurrentSession);

        // Session should NOT be removed (quit confirmation shown instead)
        assert!(!state.session_manager.is_empty());

        // Should show confirmation dialog when closing last running session
        assert_eq!(state.ui_mode, UiMode::ConfirmDialog);
    }

    #[test]
    fn test_close_session_shows_device_selector_when_multiple() {
        let mut state = AppState::new();
        state.ui_mode = UiMode::Normal;

        // Create two sessions
        let device1 = Device {
            id: "d1".to_string(),
            name: "Device 1".to_string(),
            platform: "ios".to_string(),
            emulator: false,
            category: None,
            platform_type: None,
            ephemeral: false,
            emulator_id: None,
        };
        let device2 = Device {
            id: "d2".to_string(),
            name: "Device 2".to_string(),
            platform: "android".to_string(),
            emulator: false,
            category: None,
            platform_type: None,
            ephemeral: false,
            emulator_id: None,
        };

        state.session_manager.create_session(&device1).unwrap();
        state.session_manager.create_session(&device2).unwrap();
        assert_eq!(state.session_manager.len(), 2);

        let _result = update(&mut state, Message::CloseCurrentSession);

        // One session should be removed
        assert_eq!(state.session_manager.len(), 1);

        // Should remain in normal mode (not device selector)
        assert_eq!(state.ui_mode, UiMode::Normal);
    }

    // ─────────────────────────────────────────────────────────
    // Task 02: DeviceSelected Creates Session Tests
    // ─────────────────────────────────────────────────────────

    #[test]
    fn test_device_selected_creates_session() {
        let mut state = AppState::new();
        state.ui_mode = UiMode::DeviceSelector;

        let device = Device {
            id: "device-1".to_string(),
            name: "Test Device".to_string(),
            platform: "ios".to_string(),
            emulator: false,
            category: None,
            platform_type: None,
            ephemeral: false,
            emulator_id: None,
        };

        let result = update(
            &mut state,
            Message::DeviceSelected {
                device: device.clone(),
            },
        );

        // Session should be created
        assert_eq!(state.session_manager.len(), 1);

        // Should return SpawnSession action with valid session_id
        match result.action {
            Some(UpdateAction::SpawnSession { session_id, .. }) => {
                assert!(session_id > 0, "session_id should be a valid non-zero ID");
                // Session should exist in manager
                assert!(state.session_manager.get(session_id).is_some());
            }
            _ => panic!("Expected SpawnSession action"),
        }

        // UI mode should be Normal
        assert_eq!(state.ui_mode, UiMode::Normal);
    }

    #[test]
    fn test_device_selected_prevents_duplicate() {
        let mut state = AppState::new();
        state.ui_mode = UiMode::DeviceSelector;

        let device = Device {
            id: "device-1".to_string(),
            name: "Test Device".to_string(),
            platform: "ios".to_string(),
            emulator: false,
            category: None,
            platform_type: None,
            ephemeral: false,
            emulator_id: None,
        };

        // First selection succeeds
        let _ = update(
            &mut state,
            Message::DeviceSelected {
                device: device.clone(),
            },
        );
        assert_eq!(state.session_manager.len(), 1);

        // Show device selector again
        state.ui_mode = UiMode::DeviceSelector;

        // Second selection of same device should fail
        let result = update(&mut state, Message::DeviceSelected { device });

        // Should NOT create another session
        assert_eq!(state.session_manager.len(), 1);

        // Should return no action
        assert!(result.action.is_none());

        // Should have logged an error about duplicate
        assert!(state
            .logs
            .iter()
            .any(|log| log.message.contains("already has an active session")));
    }

    #[test]
    fn test_device_selected_max_sessions_enforced() {
        use crate::app::session_manager::MAX_SESSIONS;

        let mut state = AppState::new();

        // Create MAX_SESSIONS (9) sessions
        for i in 0..MAX_SESSIONS {
            let device = Device {
                id: format!("device-{}", i),
                name: format!("Device {}", i),
                platform: "ios".to_string(),
                emulator: false,
                category: None,
                platform_type: None,
                ephemeral: false,
                emulator_id: None,
            };
            state.ui_mode = UiMode::DeviceSelector;
            let _ = update(&mut state, Message::DeviceSelected { device });
        }

        assert_eq!(state.session_manager.len(), MAX_SESSIONS);

        // 10th should fail
        let device = Device {
            id: "device-extra".to_string(),
            name: "Device Extra".to_string(),
            platform: "ios".to_string(),
            emulator: false,
            category: None,
            platform_type: None,
            ephemeral: false,
            emulator_id: None,
        };
        state.ui_mode = UiMode::DeviceSelector;
        let result = update(&mut state, Message::DeviceSelected { device });

        // Should NOT create another session
        assert_eq!(state.session_manager.len(), MAX_SESSIONS);
        assert!(result.action.is_none());

        // Should have logged an error about max sessions
        assert!(state
            .logs
            .iter()
            .any(|log| log.message.contains("Failed to create session")));
    }

    #[test]
    fn test_device_selected_session_id_in_spawn_action() {
        let mut state = AppState::new();
        state.ui_mode = UiMode::DeviceSelector;

        let device = Device {
            id: "test-device".to_string(),
            name: "Test Device".to_string(),
            platform: "android".to_string(),
            emulator: true,
            category: None,
            platform_type: None,
            ephemeral: false,
            emulator_id: None,
        };

        let result = update(
            &mut state,
            Message::DeviceSelected {
                device: device.clone(),
            },
        );

        // Verify the session_id in the action matches the created session
        match result.action {
            Some(UpdateAction::SpawnSession {
                session_id,
                device: action_device,
                config,
            }) => {
                // Session ID should match what's in the manager
                let session = state.session_manager.get(session_id).unwrap();
                assert_eq!(session.session.device_id, device.id);
                assert_eq!(session.session.device_name, device.name);

                // Device should be passed through
                assert_eq!(action_device.id, device.id);

                // No config for basic selection
                assert!(config.is_none());
            }
            _ => panic!("Expected SpawnSession action"),
        }
    }

    // ─────────────────────────────────────────────────────────
    // Task 04: Session CommandSender Storage Tests
    // ─────────────────────────────────────────────────────────

    #[test]
    fn test_reload_uses_session_when_no_cmd_sender() {
        let mut state = AppState::new();

        // Create session and mark as running
        let device = Device {
            id: "d1".to_string(),
            name: "Device 1".to_string(),
            platform: "ios".to_string(),
            emulator: false,
            category: None,
            platform_type: None,
            ephemeral: false,
            emulator_id: None,
        };
        let session_id = state.session_manager.create_session(&device).unwrap();
        state
            .session_manager
            .get_mut(session_id)
            .unwrap()
            .session
            .mark_started("app-123".to_string());

        // Without cmd_sender, should fall back to legacy
        state.current_app_id = Some("legacy-app".to_string());

        let result = update(&mut state, Message::HotReload);

        // Should use legacy app_id since session has no cmd_sender
        match result.action {
            Some(UpdateAction::SpawnTask(Task::Reload {
                session_id: task_session_id,
                app_id,
            })) => {
                // Legacy mode uses session_id 0
                assert_eq!(task_session_id, 0);
                assert_eq!(app_id, "legacy-app");
            }
            _ => panic!("Expected SpawnTask action"),
        }
    }

    #[test]
    fn test_reload_no_app_running_shows_error() {
        let mut state = AppState::new();

        // No session, no legacy app_id
        let result = update(&mut state, Message::HotReload);

        assert!(result.action.is_none());
        assert!(state
            .logs
            .iter()
            .any(|log| log.message.contains("No app running")));
    }

    #[test]
    fn test_restart_no_app_running_shows_error() {
        let mut state = AppState::new();

        let result = update(&mut state, Message::HotRestart);

        assert!(result.action.is_none());
        assert!(state
            .logs
            .iter()
            .any(|log| log.message.contains("No app running")));
    }

    #[test]
    fn test_stop_no_app_running_shows_error() {
        let mut state = AppState::new();

        let result = update(&mut state, Message::StopApp);

        assert!(result.action.is_none());
        assert!(state
            .logs
            .iter()
            .any(|log| log.message.contains("No app running")));
    }

    #[test]
    fn test_session_spawn_failed_removes_session() {
        let mut state = AppState::new();

        // Create session
        let device = Device {
            id: "d1".to_string(),
            name: "Device 1".to_string(),
            platform: "ios".to_string(),
            emulator: false,
            category: None,
            platform_type: None,
            ephemeral: false,
            emulator_id: None,
        };
        let session_id = state.session_manager.create_session(&device).unwrap();
        assert_eq!(state.session_manager.len(), 1);

        // Simulate spawn failure
        let _ = update(
            &mut state,
            Message::SessionSpawnFailed {
                session_id,
                device_id: "d1".to_string(),
                error: "Test error".to_string(),
            },
        );

        // Session should be removed
        assert_eq!(state.session_manager.len(), 0);

        // Should show device selector
        assert_eq!(state.ui_mode, UiMode::DeviceSelector);
    }

    #[test]
    fn test_session_started_logs_with_session_id() {
        let mut state = AppState::new();

        let _ = update(
            &mut state,
            Message::SessionStarted {
                session_id: 42,
                device_id: "d1".to_string(),
                device_name: "Test Device".to_string(),
                platform: "ios".to_string(),
                pid: Some(12345),
            },
        );

        // Should have logged with session_id
        assert!(state.logs.iter().any(|log| log.message.contains("42")));
    }

    #[test]
    fn test_task_enum_includes_session_id() {
        // Verify Task enum structure includes session_id
        let reload = Task::Reload {
            session_id: 1,
            app_id: "app".to_string(),
        };
        let restart = Task::Restart {
            session_id: 2,
            app_id: "app".to_string(),
        };
        let stop = Task::Stop {
            session_id: 3,
            app_id: "app".to_string(),
        };

        // Verify Debug formatting includes session_id
        let reload_debug = format!("{:?}", reload);
        let restart_debug = format!("{:?}", restart);
        let stop_debug = format!("{:?}", stop);

        assert!(reload_debug.contains("session_id: 1"));
        assert!(restart_debug.contains("session_id: 2"));
        assert!(stop_debug.contains("session_id: 3"));
    }

    // ─────────────────────────────────────────────────────────
    // Task 05: Session Daemon Event Routing Tests
    // ─────────────────────────────────────────────────────────

    #[test]
    fn test_session_daemon_event_routes_to_correct_session() {
        let mut state = AppState::new();

        // Create two sessions
        let device1 = Device {
            id: "d1".to_string(),
            name: "Device 1".to_string(),
            platform: "ios".to_string(),
            emulator: false,
            category: None,
            platform_type: None,
            ephemeral: false,
            emulator_id: None,
        };
        let device2 = Device {
            id: "d2".to_string(),
            name: "Device 2".to_string(),
            platform: "android".to_string(),
            emulator: false,
            category: None,
            platform_type: None,
            ephemeral: false,
            emulator_id: None,
        };

        let id1 = state.session_manager.create_session(&device1).unwrap();
        let id2 = state.session_manager.create_session(&device2).unwrap();

        // Send stdout event to session 1
        update(
            &mut state,
            Message::SessionDaemon {
                session_id: id1,
                event: DaemonEvent::Stdout("Test log for session 1".to_string()),
            },
        );

        // Send stdout event to session 2
        update(
            &mut state,
            Message::SessionDaemon {
                session_id: id2,
                event: DaemonEvent::Stdout("Test log for session 2".to_string()),
            },
        );

        // Check logs are in correct sessions
        let logs1 = &state.session_manager.get(id1).unwrap().session.logs;
        let logs2 = &state.session_manager.get(id2).unwrap().session.logs;

        assert!(logs1.iter().any(|l| l.message.contains("session 1")));
        assert!(!logs1.iter().any(|l| l.message.contains("session 2")));

        assert!(logs2.iter().any(|l| l.message.contains("session 2")));
        assert!(!logs2.iter().any(|l| l.message.contains("session 1")));
    }

    #[test]
    fn test_session_daemon_stderr_routes_correctly() {
        let mut state = AppState::new();

        let device = Device {
            id: "d1".to_string(),
            name: "Device 1".to_string(),
            platform: "ios".to_string(),
            emulator: false,
            category: None,
            platform_type: None,
            ephemeral: false,
            emulator_id: None,
        };

        let session_id = state.session_manager.create_session(&device).unwrap();

        // Send stderr event
        update(
            &mut state,
            Message::SessionDaemon {
                session_id,
                event: DaemonEvent::Stderr("Error message here".to_string()),
            },
        );

        // Check log was added with Error level
        let logs = &state.session_manager.get(session_id).unwrap().session.logs;
        assert!(logs.iter().any(|l| l.message.contains("Error message")));
        assert!(logs.iter().any(|l| l.is_error()));
    }

    #[test]
    fn test_session_app_start_updates_session_state() {
        let mut state = AppState::new();

        let device = Device {
            id: "d1".to_string(),
            name: "Device 1".to_string(),
            platform: "ios".to_string(),
            emulator: false,
            category: None,
            platform_type: None,
            ephemeral: false,
            emulator_id: None,
        };

        let session_id = state.session_manager.create_session(&device).unwrap();

        // Simulate app.start event via JSON
        let app_start_json = r#"[{"event":"app.start","params":{"appId":"app-123","deviceId":"d1","directory":"/app","supportsRestart":true}}]"#;

        update(
            &mut state,
            Message::SessionDaemon {
                session_id,
                event: DaemonEvent::Stdout(app_start_json.to_string()),
            },
        );

        // Check session was marked as started
        let session = &state.session_manager.get(session_id).unwrap().session;
        assert_eq!(session.app_id, Some("app-123".to_string()));
        assert_eq!(session.phase, AppPhase::Running);

        // Also check global state for legacy compatibility
        assert_eq!(state.current_app_id, Some("app-123".to_string()));
    }

    #[test]
    fn test_session_exited_updates_session_phase() {
        let mut state = AppState::new();

        let device = Device {
            id: "d1".to_string(),
            name: "Device 1".to_string(),
            platform: "ios".to_string(),
            emulator: false,
            category: None,
            platform_type: None,
            ephemeral: false,
            emulator_id: None,
        };

        let session_id = state.session_manager.create_session(&device).unwrap();

        // Mark session as running first
        state
            .session_manager
            .get_mut(session_id)
            .unwrap()
            .session
            .mark_started("app-1".to_string());

        assert_eq!(
            state.session_manager.get(session_id).unwrap().session.phase,
            AppPhase::Running
        );

        // Simulate process exit
        update(
            &mut state,
            Message::SessionDaemon {
                session_id,
                event: DaemonEvent::Exited { code: Some(0) },
            },
        );

        // Session should now be stopped (not quitting like legacy mode)
        assert_eq!(
            state.session_manager.get(session_id).unwrap().session.phase,
            AppPhase::Stopped
        );

        // App should NOT auto-quit - session remains for user to inspect
        assert!(!state.should_quit());
    }

    #[test]
    fn test_event_for_closed_session_is_discarded() {
        let mut state = AppState::new();

        let device = Device {
            id: "d1".to_string(),
            name: "Device 1".to_string(),
            platform: "ios".to_string(),
            emulator: false,
            category: None,
            platform_type: None,
            ephemeral: false,
            emulator_id: None,
        };

        let session_id = state.session_manager.create_session(&device).unwrap();

        // Remove the session
        state.session_manager.remove_session(session_id);

        // Send event to removed session - should not panic
        let result = update(
            &mut state,
            Message::SessionDaemon {
                session_id,
                event: DaemonEvent::Stdout("test".to_string()),
            },
        );

        // Should complete without error and no action
        assert!(result.action.is_none());
    }

    #[test]
    fn test_session_exited_with_error_code() {
        let mut state = AppState::new();

        let device = Device {
            id: "d1".to_string(),
            name: "Device 1".to_string(),
            platform: "ios".to_string(),
            emulator: false,
            category: None,
            platform_type: None,
            ephemeral: false,
            emulator_id: None,
        };

        let session_id = state.session_manager.create_session(&device).unwrap();

        // Simulate process exit with error code
        update(
            &mut state,
            Message::SessionDaemon {
                session_id,
                event: DaemonEvent::Exited { code: Some(1) },
            },
        );

        // Check log contains exit code
        let logs = &state.session_manager.get(session_id).unwrap().session.logs;
        assert!(logs
            .iter()
            .any(|l| l.message.contains("exited with code 1")));
    }

    #[test]
    fn test_legacy_daemon_event_still_works() {
        let mut state = AppState::new();
        state.phase = AppPhase::Running;

        // Use legacy Message::Daemon (not SessionDaemon)
        update(
            &mut state,
            Message::Daemon(DaemonEvent::Stdout("Legacy log message".to_string())),
        );

        // Should go to global logs, not session logs
        assert!(state
            .logs
            .iter()
            .any(|l| l.message.contains("Legacy log message")));
    }

    #[test]
    fn test_session_daemon_spawn_failed() {
        let mut state = AppState::new();

        let device = Device {
            id: "d1".to_string(),
            name: "Device 1".to_string(),
            platform: "ios".to_string(),
            emulator: false,
            category: None,
            platform_type: None,
            ephemeral: false,
            emulator_id: None,
        };

        let session_id = state.session_manager.create_session(&device).unwrap();

        // Simulate spawn failed event
        update(
            &mut state,
            Message::SessionDaemon {
                session_id,
                event: DaemonEvent::SpawnFailed {
                    reason: "Flutter not found".to_string(),
                },
            },
        );

        // Check error was logged to session
        let logs = &state.session_manager.get(session_id).unwrap().session.logs;
        assert!(logs.iter().any(|l| l.message.contains("Flutter not found")));
    }

    // ─────────────────────────────────────────────────────────
    // Task 06: SessionStarted Handler Tests
    // ─────────────────────────────────────────────────────────

    #[test]
    fn test_session_started_updates_session_state() {
        let mut state = AppState::new();

        let device = Device {
            id: "d1".to_string(),
            name: "iPhone 15".to_string(),
            platform: "ios".to_string(),
            emulator: false,
            category: None,
            platform_type: None,
            ephemeral: false,
            emulator_id: None,
        };

        let session_id = state.session_manager.create_session(&device).unwrap();

        // Initially Initializing
        assert_eq!(
            state.session_manager.get(session_id).unwrap().session.phase,
            AppPhase::Initializing
        );

        // Simulate SessionStarted
        update(
            &mut state,
            Message::SessionStarted {
                session_id,
                device_id: "d1".into(),
                device_name: "iPhone 15".into(),
                platform: "ios".into(),
                pid: Some(12345),
            },
        );

        let session = &state.session_manager.get(session_id).unwrap().session;

        // Phase should be Running
        assert_eq!(session.phase, AppPhase::Running);

        // started_at should be set
        assert!(session.started_at.is_some());

        // Should have a log entry with PID
        assert!(!session.logs.is_empty());
        assert!(session.logs.iter().any(|l| l.message.contains("12345")));
    }

    #[test]
    fn test_session_spawn_failed_logs_and_removes() {
        let mut state = AppState::new();

        let device = Device {
            id: "d1".to_string(),
            name: "iPhone 15".to_string(),
            platform: "ios".to_string(),
            emulator: false,
            category: None,
            platform_type: None,
            ephemeral: false,
            emulator_id: None,
        };

        let session_id = state.session_manager.create_session(&device).unwrap();
        assert_eq!(state.session_manager.len(), 1);

        // Simulate spawn failure via SessionSpawnFailed message
        update(
            &mut state,
            Message::SessionSpawnFailed {
                session_id,
                device_id: "d1".into(),
                error: "Connection refused".into(),
            },
        );

        // Session should be removed
        assert_eq!(state.session_manager.len(), 0);

        // Should show device selector
        assert_eq!(state.ui_mode, UiMode::DeviceSelector);

        // Global logs should have error
        assert!(state
            .logs
            .iter()
            .any(|l| l.message.contains("Connection refused")));
    }

    #[test]
    fn test_multiple_sessions_have_independent_start_state() {
        let mut state = AppState::new();

        let d1 = Device {
            id: "d1".to_string(),
            name: "iPhone 15".to_string(),
            platform: "ios".to_string(),
            emulator: false,
            category: None,
            platform_type: None,
            ephemeral: false,
            emulator_id: None,
        };
        let d2 = Device {
            id: "d2".to_string(),
            name: "Pixel 8".to_string(),
            platform: "android".to_string(),
            emulator: false,
            category: None,
            platform_type: None,
            ephemeral: false,
            emulator_id: None,
        };

        let id1 = state.session_manager.create_session(&d1).unwrap();
        let id2 = state.session_manager.create_session(&d2).unwrap();

        // Start session 1 only
        update(
            &mut state,
            Message::SessionStarted {
                session_id: id1,
                device_id: "d1".into(),
                device_name: "iPhone 15".into(),
                platform: "ios".into(),
                pid: Some(1000),
            },
        );

        // Session 1 should be Running, Session 2 still Initializing
        assert_eq!(
            state.session_manager.get(id1).unwrap().session.phase,
            AppPhase::Running
        );
        assert_eq!(
            state.session_manager.get(id2).unwrap().session.phase,
            AppPhase::Initializing
        );

        // Session 1 should have started_at set, Session 2 should not
        assert!(state
            .session_manager
            .get(id1)
            .unwrap()
            .session
            .started_at
            .is_some());
        assert!(state
            .session_manager
            .get(id2)
            .unwrap()
            .session
            .started_at
            .is_none());

        // Start session 2
        update(
            &mut state,
            Message::SessionStarted {
                session_id: id2,
                device_id: "d2".into(),
                device_name: "Pixel 8".into(),
                platform: "android".into(),
                pid: Some(2000),
            },
        );

        // Both should now be Running
        assert_eq!(
            state.session_manager.get(id1).unwrap().session.phase,
            AppPhase::Running
        );
        assert_eq!(
            state.session_manager.get(id2).unwrap().session.phase,
            AppPhase::Running
        );

        // Each should have their own logs with their PID
        let logs1 = &state.session_manager.get(id1).unwrap().session.logs;
        let logs2 = &state.session_manager.get(id2).unwrap().session.logs;

        assert!(logs1.iter().any(|l| l.message.contains("1000")));
        assert!(!logs1.iter().any(|l| l.message.contains("2000")));

        assert!(logs2.iter().any(|l| l.message.contains("2000")));
        assert!(!logs2.iter().any(|l| l.message.contains("1000")));
    }

    #[test]
    fn test_session_duration_calculation() {
        let mut state = AppState::new();

        let device = Device {
            id: "d1".to_string(),
            name: "iPhone 15".to_string(),
            platform: "ios".to_string(),
            emulator: false,
            category: None,
            platform_type: None,
            ephemeral: false,
            emulator_id: None,
        };

        let session_id = state.session_manager.create_session(&device).unwrap();

        // Before start, no duration
        assert!(state
            .session_manager
            .get(session_id)
            .unwrap()
            .session
            .session_duration()
            .is_none());

        // Start session
        update(
            &mut state,
            Message::SessionStarted {
                session_id,
                device_id: "d1".into(),
                device_name: "iPhone 15".into(),
                platform: "ios".into(),
                pid: Some(12345),
            },
        );

        let session = &state.session_manager.get(session_id).unwrap().session;

        // Duration should be calculable
        assert!(session.session_duration().is_some());
        assert!(session.session_duration_display().is_some());

        // Duration should be very small (just started)
        let duration = session.session_duration().unwrap();
        assert!(duration.num_seconds() < 2);

        // Display format should be HH:MM:SS
        let display = session.session_duration_display().unwrap();
        assert!(display.contains(':'));
        assert_eq!(display.len(), 8); // "00:00:00" format
    }

    #[test]
    fn test_session_started_updates_legacy_global_state() {
        let mut state = AppState::new();
        assert!(state.device_name.is_none());
        assert!(state.platform.is_none());

        let device = Device {
            id: "d1".to_string(),
            name: "iPhone 15".to_string(),
            platform: "ios".to_string(),
            emulator: false,
            category: None,
            platform_type: None,
            ephemeral: false,
            emulator_id: None,
        };

        let session_id = state.session_manager.create_session(&device).unwrap();

        update(
            &mut state,
            Message::SessionStarted {
                session_id,
                device_id: "d1".into(),
                device_name: "iPhone 15".into(),
                platform: "ios".into(),
                pid: Some(12345),
            },
        );

        // Legacy global state should be updated for backward compatibility
        assert_eq!(state.device_name, Some("iPhone 15".to_string()));
        assert_eq!(state.platform, Some("ios".to_string()));
        assert_eq!(state.phase, AppPhase::Running);
        assert!(state.session_start.is_some());
    }

    #[test]
    fn test_session_started_with_unknown_session() {
        let mut state = AppState::new();

        // Try to start a session that doesn't exist
        update(
            &mut state,
            Message::SessionStarted {
                session_id: 999, // Non-existent
                device_id: "d1".into(),
                device_name: "iPhone 15".into(),
                platform: "ios".into(),
                pid: Some(12345),
            },
        );

        // Should still update global state (for legacy compatibility)
        assert_eq!(state.device_name, Some("iPhone 15".to_string()));
        assert_eq!(state.phase, AppPhase::Running);

        // But no session should exist
        assert!(state.session_manager.get(999).is_none());
    }

    // ─────────────────────────────────────────────────────────
    // Task 08: Quit Flow Tests
    // ─────────────────────────────────────────────────────────

    #[test]
    fn test_request_quit_no_sessions_quits_immediately() {
        let mut state = AppState::new();
        state.settings.behavior.confirm_quit = true;

        // No sessions
        assert!(state.session_manager.is_empty());

        update(&mut state, Message::RequestQuit);

        // Should quit immediately
        assert_eq!(state.phase, AppPhase::Quitting);
    }

    #[test]
    fn test_request_quit_with_running_sessions_shows_dialog() {
        let mut state = AppState::new();
        state.settings.behavior.confirm_quit = true;

        // Create a running session
        let device = Device {
            id: "d1".to_string(),
            name: "iPhone 15".to_string(),
            platform: "ios".to_string(),
            emulator: false,
            category: None,
            platform_type: None,
            ephemeral: false,
            emulator_id: None,
        };
        let id = state.session_manager.create_session(&device).unwrap();
        state
            .session_manager
            .get_mut(id)
            .unwrap()
            .session
            .mark_started("app-1".into());

        update(&mut state, Message::RequestQuit);

        // Should show dialog, not quit
        assert_ne!(state.phase, AppPhase::Quitting);
        assert_eq!(state.ui_mode, UiMode::ConfirmDialog);
    }

    #[test]
    fn test_request_quit_confirm_quit_disabled_quits_immediately() {
        let mut state = AppState::new();
        state.settings.behavior.confirm_quit = false;

        // Create a running session
        let device = Device {
            id: "d1".to_string(),
            name: "iPhone 15".to_string(),
            platform: "ios".to_string(),
            emulator: false,
            category: None,
            platform_type: None,
            ephemeral: false,
            emulator_id: None,
        };
        let id = state.session_manager.create_session(&device).unwrap();
        state
            .session_manager
            .get_mut(id)
            .unwrap()
            .session
            .mark_started("app-1".into());

        update(&mut state, Message::RequestQuit);

        // Should quit immediately despite running session
        assert_eq!(state.phase, AppPhase::Quitting);
    }

    #[test]
    fn test_confirm_quit_sets_quitting_phase() {
        let mut state = AppState::new();
        state.ui_mode = UiMode::ConfirmDialog;

        update(&mut state, Message::ConfirmQuit);

        assert_eq!(state.phase, AppPhase::Quitting);
    }

    #[test]
    fn test_cancel_quit_returns_to_normal() {
        let mut state = AppState::new();
        state.ui_mode = UiMode::ConfirmDialog;

        update(&mut state, Message::CancelQuit);

        assert_eq!(state.ui_mode, UiMode::Normal);
        assert_ne!(state.phase, AppPhase::Quitting);
    }

    #[test]
    fn test_y_key_in_confirm_dialog_confirms() {
        let mut state = AppState::new();
        state.ui_mode = UiMode::ConfirmDialog;

        let key = KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE);
        let result = handle_key(&state, key);

        assert!(matches!(result, Some(Message::ConfirmQuit)));
    }

    #[test]
    fn test_n_key_in_confirm_dialog_cancels() {
        let mut state = AppState::new();
        state.ui_mode = UiMode::ConfirmDialog;

        let key = KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE);
        let result = handle_key(&state, key);

        assert!(matches!(result, Some(Message::CancelQuit)));
    }

    #[test]
    fn test_esc_in_confirm_dialog_cancels() {
        let mut state = AppState::new();
        state.ui_mode = UiMode::ConfirmDialog;

        let key = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
        let result = handle_key(&state, key);

        assert!(matches!(result, Some(Message::CancelQuit)));
    }

    #[test]
    fn test_ctrl_c_in_confirm_dialog_force_quits() {
        let mut state = AppState::new();
        state.ui_mode = UiMode::ConfirmDialog;

        let key = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        let result = handle_key(&state, key);

        // Ctrl+C should still force quit even in dialog
        assert!(matches!(result, Some(Message::Quit)));
    }

    // ─────────────────────────────────────────────────────────
    // Task 11: Device Selector Animation Tests
    // ─────────────────────────────────────────────────────────

    #[test]
    fn test_tick_advances_device_selector_animation() {
        let mut state = AppState::new();
        state.device_selector.show_loading();

        // Initial frame
        let initial_frame = state.device_selector.animation_frame;

        // Tick should advance the animation
        update(&mut state, Message::Tick);
        assert_eq!(state.device_selector.animation_frame, initial_frame + 1);

        // Multiple ticks should continue advancing
        update(&mut state, Message::Tick);
        update(&mut state, Message::Tick);
        assert_eq!(state.device_selector.animation_frame, initial_frame + 3);
    }

    #[test]
    fn test_tick_does_not_advance_when_not_loading() {
        let mut state = AppState::new();

        // Device selector visible but not loading
        state.device_selector.show();
        let initial_frame = state.device_selector.animation_frame;

        update(&mut state, Message::Tick);

        // Frame should NOT advance when not loading
        assert_eq!(state.device_selector.animation_frame, initial_frame);
    }

    #[test]
    fn test_tick_does_not_advance_when_hidden() {
        let mut state = AppState::new();

        // Device selector hidden
        state.device_selector.hide();
        state.device_selector.loading = true; // Loading but hidden
        let initial_frame = state.device_selector.animation_frame;

        update(&mut state, Message::Tick);

        // Frame should NOT advance when hidden
        assert_eq!(state.device_selector.animation_frame, initial_frame);
    }

    #[test]
    fn test_tick_advances_when_refreshing() {
        let mut state = AppState::new();

        // Set up cache first
        state.device_selector.set_devices(vec![Device {
            id: "d1".to_string(),
            name: "Device 1".to_string(),
            platform: "ios".to_string(),
            emulator: false,
            category: None,
            platform_type: None,
            ephemeral: false,
            emulator_id: None,
        }]);

        // Show refreshing mode
        state.device_selector.show_refreshing();

        assert!(state.device_selector.refreshing);
        assert!(!state.device_selector.loading);

        let initial_frame = state.device_selector.animation_frame;

        update(&mut state, Message::Tick);

        // Frame SHOULD advance when refreshing
        assert_eq!(state.device_selector.animation_frame, initial_frame + 1);
    }

    #[test]
    fn test_show_device_selector_uses_cache() {
        let mut state = AppState::new();

        // First show - no cache
        assert!(!state.device_selector.has_cache());

        let result = update(&mut state, Message::ShowDeviceSelector);

        // Should be in loading mode
        assert!(state.device_selector.loading);
        assert!(!state.device_selector.refreshing);
        assert!(matches!(result.action, Some(UpdateAction::DiscoverDevices)));

        // Simulate discovery completing
        let devices = vec![Device {
            id: "d1".to_string(),
            name: "Device 1".to_string(),
            platform: "ios".to_string(),
            emulator: false,
            category: None,
            platform_type: None,
            ephemeral: false,
            emulator_id: None,
        }];
        state.device_selector.set_devices(devices);

        // Hide and show again
        state.device_selector.hide();
        state.ui_mode = UiMode::Normal;

        let result = update(&mut state, Message::ShowDeviceSelector);

        // Should be in refreshing mode (using cache)
        assert!(!state.device_selector.loading);
        assert!(state.device_selector.refreshing);
        assert_eq!(state.device_selector.devices.len(), 1);
        assert!(matches!(result.action, Some(UpdateAction::DiscoverDevices)));
    }
}
