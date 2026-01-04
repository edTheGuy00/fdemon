//! Main update function - handles state transitions (TEA pattern)

use crate::app::message::Message;
use crate::app::state::{AppState, UiMode};
use crate::core::{AppPhase, LogSource};

use super::{
    daemon::{handle_daemon_event, handle_session_daemon_event},
    keys::handle_key,
    Task, UpdateAction, UpdateResult,
};

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
            // Try to get session info from selected session
            if let Some(handle) = state.session_manager.selected_mut() {
                // Check if THIS session is busy (not global state)
                if handle.session.is_busy() {
                    return UpdateResult::none();
                }
                if let Some(app_id) = handle.session.app_id.clone() {
                    if handle.cmd_sender.is_some() {
                        let session_id = handle.session.id;
                        // Mark the SESSION as reloading (not global state)
                        handle.session.start_reload();
                        handle.session.add_log(crate::core::LogEntry::info(
                            LogSource::App,
                            "Reloading...".to_string(),
                        ));
                        return UpdateResult::action(UpdateAction::SpawnTask(Task::Reload {
                            session_id,
                            app_id,
                        }));
                    }
                }
            }

            // Fall back to legacy global app_id (uses global state)
            if state.is_busy() {
                return UpdateResult::none();
            }
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
            // Try to get session info from selected session
            if let Some(handle) = state.session_manager.selected_mut() {
                // Check if THIS session is busy (not global state)
                if handle.session.is_busy() {
                    return UpdateResult::none();
                }
                if let Some(app_id) = handle.session.app_id.clone() {
                    if handle.cmd_sender.is_some() {
                        let session_id = handle.session.id;
                        // Mark the SESSION as reloading (not global state)
                        handle.session.start_reload();
                        handle.session.add_log(crate::core::LogEntry::info(
                            LogSource::App,
                            "Restarting...".to_string(),
                        ));
                        return UpdateResult::action(UpdateAction::SpawnTask(Task::Restart {
                            session_id,
                            app_id,
                        }));
                    }
                }
            }

            // Fall back to legacy global app_id (uses global state)
            if state.is_busy() {
                return UpdateResult::none();
            }
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

        // Session-specific reload completion (for multi-session auto-reload)
        Message::SessionReloadCompleted {
            session_id,
            time_ms,
        } => {
            if let Some(handle) = state.session_manager.get_mut(session_id) {
                handle.session.complete_reload();
                handle.session.add_log(crate::core::LogEntry::info(
                    LogSource::App,
                    format!("Reloaded in {}ms", time_ms),
                ));
            }
            UpdateResult::none()
        }

        Message::SessionReloadFailed { session_id, reason } => {
            if let Some(handle) = state.session_manager.get_mut(session_id) {
                handle.session.phase = AppPhase::Running;
                handle.session.reload_start_time = None;
                handle.session.add_log(crate::core::LogEntry::error(
                    LogSource::App,
                    format!("Reload failed: {}", reason),
                ));
            }
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

        // Session-specific restart completion (for multi-session mode)
        Message::SessionRestartCompleted { session_id } => {
            if let Some(handle) = state.session_manager.get_mut(session_id) {
                handle.session.complete_reload();
                handle.session.add_log(crate::core::LogEntry::info(
                    LogSource::App,
                    "Restarted".to_string(),
                ));
            }
            UpdateResult::none()
        }

        Message::SessionRestartFailed { session_id, reason } => {
            if let Some(handle) = state.session_manager.get_mut(session_id) {
                handle.session.phase = AppPhase::Running;
                handle.session.reload_start_time = None;
                handle.session.add_log(crate::core::LogEntry::error(
                    LogSource::App,
                    format!("Restart failed: {}", reason),
                ));
            }
            UpdateResult::none()
        }

        // ─────────────────────────────────────────────────────────
        // File Watcher Messages
        // ─────────────────────────────────────────────────────────
        Message::AutoReloadTriggered => {
            // Skip if any session is busy (to keep all devices in sync)
            if state.session_manager.any_session_busy() {
                tracing::debug!("Auto-reload skipped: some session(s) already reloading");
                return UpdateResult::none();
            }

            // Get all sessions that can be reloaded
            let reloadable = state.session_manager.reloadable_sessions();

            if !reloadable.is_empty() {
                // Mark all reloadable sessions as reloading
                for (session_id, _) in &reloadable {
                    if let Some(handle) = state.session_manager.get_mut(*session_id) {
                        handle.session.start_reload();
                    }
                }

                let count = reloadable.len();
                if count == 1 {
                    state.log_info(LogSource::Watcher, "File change detected, reloading...");
                } else {
                    state.log_info(
                        LogSource::Watcher,
                        format!("File change detected, reloading {} sessions...", count),
                    );
                }

                return UpdateResult::action(UpdateAction::ReloadAllSessions {
                    sessions: reloadable,
                });
            }

            // Fall back to legacy global app_id (for backward compatibility)
            if !state.is_busy() {
                if let Some(app_id) = state.current_app_id.clone() {
                    state.log_info(LogSource::Watcher, "File change detected, reloading...");
                    state.start_reload();
                    return UpdateResult::action(UpdateAction::SpawnTask(Task::Reload {
                        session_id: 0,
                        app_id,
                    }));
                }
            }

            // No running sessions
            tracing::debug!("Auto-reload skipped: no running sessions");
            UpdateResult::none()
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
