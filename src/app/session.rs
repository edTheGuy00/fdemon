//! Per-instance session state for a running Flutter app

use std::sync::Arc;
use std::time::Instant;

use chrono::{DateTime, Local};

use crate::config::LaunchConfig;
use crate::core::{AppPhase, FilterState, LogEntry, LogSource, SearchState};
use crate::daemon::{CommandSender, FlutterProcess, RequestTracker};
use crate::tui::widgets::LogViewState;

/// Unique identifier for a session
pub type SessionId = u64;

/// Generate a new unique session ID
static SESSION_ID_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

pub fn next_session_id() -> SessionId {
    SESSION_ID_COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
}

/// A single Flutter app session
#[derive(Debug)]
pub struct Session {
    /// Unique session identifier
    pub id: SessionId,

    /// Display name for this session (device name or config name)
    pub name: String,

    /// Current phase of this session
    pub phase: AppPhase,

    /// Log buffer for this session
    pub logs: Vec<LogEntry>,

    /// Log view scroll state
    pub log_view_state: LogViewState,

    /// Maximum log buffer size
    pub max_logs: usize,

    // ─────────────────────────────────────────────────────────
    // Filter & Search State
    // ─────────────────────────────────────────────────────────
    /// Log filter state for this session
    pub filter_state: FilterState,

    /// Search state for this session
    pub search_state: SearchState,

    // ─────────────────────────────────────────────────────────
    // Device & App Tracking
    // ─────────────────────────────────────────────────────────
    /// Device ID this session is running on
    pub device_id: String,

    /// Device display name
    pub device_name: String,

    /// Platform (e.g., "ios", "android", "macos")
    pub platform: String,

    /// Whether device is emulator/simulator
    pub is_emulator: bool,

    /// Current app ID (from daemon's app.start event)
    pub app_id: Option<String>,

    /// Launch configuration used
    pub launch_config: Option<LaunchConfig>,

    // ─────────────────────────────────────────────────────────
    // Timing
    // ─────────────────────────────────────────────────────────
    /// When this session was created
    pub created_at: DateTime<Local>,

    /// When the Flutter app started running
    pub started_at: Option<DateTime<Local>>,

    /// When the current reload started (for timing)
    pub reload_start_time: Option<Instant>,

    /// Last successful reload time
    pub last_reload_time: Option<DateTime<Local>>,

    /// Total reload count this session
    pub reload_count: u32,
}

impl Session {
    /// Create a new session for a device
    pub fn new(
        device_id: String,
        device_name: String,
        platform: String,
        is_emulator: bool,
    ) -> Self {
        Self {
            id: next_session_id(),
            name: device_name.clone(),
            phase: AppPhase::Initializing,
            logs: Vec::new(),
            log_view_state: LogViewState::new(),
            max_logs: 10_000,
            filter_state: FilterState::default(),
            search_state: SearchState::default(),
            device_id,
            device_name,
            platform,
            is_emulator,
            app_id: None,
            launch_config: None,
            created_at: Local::now(),
            started_at: None,
            reload_start_time: None,
            last_reload_time: None,
            reload_count: 0,
        }
    }

