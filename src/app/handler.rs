//! Update function - handles state transitions (TEA pattern)

use super::message::Message;
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
        device: Device,
        config: Option<Box<LaunchConfig>>,
    },
}

/// Background tasks to spawn
#[derive(Debug, Clone)]
pub enum Task {
    /// Hot reload
    Reload { app_id: String },
    /// Hot restart
    Restart { app_id: String },
    /// Stop the app
    Stop { app_id: String },
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
        Message::Quit => {
            state.phase = AppPhase::Quitting;
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

        Message::Tick => UpdateResult::none(),

        // ─────────────────────────────────────────────────────────
        // Control Messages
        // ─────────────────────────────────────────────────────────
        Message::HotReload => {
            if state.is_busy() {
                UpdateResult::none()
            } else if let Some(app_id) = state.current_app_id.clone() {
                state.start_reload();
                state.log_info(LogSource::App, "Reloading...");
                UpdateResult::action(UpdateAction::SpawnTask(Task::Reload { app_id }))
            } else {
                state.log_error(LogSource::App, "No app running to reload");
                UpdateResult::none()
            }
        }

        Message::HotRestart => {
            if state.is_busy() {
                UpdateResult::none()
            } else if let Some(app_id) = state.current_app_id.clone() {
                state.start_reload();
                state.log_info(LogSource::App, "Restarting...");
                UpdateResult::action(UpdateAction::SpawnTask(Task::Restart { app_id }))
            } else {
                state.log_error(LogSource::App, "No app running to restart");
                UpdateResult::none()
            }
        }

        Message::StopApp => {
            if state.is_busy() {
                UpdateResult::none()
            } else if let Some(app_id) = state.current_app_id.clone() {
                state.log_info(LogSource::App, "Stopping app...");
                UpdateResult::action(UpdateAction::SpawnTask(Task::Stop { app_id }))
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
                if let Some(app_id) = state.current_app_id.clone() {
                    state.log_info(LogSource::Watcher, "File change detected, reloading...");
                    state.start_reload();
                    UpdateResult::action(UpdateAction::SpawnTask(Task::Reload { app_id }))
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
            state.device_selector.show_loading();
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
            state.log_info(
                LogSource::App,
                format!("Device selected: {} ({})", device.name, device.id),
            );

            // Hide selector and switch to normal mode
            state.device_selector.hide();
            state.ui_mode = UiMode::Normal;

            // Return action to spawn session
            UpdateResult::action(UpdateAction::SpawnSession {
                device,
                config: None,
            })
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
            device_id: _,
            device_name,
            platform,
            pid,
        } => {
            // Update legacy single-session state for now
            state.device_name = Some(device_name.clone());
            state.platform = Some(platform);
            state.phase = AppPhase::Running;
            state.session_start = Some(chrono::Local::now());

            state.log_info(
                LogSource::App,
                format!(
                    "Flutter session started on {} (PID: {})",
                    device_name,
                    pid.map_or("unknown".to_string(), |p| p.to_string())
                ),
            );
            UpdateResult::none()
        }

        Message::SessionSpawnFailed {
            device_id: _,
            error,
        } => {
            state.log_error(
                LogSource::App,
                format!("Failed to start session: {}", error),
            );
            // Show device selector again so user can retry
            state.ui_mode = UiMode::DeviceSelector;
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
    match key.code {
        // Confirm quit
        KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => Some(Message::Quit),
        // Cancel
        KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => Some(Message::HideDeviceSelector), // This will cancel
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

    match key.code {
        // Quit - always allowed
        KeyCode::Char('q') | KeyCode::Esc => Some(Message::Quit),
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => Some(Message::Quit),

        // Hot reload (lowercase 'r') - only when not busy
        KeyCode::Char('r') if !is_busy => Some(Message::HotReload),

        // Hot restart (uppercase 'R') - only when not busy
        KeyCode::Char('R') if !is_busy => Some(Message::HotRestart),

        // Stop app (lowercase 's') - only when not busy
        KeyCode::Char('s') if !is_busy => Some(Message::StopApp),

        // New session (lowercase 'n') - show device selector
        KeyCode::Char('n') => Some(Message::ShowDeviceSelector),

        // Scrolling - always allowed
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
    fn test_q_key_produces_quit_message() {
        let state = AppState::new();
        let key = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE);

        let result = handle_key(&state, key);

        assert!(matches!(result, Some(Message::Quit)));
    }

    #[test]
    fn test_escape_key_produces_quit_message() {
        let state = AppState::new();
        let key = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);

        let result = handle_key(&state, key);

        assert!(matches!(result, Some(Message::Quit)));
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
}
