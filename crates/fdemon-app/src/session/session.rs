//! Per-device session state — logs, filters, search, and lifecycle.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use chrono::{DateTime, Local};

use crate::config::LaunchConfig;
use crate::handler::helpers::{detect_raw_line_level, is_block_end, is_block_start};
use crate::hyperlinks::LinkHighlightState;
use crate::log_view_state::LogViewState;
use fdemon_core::{
    strip_ansi_codes, AppPhase, ExceptionBlockParser, FeedResult, FilterState, LogEntry, LogLevel,
    LogSource, SearchState,
};

use super::block_state::LogBlockState;
use super::collapse::CollapseState;
use super::log_batcher::LogBatcher;
use super::next_session_id;
use super::performance::PerformanceState;

/// A single Flutter app session
#[derive(Debug)]
pub struct Session {
    /// Unique session identifier
    pub id: super::SessionId,

    /// Display name for this session (device name or config name)
    pub name: String,

    /// Current phase of this session
    pub phase: AppPhase,

    /// Log buffer for this session
    /// Log entries stored in a ring buffer for bounded memory usage
    pub logs: VecDeque<LogEntry>,

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

    /// Collapse state for stack traces
    pub collapse_state: CollapseState,

    /// Link highlight mode state (Phase 3.1)
    pub link_highlight_state: LinkHighlightState,

    /// Block state for Logger package block level propagation
    pub(super) block_state: LogBlockState,

    /// Exception block parser for multi-line Flutter exception detection
    exception_parser: ExceptionBlockParser,

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

    /// VM Service WebSocket URI (from app.debugPort event)
    pub ws_uri: Option<String>,

    /// Whether the VM Service WebSocket is currently connected
    pub vm_connected: bool,

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

    /// Cached count of error-level log entries (for status bar display)
    pub(super) error_count: usize,

    // ─────────────────────────────────────────────────────────
    // Log Batching (Task 04)
    // ─────────────────────────────────────────────────────────
    /// Log batcher for coalescing rapid log arrivals
    log_batcher: LogBatcher,

