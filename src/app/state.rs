//! Application state (Model in TEA pattern)

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
}
