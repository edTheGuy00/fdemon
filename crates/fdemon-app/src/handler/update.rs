//! Main update function - handles state transitions (TEA pattern)
//!
//! Handler implementations have been extracted to:
//! - `new_session/`: NewSessionDialog handlers (Phase 6.1, Task 03)
//! - `session_lifecycle`: Session lifecycle handlers (Phase 6.1, Task 04)
//! - `scroll`: Scroll message handlers (Phase 6.1, Task 04)
//! - `log_view`: Log filtering/search handlers (Phase 6.1, Task 04)
//! - `settings_handlers`: Settings page handlers (Phase 6.1, Task 04)

use crate::message::{AutoLaunchSuccess, Message};
use crate::state::{AppState, DevToolsError, DevToolsPanel, UiMode};
use fdemon_core::{AppPhase, LogSource};
use tracing::warn;

use super::{
    daemon::handle_session_daemon_event, devtools, keys::handle_key, log_view, new_session, scroll,
    session_lifecycle, settings_handlers, Task, UpdateAction, UpdateResult,
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
            handle_session_daemon_event(state, session_id, event)
        }

        // ─────────────────────────────────────────────────────────
        // Scroll Messages
        // ─────────────────────────────────────────────────────────
        Message::ScrollUp => scroll::handle_scroll_up(state),
        Message::ScrollDown => scroll::handle_scroll_down(state),
        Message::ScrollToTop => scroll::handle_scroll_to_top(state),
        Message::ScrollToBottom => scroll::handle_scroll_to_bottom(state),
        Message::PageUp => scroll::handle_page_up(state),
        Message::PageDown => scroll::handle_page_down(state),
        Message::ScrollLeft(n) => scroll::handle_scroll_left(state, n),
        Message::ScrollRight(n) => scroll::handle_scroll_right(state, n),
        Message::ScrollToLineStart => scroll::handle_scroll_to_line_start(state),
        Message::ScrollToLineEnd => scroll::handle_scroll_to_line_end(state),

        Message::Tick => {
            // Tick loading screen animation with message cycling (Task 08d)
            if state.ui_mode == UiMode::Loading && state.loading_state.is_some() {
                state.tick_loading_animation_with_cycling(true);
            }

            // Note: NewSessionDialog doesn't have animation frames to tick

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
                        handle.session.add_log(fdemon_core::LogEntry::info(
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
                handle.session.add_log(fdemon_core::LogEntry::error(
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
                        handle.session.add_log(fdemon_core::LogEntry::info(
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
                handle.session.add_log(fdemon_core::LogEntry::error(
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
                        handle.session.add_log(fdemon_core::LogEntry::info(
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
                handle.session.add_log(fdemon_core::LogEntry::error(
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
                handle.session.add_log(fdemon_core::LogEntry::info(
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
                handle.session.add_log(fdemon_core::LogEntry::error(
                    LogSource::App,
                    format!("Reload failed: {}", reason),
                ));
            }
            UpdateResult::none()
        }

        Message::SessionRestartCompleted { session_id } => {
            if let Some(handle) = state.session_manager.get_mut(session_id) {
                handle.session.complete_reload();
                handle.session.add_log(fdemon_core::LogEntry::info(
                    LogSource::App,
                    "Restarted".to_string(),
                ));
                // Invalidate the isolate ID cache — hot restart creates a new
                // Dart isolate with a different ID. The next call to
                // `main_isolate_id()` will re-fetch via `getVM` RPC so that
                // performance RPCs target the live isolate, not the dead one.
                if let Some(ref vm_handle) = handle.vm_request_handle {
                    vm_handle.invalidate_isolate_cache();
                }
            }
            UpdateResult::none()
        }

        Message::SessionRestartFailed { session_id, reason } => {
            if let Some(handle) = state.session_manager.get_mut(session_id) {
                handle.session.phase = AppPhase::Running;
                handle.session.reload_start_time = None;
                handle.session.add_log(fdemon_core::LogEntry::error(
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
                        handle.session.add_log(fdemon_core::LogEntry::info(
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
        Message::LaunchIOSSimulator => {
            tracing::info!("Launching iOS Simulator...");
            UpdateResult::action(UpdateAction::LaunchIOSSimulator)
        }

        Message::DevicesDiscovered { devices } => {
            let device_count = devices.len();

            // Update global cache FIRST (Task 08e)
            state.set_device_cache(devices.clone());

            // Update new_session_dialog_state (for Startup mode - Phase 8)
            if state.ui_mode == UiMode::Startup || state.ui_mode == UiMode::NewSessionDialog {
                // Preserve selection if possible (Task 10 - Selection Preservation)
                let previous_selection = state
                    .new_session_dialog_state
                    .target_selector
                    .selected_device_id();

                state
                    .new_session_dialog_state
                    .target_selector
                    .set_connected_devices(devices);

                // Restore selection if device still exists, otherwise reset to first
                if let Some(device_id) = previous_selection {
                    let restored = state
                        .new_session_dialog_state
                        .target_selector
                        .select_device_by_id(&device_id);

                    // If device not found, reset to first selectable device
                    if !restored {
                        state
                            .new_session_dialog_state
                            .target_selector
                            .reset_selection_to_first();
                    }
                }
            }

            // Note: Don't transition UI mode here - the caller handles that

            if device_count > 0 {
                tracing::info!("Discovered {} device(s)", device_count);
            } else {
                tracing::info!("No devices found");
            }

            UpdateResult::none()
        }

        Message::DeviceDiscoveryFailed {
            error,
            is_background,
        } => {
            if is_background {
                // Background refresh - log error but don't show to user
                // User can still select from cached devices
                tracing::warn!("Background device refresh failed: {}", error);
            } else {
                // Foreground discovery - show error to user
                // Update new_session_dialog_state if visible
                if state.ui_mode == UiMode::Startup || state.ui_mode == UiMode::NewSessionDialog {
                    // Set error on target selector
                    state
                        .new_session_dialog_state
                        .target_selector
                        .set_error(error.clone());
                }

                tracing::error!("Device discovery failed: {}", error);
            }

            UpdateResult::none()
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
            // Emulator UI deprecated - use NewSessionDialog instead
            warn!("Emulator UI is deprecated");
            UpdateResult::none()
        }

        Message::EmulatorDiscoveryFailed { error } => {
            tracing::error!("Emulator discovery failed: {}", error);
            // Emulator UI deprecated - use NewSessionDialog instead
            warn!("Emulator UI is deprecated");
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
                UpdateResult::none()
            }
        }

        // ─────────────────────────────────────────────────────────
        // Session Lifecycle Messages
        // ─────────────────────────────────────────────────────────
        Message::SessionStarted {
            session_id,
            device_id: _,
            device_name,
            platform: _,
            pid,
        } => session_lifecycle::handle_session_started(state, session_id, device_name, pid),

        Message::SessionSpawnFailed {
            session_id,
            device_id: _,
            error,
        } => session_lifecycle::handle_session_spawn_failed(state, session_id, error),

        Message::SessionProcessAttached {
            session_id,
            cmd_sender,
        } => session_lifecycle::handle_session_process_attached(state, session_id, cmd_sender),

        Message::SelectSessionByIndex(index) => {
            session_lifecycle::handle_select_session_by_index(state, index)
        }

        Message::NextSession => session_lifecycle::handle_next_session(state),

        Message::PreviousSession => session_lifecycle::handle_previous_session(state),

        Message::CloseCurrentSession => session_lifecycle::handle_close_current_session(state),

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

        Message::SelectLink(shortcut) => log_view::handle_select_link(state, shortcut),

        // ─────────────────────────────────────────────────────────
        // Settings Messages (Phase 4)
        // ─────────────────────────────────────────────────────────
        Message::ShowSettings => settings_handlers::handle_show_settings(state),

        Message::HideSettings => settings_handlers::handle_hide_settings(state),

        Message::SettingsNextTab => settings_handlers::handle_settings_next_tab(state),

        Message::SettingsPrevTab => settings_handlers::handle_settings_prev_tab(state),

        Message::SettingsGotoTab(idx) => settings_handlers::handle_settings_goto_tab(state, idx),

        Message::SettingsNextItem => settings_handlers::handle_settings_next_item(state),

        Message::SettingsPrevItem => settings_handlers::handle_settings_prev_item(state),

        Message::SettingsToggleEdit => settings_handlers::handle_settings_toggle_edit(state),

        Message::SettingsSave => settings_handlers::handle_settings_save(state),

        Message::SettingsResetItem => settings_handlers::handle_settings_reset_item(state),

        // ─────────────────────────────────────────────────────────
        // Settings Editing Messages (Phase 4, Task 10)
        // ─────────────────────────────────────────────────────────
        Message::SettingsToggleBool => settings_handlers::handle_settings_toggle_bool(state),

        Message::SettingsCycleEnumNext => settings_handlers::handle_settings_cycle_enum_next(state),

        Message::SettingsCycleEnumPrev => settings_handlers::handle_settings_cycle_enum_prev(state),

        Message::SettingsIncrement(delta) => {
            settings_handlers::handle_settings_increment(state, delta)
        }

        Message::SettingsCharInput(ch) => settings_handlers::handle_settings_char_input(state, ch),

        Message::SettingsBackspace => settings_handlers::handle_settings_backspace(state),

        Message::SettingsClearBuffer => settings_handlers::handle_settings_clear_buffer(state),

        Message::SettingsCommitEdit => settings_handlers::handle_settings_commit_edit(state),

        Message::SettingsCancelEdit => settings_handlers::handle_settings_cancel_edit(state),

        Message::SettingsRemoveListItem => {
            settings_handlers::handle_settings_remove_list_item(state)
        }

        // ─────────────────────────────────────────────────────────
        // Settings Persistence Messages (Phase 4, Task 11)
        // ─────────────────────────────────────────────────────────
        Message::SettingsSaveAndClose => settings_handlers::handle_settings_save_and_close(state),

        Message::ForceHideSettings => settings_handlers::handle_force_hide_settings(state),

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

                            // Session creation failed (e.g., max sessions reached) - show new session dialog with error
                            let configs = crate::config::load_all_configs(&state.project_path);
                            state.show_new_session_dialog(configs);
                            state
                                .new_session_dialog_state
                                .target_selector
                                .set_error(format!("Cannot create session: {}", e));
                            UpdateResult::none()
                        }
                    }
                }
                Err(error_msg) => {
                    // Clear loading before showing error dialog
                    state.clear_loading();

                    // Device discovery failed, show new session dialog with error
                    let configs = crate::config::load_all_configs(&state.project_path);
                    state.show_new_session_dialog(configs);
                    state
                        .new_session_dialog_state
                        .target_selector
                        .set_error(error_msg);
                    UpdateResult::none()
                }
            }
        }

        // ─────────────────────────────────────────────────────────
        // NewSessionDialog Messages (Phase 5 - Target Selector)
        // ─────────────────────────────────────────────────────────
        Message::OpenNewSessionDialog => new_session::handle_open_new_session_dialog(state),

        Message::CloseNewSessionDialog => new_session::handle_close_new_session_dialog(state),

        Message::HideNewSessionDialog => {
            state.hide_new_session_dialog();
            UpdateResult::none()
        }

        Message::NewSessionDialogEscape => new_session::handle_new_session_dialog_escape(state),

        Message::NewSessionDialogSwitchPane => new_session::handle_switch_pane(state),

        Message::NewSessionDialogSwitchTab(tab) => new_session::handle_switch_tab(state, tab),

        Message::NewSessionDialogToggleTab => new_session::handle_toggle_tab(state),

        Message::NewSessionDialogDeviceUp => new_session::handle_device_up(state),

        Message::NewSessionDialogDeviceDown => new_session::handle_device_down(state),

        Message::NewSessionDialogDeviceSelect => new_session::handle_device_select(state),

        Message::NewSessionDialogRefreshDevices => new_session::handle_refresh_devices(state),

        Message::NewSessionDialogConnectedDevicesReceived(devices) => {
            new_session::handle_connected_devices_received(state, devices)
        }

        Message::NewSessionDialogBootableDevicesReceived {
            ios_simulators,
            android_avds,
        } => new_session::handle_bootable_devices_received(state, ios_simulators, android_avds),

        Message::NewSessionDialogDeviceDiscoveryFailed {
            error,
            discovery_type,
        } => new_session::handle_device_discovery_failed(state, error, discovery_type),

        Message::NewSessionDialogBootStarted { device_id } => {
            new_session::handle_boot_started(state, device_id)
        }

        Message::NewSessionDialogBootCompleted { .. } => new_session::handle_boot_completed(state),

        Message::NewSessionDialogBootFailed { device_id, error } => {
            new_session::handle_boot_failed(state, device_id, error)
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
        // NewSessionDialog - Launch Context Field Navigation (Phase 6, Task 05)
        // ─────────────────────────────────────────────────────────
        Message::NewSessionDialogFieldNext => new_session::handle_field_next(state),

        Message::NewSessionDialogFieldPrev => new_session::handle_field_prev(state),

        Message::NewSessionDialogFieldActivate => new_session::handle_field_activate(state, update),

        Message::NewSessionDialogModeNext => new_session::handle_mode_next(state),

        Message::NewSessionDialogModePrev => new_session::handle_mode_prev(state),

        Message::NewSessionDialogConfigSelected { config_name } => {
            new_session::handle_config_selected(state, config_name)
        }

        Message::NewSessionDialogFlavorSelected { flavor } => {
            new_session::handle_flavor_selected(state, flavor)
        }

        Message::NewSessionDialogEntryPointSelected { entry_point } => {
            new_session::handle_entry_point_selected(state, entry_point)
        }

        Message::NewSessionDialogDartDefinesUpdated { defines } => {
            new_session::handle_dart_defines_updated(state, defines)
        }

        Message::NewSessionDialogLaunch => new_session::handle_launch(state),

        Message::NewSessionDialogConfigSaved => new_session::handle_config_saved(state),

        Message::NewSessionDialogConfigSaveFailed { error } => {
            new_session::handle_config_save_failed(state, error)
        }

        // ─────────────────────────────────────────────────────────
        // NewSessionDialog - Fuzzy Modal Handlers
        // ─────────────────────────────────────────────────────────
        Message::NewSessionDialogOpenFuzzyModal { modal_type } => {
            new_session::handle_open_fuzzy_modal(state, modal_type)
        }

        Message::NewSessionDialogCloseFuzzyModal => new_session::handle_close_fuzzy_modal(state),

        Message::NewSessionDialogFuzzyUp => new_session::handle_fuzzy_up(state),

        Message::NewSessionDialogFuzzyDown => new_session::handle_fuzzy_down(state),

        Message::NewSessionDialogFuzzyConfirm => new_session::handle_fuzzy_confirm(state, update),

        Message::NewSessionDialogFuzzyInput { c } => new_session::handle_fuzzy_input(state, c),

        Message::NewSessionDialogFuzzyBackspace => new_session::handle_fuzzy_backspace(state),

        Message::NewSessionDialogFuzzyClear => new_session::handle_fuzzy_clear(state),

        // ─────────────────────────────────────────────────────────
        // NewSessionDialog - Dart Defines Modal Handlers
        // ─────────────────────────────────────────────────────────
        Message::NewSessionDialogOpenDartDefinesModal => {
            new_session::handle_open_dart_defines_modal(state)
        }

        Message::NewSessionDialogCloseDartDefinesModal => {
            new_session::handle_close_dart_defines_modal(state, update)
        }

        Message::NewSessionDialogDartDefinesSwitchPane => {
            new_session::handle_dart_defines_switch_pane(state)
        }

        Message::NewSessionDialogDartDefinesUp => new_session::handle_dart_defines_up(state),

        Message::NewSessionDialogDartDefinesDown => new_session::handle_dart_defines_down(state),

        Message::NewSessionDialogDartDefinesConfirm => {
            new_session::handle_dart_defines_confirm(state)
        }

        Message::NewSessionDialogDartDefinesNextField => {
            new_session::handle_dart_defines_next_field(state)
        }

        Message::NewSessionDialogDartDefinesInput { c } => {
            new_session::handle_dart_defines_input(state, c)
        }

        Message::NewSessionDialogDartDefinesBackspace => {
            new_session::handle_dart_defines_backspace(state)
        }

        Message::NewSessionDialogDartDefinesSave => new_session::handle_dart_defines_save(state),

        Message::NewSessionDialogDartDefinesDelete => {
            new_session::handle_dart_defines_delete(state)
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

            // Trigger bootable device discovery now that we know which tools are available
            if state.tool_availability.xcrun_simctl || state.tool_availability.android_emulator {
                // Set loading state for bootable tab
                state
                    .new_session_dialog_state
                    .target_selector
                    .bootable_loading = true;
                UpdateResult::action(UpdateAction::DiscoverBootableDevices)
            } else {
                // No tools available - stop loading spinner
                state
                    .new_session_dialog_state
                    .target_selector
                    .bootable_loading = false;
                UpdateResult::none()
            }
        }

        Message::DiscoverBootableDevices => {
            // Trigger action to discover bootable devices
            UpdateResult::action(UpdateAction::DiscoverBootableDevices)
        }

        Message::BootableDevicesDiscovered {
            ios_simulators,
            android_avds,
        } => {
            // Cache bootable devices for instant display on subsequent dialog opens (Bug Fix: Task 03)
            state.set_bootable_cache(ios_simulators.clone(), android_avds.clone());

            // Update new session dialog state if visible
            if state.is_new_session_dialog_visible() {
                state
                    .new_session_dialog_state
                    .target_selector
                    .set_bootable_devices(ios_simulators, android_avds);
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
                    .target_selector
                    .set_error(format!("Failed to boot {}: {}", device_id, error));
            }

            UpdateResult::none()
        }

        // ─────────────────────────────────────────────────────────
        // VM Service Messages (Phase 1 DevTools Integration)
        // ─────────────────────────────────────────────────────────

        // Store the request handle in the session handle so that background tasks
        // can make on-demand RPC calls through the same WebSocket connection.
        // This message arrives before VmServiceAttached and VmServiceConnected.
        Message::VmServiceHandleReady { session_id, handle } => {
            if let Some(session_handle) = state.session_manager.get_mut(session_id) {
                session_handle.vm_request_handle = Some(handle);
                tracing::debug!(
                    "VM Service request handle stored for session {}",
                    session_id
                );
            }
            UpdateResult::none()
        }

        // Store the shutdown sender in the session handle.
        // This message arrives before VmServiceConnected to ensure the session
        // can signal shutdown at any time after the channel is created.
        Message::VmServiceAttached {
            session_id,
            vm_shutdown_tx,
        } => {
            if let Some(handle) = state.session_manager.get_mut(session_id) {
                handle.vm_shutdown_tx = Some(vm_shutdown_tx);
                tracing::debug!(
                    "VM Service shutdown channel attached for session {}",
                    session_id
                );
            }
            UpdateResult::none()
        }

        Message::VmServiceConnected { session_id } => {
            // Read config values before borrowing state mutably.
            let memory_history_size = state.settings.devtools.memory_history_size;
            let performance_refresh_ms = state.settings.devtools.performance_refresh_ms;
            let allocation_profile_interval_ms =
                state.settings.devtools.allocation_profile_interval_ms;
            let auto_repaint_rainbow = state.settings.devtools.auto_repaint_rainbow;
            let auto_performance_overlay = state.settings.devtools.auto_performance_overlay;

            if let Some(handle) = state.session_manager.get_mut(session_id) {
                handle.session.vm_connected = true;
                handle.session.add_log(fdemon_core::LogEntry::info(
                    LogSource::App,
                    "VM Service connected — enhanced logging active",
                ));
                // Reset performance state on (re)connection so stale data from
                // a previous session or hot-restart is not shown in the new one.
                // Use configurable memory history size from settings.
                handle.session.performance =
                    crate::session::PerformanceState::with_memory_history_size(memory_history_size);
            }
            // Clear any previous connection error now that we are connected.
            state.devtools_view_state.vm_connection_error = None;
            // Update rich connection status indicator.
            state.devtools_view_state.connection_status =
                crate::state::VmConnectionStatus::Connected;

            // If the user is already in DevTools/Inspector mode with no tree loaded,
            // auto-fetch the widget tree now that the VM is connected.
            let widget_tree_follow_up = if state.ui_mode == UiMode::DevTools
                && state.devtools_view_state.active_panel == DevToolsPanel::Inspector
                && state.devtools_view_state.inspector.root.is_none()
                && !state.devtools_view_state.inspector.loading
            {
                Some(Message::RequestWidgetTree { session_id })
            } else {
                None
            };

            // Auto-enable overlays: if configured, queue a ToggleDebugOverlay
            // message for the first overlay that needs enabling. Only trigger
            // when the overlay is currently disabled — the toggle action reads
            // the current state before flipping, so this is safe and idempotent.
            // Widget tree fetch takes priority over overlay toggles.
            let auto_overlay_follow_up = if widget_tree_follow_up.is_none() {
                if auto_repaint_rainbow && !state.devtools_view_state.overlay_repaint_rainbow {
                    Some(Message::ToggleDebugOverlay {
                        extension: crate::message::DebugOverlayKind::RepaintRainbow,
                    })
                } else if auto_performance_overlay && !state.devtools_view_state.overlay_performance
                {
                    Some(Message::ToggleDebugOverlay {
                        extension: crate::message::DebugOverlayKind::PerformanceOverlay,
                    })
                } else {
                    None
                }
            } else {
                None
            };

            let follow_up_msg = widget_tree_follow_up.or(auto_overlay_follow_up);

            // Start performance monitoring for this session.
            // process.rs will hydrate `handle` with the VmRequestHandle from the
            // session before dispatching the action to handle_action.
            UpdateResult {
                message: follow_up_msg,
                action: Some(UpdateAction::StartPerformanceMonitoring {
                    session_id,
                    handle: None, // hydrated by process.rs
                    performance_refresh_ms,
                    allocation_profile_interval_ms,
                }),
            }
        }

        Message::VmServiceConnectionFailed { session_id, error } => {
            warn!(
                "VM Service connection failed for session {}: {}",
                session_id, error
            );
            // Show in session logs so the user knows DevTools features are unavailable
            if let Some(handle) = state.session_manager.get_mut(session_id) {
                handle.session.add_log(fdemon_core::LogEntry::new(
                    fdemon_core::LogLevel::Warning,
                    LogSource::App,
                    format!(
                        "VM Service connection failed: {error} — DevTools features unavailable"
                    ),
                ));
            }
            // Surface the error in DevTools panels so users see the specific reason
            // instead of the generic "VM Service not connected" message.
            state.devtools_view_state.vm_connection_error =
                Some(format!("Connection failed: {error}"));
            UpdateResult::none()
        }

        Message::VmServiceDisconnected { session_id } => {
            // Update rich connection status indicator.
            state.devtools_view_state.connection_status =
                crate::state::VmConnectionStatus::Disconnected;
            if let Some(handle) = state.session_manager.get_mut(session_id) {
                handle.session.vm_connected = false;
                // Clear the request handle — the underlying channel is now closed.
                // Making this explicit signals intent even though the handle itself
                // would return Error::ChannelClosed on any subsequent call.
                handle.vm_request_handle = None;
                // Clear the shutdown sender. By the time VmServiceDisconnected is
                // dispatched, the forward_vm_events task has already exited (it sends
                // this message as its final act), so dropping the sender is safe and
                // allows maybe_connect_vm_service to attempt a fresh connection on
                // the next AppDebugPort message.
                handle.vm_shutdown_tx = None;
                // Abort the performance polling task and signal it to stop cleanly.
                if let Some(h) = handle.perf_task_handle.take() {
                    h.abort();
                }
                if let Some(ref tx) = handle.perf_shutdown_tx {
                    let _ = tx.send(true);
                }
                handle.perf_shutdown_tx = None;
                handle.session.performance.monitoring_active = false;
            }
            UpdateResult::none()
        }

        Message::VmServiceFlutterError {
            session_id,
            log_entry,
        } => {
            let dedupe_ms = state.settings.devtools.logging.dedupe_threshold_ms as i64;
            if let Some(handle) = state.session_manager.get_mut(session_id) {
                if !is_duplicate_vm_log(&handle.session.logs, &log_entry, dedupe_ms) {
                    handle.session.add_log(log_entry);
                }
            }
            UpdateResult::none()
        }

        Message::VmServiceLogRecord {
            session_id,
            log_entry,
        } => {
            let dedupe_ms = state.settings.devtools.logging.dedupe_threshold_ms as i64;
            if let Some(handle) = state.session_manager.get_mut(session_id) {
                if !is_duplicate_vm_log(&handle.session.logs, &log_entry, dedupe_ms) {
                    handle.session.add_log(log_entry);
                }
            }
            UpdateResult::none()
        }

        // ─────────────────────────────────────────────────────────
        // VM Service Performance Messages (Phase 3, Task 05)
        // ─────────────────────────────────────────────────────────
        Message::VmServiceMemorySnapshot { session_id, memory } => {
            if let Some(handle) = state.session_manager.get_mut(session_id) {
                handle.session.performance.memory_history.push(memory);
                handle.session.performance.monitoring_active = true;
                // Recompute stats on every memory poll cycle (2-second backstop
                // for when frame events are sparse — e.g. idle or backgrounded).
                handle.session.performance.recompute_stats();
            }
            UpdateResult::none()
        }

        Message::VmServiceGcEvent {
            session_id,
            gc_event,
        } => {
            if let Some(handle) = state.session_manager.get_mut(session_id) {
                // Only store major GC events (MarkSweep, MarkCompact) to prevent
                // frequent Scavenge events from filling the ring buffer and pushing
                // out the more informative major GC entries.
                if gc_event.is_major_gc() {
                    handle.session.performance.gc_history.push(gc_event);
                } else {
                    tracing::trace!(
                        "Filtered Scavenge GC event for session {} (minor GC)",
                        session_id
                    );
                }
            }
            UpdateResult::none()
        }

        // ─────────────────────────────────────────────────────────
        // VM Service Frame Timing Messages (Phase 3, Task 06)
        // ─────────────────────────────────────────────────────────
        Message::VmServiceFrameTiming { session_id, timing } => {
            if let Some(handle) = state.session_manager.get_mut(session_id) {
                handle.session.performance.frame_history.push(timing);
                // Recompute stats every STATS_RECOMPUTE_INTERVAL frames to
                // avoid per-frame allocation overhead. At 60 FPS this produces
                // ~6 stats updates/second — fast enough for a ~30 FPS TUI.
                let len = handle.session.performance.frame_history.len();
                if len % crate::session::STATS_RECOMPUTE_INTERVAL == 0 {
                    handle.session.performance.recompute_stats();
                }
            }
            UpdateResult::none()
        }

        Message::VmServicePerformanceMonitoringStarted {
            session_id,
            perf_shutdown_tx,
            perf_task_handle,
        } => {
            if let Some(handle) = state.session_manager.get_mut(session_id) {
                handle.perf_shutdown_tx = Some(perf_shutdown_tx);
                // Take the JoinHandle out of the Arc<Mutex<Option<>>> so it is
                // owned by the SessionHandle and can be awaited/aborted on close.
                handle.perf_task_handle = perf_task_handle.lock().ok().and_then(|mut g| g.take());
            }
            UpdateResult::none()
        }

        // ─────────────────────────────────────────────────────────
        // DevTools Mode Messages (Phase 4, Task 02)
        // ─────────────────────────────────────────────────────────
        Message::EnterDevToolsMode => devtools::handle_enter_devtools_mode(state),

        Message::ExitDevToolsMode => devtools::handle_exit_devtools_mode(state),

        Message::SwitchDevToolsPanel(panel) => devtools::handle_switch_panel(state, panel),

        Message::OpenBrowserDevTools => devtools::handle_open_browser_devtools(state),

        Message::RequestWidgetTree { session_id } => {
            // Cooldown: suppress rapid refreshes while loading or within 2 seconds
            // of the last fetch. This prevents RPC spam when the user holds `r`.
            if state.devtools_view_state.inspector.is_fetch_debounced() {
                return UpdateResult::none();
            }

            let vm_connected = state
                .session_manager
                .get(session_id)
                .map(|h| h.session.vm_connected)
                .unwrap_or(false);

            if vm_connected {
                // Clear any previous error so a fresh fetch starts cleanly.
                state.devtools_view_state.inspector.error = None;
                state.devtools_view_state.inspector.record_fetch_start();
                UpdateResult::action(UpdateAction::FetchWidgetTree {
                    session_id,
                    vm_handle: None, // hydrated by process.rs
                    tree_max_depth: state.settings.devtools.tree_max_depth,
                })
            } else {
                state.devtools_view_state.inspector.error = Some(DevToolsError::new(
                    "VM Service not available",
                    "Ensure the app is running in debug mode",
                ));
                UpdateResult::none()
            }
        }

        Message::WidgetTreeFetched { session_id, root } => {
            devtools::handle_widget_tree_fetched(state, session_id, root)
        }

        Message::WidgetTreeFetchFailed { session_id, error } => {
            devtools::handle_widget_tree_fetch_failed(state, session_id, error)
        }

        Message::RequestLayoutData {
            session_id,
            node_id,
        } => {
            let vm_connected = state
                .session_manager
                .get(session_id)
                .map(|h| h.session.vm_connected)
                .unwrap_or(false);

            if vm_connected {
                // Clear any previous error so a fresh fetch starts cleanly.
                state.devtools_view_state.inspector.layout_error = None;
                state.devtools_view_state.inspector.layout_loading = true;
                // Track which node we are fetching so we can record it on success.
                state.devtools_view_state.inspector.pending_node_id = Some(node_id.clone());
                UpdateResult::action(UpdateAction::FetchLayoutData {
                    session_id,
                    node_id,
                    vm_handle: None, // hydrated by process.rs
                })
            } else {
                state.devtools_view_state.inspector.layout_error = Some(DevToolsError::new(
                    "VM Service not available",
                    "Ensure the app is running in debug mode",
                ));
                UpdateResult::none()
            }
        }

        Message::LayoutDataFetched { session_id, layout } => {
            devtools::handle_layout_data_fetched(state, session_id, *layout)
        }

        Message::LayoutDataFetchFailed { session_id, error } => {
            devtools::handle_layout_data_fetch_failed(state, session_id, error)
        }

        Message::ToggleDebugOverlay { extension } => {
            // Debounce: suppress rapid key presses within 500 ms to avoid
            // multiple in-flight RPC calls for the same overlay toggle.
            if state.devtools_view_state.is_overlay_toggle_debounced() {
                return UpdateResult::none();
            }

            // Find active session_id for the toggle action
            if let Some(handle) = state.session_manager.selected() {
                let session_id = handle.session.id;
                state.devtools_view_state.record_overlay_toggle();
                return UpdateResult::action(UpdateAction::ToggleOverlay {
                    session_id,
                    extension,
                    vm_handle: None, // hydrated by process.rs
                });
            }
            UpdateResult::none()
        }

        Message::DebugOverlayToggled { extension, enabled } => {
            devtools::handle_debug_overlay_toggled(state, extension, enabled)
        }

        Message::DevToolsInspectorNavigate(nav) => devtools::handle_inspector_navigate(state, nav),

        // ─────────────────────────────────────────────────────────
        // VM Service Connection State Messages (Phase 5, Task 02)
        // ─────────────────────────────────────────────────────────
        Message::VmServiceReconnecting {
            session_id,
            attempt,
            max_attempts,
        } => devtools::handle_vm_service_reconnecting(state, session_id, attempt, max_attempts),

        Message::WidgetTreeFetchTimeout { session_id } => {
            devtools::handle_widget_tree_fetch_timeout(state, session_id)
        }

        Message::LayoutDataFetchTimeout { session_id } => {
            devtools::handle_layout_data_fetch_timeout(state, session_id)
        }

        // ─────────────────────────────────────────────────────────
        // Entry Point Discovery Messages (Phase 3, Task 09)
        // ─────────────────────────────────────────────────────────
        Message::EntryPointsDiscovered { entry_points } => {
            // Clear loading flag
            state
                .new_session_dialog_state
                .launch_context
                .entry_points_loading = false;

            // Cache discovered entry points
            state
                .new_session_dialog_state
                .launch_context
                .set_available_entry_points(entry_points);

            // Update modal if open
            if let Some(ref mut modal) = state.new_session_dialog_state.fuzzy_modal {
                if modal.modal_type == crate::new_session_dialog::FuzzyModalType::EntryPoint {
                    let items = state
                        .new_session_dialog_state
                        .launch_context
                        .entry_point_modal_items();
                    modal.items = items;

                    // Reapply fuzzy filter with current query
                    use crate::new_session_dialog::fuzzy::fuzzy_filter;
                    let filtered = fuzzy_filter(&modal.query, &modal.items);
                    modal.update_filter(filtered);
                }
            }

            UpdateResult::none()
        }

        // ─────────────────────────────────────────────────────────────────────
        // VM Service Performance Messages — Phase 3 extensions (Task 02/04)
        // ─────────────────────────────────────────────────────────────────────
        Message::SelectPerformanceFrame { index } => {
            devtools::handle_select_performance_frame(state, index)
        }

        Message::VmServiceMemorySample { session_id, sample } => {
            devtools::handle_memory_sample_received(state, session_id, sample)
        }

        Message::VmServiceAllocationProfileReceived {
            session_id,
            profile,
        } => devtools::handle_allocation_profile_received(state, session_id, profile),
    }
}

/// Number of recent log entries to scan for deduplication.
const DEDUP_SCAN_DEPTH: usize = 10;

/// Check if a log entry is a duplicate of a recent VM Service entry.
///
/// Scans the last [`DEDUP_SCAN_DEPTH`] entries in the log buffer and returns
/// `true` if an entry with the same message was added within `threshold_ms`
/// milliseconds.
fn is_duplicate_vm_log(
    logs: &std::collections::VecDeque<fdemon_core::LogEntry>,
    entry: &fdemon_core::LogEntry,
    threshold_ms: i64,
) -> bool {
    let threshold = chrono::TimeDelta::milliseconds(threshold_ms);
    logs.iter().rev().take(DEDUP_SCAN_DEPTH).any(|existing| {
        existing.message == entry.message
            && (existing.timestamp - entry.timestamp).abs() < threshold
    })
}

/// Scroll the log view to show a specific log entry
fn scroll_to_log_entry(session: &mut crate::session::Session, entry_index: usize) {
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

// Tests have been moved to src/config/launch.rs where parse_dart_defines is now defined
