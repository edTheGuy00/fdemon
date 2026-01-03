//! Application state (Model in TEA pattern)

use std::time::Instant;

use chrono::{DateTime, Local};

use crate::core::{AppPhase, LogEntry, LogSource};
use crate::tui::widgets::LogViewState;

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
    // App Tracking
    // ─────────────────────────────────────────────────────────
    /// Current app ID (from daemon's app.start event)
    pub current_app_id: Option<String>,

    /// Device name (e.g., "iPhone 15 Pro")
    pub device_name: Option<String>,

    /// Platform (e.g., "ios", "android", "macos")
    pub platform: Option<String>,

    /// Flutter SDK version (if detected)
    pub flutter_version: Option<String>,

    /// When the Flutter app started
    pub session_start: Option<DateTime<Local>>,

    // ─────────────────────────────────────────────────────────
    // Reload Tracking
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
            current_app_id: None,
            device_name: None,
            platform: None,
            flutter_version: None,
            session_start: None,
            reload_start_time: None,
            last_reload_time: None,
            reload_count: 0,
        }
    }

    /// Add a log entry
    pub fn add_log(&mut self, entry: LogEntry) {
        self.logs.push(entry);

        // Trim if over max size
        if self.logs.len() > self.max_logs {
            let drain_count = self.logs.len() - self.max_logs;
            self.logs.drain(0..drain_count);

            // Adjust scroll offset
            self.log_view_state.offset = self.log_view_state.offset.saturating_sub(drain_count);
        }
    }

    /// Add an info log
    pub fn log_info(&mut self, source: LogSource, message: impl Into<String>) {
        self.add_log(LogEntry::info(source, message));
    }

    /// Add an error log
    pub fn log_error(&mut self, source: LogSource, message: impl Into<String>) {
        self.add_log(LogEntry::error(source, message));
    }

    /// Check if the app should quit
    pub fn should_quit(&self) -> bool {
        self.phase == AppPhase::Quitting
    }

    /// Called when a reload starts
    pub fn start_reload(&mut self) {
        self.reload_start_time = Some(Instant::now());
        self.phase = AppPhase::Reloading;
    }

    /// Called when a reload completes successfully
    pub fn record_reload_complete(&mut self) {
        self.reload_count += 1;
        self.last_reload_time = Some(Local::now());
        self.reload_start_time = None;
        self.phase = AppPhase::Running;
    }

    /// Get elapsed time since reload started
    pub fn reload_elapsed(&self) -> Option<std::time::Duration> {
        self.reload_start_time.map(|start| start.elapsed())
    }

    /// Format last reload time for display
    pub fn last_reload_display(&self) -> Option<String> {
        self.last_reload_time
            .map(|t| t.format("%H:%M:%S").to_string())
    }

    /// Calculate session duration from start time
    pub fn session_duration(&self) -> Option<chrono::Duration> {
        self.session_start.map(|start| Local::now() - start)
    }

    /// Format session duration as HH:MM:SS
    pub fn session_duration_display(&self) -> Option<String> {
        self.session_duration().map(|d| {
            let total_secs = d.num_seconds().max(0);
            let hours = total_secs / 3600;
            let minutes = (total_secs % 3600) / 60;
            let seconds = total_secs % 60;
            format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
        })
    }

    /// Mark the session as started (sets session_start to now)
    pub fn start_session(&mut self) {
        self.session_start = Some(Local::now());
        self.phase = AppPhase::Running;
    }

    /// Update device information
    pub fn set_device_info(&mut self, name: Option<String>, platform: Option<String>) {
        self.device_name = name;
        self.platform = platform;
    }

    /// Check if currently in a reload/restart operation
    pub fn is_busy(&self) -> bool {
        matches!(self.phase, AppPhase::Reloading)
    }
}