    // ─────────────────────────────────────────────────────────
    // Performance Monitoring (Phase 3, Task 05)
    // ─────────────────────────────────────────────────────────
    /// Performance monitoring state (memory, GC, frames).
    pub performance: PerformanceState,
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
            logs: VecDeque::with_capacity(10_000),
            log_view_state: LogViewState::new(),
            max_logs: 10_000,
            filter_state: FilterState::default(),
            search_state: SearchState::default(),
            collapse_state: CollapseState::new(),
            link_highlight_state: LinkHighlightState::new(),
            block_state: LogBlockState::default(),
            exception_parser: ExceptionBlockParser::new(),
            device_id,
            device_name,
            platform,
            is_emulator,
            app_id: None,
            ws_uri: None,
            vm_connected: false,
            launch_config: None,
            created_at: Local::now(),
            started_at: None,
            reload_start_time: None,
            last_reload_time: None,
            reload_count: 0,
            error_count: 0,
            log_batcher: LogBatcher::new(),
            performance: PerformanceState::default(),
        }
    }

    /// Create session with a launch configuration
    pub fn with_config(mut self, config: LaunchConfig) -> Self {
        self.name = config.name.clone();
        self.launch_config = Some(config);
        self
    }

    /// Add a log entry
    ///
    /// Automatically detects Logger package blocks (from ┌ to └) and propagates
    /// the highest severity level found in the block to all lines within it.
    ///
    /// Uses incremental state tracking (O(1) per line) instead of backward
    /// scanning (O(N*M)) for block level propagation.
    pub fn add_log(&mut self, entry: LogEntry) {
        let idx = self.logs.len();

        // Check for block boundaries BEFORE pushing
        let is_start = is_block_start(&entry.message);
        let is_end = is_block_end(&entry.message);

        // Track block state as we go
        if is_start {
            // New block starting - record position and initialize max level
            self.block_state.block_start = Some(idx);
            self.block_state.block_max_level = entry.level;
        } else if self.block_state.block_start.is_some() {
            // Inside a block - update max level if this entry is more severe
            self.block_state.block_max_level =
                self.block_state.block_max_level.max_severity(entry.level);
        }

        // Track error count before adding
        if entry.is_error() {
            self.error_count += 1;
        }

        // Push the entry to the back of the ring buffer
        self.logs.push_back(entry);

        // Block ended - apply max level to all block lines
        if is_end && self.block_state.block_start.is_some() {
            let start = self.block_state.block_start.take().unwrap();
            let max_level = self.block_state.block_max_level;

            // Only propagate if we found something more severe than Info
            if max_level.is_more_severe_than(&LogLevel::Info) {
                // Track error count changes
                let mut error_delta: i32 = 0;

                for i in start..=idx {
                    let old_level = self.logs[i].level;
                    if old_level != max_level {
                        // Update error counts
                        if old_level == LogLevel::Error {
                            error_delta -= 1;
                        }
                        if max_level == LogLevel::Error {
                            error_delta += 1;
                        }
                        self.logs[i].level = max_level;
                    }
                }

                // Apply error count delta
                if error_delta > 0 {
                    self.error_count += error_delta as usize;
                } else if error_delta < 0 {
                    self.error_count = self.error_count.saturating_sub((-error_delta) as usize);
                }
            }

            // Reset block state for next block
            self.block_state = LogBlockState::default();
        }

        // Trim oldest entries if over max size (ring buffer behavior)
        while self.logs.len() > self.max_logs {
            if let Some(evicted) = self.logs.pop_front() {
                // Update error count if evicting an error
                if evicted.is_error() {
                    self.error_count = self.error_count.saturating_sub(1);
                }
            }

            // Adjust block_start index since we removed from front
            if let Some(start) = self.block_state.block_start {
                if start == 0 {
                    // Block start is being evicted - cancel block tracking
                    self.block_state = LogBlockState::default();
                } else {
                    // Shift block start index down
                    self.block_state.block_start = Some(start - 1);
                }
            }

            // Adjust scroll offset
            self.log_view_state.offset = self.log_view_state.offset.saturating_sub(1);
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

    /// Clear all logs and reset error count
    pub fn clear_logs(&mut self) {
        self.logs.clear();
        self.log_view_state.offset = 0;
        self.error_count = 0;
        // Clear search matches since logs are gone
        self.search_state.matches.clear();
        self.search_state.current_match = None;
    }

    // ─────────────────────────────────────────────────────────
    // Log Batching Methods (Task 04)
    // ─────────────────────────────────────────────────────────

    /// Queue a log entry for batched processing
    ///
    /// Instead of immediately processing the log, this adds it to a batch
    /// that will be flushed when the time or size threshold is reached.
    /// Returns true if the batch should be flushed now.
    ///
    /// Use `flush_batched_logs()` to process the pending batch.
    pub fn queue_log(&mut self, entry: LogEntry) -> bool {
        self.log_batcher.add(entry)
    }

    /// Check if there are pending batched logs
    pub fn has_pending_logs(&self) -> bool {
        self.log_batcher.has_pending()
    }

    /// Check if batched logs should be flushed
    pub fn should_flush_logs(&self) -> bool {
        self.log_batcher.should_flush()
    }

    /// Flush pending batched logs
    ///
    /// Processes all pending log entries through the normal add_log path,
    /// which handles block-level propagation and ring buffer management.
    /// Returns the number of logs that were flushed.
    pub fn flush_batched_logs(&mut self) -> usize {
        let entries = self.log_batcher.flush();
        let count = entries.len();
        for entry in entries {
            self.add_log(entry);
        }
        count
    }

    /// Add multiple log entries at once (batch insertion)
    ///
    /// Each entry is processed through add_log to ensure proper
    /// block-level propagation and ring buffer management.
    pub fn add_logs_batch(&mut self, entries: Vec<LogEntry>) {
        for entry in entries {
            self.add_log(entry);
        }
    }

    /// Get time until next scheduled batch flush
    ///
    /// Useful for event loop timing to know when to check for pending logs.
    pub fn time_until_batch_flush(&self) -> Duration {
        self.log_batcher.time_until_flush()
    }

    // ─────────────────────────────────────────────────────────
    // Exception Block Processing (Phase 1 Task 02)
    // ─────────────────────────────────────────────────────────

    /// Process a raw line (from stderr or non-JSON stdout) through exception detection.
    ///
    /// Returns zero or more LogEntry items to be queued:
    /// - If the line is part of an exception block: returns empty (buffered)
    /// - If the line completes an exception block: returns the exception LogEntry
    /// - If the line is not part of an exception: returns a normal LogEntry
    /// - If the line is a "Another exception was thrown:" one-liner: returns an Error entry
    pub fn process_raw_line(&mut self, line: &str) -> Vec<LogEntry> {
        match self.exception_parser.feed_line(line) {
            FeedResult::Buffered => {
                // Line consumed by exception parser, nothing to emit yet
                vec![]
            }
            FeedResult::Complete(block) => {
                // Exception block complete — convert to LogEntry with stack trace
                vec![block.to_log_entry()]
            }
            FeedResult::OneLineException(message) => {
                // "Another exception was thrown: ..." one-liner
                vec![LogEntry::error(LogSource::Flutter, message)]
            }
            FeedResult::NotConsumed => {
                // Normal line — use existing level detection
                let cleaned = strip_ansi_codes(line);
                let (level, message) = detect_raw_line_level(&cleaned);
                if message.is_empty() {
                    vec![]
                } else {
                    vec![LogEntry::new(level, LogSource::Flutter, message)]
                }
            }
        }
    }

    /// Process a log line through exception detection, using provided fallback
    /// for non-exception lines. Used for app.log events that already have
    /// level/source from the daemon protocol.
    pub fn process_log_line_with_fallback(
        &mut self,
        line: &str,
        fallback_level: LogLevel,
        fallback_source: LogSource,
        fallback_message: String,
    ) -> Vec<LogEntry> {
        match self.exception_parser.feed_line(line) {
            FeedResult::Buffered => vec![],
            FeedResult::Complete(block) => vec![block.to_log_entry()],
            FeedResult::OneLineException(msg) => {
                vec![LogEntry::error(LogSource::Flutter, msg)]
            }
            FeedResult::NotConsumed => {
                vec![LogEntry::new(
                    fallback_level,
                    fallback_source,
                    fallback_message,
                )]
            }
        }
    }

    /// Flush any pending exception buffer (e.g., on session exit).
    ///
    /// Returns a LogEntry if there was a partial exception block being accumulated.
    pub fn flush_exception_buffer(&mut self) -> Option<LogEntry> {
        self.exception_parser
            .flush()
            .map(|block| block.to_log_entry())
    }

    // ─────────────────────────────────────────────────────────
    // Virtualized Log Access (Task 05)
    // ─────────────────────────────────────────────────────────

    /// Get logs in a specific range for virtualized rendering
    ///
    /// Returns an iterator over log entries in the specified range.
    /// Bounds are clamped to the valid range [0, len).
    pub fn get_logs_range(&self, start: usize, end: usize) -> impl Iterator<Item = &LogEntry> + '_ {
        let end = end.min(self.logs.len());
        let start = start.min(end);
        self.logs.range(start..end)
    }

    /// Get total number of log entries
    pub fn log_count(&self) -> usize {
        self.logs.len()
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
        // Char-aware truncation to avoid panic on multi-byte UTF-8 (e.g. Chinese device names)
        let name = if self.name.chars().count() > 15 {
            format!("{}…", self.name.chars().take(14).collect::<String>())
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

    /// Get the current error count (cached for performance)
    pub fn error_count(&self) -> usize {
        self.error_count
    }

    /// Recalculate error count from logs (for consistency/debugging)
    pub fn recalculate_error_count(&mut self) {
        self.error_count = self.logs.iter().filter(|e| e.is_error()).count();
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

    // ─────────────────────────────────────────────────────────
    // Stack Trace Collapse Methods (Phase 2 Task 6)
    // ─────────────────────────────────────────────────────────

    /// Get the currently focused log entry (at scroll position)
    pub fn focused_entry(&self) -> Option<&LogEntry> {
        let pos = self.current_log_position();
        self.logs.get(pos)
    }

    /// Get the focused entry's ID
    pub fn focused_entry_id(&self) -> Option<u64> {
        self.focused_entry().map(|e| e.id)
    }

    /// Toggle stack trace collapse for a specific entry
    pub fn toggle_stack_trace(&mut self, entry_id: u64, default_collapsed: bool) {
        self.collapse_state.toggle(entry_id, default_collapsed);
    }

    /// Check if a specific entry's stack trace should be shown expanded
    pub fn is_stack_trace_expanded(&self, entry_id: u64, default_collapsed: bool) -> bool {
        self.collapse_state.is_expanded(entry_id, default_collapsed)
    }
}
