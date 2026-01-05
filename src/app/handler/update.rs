//! Main update function - handles state transitions (TEA pattern)

use crate::app::message::Message;
use crate::app::state::{AppState, UiMode};
use crate::core::{AppPhase, LogSource};
use crate::tui::editor::{open_in_editor, sanitize_path};

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
            state.device_selector.set_devices(devices);

            // If we were in Loading mode, transition to DeviceSelector
            if state.ui_mode == UiMode::Loading {
                state.ui_mode = UiMode::DeviceSelector;
            }

            if device_count > 0 {
                tracing::info!("Discovered {} device(s)", device_count);
            } else {
                tracing::info!("No devices found");
            }

            UpdateResult::none()
        }

        Message::DeviceDiscoveryFailed { error } => {
            state.device_selector.set_error(error.clone());

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
            // Check for unsaved changes (future enhancement: show confirmation)
            if state.settings_view_state.dirty {
                // Could show confirmation dialog here
            }
            state.hide_settings();
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
            // Toggle edit mode - actual editing logic will be in widget
            if state.settings_view_state.editing {
                state.settings_view_state.stop_editing();
            } else {
                // Start editing with empty buffer for now
                state.settings_view_state.start_editing("");
            }
            UpdateResult::none()
        }

        Message::SettingsSave => {
            // Save settings - actual save logic will be implemented when widget is ready
            state.settings_view_state.clear_dirty();
            UpdateResult::none()
        }

        Message::SettingsResetItem => {
            // Reset setting to default - actual logic will be implemented with widget
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