    /// Create session with a launch configuration
    pub fn with_config(mut self, config: LaunchConfig) -> Self {
        self.name = config.name.clone();
        self.launch_config = Some(config);
        self
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

    /// Clear all logs
    pub fn clear_logs(&mut self) {
        self.logs.clear();
        self.log_view_state.offset = 0;
        // Clear search matches since logs are gone
        self.search_state.matches.clear();
        self.search_state.current_match = None;
    }

    /// Mark session as started
    pub fn mark_started(&mut self, app_id: String) {
        self.app_id = Some(app_id);
        self.started_at = Some(Local::now());
        self.phase = AppPhase::Running;
    }

    /// Mark session as stopped
    pub fn mark_stopped(&mut self) {
        self.phase = AppPhase::Stopped;
    }

    /// Called when a reload starts
    pub fn start_reload(&mut self) {
        self.reload_start_time = Some(Instant::now());
        self.phase = AppPhase::Reloading;
    }

    /// Called when a reload completes successfully
    pub fn complete_reload(&mut self) {
        self.reload_count += 1;
        self.last_reload_time = Some(Local::now());
        self.reload_start_time = None;
        self.phase = AppPhase::Running;
    }

    /// Get elapsed time since reload started
    pub fn reload_elapsed(&self) -> Option<std::time::Duration> {
        self.reload_start_time.map(|start| start.elapsed())
    }

    /// Calculate session duration from start time
    pub fn session_duration(&self) -> Option<chrono::Duration> {
        self.started_at.map(|start| Local::now() - start)
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

    /// Alias for status bar widget compatibility
    pub fn duration_display(&self) -> Option<String> {
        self.session_duration_display()
    }

    /// Format last reload time for display
    pub fn last_reload_display(&self) -> Option<String> {
        self.last_reload_time
            .map(|t| t.format("%H:%M:%S").to_string())
    }

    /// Check if session is running
    pub fn is_running(&self) -> bool {
        matches!(self.phase, AppPhase::Running | AppPhase::Reloading)
    }

    /// Check if session is in a busy state (reload/restart in progress)
    pub fn is_busy(&self) -> bool {
        matches!(self.phase, AppPhase::Reloading)
    }

    /// Get status indicator character
    pub fn status_icon(&self) -> &'static str {
        match self.phase {
            AppPhase::Initializing => "○",
            AppPhase::Running => "●",
            AppPhase::Reloading => "↻",
            AppPhase::Stopped => "○",
            AppPhase::Quitting => "×",
        }
    }

    /// Get a short display title for tabs
    pub fn tab_title(&self) -> String {
        let icon = self.status_icon();
        let name = if self.name.len() > 15 {
            format!("{}…", &self.name[..14])
        } else {
            self.name.clone()
        };
        format!("{} {}", icon, name)
    }

    // ─────────────────────────────────────────────────────────
    // Filter Methods
    // ─────────────────────────────────────────────────────────

    /// Cycle the log level filter
    pub fn cycle_level_filter(&mut self) {
        self.filter_state.level_filter = self.filter_state.level_filter.cycle();
    }

    /// Cycle the log source filter
    pub fn cycle_source_filter(&mut self) {
        self.filter_state.source_filter = self.filter_state.source_filter.cycle();
    }

    /// Reset all filters to default
    pub fn reset_filters(&mut self) {
        self.filter_state.reset();
    }

    /// Get filtered logs (returns indices of matching entries)
    pub fn filtered_log_indices(&self) -> Vec<usize> {
        self.logs
            .iter()
            .enumerate()
            .filter(|(_, entry)| self.filter_state.matches(entry))
            .map(|(i, _)| i)
            .collect()
    }

    /// Check if any filter is active
    pub fn has_active_filter(&self) -> bool {
        self.filter_state.is_active()
    }

    // ─────────────────────────────────────────────────────────
    // Search Methods
    // ─────────────────────────────────────────────────────────

    /// Start search mode
    pub fn start_search(&mut self) {
        self.search_state.activate();
    }

    /// Cancel search mode
    pub fn cancel_search(&mut self) {
        self.search_state.deactivate();
    }

    /// Clear search completely
    pub fn clear_search(&mut self) {
        self.search_state.clear();
    }

    /// Update search query
    pub fn set_search_query(&mut self, query: &str) {
        self.search_state.set_query(query);
    }

    /// Check if search mode is active
    pub fn is_searching(&self) -> bool {
        self.search_state.is_active
    }

    // ─────────────────────────────────────────────────────────
    // Error Navigation Methods
    // ─────────────────────────────────────────────────────────

    /// Get indices of all error log entries
    pub fn error_indices(&self) -> Vec<usize> {
        self.logs
            .iter()
            .enumerate()
            .filter(|(_, entry)| entry.is_error())
            .map(|(i, _)| i)
            .collect()
    }

    /// Get indices of errors that pass the current filter
    pub fn filtered_error_indices(&self) -> Vec<usize> {
        self.logs
            .iter()
            .enumerate()
            .filter(|(_, entry)| entry.is_error() && self.filter_state.matches(entry))
            .map(|(i, _)| i)
            .collect()
    }

    /// Count total errors in log
    pub fn error_count(&self) -> usize {
        self.logs.iter().filter(|e| e.is_error()).count()
    }

    /// Find next error after current scroll position
    /// Returns the log entry index of the next error
    pub fn find_next_error(&self) -> Option<usize> {
        let errors = self.filtered_error_indices();
        if errors.is_empty() {
            return None;
        }

        let current_pos = self.current_log_position();

        // Find first error after current position
        for &error_idx in &errors {
            if error_idx > current_pos {
                return Some(error_idx);
            }
        }

        // Wrap around to first error
        Some(errors[0])
    }

    /// Find previous error before current scroll position
    /// Returns the log entry index of the previous error
    pub fn find_prev_error(&self) -> Option<usize> {
        let errors = self.filtered_error_indices();
        if errors.is_empty() {
            return None;
        }

        let current_pos = self.current_log_position();

        // Find last error before current position
        for &error_idx in errors.iter().rev() {
            if error_idx < current_pos {
                return Some(error_idx);
            }
        }

        // Wrap around to last error
        errors.last().copied()
    }

    /// Get the current log position based on scroll offset
    /// Accounts for filtering
    fn current_log_position(&self) -> usize {
        if self.filter_state.is_active() {
            // Map filtered offset to original index
            let filtered = self.filtered_log_indices();
            filtered
                .get(self.log_view_state.offset)
                .copied()
                .unwrap_or(0)
        } else {
            self.log_view_state.offset
        }
    }
}

/// Handle for controlling a session's Flutter process
pub struct SessionHandle {
    /// The session state
    pub session: Session,

