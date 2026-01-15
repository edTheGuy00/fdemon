//! Main update function - handles state transitions (TEA pattern)

use crate::app::message::{AutoLaunchSuccess, Message};
use crate::app::state::{AppState, UiMode};
use crate::core::{AppPhase, LogSource};
use crate::tui::editor::{open_in_editor, sanitize_path};
use tracing::warn;

use super::{
    daemon::handle_session_daemon_event, keys::handle_key, Task, UpdateAction, UpdateResult,
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

        Message::SessionDaemon { session_id, event } => {
            handle_session_daemon_event(state, session_id, event);
            UpdateResult::none()
        }

        Message::ScrollUp => {
            if let Some(handle) = state.session_manager.selected_mut() {
                handle.session.log_view_state.scroll_up(1);
            }
            rescan_links_if_active(state);
            UpdateResult::none()
        }

        Message::ScrollDown => {
            if let Some(handle) = state.session_manager.selected_mut() {
                handle.session.log_view_state.scroll_down(1);
            }
            rescan_links_if_active(state);
            UpdateResult::none()
        }

        Message::ScrollToTop => {
            if let Some(handle) = state.session_manager.selected_mut() {
                handle.session.log_view_state.scroll_to_top();
            }
            rescan_links_if_active(state);
            UpdateResult::none()
        }

        Message::ScrollToBottom => {
            if let Some(handle) = state.session_manager.selected_mut() {
                handle.session.log_view_state.scroll_to_bottom();
            }
            rescan_links_if_active(state);
            UpdateResult::none()
        }

        Message::PageUp => {
            if let Some(handle) = state.session_manager.selected_mut() {
                handle.session.log_view_state.page_up();
            }
            rescan_links_if_active(state);
            UpdateResult::none()
        }

        Message::PageDown => {
            if let Some(handle) = state.session_manager.selected_mut() {
                handle.session.log_view_state.page_down();
            }
            rescan_links_if_active(state);
            UpdateResult::none()
        }

        // ─────────────────────────────────────────────────────────
        // Horizontal Scroll Messages (Phase 2 Task 12)
        // ─────────────────────────────────────────────────────────
        Message::ScrollLeft(n) => {
            if let Some(handle) = state.session_manager.selected_mut() {
                handle.session.log_view_state.scroll_left(n);
            }
            UpdateResult::none()
        }

        Message::ScrollRight(n) => {
            if let Some(handle) = state.session_manager.selected_mut() {
                handle.session.log_view_state.scroll_right(n);
            }
            UpdateResult::none()
        }

        Message::ScrollToLineStart => {
            if let Some(handle) = state.session_manager.selected_mut() {
                handle.session.log_view_state.scroll_to_line_start();
            }
            UpdateResult::none()
        }

        Message::ScrollToLineEnd => {
            if let Some(handle) = state.session_manager.selected_mut() {
                handle.session.log_view_state.scroll_to_line_end();
            }
            UpdateResult::none()
        }

        Message::Tick => {
            // Advance device selector animation when visible and loading or refreshing
            if state.device_selector.visible
                && (state.device_selector.loading || state.device_selector.refreshing)
            {
                state.device_selector.tick();
            }

            // Also tick startup dialog when visible and loading/refreshing
            if state.ui_mode == UiMode::StartupDialog
                && (state.startup_dialog_state.loading || state.startup_dialog_state.refreshing)
            {
                state.startup_dialog_state.tick();
            }

            // Tick loading screen animation with message cycling (Task 08d)
            if state.ui_mode == UiMode::Loading && state.loading_state.is_some() {
                state.tick_loading_animation_with_cycling(true);
            }

            // Task 10c: Check if startup dialog needs to save (debounced)
            if state.ui_mode == UiMode::StartupDialog && state.startup_dialog_state.should_save() {
                return UpdateResult::message(Message::SaveStartupDialogConfig);
            }

            UpdateResult::none()
        }

        // ─────────────────────────────────────────────────────────
        // Control Messages
        // ─────────────────────────────────────────────────────────
        Message::HotReload => {
            // Try to get session info from selected session
            if let Some(handle) = state.session_manager.selected_mut() {
                // Check if THIS session is busy
                if handle.session.is_busy() {
                    return UpdateResult::none();
                }
                if let Some(app_id) = handle.session.app_id.clone() {
                    if handle.cmd_sender.is_some() {
                        let session_id = handle.session.id;
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
                // No app running - log error to session
                handle.session.add_log(crate::core::LogEntry::error(
                    LogSource::App,
                    "No app running to reload".to_string(),
                ));
            }
            UpdateResult::none()
        }

        Message::HotRestart => {
            // Try to get session info from selected session
            if let Some(handle) = state.session_manager.selected_mut() {
                // Check if THIS session is busy
                if handle.session.is_busy() {
                    return UpdateResult::none();
                }
                if let Some(app_id) = handle.session.app_id.clone() {
                    if handle.cmd_sender.is_some() {
                        let session_id = handle.session.id;
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
                // No app running - log error to session
                handle.session.add_log(crate::core::LogEntry::error(
                    LogSource::App,
                    "No app running to restart".to_string(),
                ));
            }
            UpdateResult::none()
        }

        Message::StopApp => {
            // Try to get session info from selected session
            if let Some(handle) = state.session_manager.selected_mut() {
                // Check if THIS session is busy
                if handle.session.is_busy() {
                    return UpdateResult::none();
                }
                if let Some(app_id) = handle.session.app_id.clone() {
                    if handle.cmd_sender.is_some() {
                        let session_id = handle.session.id;
                        handle.session.add_log(crate::core::LogEntry::info(
                            LogSource::App,
                            "Stopping app...".to_string(),
                        ));
                        return UpdateResult::action(UpdateAction::SpawnTask(Task::Stop {
                            session_id,
                            app_id,
                        }));
                    }
                }
                // No app running - log error to session
                handle.session.add_log(crate::core::LogEntry::error(
                    LogSource::App,
                    "No app running to stop".to_string(),
                ));
            }
            UpdateResult::none()
        }

        // ─────────────────────────────────────────────────────────
        // Session Reload/Restart Completion (multi-session mode)
        // ─────────────────────────────────────────────────────────
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
                let count = reloadable.len();

                // Mark all reloadable sessions as reloading and log to each
                for (session_id, _) in &reloadable {
                    if let Some(handle) = state.session_manager.get_mut(*session_id) {
                        handle.session.start_reload();
                        handle.session.add_log(crate::core::LogEntry::info(
                            LogSource::Watcher,
                            "File change detected, reloading...".to_string(),
                        ));
                    }
                }

                tracing::info!("Auto-reload triggered for {} session(s)", count);

                return UpdateResult::action(UpdateAction::ReloadAllSessions {
                    sessions: reloadable,
                });
            }

            // No running sessions to reload
            tracing::debug!("Auto-reload skipped: no running sessions");
            UpdateResult::none()
        }

        Message::FilesChanged { count } => {
            tracing::debug!("{} file(s) changed", count);
            UpdateResult::none()
        }

        Message::WatcherError { message } => {
            tracing::error!("Watcher error: {}", message);
            UpdateResult::none()
        }

        // ─────────────────────────────────────────────────────────
        // Device Selector Messages
        // ─────────────────────────────────────────────────────────
        Message::ShowDeviceSelector => {
            state.ui_mode = UiMode::DeviceSelector;

            // Use global cache if available for instant display (Task 08e)
            let cached_devices = state.get_cached_devices().cloned();
            if let Some(cached) = cached_devices {
                // Manually set devices and refreshing state to avoid clearing refreshing flag
                let cached_len = cached.len();
                state.device_selector.devices = cached;
                state.device_selector.visible = true;
                state.device_selector.loading = false;
                state.device_selector.refreshing = true;
                state.device_selector.error = None;
                state.device_selector.animation_frame = 0;
                if state.device_selector.selected_index >= cached_len {
                    state.device_selector.selected_index = 0;
                }
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
                tracing::warn!("Device '{}' already has an active session", device.name);
                // Stay in device selector to pick another device
                return UpdateResult::none();
            }

            // Create session in manager FIRST
            match state.session_manager.create_session(&device) {
                Ok(session_id) => {
                    tracing::info!(
                        "Session created for {} (id: {}, device: {})",
                        device.name,
                        session_id,
                        device.id
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
                    tracing::error!("Failed to create session: {}", e);
                    UpdateResult::none()
                }
            }
        }

        Message::LaunchAndroidEmulator => {
            tracing::info!("Discovering Android emulators...");
            state.ui_mode = UiMode::EmulatorSelector;
            UpdateResult::action(UpdateAction::DiscoverEmulators)
        }

        Message::LaunchIOSSimulator => {
            tracing::info!("Launching iOS Simulator...");
            UpdateResult::action(UpdateAction::LaunchIOSSimulator)
        }

        Message::DevicesDiscovered { devices } => {
            let device_count = devices.len();

            // Update global cache FIRST (Task 08e)
            state.set_device_cache(devices.clone());

            // Update device_selector (for add-session use case)
            state.device_selector.set_devices(devices.clone());

            // ALSO update startup_dialog_state (for initial startup)
            if state.ui_mode == UiMode::StartupDialog {
                state.startup_dialog_state.set_devices(devices);
            }

            // Note: Don't transition UI mode here - the caller handles that
            // (e.g., ShowDeviceSelector sets DeviceSelector mode, AutoLaunch stays in Loading)

            if device_count > 0 {
                tracing::info!("Discovered {} device(s)", device_count);
            } else {
                tracing::info!("No devices found");
            }

            UpdateResult::none()
        }

        Message::DeviceDiscoveryFailed { error } => {
            // Update device_selector
            state.device_selector.set_error(error.clone());

            // ALSO update startup_dialog_state
            if state.ui_mode == UiMode::StartupDialog {
                state.startup_dialog_state.set_error(error.clone());
            }

            // If we were in Loading mode, transition to DeviceSelector to show error
            if state.ui_mode == UiMode::Loading {
                state.ui_mode = UiMode::DeviceSelector;
            }

            tracing::error!("Device discovery failed: {}", error);
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
            tracing::info!("Discovering emulators...");
            UpdateResult::action(UpdateAction::DiscoverEmulators)
        }

        Message::EmulatorsDiscovered { emulators } => {
            let count = emulators.len();
            if count > 0 {
                tracing::info!("Found {} emulator(s)", count);
                // TODO: Task 09 - Show emulator selector UI with the emulators
            } else {
                tracing::info!("No emulators available");
            }
            // For now, go back to device selector - emulator selector UI is Task 09
            state.ui_mode = UiMode::DeviceSelector;
            UpdateResult::none()
        }

        Message::EmulatorDiscoveryFailed { error } => {
            tracing::error!("Emulator discovery failed: {}", error);
            // Go back to device selector on failure
            state.ui_mode = UiMode::DeviceSelector;
            UpdateResult::none()
        }

        Message::LaunchEmulator { emulator_id } => {
            tracing::info!("Launching emulator: {}", emulator_id);
            UpdateResult::action(UpdateAction::LaunchEmulator { emulator_id })
        }

        Message::EmulatorLaunched { result } => {
            if result.success {
                tracing::info!(
                    "Emulator '{}' launched successfully ({:?})",
                    result.emulator_id,
                    result.elapsed
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
                tracing::error!(
                    "Failed to launch emulator '{}': {}",
                    result.emulator_id,
                    error_msg
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
            platform: _,
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
                        "Flutter process started on {} (PID: {})",
                        device_name,
                        pid.map_or("unknown".to_string(), |p| p.to_string())
                    ),
                );
            }

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

            tracing::error!("Failed to start session {}: {}", session_id, error);

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
                tracing::debug!("Command sender attached to session {}", session_id);
            } else {
                tracing::error!("Cannot attach cmd_sender: session {} not found", session_id);
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
                    tracing::info!(
                        "Closing session {} (app: {})...",
                        current_session_id,
                        app_id
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
            }
            // No fallback needed - only clear logs if a session is selected
            UpdateResult::none()
        }

        // ─────────────────────────────────────────────────────────
        // Log Filter Messages (Phase 1 - Task 4)
        // ─────────────────────────────────────────────────────────
        Message::CycleLevelFilter => {
            if let Some(handle) = state.session_manager.selected_mut() {
                handle.session.cycle_level_filter();
            }
            UpdateResult::none()
        }

        Message::CycleSourceFilter => {
            if let Some(handle) = state.session_manager.selected_mut() {
                handle.session.cycle_source_filter();
            }
            UpdateResult::none()
        }

        Message::ResetFilters => {
            if let Some(handle) = state.session_manager.selected_mut() {
                handle.session.reset_filters();
            }
            UpdateResult::none()
        }

        // ─────────────────────────────────────────────────────────
        // Log Search Messages (Phase 1 - Tasks 5-6)
        // ─────────────────────────────────────────────────────────
        Message::StartSearch => {
            if let Some(handle) = state.session_manager.selected_mut() {
                handle.session.start_search();
            }
            state.ui_mode = UiMode::SearchInput;
            UpdateResult::none()
        }

        Message::CancelSearch => {
            if let Some(handle) = state.session_manager.selected_mut() {
                handle.session.cancel_search();
            }
            state.ui_mode = UiMode::Normal;
            UpdateResult::none()
        }

        Message::ClearSearch => {
            if let Some(handle) = state.session_manager.selected_mut() {
                handle.session.clear_search();
            }
            state.ui_mode = UiMode::Normal;
            UpdateResult::none()
        }

        Message::SearchInput { text } => {
            if let Some(handle) = state.session_manager.selected_mut() {
                handle.session.set_search_query(&text);

                // Execute search immediately
                handle
                    .session
                    .search_state
                    .execute_search(&handle.session.logs);

                // Scroll to first match if found
                if let Some(entry_index) = handle.session.search_state.current_match_entry_index() {
                    scroll_to_log_entry(&mut handle.session, entry_index);
                }
            }
            UpdateResult::none()
        }

        Message::NextSearchMatch => {
            if let Some(handle) = state.session_manager.selected_mut() {
                handle.session.search_state.next_match();

                // Scroll to new current match
                if let Some(entry_index) = handle.session.search_state.current_match_entry_index() {
                    scroll_to_log_entry(&mut handle.session, entry_index);
                }
            }
            UpdateResult::none()
        }

        Message::PrevSearchMatch => {
            if let Some(handle) = state.session_manager.selected_mut() {
                handle.session.search_state.prev_match();

                // Scroll to new current match
                if let Some(entry_index) = handle.session.search_state.current_match_entry_index() {
                    scroll_to_log_entry(&mut handle.session, entry_index);
                }
            }
            UpdateResult::none()
        }

        Message::SearchCompleted { matches } => {
            if let Some(handle) = state.session_manager.selected_mut() {
                handle.session.search_state.update_matches(matches);
            }
            UpdateResult::none()
        }

        // ─────────────────────────────────────────────────────────
        // Error Navigation Messages (Phase 1)
        // ─────────────────────────────────────────────────────────
        Message::NextError => {
            if let Some(handle) = state.session_manager.selected_mut() {
                if let Some(error_idx) = handle.session.find_next_error() {
                    scroll_to_log_entry(&mut handle.session, error_idx);
                }
            }
            UpdateResult::none()
        }

        Message::PrevError => {
            if let Some(handle) = state.session_manager.selected_mut() {
                if let Some(error_idx) = handle.session.find_prev_error() {
                    scroll_to_log_entry(&mut handle.session, error_idx);
                }
            }
            UpdateResult::none()
        }

        // ─────────────────────────────────────────────────────────
        // Stack Trace Collapse Messages (Phase 2 Task 6)
        // ─────────────────────────────────────────────────────────
        Message::ToggleStackTrace => {
            if let Some(handle) = state.session_manager.selected_mut() {
                if let Some(entry_id) = handle.session.focused_entry_id() {
                    let default_collapsed = state.settings.ui.stack_trace_collapsed;
                    handle
                        .session
                        .toggle_stack_trace(entry_id, default_collapsed);
                }
            }
            UpdateResult::none()
        }

        // ─────────────────────────────────────────────────────────
        // Link Highlight Mode (Phase 3.1)
        // ─────────────────────────────────────────────────────────
        Message::EnterLinkMode => {
            if let Some(handle) = state.session_manager.selected_mut() {
                // Get visible range from log view state
                let (visible_start, visible_end) = handle.session.log_view_state.visible_range();

                // Scan viewport for links
                handle.session.link_highlight_state.scan_viewport(
                    &handle.session.logs,
                    visible_start,
                    visible_end,
                    Some(&handle.session.filter_state),
                    &handle.session.collapse_state,
                    state.settings.ui.stack_trace_collapsed,
                    state.settings.ui.stack_trace_max_frames,
                );

                // Only enter link mode if there are links to show
                if handle.session.link_highlight_state.has_links() {
                    handle.session.link_highlight_state.activate();
                    state.ui_mode = UiMode::LinkHighlight;
                    tracing::debug!(
                        "Entered link mode with {} links",
                        handle.session.link_highlight_state.link_count()
                    );
                } else {
                    tracing::debug!("No links found in viewport");
                }
            }
            UpdateResult::none()
        }

        Message::ExitLinkMode => {
            if let Some(handle) = state.session_manager.selected_mut() {
                handle.session.link_highlight_state.deactivate();
            }
            state.ui_mode = UiMode::Normal;
            tracing::debug!("Exited link mode");
            UpdateResult::none()
        }

        Message::SelectLink(shortcut) => {
            // Find the link by shortcut before exiting link mode
            let file_ref = if let Some(handle) = state.session_manager.selected_mut() {
                handle
                    .session
                    .link_highlight_state
                    .link_by_shortcut(shortcut)
                    .map(|link| link.file_ref.clone())
            } else {
                None
            };

            // Exit link mode
            if let Some(handle) = state.session_manager.selected_mut() {
                handle.session.link_highlight_state.deactivate();
            }
            state.ui_mode = UiMode::Normal;

            // Open the file if we found a matching link
            if let Some(file_ref) = file_ref {
                // Sanitize path
                if sanitize_path(&file_ref.path).is_none() {
                    tracing::warn!("Rejected suspicious file path: {}", file_ref.path);
                    return UpdateResult::none();
                }

                // Open in editor
                match open_in_editor(&file_ref, &state.settings.editor, &state.project_path) {
                    Ok(result) => {
                        if result.used_parent_ide {
                            tracing::info!(
                                "Opened {}:{} in {} (parent IDE)",
                                result.file,
                                result.line,
                                result.editor_display_name
                            );
                        } else {
                            tracing::info!(
                                "Opened {}:{} in {}",
                                result.file,
                                result.line,
                                result.editor_display_name
                            );
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Failed to open file: {}", e);
                    }
                }
            } else {
                tracing::debug!("No link found for shortcut '{}'", shortcut);
            }

            UpdateResult::none()
        }

        // ─────────────────────────────────────────────────────────
        // Settings Messages (Phase 4)
        // ─────────────────────────────────────────────────────────
        Message::ShowSettings => {
            state.show_settings();
            UpdateResult::none()
        }

        Message::HideSettings => {
            // Check for unsaved changes - show confirmation dialog if dirty
            if state.settings_view_state.dirty {
                use crate::tui::widgets::ConfirmDialogState;
                state.confirm_dialog_state = Some(ConfirmDialogState::new(
                    "Unsaved Changes",
                    "You have unsaved changes. What do you want to do?",
                    vec![
                        ("Save & Close", Message::SettingsSaveAndClose),
                        ("Discard Changes", Message::ForceHideSettings),
                        ("Cancel", Message::CancelQuit),
                    ],
                ));
                state.ui_mode = crate::app::state::UiMode::ConfirmDialog;
            } else {
                state.hide_settings();
            }
            UpdateResult::none()
        }

        Message::SettingsNextTab => {
            state.settings_view_state.next_tab();
            UpdateResult::none()
        }

        Message::SettingsPrevTab => {
            state.settings_view_state.prev_tab();
            UpdateResult::none()
        }

        Message::SettingsGotoTab(idx) => {
            use crate::config::SettingsTab;
            if let Some(tab) = SettingsTab::from_index(idx) {
                state.settings_view_state.goto_tab(tab);
            }
            UpdateResult::none()
        }

        Message::SettingsNextItem => {
            let item_count =
                get_item_count_for_tab(&state.settings, state.settings_view_state.active_tab);
            state.settings_view_state.select_next(item_count);
            UpdateResult::none()
        }

        Message::SettingsPrevItem => {
            let item_count =
                get_item_count_for_tab(&state.settings, state.settings_view_state.active_tab);
            state.settings_view_state.select_previous(item_count);
            UpdateResult::none()
        }

        Message::SettingsToggleEdit => {
            // Toggle edit mode
            if state.settings_view_state.editing {
                state.settings_view_state.stop_editing();
            } else {
                // Get the current item and start editing with its value
                use crate::config::SettingValue;
                use crate::tui::widgets::SettingsPanel;

                let panel = SettingsPanel::new(&state.settings, &state.project_path);
                if let Some(item) = panel.get_selected_item(&state.settings_view_state) {
                    // Start editing based on value type
                    match &item.value {
                        SettingValue::Bool(_) => {
                            // Bool toggles directly without edit mode
                            return update(state, Message::SettingsToggleBool);
                        }
                        SettingValue::Enum { .. } => {
                            // Enums cycle through options
                            return update(state, Message::SettingsCycleEnumNext);
                        }
                        SettingValue::Number(n) => {
                            state.settings_view_state.start_editing(&n.to_string());
                        }
                        SettingValue::Float(f) => {
                            state.settings_view_state.start_editing(&f.to_string());
                        }
                        SettingValue::String(s) => {
                            state.settings_view_state.start_editing(s);
                        }
                        SettingValue::List(_) => {
                            // List starts with empty buffer to add new item
                            state.settings_view_state.start_editing("");
                        }
                    }
                }
            }
            UpdateResult::none()
        }

        Message::SettingsSave => {
            use crate::config::{
                launch::save_launch_configs, save_settings, save_user_preferences, SettingsTab,
            };

            let result = match state.settings_view_state.active_tab {
                SettingsTab::Project => {
                    // Save project settings (config.toml)
                    save_settings(&state.project_path, &state.settings)
                }
                SettingsTab::UserPrefs => {
                    // Save user preferences (settings.local.toml)
                    save_user_preferences(
                        &state.project_path,
                        &state.settings_view_state.user_prefs,
                    )
                }
                SettingsTab::LaunchConfig => {
                    // Save launch configs (launch.toml)
                    use crate::config::launch::load_launch_configs;
                    let configs = load_launch_configs(&state.project_path);
                    let config_vec: Vec<_> = configs.iter().map(|r| r.config.clone()).collect();
                    save_launch_configs(&state.project_path, &config_vec)
                }
                SettingsTab::VSCodeConfig => {
                    // Read-only - nothing to save
                    Ok(())
                }
            };

            match result {
                Ok(()) => {
                    state.settings_view_state.clear_dirty();
                    state.settings_view_state.error = None;
                    tracing::info!("Settings saved successfully");
                }
                Err(e) => {
                    let error_msg = format!("Save failed: {}", e);
                    tracing::error!("{}", error_msg);
                    state.settings_view_state.error = Some(error_msg);
                }
            }

            UpdateResult::none()
        }

        Message::SettingsResetItem => {
            // Reset setting to default - actual logic will be implemented with widget
            UpdateResult::none()
        }

        // ─────────────────────────────────────────────────────────
        // Settings Editing Messages (Phase 4, Task 10)
        // ─────────────────────────────────────────────────────────
        Message::SettingsToggleBool => {
            use crate::config::{SettingValue, SettingsTab};
            use crate::tui::widgets::SettingsPanel;

            let panel = SettingsPanel::new(&state.settings, &state.project_path);
            if let Some(item) = panel.get_selected_item(&state.settings_view_state) {
                // Only toggle if it's a boolean value
                if let SettingValue::Bool(val) = &item.value {
                    // Create new item with flipped value
                    let new_value = SettingValue::Bool(!val);
                    let mut toggled_item = item.clone();
                    toggled_item.value = new_value;

                    // Apply based on active tab
                    match state.settings_view_state.active_tab {
                        SettingsTab::Project => {
                            super::settings::apply_project_setting(
                                &mut state.settings,
                                &toggled_item,
                            );
                            state.settings_view_state.mark_dirty();
                        }
                        SettingsTab::UserPrefs => {
                            super::settings::apply_user_preference(
                                &mut state.settings_view_state.user_prefs,
                                &toggled_item,
                            );
                            state.settings_view_state.mark_dirty();
                        }
                        SettingsTab::LaunchConfig => {
                            // For launch configs, we need to load, modify, and save
                            // Extract config index from item ID (format: "launch.{idx}.field")
                            let parts: Vec<&str> = toggled_item.id.split('.').collect();
                            if parts.len() >= 3 && parts[0] == "launch" {
                                if let Ok(config_idx) = parts[1].parse::<usize>() {
                                    use crate::config::launch::{
                                        load_launch_configs, save_launch_configs,
                                    };
                                    let mut configs = load_launch_configs(&state.project_path);
                                    if let Some(resolved) = configs.get_mut(config_idx) {
                                        super::settings::apply_launch_config_change(
                                            &mut resolved.config,
                                            &toggled_item,
                                        );
                                        // Save the modified configs back to disk
                                        let config_vec: Vec<_> =
                                            configs.iter().map(|r| r.config.clone()).collect();
                                        if let Err(e) =
                                            save_launch_configs(&state.project_path, &config_vec)
                                        {
                                            tracing::error!("Failed to save launch configs: {}", e);
                                        } else {
                                            state.settings_view_state.mark_dirty();
                                        }
                                    }
                                }
                            }
                        }
                        SettingsTab::VSCodeConfig => {
                            // Read-only tab - ignore toggle
                        }
                    }
                }
            }
            UpdateResult::none()
        }

        Message::SettingsCycleEnumNext => {
            // Cycle enum to next value
            state.settings_view_state.mark_dirty();
            UpdateResult::none()
        }

        Message::SettingsCycleEnumPrev => {
            // Cycle enum to previous value
            state.settings_view_state.mark_dirty();
            UpdateResult::none()
        }

        Message::SettingsIncrement(_delta) => {
            // Increment/decrement number value
            // For direct increment without edit mode
            // Actual implementation will be in Task 11 (persistence)
            if !state.settings_view_state.editing {
                state.settings_view_state.mark_dirty();
            }
            UpdateResult::none()
        }

        Message::SettingsCharInput(ch) => {
            // Add character to edit buffer
            if state.settings_view_state.editing {
                state.settings_view_state.edit_buffer.push(ch);
            }
            UpdateResult::none()
        }

        Message::SettingsBackspace => {
            // Remove last character from edit buffer
            if state.settings_view_state.editing {
                state.settings_view_state.edit_buffer.pop();
            }
            UpdateResult::none()
        }

        Message::SettingsClearBuffer => {
            // Clear entire edit buffer
            if state.settings_view_state.editing {
                state.settings_view_state.edit_buffer.clear();
            }
            UpdateResult::none()
        }

        Message::SettingsCommitEdit => {
            // Commit the current edit
            // Actual value update needs to happen here
            if state.settings_view_state.editing {
                state.settings_view_state.mark_dirty();
                state.settings_view_state.stop_editing();
            }
            UpdateResult::none()
        }

        Message::SettingsCancelEdit => {
            // Cancel the current edit
            state.settings_view_state.stop_editing();
            UpdateResult::none()
        }

        Message::SettingsRemoveListItem => {
            // Remove last item from list
            state.settings_view_state.mark_dirty();
            UpdateResult::none()
        }

        // ─────────────────────────────────────────────────────────
        // Settings Persistence Messages (Phase 4, Task 11)
        // ─────────────────────────────────────────────────────────
        Message::SettingsSaveAndClose => {
            // Save then close
            use crate::config::{
                launch::save_launch_configs, save_settings, save_user_preferences, SettingsTab,
            };

            let result = match state.settings_view_state.active_tab {
                SettingsTab::Project => save_settings(&state.project_path, &state.settings),
                SettingsTab::UserPrefs => save_user_preferences(
                    &state.project_path,
                    &state.settings_view_state.user_prefs,
                ),
                SettingsTab::LaunchConfig => {
                    use crate::config::launch::load_launch_configs;
                    let configs = load_launch_configs(&state.project_path);
                    let config_vec: Vec<_> = configs.iter().map(|r| r.config.clone()).collect();
                    save_launch_configs(&state.project_path, &config_vec)
                }
                SettingsTab::VSCodeConfig => Ok(()),
            };

            match result {
                Ok(()) => {
                    state.settings_view_state.clear_dirty();
                    state.settings_view_state.error = None;
                    state.hide_settings();
                    tracing::info!("Settings saved and closed");
                }
                Err(e) => {
                    let error_msg = format!("Save failed: {}", e);
                    tracing::error!("{}", error_msg);
                    state.settings_view_state.error = Some(error_msg);
                    // Don't close on error - stay in settings to show error
                }
            }

            UpdateResult::none()
        }

        Message::ForceHideSettings => {
            // Force close without saving (discard changes)
            state.settings_view_state.clear_dirty();
            state.hide_settings();
            UpdateResult::none()
        }

        // ─────────────────────────────────────────────────────────
        // Startup Dialog Messages (Phase 5)
        // ─────────────────────────────────────────────────────────
        // TODO: These are stub implementations for Task 02.
        // Full implementations will be added in later tasks.
        Message::ShowStartupDialog => {
            // Load all configs (launch.toml + launch.json)
            let configs = crate::config::load_all_configs(&state.project_path);

            // Show the dialog with configs (will use cache if available - Task 08e)
            state.show_startup_dialog(configs);

            // Trigger device discovery (background refresh if cache exists)
            UpdateResult::action(UpdateAction::DiscoverDevices)
        }

        Message::HideStartupDialog => {
            // Task 10d: Reset creating_new_config flag on cancel
            if state.startup_dialog_state.creating_new_config {
                state.startup_dialog_state.creating_new_config = false;
                state.startup_dialog_state.dirty = false;
            }
            state.hide_startup_dialog();
            UpdateResult::none()
        }

        Message::StartupDialogUp => {
            state.startup_dialog_state.navigate_up();
            UpdateResult::none()
        }

        Message::StartupDialogDown => {
            state.startup_dialog_state.navigate_down();
            UpdateResult::none()
        }

        Message::StartupDialogNextSection => {
            state.startup_dialog_state.next_section();
            UpdateResult::none()
        }

        Message::StartupDialogPrevSection => {
            state.startup_dialog_state.prev_section();
            UpdateResult::none()
        }

        // Task 10b: Skip disabled fields during Tab navigation
        Message::StartupDialogNextSectionSkipDisabled => {
            use crate::app::state::DialogSection;
            let mut next = state.startup_dialog_state.active_section.next();

            // Skip Flavor and DartDefines if they're disabled
            while matches!(next, DialogSection::Flavor | DialogSection::DartDefines) {
                next = next.next();
            }

            state.startup_dialog_state.editing = false;
            state.startup_dialog_state.active_section = next;
            UpdateResult::none()
        }

        Message::StartupDialogPrevSectionSkipDisabled => {
            use crate::app::state::DialogSection;
            let mut prev = state.startup_dialog_state.active_section.prev();

            // Skip Flavor and DartDefines if they're disabled
            while matches!(prev, DialogSection::Flavor | DialogSection::DartDefines) {
                prev = prev.prev();
            }

            state.startup_dialog_state.editing = false;
            state.startup_dialog_state.active_section = prev;
            UpdateResult::none()
        }

        Message::StartupDialogSelectConfig(idx) => {
            // Task 10b: Use on_config_selected to handle VSCode config field population
            state.startup_dialog_state.on_config_selected(Some(idx));
            UpdateResult::none()
        }

        Message::StartupDialogSelectDevice(idx) => {
            state.startup_dialog_state.selected_device = Some(idx);
            UpdateResult::none()
        }

        Message::StartupDialogSetMode(mode) => {
            state.startup_dialog_state.mode = mode;
            UpdateResult::none()
        }

        Message::StartupDialogCharInput(c) => {
            // Task 10b: Block input on disabled fields (VSCode configs)
            if state.startup_dialog_state.editing && state.startup_dialog_state.flavor_editable() {
                match state.startup_dialog_state.active_section {
                    crate::app::state::DialogSection::Flavor => {
                        state.startup_dialog_state.flavor.push(c);
                        // Task 10c: Mark dirty for auto-save
                        state.startup_dialog_state.mark_dirty();
                    }
                    crate::app::state::DialogSection::DartDefines => {
                        state.startup_dialog_state.dart_defines.push(c);
                        // Task 10c: Mark dirty for auto-save
                        state.startup_dialog_state.mark_dirty();
                    }
                    _ => {}
                }
            }
            UpdateResult::none()
        }

        Message::StartupDialogBackspace => {
            // Task 10b: Block backspace on disabled fields (VSCode configs)
            if state.startup_dialog_state.editing && state.startup_dialog_state.flavor_editable() {
                match state.startup_dialog_state.active_section {
                    crate::app::state::DialogSection::Flavor => {
                        state.startup_dialog_state.flavor.pop();
                        // Task 10c: Mark dirty for auto-save
                        state.startup_dialog_state.mark_dirty();
                    }
                    crate::app::state::DialogSection::DartDefines => {
                        state.startup_dialog_state.dart_defines.pop();
                        // Task 10c: Mark dirty for auto-save
                        state.startup_dialog_state.mark_dirty();
                    }
                    _ => {}
                }
            }
            UpdateResult::none()
        }

        Message::StartupDialogClearInput => {
            match state.startup_dialog_state.active_section {
                crate::app::state::DialogSection::Flavor => {
                    state.startup_dialog_state.flavor.clear();
                    // Task 10c: Mark dirty for auto-save
                    state.startup_dialog_state.mark_dirty();
                }
                crate::app::state::DialogSection::DartDefines => {
                    state.startup_dialog_state.dart_defines.clear();
                    // Task 10c: Mark dirty for auto-save
                    state.startup_dialog_state.mark_dirty();
                }
                _ => {}
            }
            UpdateResult::none()
        }

        Message::StartupDialogConfirm => handle_startup_dialog_confirm(state),

        Message::SaveStartupDialogConfig => {
            let dialog = &mut state.startup_dialog_state;

            // Task 10d: Handle new config creation (no config selected, user entered data)
            if dialog.creating_new_config && dialog.dirty {
                // Don't create empty config
                if dialog.flavor.is_empty() && dialog.dart_defines.is_empty() {
                    dialog.creating_new_config = false;
                    dialog.mark_saved();
                    return UpdateResult::none();
                }

                let project_path = state.project_path.clone();
                let new_config = crate::config::LaunchConfig {
                    name: dialog.new_config_name.clone(),
                    device: "auto".to_string(),
                    mode: dialog.mode,
                    flavor: if dialog.flavor.is_empty() {
                        None
                    } else {
                        Some(dialog.flavor.clone())
                    },
                    dart_defines: crate::config::parse_dart_defines(&dialog.dart_defines),
                    ..Default::default()
                };

                match crate::config::add_launch_config(&project_path, new_config) {
                    Ok(()) => {
                        tracing::info!("Created new config: {}", dialog.new_config_name);

                        // Reload configs to show the new one
                        let reloaded = crate::config::load_all_configs(&project_path);

                        // Find the actual name (may have been renamed due to collision)
                        let new_idx = reloaded
                            .configs
                            .iter()
                            .position(|c| c.config.name.starts_with(&dialog.new_config_name))
                            .or_else(|| {
                                // Fallback: find last config (most recently added)
                                if !reloaded.configs.is_empty() {
                                    Some(reloaded.configs.len() - 1)
                                } else {
                                    None
                                }
                            });

                        let actual_name = new_idx
                            .and_then(|idx| reloaded.configs.get(idx))
                            .map(|c| c.config.name.clone())
                            .unwrap_or_else(|| dialog.new_config_name.clone());

                        dialog.configs = reloaded;
                        dialog.selected_config = new_idx;
                        dialog.creating_new_config = false;
                        dialog.editing_config_name = Some(actual_name);
                        dialog.mark_saved();
                    }
                    Err(e) => {
                        warn!("Failed to create config: {}", e);
                        // Could show error to user in the future
                    }
                }
            }
            // Task 10c: Save FDemon config edits (flavor, dart_defines)
            else if let Some(ref config_name) = dialog.editing_config_name {
                if dialog.dirty {
                    let project_path = state.project_path.clone();
                    let name = config_name.clone();
                    let flavor = dialog.flavor.clone();
                    let dart_defines = dialog.dart_defines.clone();

                    // Save flavor
                    if let Err(e) = crate::config::update_launch_config_field(
                        &project_path,
                        &name,
                        "flavor",
                        &flavor,
                    ) {
                        warn!("Failed to save flavor: {}", e);
                    }

                    // Save dart_defines
                    if let Err(e) = crate::config::update_launch_config_dart_defines(
                        &project_path,
                        &name,
                        &dart_defines,
                    ) {
                        warn!("Failed to save dart_defines: {}", e);
                    }

                    dialog.mark_saved();
                }
            }
            UpdateResult::none()
        }

        Message::StartupDialogRefreshDevices => {
            // Mark as refreshing (shows loading indicator but keeps existing devices)
            state.startup_dialog_state.refreshing = true;

            // Trigger device discovery
            UpdateResult::action(UpdateAction::DiscoverDevices)
        }

        Message::StartupDialogJumpToSection(section) => {
            state.startup_dialog_state.jump_to_section(section);
            UpdateResult::none()
        }

        Message::StartupDialogEnterEdit => {
            // Task 10b: Prevent entering edit mode on disabled fields
            if state.startup_dialog_state.flavor_editable() {
                state.startup_dialog_state.enter_edit();
            }
            UpdateResult::none()
        }

        Message::StartupDialogExitEdit => {
            state.startup_dialog_state.exit_edit();
            UpdateResult::none()
        }

        // ─────────────────────────────────────────────────────────
        // Launch Config Editing Messages (Phase 5, Task 07)
        // ─────────────────────────────────────────────────────────
        Message::LaunchConfigCreate => {
            use crate::config::{add_launch_config, create_default_launch_config};

            let new_config = create_default_launch_config();
            match add_launch_config(&state.project_path, new_config) {
                Ok(()) => {
                    state.settings_view_state.mark_dirty();
                    state.settings_view_state.error = None;
                    tracing::info!("Created new launch configuration");
                }
                Err(e) => {
                    let error_msg = format!("Failed to create config: {}", e);
                    tracing::error!("{}", error_msg);
                    state.settings_view_state.error = Some(error_msg);
                }
            }
            UpdateResult::none()
        }

        Message::LaunchConfigDelete(idx) => {
            use crate::config::{delete_launch_config, load_launch_configs};

            // Get config name at index
            let configs = load_launch_configs(&state.project_path);
            if let Some(resolved) = configs.get(idx) {
                match delete_launch_config(&state.project_path, &resolved.config.name) {
                    Ok(()) => {
                        state.settings_view_state.mark_dirty();
                        state.settings_view_state.error = None;
                        tracing::info!("Deleted launch configuration: {}", resolved.config.name);
                    }
                    Err(e) => {
                        let error_msg = format!("Failed to delete: {}", e);
                        tracing::error!("{}", error_msg);
                        state.settings_view_state.error = Some(error_msg);
                    }
                }
            } else {
                state.settings_view_state.error =
                    Some(format!("Config at index {} not found", idx));
            }
            UpdateResult::none()
        }

        Message::LaunchConfigUpdate {
            config_idx,
            field,
            value,
        } => {
            use crate::config::{load_launch_configs, update_launch_config_field};

            let configs = load_launch_configs(&state.project_path);
            if let Some(resolved) = configs.get(config_idx) {
                match update_launch_config_field(
                    &state.project_path,
                    &resolved.config.name,
                    &field,
                    &value,
                ) {
                    Ok(()) => {
                        state.settings_view_state.mark_dirty();
                        state.settings_view_state.error = None;
                        tracing::info!(
                            "Updated config '{}' field '{}' to '{}'",
                            resolved.config.name,
                            field,
                            value
                        );
                    }
                    Err(e) => {
                        let error_msg = format!("Failed to update: {}", e);
                        tracing::error!("{}", error_msg);
                        state.settings_view_state.error = Some(error_msg);
                    }
                }
            } else {
                state.settings_view_state.error =
                    Some(format!("Config at index {} not found", config_idx));
            }
            UpdateResult::none()
        }

        // ─────────────────────────────────────────────────────────
        // Auto-Launch Messages (Startup Flow Consistency)
        // ─────────────────────────────────────────────────────────
        Message::StartAutoLaunch { configs } => {
            // Guard against concurrent auto-launch (already in loading mode)
            if state.ui_mode == UiMode::Loading {
                return UpdateResult::none();
            }

            // Show loading overlay on top of normal UI
            state.set_loading_phase("Starting...");
            UpdateResult::action(UpdateAction::DiscoverDevicesAndAutoLaunch { configs })
        }

        Message::AutoLaunchProgress { message } => {
            // Update loading overlay message
            state.update_loading_message(&message);
            UpdateResult::none()
        }

        Message::AutoLaunchResult { result } => {
            match result {
                Ok(success) => {
                    // Clear loading before transitioning to session
                    state.clear_loading();

                    // Create session and spawn
                    let AutoLaunchSuccess { device, config } = success;

                    let session_result = if let Some(cfg) = &config {
                        state
                            .session_manager
                            .create_session_with_config(&device, cfg.clone())
                    } else {
                        state.session_manager.create_session(&device)
                    };

                    match session_result {
                        Ok(session_id) => {
                            // Save selection for next time
                            let _ = crate::config::save_last_selection(
                                &state.project_path,
                                config.as_ref().map(|c| c.name.as_str()),
                                Some(&device.id),
                            );

                            UpdateResult::action(UpdateAction::SpawnSession {
                                session_id,
                                device,
                                config: config.map(Box::new),
                            })
                        }
                        Err(e) => {
                            // Clear loading before showing error dialog
                            state.clear_loading();

                            // Session creation failed (e.g., max sessions reached) - show startup dialog with error
                            let configs = crate::config::load_all_configs(&state.project_path);
                            state.show_startup_dialog(configs);
                            state
                                .startup_dialog_state
                                .set_error(format!("Cannot create session: {}", e));
                            UpdateResult::none()
                        }
                    }
                }
                Err(error_msg) => {
                    // Clear loading before showing error dialog
                    state.clear_loading();

                    // Device discovery failed, show startup dialog with error
                    let configs = crate::config::load_all_configs(&state.project_path);
                    state.show_startup_dialog(configs);
                    state.startup_dialog_state.set_error(error_msg);
                    UpdateResult::none()
                }
            }
        }

        // ─────────────────────────────────────────────────────────
        // NewSessionDialog Messages (Phase 5 - Target Selector)
        // ─────────────────────────────────────────────────────────
        Message::HideNewSessionDialog => {
            state.hide_new_session_dialog();
            UpdateResult::none()
        }

        Message::NewSessionDialogSwitchPane => {
            state.new_session_dialog_state.switch_pane();
            UpdateResult::none()
        }

        Message::NewSessionDialogSwitchTab(tab) => {
            // Check if we need to trigger discovery BEFORE switch_tab modifies state
            let needs_bootable_discovery = tab == crate::tui::widgets::TargetTab::Bootable
                && state.new_session_dialog_state.bootable_devices.is_empty()
                && !state.new_session_dialog_state.loading_bootable;

            state.new_session_dialog_state.switch_tab(tab);

            // Trigger bootable device discovery if switching to Bootable tab and not loaded
            if needs_bootable_discovery {
                state.new_session_dialog_state.loading_bootable = true;
                return UpdateResult::action(UpdateAction::DiscoverBootableDevices);
            }
            UpdateResult::none()
        }

        Message::NewSessionDialogToggleTab => {
            let new_tab = state.new_session_dialog_state.target_tab.toggle();
            update(state, Message::NewSessionDialogSwitchTab(new_tab))
        }

        Message::NewSessionDialogDeviceUp => {
            state.new_session_dialog_state.target_up();
            UpdateResult::none()
        }

        Message::NewSessionDialogDeviceDown => {
            state.new_session_dialog_state.target_down();
            UpdateResult::none()
        }

        Message::NewSessionDialogDeviceSelect => {
            use crate::tui::widgets::TargetTab;
            match state.new_session_dialog_state.target_tab {
                TargetTab::Connected => {
                    // Select device for launch - actual launch happens in Launch Context
                    // For now, just acknowledge the selection
                    if state
                        .new_session_dialog_state
                        .selected_connected_device()
                        .is_none()
                    {
                        warn!("Cannot select device: no device selected on Connected tab");
                    }
                    UpdateResult::none()
                }
                TargetTab::Bootable => {
                    // Boot the selected device
                    if let Some(device) = state.new_session_dialog_state.selected_bootable_device()
                    {
                        let device_id = device.id.clone();
                        let platform = device.platform.to_string();
                        return UpdateResult::action(UpdateAction::BootDevice {
                            device_id,
                            platform,
                        });
                    }
                    warn!("Cannot boot device: no device selected on Bootable tab");
                    UpdateResult::none()
                }
            }
        }

        Message::NewSessionDialogRefreshDevices => {
            use crate::tui::widgets::TargetTab;
            match state.new_session_dialog_state.target_tab {
                TargetTab::Connected => {
                    state.new_session_dialog_state.loading_connected = true;
                    UpdateResult::action(UpdateAction::DiscoverDevices)
                }
                TargetTab::Bootable => {
                    state.new_session_dialog_state.loading_bootable = true;
                    UpdateResult::action(UpdateAction::DiscoverBootableDevices)
                }
            }
        }

        Message::NewSessionDialogConnectedDevicesReceived(devices) => {
            state
                .new_session_dialog_state
                .set_connected_devices(devices);
            UpdateResult::none()
        }

        Message::NewSessionDialogBootableDevicesReceived {
            ios_simulators,
            android_avds,
        } => {
            // Convert to BootableDevice using BootCommand
            let mut bootable_devices = Vec::new();

            for sim in ios_simulators {
                let cmd = crate::daemon::BootCommand::IosSimulator(sim);
                bootable_devices.push(cmd.into());
            }

            for avd in android_avds {
                let cmd = crate::daemon::BootCommand::AndroidAvd(avd);
                bootable_devices.push(cmd.into());
            }

            state
                .new_session_dialog_state
                .set_bootable_devices(bootable_devices);
            UpdateResult::none()
        }

        Message::NewSessionDialogDeviceDiscoveryFailed {
            error,
            discovery_type,
        } => {
            use crate::app::message::DiscoveryType;

            // Only clear the loading flag for the type that failed
            match discovery_type {
                DiscoveryType::Connected => {
                    state.new_session_dialog_state.loading_connected = false;
                }
                DiscoveryType::Bootable => {
                    state.new_session_dialog_state.loading_bootable = false;
                }
            }
            state.new_session_dialog_state.set_error(error);
            UpdateResult::none()
        }

        Message::NewSessionDialogBootStarted { device_id } => {
            state
                .new_session_dialog_state
                .mark_device_booting(&device_id);
            UpdateResult::none()
        }

        Message::NewSessionDialogBootCompleted { .. } => {
            // Switch to Connected tab and trigger device refresh
            state.new_session_dialog_state.handle_device_booted();
            UpdateResult::action(UpdateAction::DiscoverDevices)
        }

        Message::NewSessionDialogBootFailed { device_id, error } => {
            state
                .new_session_dialog_state
                .set_error(format!("Failed to boot device {}: {}", device_id, error));
            UpdateResult::none()
        }

        // Deprecated - redirect to new message
        Message::NewSessionDialogDeviceBooted { device_id } => {
            update(state, Message::NewSessionDialogBootCompleted { device_id })
        }

        Message::ShowNewSessionDialog
        | Message::NewSessionDialogUp
        | Message::NewSessionDialogDown
        | Message::NewSessionDialogConfirm
        | Message::NewSessionDialogBootDevice { .. }
        | Message::NewSessionDialogSetConnectedDevices { .. }
        | Message::NewSessionDialogSetBootableDevices { .. }
        | Message::NewSessionDialogSetError { .. }
        | Message::NewSessionDialogClearError
        | Message::NewSessionDialogSelectConfig { .. }
        | Message::NewSessionDialogSetMode { .. }
        | Message::NewSessionDialogSetFlavor { .. }
        | Message::NewSessionDialogSetDartDefines { .. } => {
            // Handlers for these will be implemented in subsequent tasks
            UpdateResult::none()
        }

        // ─────────────────────────────────────────────────────────
        // NewSessionDialog - Fuzzy Modal Handlers
        // ─────────────────────────────────────────────────────────
        Message::NewSessionDialogOpenFuzzyModal { modal_type } => {
            // Prevent opening a modal when another is already open
            if state.new_session_dialog_state.has_modal_open() {
                warn!("Cannot open fuzzy modal while another modal is open");
                return UpdateResult::none();
            }

            let items = match modal_type {
                crate::tui::widgets::FuzzyModalType::Config => state
                    .new_session_dialog_state
                    .configs
                    .configs
                    .iter()
                    .map(|c| c.display_name.clone())
                    .collect(),
                crate::tui::widgets::FuzzyModalType::Flavor => {
                    // TODO: Get flavors from project analysis
                    // For now, use any existing flavor as suggestion
                    let mut flavors = Vec::new();
                    if !state.new_session_dialog_state.flavor.is_empty() {
                        flavors.push(state.new_session_dialog_state.flavor.clone());
                    }
                    flavors
                }
            };

            state
                .new_session_dialog_state
                .open_fuzzy_modal(modal_type, items);
            UpdateResult::none()
        }

        Message::NewSessionDialogCloseFuzzyModal => {
            state.new_session_dialog_state.close_fuzzy_modal();
            UpdateResult::none()
        }

        Message::NewSessionDialogFuzzyUp => {
            if let Some(ref mut modal) = state.new_session_dialog_state.fuzzy_modal {
                modal.navigate_up();
            }
            UpdateResult::none()
        }

        Message::NewSessionDialogFuzzyDown => {
            if let Some(ref mut modal) = state.new_session_dialog_state.fuzzy_modal {
                modal.navigate_down();
            }
            UpdateResult::none()
        }

        Message::NewSessionDialogFuzzyConfirm => {
            if let Some(ref modal) = state.new_session_dialog_state.fuzzy_modal {
                if let Some(value) = modal.selected_value() {
                    match modal.modal_type {
                        crate::tui::widgets::FuzzyModalType::Config => {
                            // Find config index by name
                            let idx = state
                                .new_session_dialog_state
                                .configs
                                .configs
                                .iter()
                                .position(|c| c.display_name == value);
                            state.new_session_dialog_state.select_config(idx);
                        }
                        crate::tui::widgets::FuzzyModalType::Flavor => {
                            state.new_session_dialog_state.flavor = value;
                        }
                    }
                }
            }
            state.new_session_dialog_state.close_fuzzy_modal();
            UpdateResult::none()
        }

        Message::NewSessionDialogFuzzyInput { c } => {
            if let Some(ref mut modal) = state.new_session_dialog_state.fuzzy_modal {
                modal.input_char(c);
            }
            UpdateResult::none()
        }

        Message::NewSessionDialogFuzzyBackspace => {
            if let Some(ref mut modal) = state.new_session_dialog_state.fuzzy_modal {
                modal.backspace();
            }
            UpdateResult::none()
        }

        Message::NewSessionDialogFuzzyClear => {
            if let Some(ref mut modal) = state.new_session_dialog_state.fuzzy_modal {
                modal.clear_query();
            }
            UpdateResult::none()
        }

        // ─────────────────────────────────────────────────────────
        // NewSessionDialog - Dart Defines Modal Handlers
        // ─────────────────────────────────────────────────────────
        Message::NewSessionDialogOpenDartDefinesModal => {
            // Copy current dart defines into modal state
            state.new_session_dialog_state.open_dart_defines_modal();
            UpdateResult::none()
        }

        Message::NewSessionDialogCloseDartDefinesModal => {
            // Save changes back to main state
            state.new_session_dialog_state.close_dart_defines_modal();
            UpdateResult::none()
        }

        Message::NewSessionDialogDartDefinesSwitchPane => {
            if let Some(ref mut modal) = state.new_session_dialog_state.dart_defines_modal {
                modal.switch_pane();
            }
            UpdateResult::none()
        }

        Message::NewSessionDialogDartDefinesUp => {
            if let Some(ref mut modal) = state.new_session_dialog_state.dart_defines_modal {
                use crate::tui::widgets::DartDefinesPane;
                if modal.active_pane == DartDefinesPane::List {
                    modal.navigate_up();
                }
            }
            UpdateResult::none()
        }

        Message::NewSessionDialogDartDefinesDown => {
            if let Some(ref mut modal) = state.new_session_dialog_state.dart_defines_modal {
                use crate::tui::widgets::DartDefinesPane;
                if modal.active_pane == DartDefinesPane::List {
                    modal.navigate_down();
                }
            }
            UpdateResult::none()
        }

        Message::NewSessionDialogDartDefinesConfirm => {
            if let Some(ref mut modal) = state.new_session_dialog_state.dart_defines_modal {
                use crate::tui::widgets::{DartDefinesEditField, DartDefinesPane};
                match modal.active_pane {
                    DartDefinesPane::List => {
                        // Load selected item into edit form
                        modal.load_selected_into_edit();
                    }
                    DartDefinesPane::Edit => {
                        // Activate current button or confirm field
                        match modal.edit_field {
                            DartDefinesEditField::Key | DartDefinesEditField::Value => {
                                // Move to next field
                                modal.next_field();
                            }
                            DartDefinesEditField::Save => {
                                if !modal.save_edit() {
                                    // Save failed (key is empty) - return focus to Key field
                                    modal.edit_field = DartDefinesEditField::Key;
                                }
                            }
                            DartDefinesEditField::Delete => {
                                modal.delete_selected();
                            }
                        }
                    }
                }
            }
            UpdateResult::none()
        }

        Message::NewSessionDialogDartDefinesNextField => {
            if let Some(ref mut modal) = state.new_session_dialog_state.dart_defines_modal {
                use crate::tui::widgets::DartDefinesPane;
                if modal.active_pane == DartDefinesPane::Edit {
                    modal.next_field();
                }
            }
            UpdateResult::none()
        }

        Message::NewSessionDialogDartDefinesInput { c } => {
            if let Some(ref mut modal) = state.new_session_dialog_state.dart_defines_modal {
                use crate::tui::widgets::DartDefinesPane;
                if modal.active_pane == DartDefinesPane::Edit {
                    modal.input_char(c);
                }
            }
            UpdateResult::none()
        }

        Message::NewSessionDialogDartDefinesBackspace => {
            if let Some(ref mut modal) = state.new_session_dialog_state.dart_defines_modal {
                use crate::tui::widgets::DartDefinesPane;
                if modal.active_pane == DartDefinesPane::Edit {
                    modal.backspace();
                }
            }
            UpdateResult::none()
        }

        Message::NewSessionDialogDartDefinesSave => {
            if let Some(ref mut modal) = state.new_session_dialog_state.dart_defines_modal {
                modal.save_edit();
            }
            UpdateResult::none()
        }

        Message::NewSessionDialogDartDefinesDelete => {
            if let Some(ref mut modal) = state.new_session_dialog_state.dart_defines_modal {
                modal.delete_selected();
            }
            UpdateResult::none()
        }

        // ─────────────────────────────────────────────────────────────
        // Tool Availability & Device Discovery Messages (Phase 4, Task 05)
        // ─────────────────────────────────────────────────────────────
        Message::ToolAvailabilityChecked { availability } => {
            state.tool_availability = availability;

            // Log availability for debugging
            tracing::info!(
                "Tool availability: xcrun_simctl={}, android_emulator={}",
                state.tool_availability.xcrun_simctl,
                state.tool_availability.android_emulator
            );

            UpdateResult::none()
        }

        Message::DiscoverBootableDevices => {
            // Trigger action to discover bootable devices
            UpdateResult::action(UpdateAction::DiscoverBootableDevices)
        }

        Message::BootableDevicesDiscovered {
            ios_simulators,
            android_avds,
        } => {
            // Store discovered bootable devices in the new session dialog
            // Convert to core::BootableDevice for unified handling using BootCommand
            let mut bootable_devices = Vec::new();

            // Convert iOS simulators via BootCommand
            for sim in ios_simulators {
                let cmd = crate::daemon::BootCommand::IosSimulator(sim);
                bootable_devices.push(cmd.into());
            }

            // Convert Android AVDs via BootCommand
            for avd in android_avds {
                let cmd = crate::daemon::BootCommand::AndroidAvd(avd);
                bootable_devices.push(cmd.into());
            }

            // Update new session dialog state
            if state.is_new_session_dialog_visible() {
                state
                    .new_session_dialog_state
                    .set_bootable_devices(bootable_devices);
            }

            UpdateResult::none()
        }

        Message::BootDevice {
            device_id,
            platform,
        } => {
            // Trigger action to boot the device
            UpdateResult::action(UpdateAction::BootDevice {
                device_id,
                platform,
            })
        }

        Message::DeviceBootCompleted { device_id } => {
            tracing::info!("Device boot completed: {}", device_id);

            // Trigger device discovery to refresh connected devices list
            UpdateResult::action(UpdateAction::DiscoverDevices)
        }

        Message::DeviceBootFailed { device_id, error } => {
            warn!("Device boot failed: {} - {}", device_id, error);

            // Show error in new session dialog if visible
            if state.is_new_session_dialog_visible() {
                state
                    .new_session_dialog_state
                    .set_error(format!("Failed to boot {}: {}", device_id, error));
            }

            UpdateResult::none()
        }
    }
}

/// Get the number of items in a settings tab
fn get_item_count_for_tab(
    _settings: &crate::config::Settings,
    tab: crate::config::SettingsTab,
) -> usize {
    use crate::config::SettingsTab;

    match tab {
        SettingsTab::Project => {
            // behavior (2) + watcher (4) + ui (6) + devtools (2) + editor (2) = 16
            16
        }
        SettingsTab::UserPrefs => {
            // editor (2) + theme (1) + last_device (1) + last_config (1) = 5
            5
        }
        SettingsTab::LaunchConfig => {
            // Dynamic based on loaded configs
            // For now, estimate
            10
        }
        SettingsTab::VSCodeConfig => {
            // Dynamic based on loaded configs
            5
        }
    }
}

/// Scroll the log view to show a specific log entry
fn scroll_to_log_entry(session: &mut crate::app::session::Session, entry_index: usize) {
    // Account for filtering if active
    let visible_index = if session.filter_state.is_active() {
        // Find the position in filtered list
        session
            .logs
            .iter()
            .enumerate()
            .filter(|(_, e)| session.filter_state.matches(e))
            .position(|(i, _)| i == entry_index)
    } else {
        Some(entry_index)
    };

    if let Some(idx) = visible_index {
        // Center the match in the view if possible
        let visible_lines = session.log_view_state.visible_lines;
        let center_offset = visible_lines / 2;
        session.log_view_state.offset = idx.saturating_sub(center_offset);
        session.log_view_state.auto_scroll = false;
    }
}

/// Re-scan links if in link highlight mode (called after scroll operations).
///
/// When the user scrolls while in link mode, the viewport changes and we need
/// to re-scan for file references to update the shortcut assignments.
fn rescan_links_if_active(state: &mut AppState) {
    if state.ui_mode != UiMode::LinkHighlight {
        return;
    }

    if let Some(handle) = state.session_manager.selected_mut() {
        let (visible_start, visible_end) = handle.session.log_view_state.visible_range();

        handle.session.link_highlight_state.scan_viewport(
            &handle.session.logs,
            visible_start,
            visible_end,
            Some(&handle.session.filter_state),
            &handle.session.collapse_state,
            state.settings.ui.stack_trace_collapsed,
            state.settings.ui.stack_trace_max_frames,
        );

        tracing::debug!(
            "Re-scanned links after scroll: {} links found",
            handle.session.link_highlight_state.link_count()
        );
    }
}

/// Handle startup dialog confirm (launch session with selected config and device)
fn handle_startup_dialog_confirm(state: &mut AppState) -> UpdateResult {
    // Task 10d: Save config before launching if dirty (user clicked launch before debounce)
    if state.startup_dialog_state.dirty {
        if state.startup_dialog_state.creating_new_config {
            // Save new config synchronously before launch
            if !state.startup_dialog_state.flavor.is_empty()
                || !state.startup_dialog_state.dart_defines.is_empty()
            {
                let project_path = state.project_path.clone();
                let dialog = &mut state.startup_dialog_state;

                let new_config = crate::config::LaunchConfig {
                    name: dialog.new_config_name.clone(),
                    device: "auto".to_string(),
                    mode: dialog.mode,
                    flavor: if dialog.flavor.is_empty() {
                        None
                    } else {
                        Some(dialog.flavor.clone())
                    },
                    dart_defines: crate::config::parse_dart_defines(&dialog.dart_defines),
                    ..Default::default()
                };

                if let Ok(()) = crate::config::add_launch_config(&project_path, new_config) {
                    tracing::info!(
                        "Created new config before launch: {}",
                        dialog.new_config_name
                    );
                    dialog.creating_new_config = false;
                }
            }
        } else if let Some(ref config_name) = state.startup_dialog_state.editing_config_name {
            // Save existing config edits before launch
            let project_path = state.project_path.clone();
            let name = config_name.clone();
            let flavor = state.startup_dialog_state.flavor.clone();
            let dart_defines = state.startup_dialog_state.dart_defines.clone();

            let _ =
                crate::config::update_launch_config_field(&project_path, &name, "flavor", &flavor);
            let _ = crate::config::update_launch_config_dart_defines(
                &project_path,
                &name,
                &dart_defines,
            );
        }
        state.startup_dialog_state.mark_saved();
    }

    let dialog = &state.startup_dialog_state;

    // Get selected device (required)
    let device = match dialog.selected_device() {
        Some(d) => d.clone(),
        None => {
            // No device selected - show error
            state.startup_dialog_state.error = Some("Please select a device".to_string());
            return UpdateResult::none();
        }
    };

    // Build config: start from selected config OR create ad-hoc if user entered values
    let config: Option<crate::config::LaunchConfig> = {
        // Check if user entered any custom values
        let has_custom_flavor = !dialog.flavor.is_empty();
        let has_custom_defines = !dialog.dart_defines.is_empty();
        let has_custom_mode = dialog.mode != crate::config::FlutterMode::Debug;

        if let Some(sourced) = dialog.selected_config() {
            // User selected a config - clone and override
            let mut cfg = sourced.config.clone();

            // Override mode
            cfg.mode = dialog.mode;

            // Override flavor if user entered one
            if has_custom_flavor {
                cfg.flavor = Some(dialog.flavor.clone());
            }

            // Override dart-defines if user entered any
            if has_custom_defines {
                cfg.dart_defines = crate::config::parse_dart_defines(&dialog.dart_defines);
            }

            Some(cfg)
        } else if has_custom_flavor || has_custom_defines || has_custom_mode {
            // No config selected but user entered custom values
            // Create an ad-hoc config with the entered values
            Some(crate::config::LaunchConfig {
                name: "Ad-hoc Launch".to_string(),
                device: device.id.clone(),
                mode: dialog.mode,
                flavor: if has_custom_flavor {
                    Some(dialog.flavor.clone())
                } else {
                    None
                },
                dart_defines: if has_custom_defines {
                    crate::config::parse_dart_defines(&dialog.dart_defines)
                } else {
                    std::collections::HashMap::new()
                },
                entry_point: None,
                extra_args: Vec::new(),
                auto_start: false,
            })
        } else {
            // No config, no custom values - bare run
            None
        }
    };

    // Save selection (only if a named config was selected)
    let _ = crate::config::save_last_selection(
        &state.project_path,
        dialog.selected_config().map(|c| c.config.name.as_str()),
        Some(&device.id),
    );

    // Create session
    let result = if let Some(ref cfg) = config {
        state
            .session_manager
            .create_session_with_config(&device, cfg.clone())
    } else {
        state.session_manager.create_session(&device)
    };

    match result {
        Ok(session_id) => {
            state.ui_mode = UiMode::Normal;
            UpdateResult::action(UpdateAction::SpawnSession {
                session_id,
                device,
                config: config.map(Box::new),
            })
        }
        Err(e) => {
            state.startup_dialog_state.error = Some(format!("Failed to create session: {}", e));
            UpdateResult::none()
        }
    }
}

// Tests have been moved to src/config/launch.rs where parse_dart_defines is now defined