    /// The Flutter process (if running)
    pub process: Option<FlutterProcess>,

    /// Command sender for this session
    pub cmd_sender: Option<CommandSender>,

    /// Request tracker for response matching
    pub request_tracker: Arc<RequestTracker>,
}

impl std::fmt::Debug for SessionHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionHandle")
            .field("session", &self.session)
            .field("has_process", &self.process.is_some())
            .field("has_cmd_sender", &self.cmd_sender.is_some())
            .finish()
    }
}

impl SessionHandle {
    /// Create a new session handle
    pub fn new(session: Session) -> Self {
        Self {
            session,
            process: None,
            cmd_sender: None,
            request_tracker: Arc::new(RequestTracker::default()),
        }
    }

    /// Attach a Flutter process to this session
    pub fn attach_process(&mut self, process: FlutterProcess) {
        let sender = process.command_sender(self.request_tracker.clone());
        self.cmd_sender = Some(sender);
        self.process = Some(process);
        self.session.phase = AppPhase::Initializing;
    }

    /// Check if process is running
    pub fn has_process(&self) -> bool {
        self.process.is_some()
    }

    /// Get the app_id if available
    pub fn app_id(&self) -> Option<&str> {
        self.session.app_id.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_creation() {
        let session = Session::new(
            "device-123".to_string(),
            "iPhone 15 Pro".to_string(),
            "ios".to_string(),
            true,
        );

        assert_eq!(session.device_id, "device-123");
        assert_eq!(session.device_name, "iPhone 15 Pro");
        assert_eq!(session.name, "iPhone 15 Pro");
        assert!(session.is_emulator);
        assert_eq!(session.phase, AppPhase::Initializing);
        assert!(session.logs.is_empty());
    }

    #[test]
    fn test_session_id_uniqueness() {
        let s1 = Session::new("a".into(), "A".into(), "ios".into(), false);
        let s2 = Session::new("b".into(), "B".into(), "ios".into(), false);
        let s3 = Session::new("c".into(), "C".into(), "ios".into(), false);

        assert_ne!(s1.id, s2.id);
        assert_ne!(s2.id, s3.id);
        assert_ne!(s1.id, s3.id);
    }

    #[test]
    fn test_session_logging() {
        let mut session = Session::new("d".into(), "Device".into(), "android".into(), false);

        session.log_info(LogSource::App, "Test message");
        session.log_error(LogSource::Daemon, "Error message");

        assert_eq!(session.logs.len(), 2);
    }

    #[test]
    fn test_session_log_trimming() {
        let mut session = Session::new("d".into(), "Device".into(), "ios".into(), false);
        session.max_logs = 5;

        for i in 0..10 {
            session.log_info(LogSource::App, format!("Message {}", i));
        }

        assert_eq!(session.logs.len(), 5);
        // Should have messages 5-9
        assert!(session.logs[0].message.contains('5'));
        assert!(session.logs[4].message.contains('9'));
    }

    #[test]
    fn test_session_lifecycle() {
        let mut session = Session::new("d".into(), "Device".into(), "ios".into(), false);

        assert_eq!(session.phase, AppPhase::Initializing);
        assert!(session.app_id.is_none());

        session.mark_started("app-123".to_string());
        assert_eq!(session.phase, AppPhase::Running);
        assert_eq!(session.app_id, Some("app-123".to_string()));
        assert!(session.started_at.is_some());

        session.start_reload();
        assert_eq!(session.phase, AppPhase::Reloading);
        assert!(session.reload_start_time.is_some());

        session.complete_reload();
        assert_eq!(session.phase, AppPhase::Running);
        assert_eq!(session.reload_count, 1);
        assert!(session.last_reload_time.is_some());

        session.mark_stopped();
        assert_eq!(session.phase, AppPhase::Stopped);
    }

    #[test]
    fn test_session_status_icons() {
        let mut session = Session::new("d".into(), "Device".into(), "ios".into(), false);

        assert_eq!(session.status_icon(), "○"); // Initializing

        session.phase = AppPhase::Running;
        assert_eq!(session.status_icon(), "●");

        session.phase = AppPhase::Reloading;
        assert_eq!(session.status_icon(), "↻");

        session.phase = AppPhase::Stopped;
        assert_eq!(session.status_icon(), "○");
    }

    #[test]
    fn test_tab_title_truncation() {
        let short = Session::new("d".into(), "iPhone".into(), "ios".into(), false);
        assert_eq!(short.tab_title(), "○ iPhone");

        let long = Session::new(
            "d".into(),
            "Very Long Device Name Here".into(),
            "ios".into(),
            false,
        );
        assert!(long.tab_title().contains('…'));
        // Use chars().count() for character count, not byte length
        assert!(long.tab_title().chars().count() < 20);
    }

    #[test]
    fn test_session_with_config() {
        let session = Session::new("d".into(), "Device".into(), "ios".into(), false);
        let config = LaunchConfig {
            name: "My Config".to_string(),
            ..Default::default()
        };

        let session = session.with_config(config);
        assert_eq!(session.name, "My Config");
        assert!(session.launch_config.is_some());
    }

    #[test]
    fn test_is_running() {
        let mut session = Session::new("d".into(), "Device".into(), "ios".into(), false);

        assert!(!session.is_running()); // Initializing

        session.phase = AppPhase::Running;
        assert!(session.is_running());

        session.phase = AppPhase::Reloading;
        assert!(session.is_running());

        session.phase = AppPhase::Stopped;
        assert!(!session.is_running());
    }

    #[test]
    fn test_is_busy() {
        let mut session = Session::new("d".into(), "Device".into(), "ios".into(), false);

        assert!(!session.is_busy());

        session.phase = AppPhase::Reloading;
        assert!(session.is_busy());

        session.phase = AppPhase::Running;
        assert!(!session.is_busy());
    }

    #[test]
    fn test_clear_logs() {
        let mut session = Session::new("d".into(), "Device".into(), "ios".into(), false);
        session.log_info(LogSource::App, "Test");
        session.log_view_state.offset = 5;

        session.clear_logs();

        assert!(session.logs.is_empty());
        assert_eq!(session.log_view_state.offset, 0);
    }

    #[test]
    fn test_session_handle_creation() {
        let session = Session::new("d".into(), "Device".into(), "ios".into(), false);
        let handle = SessionHandle::new(session);

        assert!(!handle.has_process());
        assert!(handle.app_id().is_none());
    }

    // ─────────────────────────────────────────────────────────
    // Filter & Search Tests
    // ─────────────────────────────────────────────────────────

    #[test]
    fn test_session_has_filter_state() {
        use crate::core::{LogLevelFilter, LogSourceFilter};

        let session = Session::new("device".into(), "Device".into(), "ios".into(), false);
        assert_eq!(session.filter_state.level_filter, LogLevelFilter::All);
        assert_eq!(session.filter_state.source_filter, LogSourceFilter::All);
    }

    #[test]
    fn test_session_has_search_state() {
        let session = Session::new("device".into(), "Device".into(), "ios".into(), false);
        assert!(session.search_state.query.is_empty());
        assert!(!session.search_state.is_active);
    }

    #[test]
    fn test_session_cycle_level_filter() {
        use crate::core::LogLevelFilter;

        let mut session = Session::new("device".into(), "Device".into(), "ios".into(), false);
        assert_eq!(session.filter_state.level_filter, LogLevelFilter::All);

        session.cycle_level_filter();
        assert_eq!(session.filter_state.level_filter, LogLevelFilter::Errors);

        session.cycle_level_filter();
        assert_eq!(session.filter_state.level_filter, LogLevelFilter::Warnings);
    }

    #[test]
    fn test_session_cycle_source_filter() {
        use crate::core::LogSourceFilter;

        let mut session = Session::new("device".into(), "Device".into(), "ios".into(), false);
        assert_eq!(session.filter_state.source_filter, LogSourceFilter::All);

        session.cycle_source_filter();
        assert_eq!(session.filter_state.source_filter, LogSourceFilter::App);

        session.cycle_source_filter();
        assert_eq!(session.filter_state.source_filter, LogSourceFilter::Daemon);
    }

    #[test]
    fn test_session_reset_filters() {
        let mut session = Session::new("device".into(), "Device".into(), "ios".into(), false);
        session.cycle_level_filter();
        session.cycle_source_filter();
        assert!(session.has_active_filter());

        session.reset_filters();
        assert!(!session.has_active_filter());
    }

    #[test]
    fn test_session_filtered_log_indices() {
        use crate::core::LogLevelFilter;

        let mut session = Session::new("device".into(), "Device".into(), "ios".into(), false);
        session.log_info(LogSource::App, "info message");
        session.log_error(LogSource::App, "error message");
        session.log_info(LogSource::Flutter, "flutter info");

        // No filter - all logs
        let indices = session.filtered_log_indices();
        assert_eq!(indices.len(), 3);
        assert_eq!(indices, vec![0, 1, 2]);

        // Errors only
        session.filter_state.level_filter = LogLevelFilter::Errors;
        let indices = session.filtered_log_indices();
        assert_eq!(indices.len(), 1);
        assert_eq!(indices[0], 1); // The error message
    }

    #[test]
    fn test_session_search_mode() {
        let mut session = Session::new("device".into(), "Device".into(), "ios".into(), false);
        assert!(!session.is_searching());

        session.start_search();
        assert!(session.is_searching());

        session.cancel_search();
        assert!(!session.is_searching());
    }

    #[test]
    fn test_session_set_search_query() {
        let mut session = Session::new("device".into(), "Device".into(), "ios".into(), false);

        session.set_search_query("error");
        assert_eq!(session.search_state.query, "error");
        assert!(session.search_state.is_valid);
    }

    #[test]
    fn test_session_clear_search() {
        let mut session = Session::new("device".into(), "Device".into(), "ios".into(), false);
        session.set_search_query("test");
        session.start_search();

        session.clear_search();

        assert!(session.search_state.query.is_empty());
        assert!(!session.search_state.is_active);
    }

    #[test]
    fn test_session_clear_logs_clears_search() {
        use crate::core::SearchMatch;

        let mut session = Session::new("device".into(), "Device".into(), "ios".into(), false);
        session.log_info(LogSource::App, "test");
        session
            .search_state
            .update_matches(vec![SearchMatch::new(0, 0, 4)]);
        session.search_state.current_match = Some(0);

        session.clear_logs();

        assert!(session.search_state.matches.is_empty());
        assert!(session.search_state.current_match.is_none());
    }

    // ─────────────────────────────────────────────────────────
    // Error Navigation Tests (Task 7)
    // ─────────────────────────────────────────────────────────

    fn create_session_with_logs() -> Session {
        let mut session = Session::new("device".into(), "Device".into(), "ios".into(), false);
        session.log_info(LogSource::App, "info 0"); // index 0
        session.log_error(LogSource::App, "error 1"); // index 1
        session.log_info(LogSource::App, "info 2"); // index 2
        session.log_error(LogSource::App, "error 3"); // index 3
        session.log_info(LogSource::App, "info 4"); // index 4
        session.log_error(LogSource::App, "error 5"); // index 5
        session
    }

    #[test]
    fn test_error_indices() {
        let session = create_session_with_logs();
        let errors = session.error_indices();
        assert_eq!(errors, vec![1, 3, 5]);
    }

    #[test]
    fn test_error_count() {
        let session = create_session_with_logs();
        assert_eq!(session.error_count(), 3);
    }

    #[test]
    fn test_find_next_error_from_start() {
        let session = create_session_with_logs();
        // Scroll offset 0, should find first error at index 1
        let next = session.find_next_error();
        assert_eq!(next, Some(1));
    }

    #[test]
    fn test_find_next_error_wraps() {
        let mut session = create_session_with_logs();
        session.log_view_state.offset = 5; // After last error

        let next = session.find_next_error();
        assert_eq!(next, Some(1)); // Wraps to first error
    }

    #[test]
    fn test_find_prev_error_from_end() {
        let mut session = create_session_with_logs();
        session.log_view_state.offset = 5;

        let prev = session.find_prev_error();
        assert_eq!(prev, Some(3)); // Error before position 5
    }

    #[test]
    fn test_find_prev_error_wraps() {
        let mut session = create_session_with_logs();
        session.log_view_state.offset = 0; // Before first error

        let prev = session.find_prev_error();
        assert_eq!(prev, Some(5)); // Wraps to last error
    }

    #[test]
    fn test_find_error_no_errors() {
        let mut session = Session::new("device".into(), "Device".into(), "ios".into(), false);
        session.log_info(LogSource::App, "info only");

        assert_eq!(session.find_next_error(), None);
        assert_eq!(session.find_prev_error(), None);
    }

    #[test]
    fn test_find_error_respects_filter() {
        use crate::core::LogSourceFilter;

        let mut session = create_session_with_logs();

        // Filter to App source only (all errors are from App, so all visible)
        session.filter_state.source_filter = LogSourceFilter::App;
        let errors = session.filtered_error_indices();
        assert_eq!(errors.len(), 3);

        // Filter to Daemon source (no errors)
        session.filter_state.source_filter = LogSourceFilter::Daemon;
        let errors = session.filtered_error_indices();
        assert!(errors.is_empty());
    }

    #[test]
    fn test_find_next_error_from_middle() {
        let mut session = create_session_with_logs();
        session.log_view_state.offset = 2; // Between first and second error

        let next = session.find_next_error();
        assert_eq!(next, Some(3)); // Next error after position 2
    }

    #[test]
    fn test_find_prev_error_from_middle() {
        let mut session = create_session_with_logs();
        session.log_view_state.offset = 4; // Between second and third error

        let prev = session.find_prev_error();
        assert_eq!(prev, Some(3)); // Previous error before position 4
    }

    #[test]
    fn test_error_count_empty() {
        let session = Session::new("device".into(), "Device".into(), "ios".into(), false);
        assert_eq!(session.error_count(), 0);
    }
}
