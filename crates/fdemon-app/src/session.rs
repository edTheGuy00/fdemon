//! Per-instance session state for a running Flutter app

use std::collections::{HashSet, VecDeque};
use std::sync::Arc;
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
use fdemon_daemon::{CommandSender, FlutterProcess, RequestTracker};

// ─────────────────────────────────────────────────────────
// Log Batching for Performance (Task 04)
// ─────────────────────────────────────────────────────────

/// Default batch flush interval (~60fps)
const BATCH_FLUSH_INTERVAL: Duration = Duration::from_millis(16);

/// Maximum batch size before forced flush
const BATCH_MAX_SIZE: usize = 100;

/// Batches rapid log arrivals to reduce processing overhead
///
/// During high-volume logging (hot reload, verbose debugging, etc.),
/// each log line would normally trigger processing and potentially
/// a UI re-render. This struct batches logs and flushes them
/// at a controlled rate (~60fps) or when a size threshold is reached.
#[derive(Debug)]
pub struct LogBatcher {
    /// Pending log entries awaiting flush
    pending: Vec<LogEntry>,
    /// Timestamp of last flush
    last_flush: Instant,
}

impl Default for LogBatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl LogBatcher {
    /// Create a new log batcher
    pub fn new() -> Self {
        Self {
            pending: Vec::with_capacity(BATCH_MAX_SIZE),
            last_flush: Instant::now(),
        }
    }

    /// Add a log entry to the batch
    ///
    /// Returns true if the batch should be flushed (size or time threshold reached)
    pub fn add(&mut self, entry: LogEntry) -> bool {
        self.pending.push(entry);
        self.should_flush()
    }

    /// Check if batch should be flushed
    ///
    /// Returns true if:
    /// - Batch has reached max size (100 entries), OR
    /// - Time since last flush has exceeded interval (16ms)
    pub fn should_flush(&self) -> bool {
        self.pending.len() >= BATCH_MAX_SIZE
            || (!self.pending.is_empty() && self.last_flush.elapsed() >= BATCH_FLUSH_INTERVAL)
    }

    /// Flush and return pending entries
    ///
    /// Resets the flush timer and returns all pending entries.
    pub fn flush(&mut self) -> Vec<LogEntry> {
        self.last_flush = Instant::now();
        std::mem::take(&mut self.pending)
    }

    /// Check if there are pending entries
    pub fn has_pending(&self) -> bool {
        !self.pending.is_empty()
    }

    /// Get count of pending entries
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    /// Time until next scheduled flush (for event loop timing)
    pub fn time_until_flush(&self) -> Duration {
        BATCH_FLUSH_INTERVAL.saturating_sub(self.last_flush.elapsed())
    }
}

// ─────────────────────────────────────────────────────────
// Stateful Block Tracking for Logger Package Blocks
// ─────────────────────────────────────────────────────────

/// Tracks state for Logger package block detection
///
/// Instead of backward-scanning on every block end (O(N*M)), this struct
/// tracks block state incrementally as lines arrive (O(1) per line).
#[derive(Debug, Clone)]
pub struct LogBlockState {
    /// Index where current block started (if any)
    block_start: Option<usize>,
    /// Highest severity seen in current block
    block_max_level: LogLevel,
}

impl Default for LogBlockState {
    fn default() -> Self {
        Self {
            block_start: None,
            block_max_level: LogLevel::Info,
        }
    }
}

// ─────────────────────────────────────────────────────────
// Collapse State for Stack Traces (Phase 2 Task 6)
// ─────────────────────────────────────────────────────────

/// Tracks which log entries have expanded/collapsed stack traces
#[derive(Debug, Clone, Default)]
pub struct CollapseState {
    /// Set of log entry IDs that are currently expanded
    /// (by default, entries are collapsed based on config)
    expanded_entries: HashSet<u64>,

    /// Set of log entry IDs that are explicitly collapsed
    /// (overrides default when default is expanded)
    collapsed_entries: HashSet<u64>,
}

impl CollapseState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if an entry's stack trace should be shown expanded
    pub fn is_expanded(&self, entry_id: u64, default_collapsed: bool) -> bool {
        if default_collapsed {
            // Default is collapsed, check if user expanded it
            self.expanded_entries.contains(&entry_id)
        } else {
            // Default is expanded, check if user collapsed it
            !self.collapsed_entries.contains(&entry_id)
        }
    }

    /// Toggle the collapse state of an entry
    pub fn toggle(&mut self, entry_id: u64, default_collapsed: bool) {
        if default_collapsed {
            if self.expanded_entries.contains(&entry_id) {
                self.expanded_entries.remove(&entry_id);
            } else {
                self.expanded_entries.insert(entry_id);
            }
        } else if self.collapsed_entries.contains(&entry_id) {
            self.collapsed_entries.remove(&entry_id);
        } else {
            self.collapsed_entries.insert(entry_id);
        }
    }

    /// Collapse all stack traces
    pub fn collapse_all(&mut self) {
        self.expanded_entries.clear();
        self.collapsed_entries.clear(); // Let default take over
    }

    /// Expand all stack traces for the given entry IDs
    pub fn expand_all(&mut self, entry_ids: impl Iterator<Item = u64>) {
        self.collapsed_entries.clear();
        self.expanded_entries.extend(entry_ids);
    }
}

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
    block_state: LogBlockState,

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
    error_count: usize,

    // ─────────────────────────────────────────────────────────
    // Log Batching (Task 04)
    // ─────────────────────────────────────────────────────────
    /// Log batcher for coalescing rapid log arrivals
    log_batcher: LogBatcher,
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
            launch_config: None,
            created_at: Local::now(),
            started_at: None,
            reload_start_time: None,
            last_reload_time: None,
            reload_count: 0,
            error_count: 0,
            log_batcher: LogBatcher::new(),
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
        use fdemon_core::{LogLevelFilter, LogSourceFilter};

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
        use fdemon_core::LogLevelFilter;

        let mut session = Session::new("device".into(), "Device".into(), "ios".into(), false);
        assert_eq!(session.filter_state.level_filter, LogLevelFilter::All);

        session.cycle_level_filter();
        assert_eq!(session.filter_state.level_filter, LogLevelFilter::Errors);

        session.cycle_level_filter();
        assert_eq!(session.filter_state.level_filter, LogLevelFilter::Warnings);
    }

    #[test]
    fn test_session_cycle_source_filter() {
        use fdemon_core::LogSourceFilter;

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
        use fdemon_core::LogLevelFilter;

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
        use fdemon_core::SearchMatch;

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
        use fdemon_core::LogSourceFilter;

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

    // ─────────────────────────────────────────────────────────
    // Collapse State Tests (Phase 2 Task 6)
    // ─────────────────────────────────────────────────────────

    #[test]
    fn test_collapse_state_default() {
        let state = CollapseState::new();

        // With default collapsed=true, entries should show as collapsed
        assert!(!state.is_expanded(1, true));

        // With default collapsed=false, entries should show as expanded
        assert!(state.is_expanded(1, false));
    }

    #[test]
    fn test_collapse_state_toggle() {
        let mut state = CollapseState::new();

        // Toggle from collapsed (default) to expanded
        state.toggle(42, true);
        assert!(state.is_expanded(42, true));

        // Toggle back to collapsed
        state.toggle(42, true);
        assert!(!state.is_expanded(42, true));
    }

    #[test]
    fn test_collapse_state_toggle_default_expanded() {
        let mut state = CollapseState::new();

        // With default_collapsed=false, entries start expanded
        assert!(state.is_expanded(42, false));

        // Toggle to collapsed
        state.toggle(42, false);
        assert!(!state.is_expanded(42, false));

        // Toggle back to expanded
        state.toggle(42, false);
        assert!(state.is_expanded(42, false));
    }

    #[test]
    fn test_collapse_state_multiple_entries() {
        let mut state = CollapseState::new();

        state.toggle(1, true); // Expand entry 1
        state.toggle(3, true); // Expand entry 3

        assert!(state.is_expanded(1, true));
        assert!(!state.is_expanded(2, true)); // Not toggled
        assert!(state.is_expanded(3, true));
    }

    #[test]
    fn test_collapse_all() {
        let mut state = CollapseState::new();

        state.toggle(1, true);
        state.toggle(2, true);
        state.toggle(3, true);

        state.collapse_all();

        assert!(!state.is_expanded(1, true));
        assert!(!state.is_expanded(2, true));
        assert!(!state.is_expanded(3, true));
    }

    #[test]
    fn test_expand_all() {
        let mut state = CollapseState::new();

        // With default collapsed, expand all should mark entries as expanded
        state.expand_all([1, 2, 3].into_iter());

        assert!(state.is_expanded(1, true));
        assert!(state.is_expanded(2, true));
        assert!(state.is_expanded(3, true));
    }

    #[test]
    fn test_session_has_collapse_state() {
        let session = Session::new("device".into(), "Device".into(), "ios".into(), false);
        assert!(!session.collapse_state.is_expanded(1, true));
    }

    #[test]
    fn test_session_toggle_stack_trace() {
        let mut session = Session::new("device".into(), "Device".into(), "ios".into(), false);

        // Toggle stack trace for entry ID 42
        session.toggle_stack_trace(42, true);
        assert!(session.is_stack_trace_expanded(42, true));

        // Toggle again
        session.toggle_stack_trace(42, true);
        assert!(!session.is_stack_trace_expanded(42, true));
    }

    // ─────────────────────────────────────────────────────────
    // Cached Error Count Tests (Phase 2 Task 7)
    // ─────────────────────────────────────────────────────────

    #[test]
    fn test_error_count_increments_on_error() {
        let mut session = Session::new("device".into(), "Device".into(), "ios".into(), false);

        session.add_log(LogEntry::info(LogSource::App, "info message"));
        assert_eq!(session.error_count(), 0);

        session.add_log(LogEntry::error(LogSource::App, "error 1"));
        assert_eq!(session.error_count(), 1);

        session.add_log(LogEntry::error(LogSource::App, "error 2"));
        assert_eq!(session.error_count(), 2);

        // Warnings don't count as errors
        session.add_log(LogEntry::warn(LogSource::App, "warning"));
        assert_eq!(session.error_count(), 2);
    }

    #[test]
    fn test_error_count_resets_on_clear() {
        let mut session = Session::new("device".into(), "Device".into(), "ios".into(), false);

        session.add_log(LogEntry::error(LogSource::App, "error 1"));
        session.add_log(LogEntry::error(LogSource::App, "error 2"));
        assert_eq!(session.error_count(), 2);

        session.clear_logs();
        assert_eq!(session.error_count(), 0);
    }

    #[test]
    fn test_error_count_adjusts_on_log_trim() {
        let mut session = Session::new("device".into(), "Device".into(), "ios".into(), false);
        session.max_logs = 5;

        // Add 3 errors at the start
        session.add_log(LogEntry::error(LogSource::App, "error 0"));
        session.add_log(LogEntry::error(LogSource::App, "error 1"));
        session.add_log(LogEntry::error(LogSource::App, "error 2"));
        session.add_log(LogEntry::info(LogSource::App, "info 3"));
        session.add_log(LogEntry::info(LogSource::App, "info 4"));
        assert_eq!(session.error_count(), 3);
        assert_eq!(session.logs.len(), 5);

        // Add 2 more non-error logs, which should trim the first 2 errors
        session.add_log(LogEntry::info(LogSource::App, "info 5"));
        session.add_log(LogEntry::info(LogSource::App, "info 6"));
        assert_eq!(session.logs.len(), 5);
        // First 2 errors trimmed, 1 error remains
        assert_eq!(session.error_count(), 1);
    }

    #[test]
    fn test_recalculate_error_count() {
        let mut session = Session::new("device".into(), "Device".into(), "ios".into(), false);

        session.add_log(LogEntry::error(LogSource::App, "error 1"));
        session.add_log(LogEntry::error(LogSource::App, "error 2"));
        session.add_log(LogEntry::info(LogSource::App, "info"));

        // Manually set wrong count (simulating a bug scenario)
        session.error_count = 999;
        assert_eq!(session.error_count(), 999);

        // Recalculate should fix it
        session.recalculate_error_count();
        assert_eq!(session.error_count(), 2);
    }

    #[test]
    fn test_error_count_with_log_helpers() {
        let mut session = Session::new("device".into(), "Device".into(), "ios".into(), false);

        session.log_info(LogSource::App, "info");
        assert_eq!(session.error_count(), 0);

        session.log_error(LogSource::App, "error");
        assert_eq!(session.error_count(), 1);
    }

    #[test]
    fn test_error_count_matches_actual_errors() {
        let mut session = Session::new("device".into(), "Device".into(), "ios".into(), false);

        // Add various log types
        session.add_log(LogEntry::info(LogSource::App, "info"));
        session.add_log(LogEntry::error(LogSource::Flutter, "flutter error"));
        session.add_log(LogEntry::warn(LogSource::Daemon, "warning"));
        session.add_log(LogEntry::error(
            LogSource::FlutterError,
            "flutter stderr error",
        ));
        session.add_log(LogEntry::new(LogLevel::Debug, LogSource::Watcher, "debug"));

        // Cached count should match actual count
        let actual_errors = session.logs.iter().filter(|e| e.is_error()).count();
        assert_eq!(session.error_count(), actual_errors);
        assert_eq!(session.error_count(), 2);
    }

    // ─────────────────────────────────────────────────────────
    // Logger Block Level Propagation Tests (Phase 2 Task 11)
    // ─────────────────────────────────────────────────────────

    #[test]
    fn test_error_block_propagation() {
        let mut session = Session::new("device".into(), "Device".into(), "ios".into(), false);

        // Simulate Logger error block - only one line has error level
        session.add_log(LogEntry::info(
            LogSource::Flutter,
            "┌───────────────────────",
        ));
        session.add_log(LogEntry::error(LogSource::Flutter, "│ ⛔ Error: failed"));
        session.add_log(LogEntry::info(LogSource::Flutter, "│ #0 stack trace line"));
        session.add_log(LogEntry::info(LogSource::Flutter, "│ #1 more stack trace"));
        session.add_log(LogEntry::info(
            LogSource::Flutter,
            "└───────────────────────",
        ));

        // All lines should now be Error level
        assert!(
            session.logs.iter().all(|e| e.level == LogLevel::Error),
            "All block lines should be Error level after propagation"
        );
    }

    #[test]
    fn test_warning_block_propagation() {
        let mut session = Session::new("device".into(), "Device".into(), "ios".into(), false);

        // Simulate Logger warning block
        session.add_log(LogEntry::info(
            LogSource::Flutter,
            "┌───────────────────────",
        ));
        session.add_log(LogEntry::warn(
            LogSource::Flutter,
            "│ ⚠ Warning: deprecated",
        ));
        session.add_log(LogEntry::info(LogSource::Flutter, "│ Additional info"));
        session.add_log(LogEntry::info(
            LogSource::Flutter,
            "└───────────────────────",
        ));

        // All lines should now be Warning level
        assert!(
            session.logs.iter().all(|e| e.level == LogLevel::Warning),
            "All block lines should be Warning level after propagation"
        );
    }

    #[test]
    fn test_non_block_lines_unchanged() {
        let mut session = Session::new("device".into(), "Device".into(), "ios".into(), false);

        // Regular logs (not Logger blocks)
        session.add_log(LogEntry::info(LogSource::Flutter, "Regular info"));
        session.add_log(LogEntry::error(LogSource::Flutter, "Standalone error"));
        session.add_log(LogEntry::info(LogSource::Flutter, "Another info"));

        // Levels should remain as originally set
        assert_eq!(session.logs[0].level, LogLevel::Info);
        assert_eq!(session.logs[1].level, LogLevel::Error);
        assert_eq!(session.logs[2].level, LogLevel::Info);
    }

    #[test]
    fn test_block_propagation_error_count() {
        let mut session = Session::new("device".into(), "Device".into(), "ios".into(), false);

        // Before block: 0 errors
        assert_eq!(session.error_count(), 0);

        // Add error block - only one line marked error initially
        session.add_log(LogEntry::info(
            LogSource::Flutter,
            "┌───────────────────────",
        ));
        session.add_log(LogEntry::error(LogSource::Flutter, "│ ⛔ Error: failed"));
        session.add_log(LogEntry::info(LogSource::Flutter, "│ Stack trace"));
        session.add_log(LogEntry::info(
            LogSource::Flutter,
            "└───────────────────────",
        ));

        // After propagation: 4 errors (all lines promoted to Error)
        assert_eq!(session.error_count(), 4);
    }

    #[test]
    fn test_info_only_block_not_propagated() {
        let mut session = Session::new("device".into(), "Device".into(), "ios".into(), false);

        // Logger block with only Info level (e.g., debug output)
        session.add_log(LogEntry::info(
            LogSource::Flutter,
            "┌───────────────────────",
        ));
        session.add_log(LogEntry::info(LogSource::Flutter, "│ 💡 Info: message"));
        session.add_log(LogEntry::info(LogSource::Flutter, "│ Some details"));
        session.add_log(LogEntry::info(
            LogSource::Flutter,
            "└───────────────────────",
        ));

        // All lines should stay Info (no propagation needed)
        assert!(session.logs.iter().all(|e| e.level == LogLevel::Info));
        assert_eq!(session.error_count(), 0);
    }

    #[test]
    fn test_incomplete_block_not_propagated() {
        let mut session = Session::new("device".into(), "Device".into(), "ios".into(), false);

        // Block without ending (e.g., truncated output)
        session.add_log(LogEntry::info(
            LogSource::Flutter,
            "┌───────────────────────",
        ));
        session.add_log(LogEntry::error(LogSource::Flutter, "│ ⛔ Error: failed"));
        session.add_log(LogEntry::info(LogSource::Flutter, "│ Stack trace"));
        // No closing └

        // Error propagation shouldn't happen (block not complete)
        assert_eq!(session.logs[0].level, LogLevel::Info); // Block start still Info
        assert_eq!(session.logs[1].level, LogLevel::Error); // Error line
        assert_eq!(session.logs[2].level, LogLevel::Info); // Stack trace still Info
    }

    #[test]
    fn test_block_end_without_start_not_propagated() {
        let mut session = Session::new("device".into(), "Device".into(), "ios".into(), false);

        // Block end without matching start (orphaned end)
        session.add_log(LogEntry::info(LogSource::Flutter, "│ Some content"));
        session.add_log(LogEntry::error(LogSource::Flutter, "│ ⛔ Error: failed"));
        session.add_log(LogEntry::info(
            LogSource::Flutter,
            "└───────────────────────",
        ));

        // Should not propagate (scan will hit 50-line limit without finding start)
        // Actually, with only 3 lines it won't hit limit, but no ┌ means block_start == block_end
        assert_eq!(session.logs[0].level, LogLevel::Info);
        assert_eq!(session.logs[1].level, LogLevel::Error);
        assert_eq!(session.logs[2].level, LogLevel::Info); // The └ line stays Info
    }

    #[test]
    fn test_multiple_blocks_independent() {
        let mut session = Session::new("device".into(), "Device".into(), "ios".into(), false);

        // First block - warning
        session.add_log(LogEntry::info(
            LogSource::Flutter,
            "┌───────────────────────",
        ));
        session.add_log(LogEntry::warn(LogSource::Flutter, "│ ⚠ Warning"));
        session.add_log(LogEntry::info(
            LogSource::Flutter,
            "└───────────────────────",
        ));

        // Second block - error
        session.add_log(LogEntry::info(
            LogSource::Flutter,
            "┌───────────────────────",
        ));
        session.add_log(LogEntry::error(LogSource::Flutter, "│ ⛔ Error"));
        session.add_log(LogEntry::info(
            LogSource::Flutter,
            "└───────────────────────",
        ));

        // First block should be Warning
        assert_eq!(session.logs[0].level, LogLevel::Warning);
        assert_eq!(session.logs[1].level, LogLevel::Warning);
        assert_eq!(session.logs[2].level, LogLevel::Warning);

        // Second block should be Error
        assert_eq!(session.logs[3].level, LogLevel::Error);
        assert_eq!(session.logs[4].level, LogLevel::Error);
        assert_eq!(session.logs[5].level, LogLevel::Error);
    }

    #[test]
    fn test_mixed_content_between_blocks() {
        let mut session = Session::new("device".into(), "Device".into(), "ios".into(), false);

        // Regular log
        session.add_log(LogEntry::info(LogSource::Flutter, "Regular message"));

        // Block
        session.add_log(LogEntry::info(
            LogSource::Flutter,
            "┌───────────────────────",
        ));
        session.add_log(LogEntry::error(LogSource::Flutter, "│ ⛔ Error"));
        session.add_log(LogEntry::info(
            LogSource::Flutter,
            "└───────────────────────",
        ));

        // Another regular log
        session.add_log(LogEntry::info(LogSource::Flutter, "Another regular"));

        // Regular logs should stay Info
        assert_eq!(session.logs[0].level, LogLevel::Info);
        assert_eq!(session.logs[4].level, LogLevel::Info);

        // Block should be Error
        assert_eq!(session.logs[1].level, LogLevel::Error);
        assert_eq!(session.logs[2].level, LogLevel::Error);
        assert_eq!(session.logs[3].level, LogLevel::Error);
    }

    #[test]
    fn test_block_with_leading_whitespace() {
        let mut session = Session::new("device".into(), "Device".into(), "ios".into(), false);

        // Block with leading whitespace (common in Flutter output)
        session.add_log(LogEntry::info(
            LogSource::Flutter,
            "   ┌───────────────────────",
        ));
        session.add_log(LogEntry::error(LogSource::Flutter, "   │ ⛔ Error"));
        session.add_log(LogEntry::info(
            LogSource::Flutter,
            "   └───────────────────────",
        ));

        // Should still propagate correctly
        assert!(session.logs.iter().all(|e| e.level == LogLevel::Error));
    }

    // ─────────────────────────────────────────────────────────
    // Stateful Block Tracking Tests (Bug Fix: Logger Block Propagation)
    // ─────────────────────────────────────────────────────────

    #[test]
    fn test_stateful_empty_block_handled() {
        let mut session = Session::new("device".into(), "Device".into(), "ios".into(), false);

        // Empty block (┌ immediately followed by └)
        session.add_log(LogEntry::info(
            LogSource::Flutter,
            "┌───────────────────────",
        ));
        session.add_log(LogEntry::info(
            LogSource::Flutter,
            "└───────────────────────",
        ));

        // Both lines should remain Info (no errors to propagate)
        assert_eq!(session.logs.len(), 2);
        assert!(session.logs.iter().all(|e| e.level == LogLevel::Info));
    }

    #[test]
    fn test_stateful_back_to_back_blocks() {
        let mut session = Session::new("device".into(), "Device".into(), "ios".into(), false);

        // First block (error)
        session.add_log(LogEntry::info(
            LogSource::Flutter,
            "┌───────────────────────",
        ));
        session.add_log(LogEntry::error(LogSource::Flutter, "│ ⛔ Error"));
        session.add_log(LogEntry::info(
            LogSource::Flutter,
            "└───────────────────────",
        ));

        // Second block (warning) - immediately after first
        session.add_log(LogEntry::info(
            LogSource::Flutter,
            "┌───────────────────────",
        ));
        session.add_log(LogEntry::warn(LogSource::Flutter, "│ ⚠ Warning"));
        session.add_log(LogEntry::info(
            LogSource::Flutter,
            "└───────────────────────",
        ));

        // Third block (info only) - immediately after second
        session.add_log(LogEntry::info(
            LogSource::Flutter,
            "┌───────────────────────",
        ));
        session.add_log(LogEntry::info(LogSource::Flutter, "│ 💡 Info"));
        session.add_log(LogEntry::info(
            LogSource::Flutter,
            "└───────────────────────",
        ));

        // First block should be Error
        assert_eq!(session.logs[0].level, LogLevel::Error);
        assert_eq!(session.logs[1].level, LogLevel::Error);
        assert_eq!(session.logs[2].level, LogLevel::Error);

        // Second block should be Warning
        assert_eq!(session.logs[3].level, LogLevel::Warning);
        assert_eq!(session.logs[4].level, LogLevel::Warning);
        assert_eq!(session.logs[5].level, LogLevel::Warning);

        // Third block should remain Info (no promotion needed)
        assert_eq!(session.logs[6].level, LogLevel::Info);
        assert_eq!(session.logs[7].level, LogLevel::Info);
        assert_eq!(session.logs[8].level, LogLevel::Info);
    }

    #[test]
    fn test_stateful_block_start_trimmed_during_rotation() {
        let mut session = Session::new("device".into(), "Device".into(), "ios".into(), false);
        session.max_logs = 3;

        // Start a block
        session.add_log(LogEntry::info(
            LogSource::Flutter,
            "┌───────────────────────",
        ));
        // logs = ["┌"], block_start = Some(0)

        // Add content (within limit)
        session.add_log(LogEntry::info(LogSource::Flutter, "│ Content line 1"));
        // logs = ["┌", "│ Content 1"], block_start = Some(0)

        session.add_log(LogEntry::info(LogSource::Flutter, "│ Content line 2"));
        // logs = ["┌", "│ Content 1", "│ Content 2"], block_start = Some(0)

        // This will trigger trim, removing block start!
        session.add_log(LogEntry::error(LogSource::Flutter, "│ ⛔ Error"));
        // Before trim: logs = ["┌", "│1", "│2", "│⛔"], block_start = Some(0)
        // After trim (remove 1): logs = ["│1", "│2", "│⛔"]
        // block_start was 0, which is < drain_count (1), so block_state is reset!

        // End the block - but start was trimmed, so no propagation should happen
        session.add_log(LogEntry::info(
            LogSource::Flutter,
            "└───────────────────────",
        ));

        // Block state should have been reset when block_start was trimmed
        // Only the error line should be Error level (no propagation occurred)
        let error_count = session
            .logs
            .iter()
            .filter(|e| e.level == LogLevel::Error)
            .count();
        assert_eq!(
            error_count, 1,
            "Only the explicit error line should be Error level after block_start was trimmed"
        );

        // Verify the block state was reset
        assert!(
            session.block_state.block_start.is_none(),
            "Block state should be reset"
        );
    }

    #[test]
    fn test_stateful_large_block_no_50_line_limit() {
        let mut session = Session::new("device".into(), "Device".into(), "ios".into(), false);

        // Start a block
        session.add_log(LogEntry::info(
            LogSource::Flutter,
            "┌───────────────────────",
        ));

        // Add 100 lines in the block (would exceed old 50-line scan limit)
        for i in 0..100 {
            session.add_log(LogEntry::info(LogSource::Flutter, format!("│ Line {}", i)));
        }

        // Add an error in the middle (but we're past the 50-line mark)
        session.add_log(LogEntry::error(
            LogSource::Flutter,
            "│ ⛔ Error at line 101",
        ));

        // More content
        for i in 0..10 {
            session.add_log(LogEntry::info(
                LogSource::Flutter,
                format!("│ Line {}", 102 + i),
            ));
        }

        // End the block
        session.add_log(LogEntry::info(
            LogSource::Flutter,
            "└───────────────────────",
        ));

        // With stateful tracking, ALL lines should be promoted to Error
        // (old implementation would fail after 50 lines)
        assert!(
            session.logs.iter().all(|e| e.level == LogLevel::Error),
            "All {} lines should be Error level with stateful tracking",
            session.logs.len()
        );
    }

    #[test]
    fn test_stateful_block_state_reset_after_complete() {
        let mut session = Session::new("device".into(), "Device".into(), "ios".into(), false);

        // Complete block
        session.add_log(LogEntry::info(
            LogSource::Flutter,
            "┌───────────────────────",
        ));
        session.add_log(LogEntry::error(LogSource::Flutter, "│ ⛔ Error"));
        session.add_log(LogEntry::info(
            LogSource::Flutter,
            "└───────────────────────",
        ));

        // Block state should be reset
        assert!(session.block_state.block_start.is_none());
        assert_eq!(session.block_state.block_max_level, LogLevel::Info);

        // Next entry should not be affected by previous block state
        session.add_log(LogEntry::info(LogSource::Flutter, "Plain message"));
        assert_eq!(session.logs[3].level, LogLevel::Info);
    }

    // ─────────────────────────────────────────────────────────
    // Log Batching Tests (Task 04)
    // ─────────────────────────────────────────────────────────

    #[test]
    fn test_log_batcher_new() {
        let batcher = LogBatcher::new();
        assert!(!batcher.has_pending());
        assert_eq!(batcher.pending_count(), 0);
        assert!(!batcher.should_flush()); // Empty batch shouldn't flush
    }

    #[test]
    fn test_log_batcher_add_single() {
        let mut batcher = LogBatcher::new();
        let entry = LogEntry::info(LogSource::App, "Test message");

        let should_flush = batcher.add(entry);

        assert!(batcher.has_pending());
        assert_eq!(batcher.pending_count(), 1);
        // Single entry shouldn't trigger flush (unless time elapsed)
        assert!(!should_flush || batcher.pending_count() >= BATCH_MAX_SIZE);
    }

    #[test]
    fn test_log_batcher_size_threshold() {
        let mut batcher = LogBatcher::new();

        // Add entries up to max size - 1
        for i in 0..(BATCH_MAX_SIZE - 1) {
            let entry = LogEntry::info(LogSource::App, format!("Log {}", i));
            let should_flush = batcher.add(entry);
            assert!(!should_flush, "Should not flush before max size");
        }

        assert_eq!(batcher.pending_count(), BATCH_MAX_SIZE - 1);

        // This entry should trigger flush due to size
        let entry = LogEntry::info(LogSource::App, "Final log");
        let should_flush = batcher.add(entry);
        assert!(should_flush, "Should flush at max size");
        assert_eq!(batcher.pending_count(), BATCH_MAX_SIZE);
    }

    #[test]
    fn test_log_batcher_flush() {
        let mut batcher = LogBatcher::new();

        batcher.add(LogEntry::info(LogSource::App, "Log 1"));
        batcher.add(LogEntry::error(LogSource::Flutter, "Log 2"));
        batcher.add(LogEntry::warn(LogSource::Daemon, "Log 3"));

        assert_eq!(batcher.pending_count(), 3);

        let entries = batcher.flush();

        assert_eq!(entries.len(), 3);
        assert!(!batcher.has_pending());
        assert_eq!(batcher.pending_count(), 0);

        // Verify entry contents
        assert_eq!(entries[0].level, LogLevel::Info);
        assert_eq!(entries[1].level, LogLevel::Error);
        assert_eq!(entries[2].level, LogLevel::Warning);
    }

    #[test]
    fn test_log_batcher_time_until_flush() {
        let batcher = LogBatcher::new();

        // Just created - should have nearly full interval remaining
        let time_remaining = batcher.time_until_flush();
        assert!(time_remaining <= BATCH_FLUSH_INTERVAL);
    }

    #[test]
    fn test_session_queue_and_flush() {
        let mut session = Session::new("device".into(), "Device".into(), "ios".into(), false);

        // Queue some logs
        session.queue_log(LogEntry::info(LogSource::App, "Queued 1"));
        session.queue_log(LogEntry::info(LogSource::App, "Queued 2"));
        session.queue_log(LogEntry::info(LogSource::App, "Queued 3"));

        assert!(session.has_pending_logs());
        assert_eq!(session.logs.len(), 0); // Not yet flushed to main log buffer

        // Flush the batch
        let flushed_count = session.flush_batched_logs();

        assert_eq!(flushed_count, 3);
        assert!(!session.has_pending_logs());
        assert_eq!(session.logs.len(), 3); // Now in main log buffer
    }

    #[test]
    fn test_session_add_logs_batch() {
        let mut session = Session::new("device".into(), "Device".into(), "ios".into(), false);

        let entries = vec![
            LogEntry::info(LogSource::App, "Batch 1"),
            LogEntry::error(LogSource::App, "Batch 2"),
            LogEntry::warn(LogSource::App, "Batch 3"),
        ];

        session.add_logs_batch(entries);

        assert_eq!(session.logs.len(), 3);
        assert_eq!(session.error_count(), 1);
    }

    #[test]
    fn test_session_batched_block_propagation() {
        // Verify block propagation works correctly with batched logs
        let mut session = Session::new("device".into(), "Device".into(), "ios".into(), false);

        // Queue a complete block
        session.queue_log(LogEntry::info(
            LogSource::Flutter,
            "┌───────────────────────",
        ));
        session.queue_log(LogEntry::error(LogSource::Flutter, "│ ⛔ Error"));
        session.queue_log(LogEntry::info(LogSource::Flutter, "│ More content"));
        session.queue_log(LogEntry::info(
            LogSource::Flutter,
            "└───────────────────────",
        ));

        // Flush the batch
        session.flush_batched_logs();

        // All lines should be promoted to Error level
        assert!(
            session.logs.iter().all(|e| e.level == LogLevel::Error),
            "Block propagation should work with batched logs"
        );
    }

    #[test]
    fn test_session_batched_error_count() {
        let mut session = Session::new("device".into(), "Device".into(), "ios".into(), false);

        // Queue logs with errors
        session.queue_log(LogEntry::info(LogSource::App, "Info"));
        session.queue_log(LogEntry::error(LogSource::App, "Error 1"));
        session.queue_log(LogEntry::warn(LogSource::App, "Warning"));
        session.queue_log(LogEntry::error(LogSource::App, "Error 2"));

        // Before flush - error count should be 0
        assert_eq!(session.error_count(), 0);

        session.flush_batched_logs();

        // After flush - error count should reflect actual errors
        assert_eq!(session.error_count(), 2);
    }

    #[test]
    fn test_log_batcher_empty_flush() {
        let mut batcher = LogBatcher::new();

        // Flush empty batcher
        let entries = batcher.flush();

        assert!(entries.is_empty());
        assert!(!batcher.has_pending());
    }

    #[test]
    fn test_session_queue_auto_flush_on_size() {
        let mut session = Session::new("device".into(), "Device".into(), "ios".into(), false);

        // Queue many logs - should auto-flush when we check
        for i in 0..150 {
            let should_flush =
                session.queue_log(LogEntry::info(LogSource::App, format!("Log {}", i)));
            if should_flush {
                session.flush_batched_logs();
            }
        }

        // All logs should have been flushed to main buffer
        // (100 flushed at threshold, 50 remaining may or may not be flushed)
        assert!(session.logs.len() >= 100);
    }

    // ─────────────────────────────────────────────────────────
    // Virtualized Log Access Tests (Task 05)
    // ─────────────────────────────────────────────────────────

    #[test]
    fn test_get_logs_range_basic() {
        let mut session = Session::new("device".into(), "Device".into(), "ios".into(), false);

        for i in 0..10 {
            session.add_log(LogEntry::info(LogSource::App, format!("Log {}", i)));
        }

        let range: Vec<_> = session.get_logs_range(2, 5).collect();

        assert_eq!(range.len(), 3);
        assert!(range[0].message.contains("Log 2"));
        assert!(range[1].message.contains("Log 3"));
        assert!(range[2].message.contains("Log 4"));
    }

    #[test]
    fn test_get_logs_range_start_at_zero() {
        let mut session = Session::new("device".into(), "Device".into(), "ios".into(), false);

        for i in 0..5 {
            session.add_log(LogEntry::info(LogSource::App, format!("Log {}", i)));
        }

        let range: Vec<_> = session.get_logs_range(0, 3).collect();

        assert_eq!(range.len(), 3);
        assert!(range[0].message.contains("Log 0"));
    }

    #[test]
    fn test_get_logs_range_to_end() {
        let mut session = Session::new("device".into(), "Device".into(), "ios".into(), false);

        for i in 0..5 {
            session.add_log(LogEntry::info(LogSource::App, format!("Log {}", i)));
        }

        let range: Vec<_> = session.get_logs_range(3, 10).collect();

        // End is clamped to len
        assert_eq!(range.len(), 2);
        assert!(range[0].message.contains("Log 3"));
        assert!(range[1].message.contains("Log 4"));
    }

    #[test]
    fn test_get_logs_range_out_of_bounds() {
        let mut session = Session::new("device".into(), "Device".into(), "ios".into(), false);

        for i in 0..5 {
            session.add_log(LogEntry::info(LogSource::App, format!("Log {}", i)));
        }

        let range: Vec<_> = session.get_logs_range(10, 20).collect();

        // Both out of bounds, should be empty
        assert!(range.is_empty());
    }

    #[test]
    fn test_get_logs_range_empty_session() {
        let session = Session::new("device".into(), "Device".into(), "ios".into(), false);

        let range: Vec<_> = session.get_logs_range(0, 10).collect();

        assert!(range.is_empty());
    }

    #[test]
    fn test_get_logs_range_inverted_bounds() {
        let mut session = Session::new("device".into(), "Device".into(), "ios".into(), false);

        for i in 0..5 {
            session.add_log(LogEntry::info(LogSource::App, format!("Log {}", i)));
        }

        // Start > end (after clamping)
        let range: Vec<_> = session.get_logs_range(10, 5).collect();

        // Should handle gracefully
        assert!(range.is_empty());
    }

    #[test]
    fn test_log_count() {
        let mut session = Session::new("device".into(), "Device".into(), "ios".into(), false);

        assert_eq!(session.log_count(), 0);

        for i in 0..5 {
            session.add_log(LogEntry::info(LogSource::App, format!("Log {}", i)));
        }

        assert_eq!(session.log_count(), 5);
    }

    #[test]
    fn test_get_logs_range_full_range() {
        let mut session = Session::new("device".into(), "Device".into(), "ios".into(), false);

        for i in 0..10 {
            session.add_log(LogEntry::info(LogSource::App, format!("Log {}", i)));
        }

        let range: Vec<_> = session.get_logs_range(0, 10).collect();

        assert_eq!(range.len(), 10);
    }

    // ─────────────────────────────────────────────────────────
    // Exception Block Processing Tests (Phase 1 Task 02)
    // ─────────────────────────────────────────────────────────

    fn create_test_session() -> Session {
        Session::new(
            "device-test".to_string(),
            "Test Device".to_string(),
            "ios".to_string(),
            false,
        )
    }

    #[test]
    fn test_process_raw_line_normal() {
        let mut session = create_test_session();

        let entries = session.process_raw_line("flutter: Hello World");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].level, LogLevel::Info);
        assert_eq!(entries[0].message, "Hello World"); // "flutter: " stripped
    }

    #[test]
    fn test_process_raw_line_exception_buffered() {
        let mut session = create_test_session();

        let entries =
            session.process_raw_line("══╡ EXCEPTION CAUGHT BY WIDGETS LIBRARY ╞═══════════");
        assert!(entries.is_empty()); // buffered, not emitted yet
    }

    #[test]
    fn test_process_raw_line_exception_complete() {
        let mut session = create_test_session();

        // Feed exception block
        assert!(session
            .process_raw_line("══╡ EXCEPTION CAUGHT BY WIDGETS LIBRARY ╞═══════════")
            .is_empty());
        assert!(session.process_raw_line("Error description").is_empty());
        assert!(session
            .process_raw_line("#0      main (package:app/main.dart:15:3)")
            .is_empty());

        // Footer completes the block
        let entries =
            session.process_raw_line("════════════════════════════════════════════════════════");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].level, LogLevel::Error);
        assert!(entries[0].stack_trace.is_some());
    }

    #[test]
    fn test_process_raw_line_another_exception() {
        let mut session = create_test_session();

        let entries = session
            .process_raw_line("Another exception was thrown: RangeError (index): Invalid value");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].level, LogLevel::Error);
    }

    #[test]
    fn test_flush_exception_buffer_on_exit() {
        let mut session = create_test_session();

        // Start an exception block but don't finish it
        session.process_raw_line("══╡ EXCEPTION CAUGHT BY WIDGETS LIBRARY ╞═══════════");
        session.process_raw_line("Error description");

        // Flush should return partial block
        let entry = session.flush_exception_buffer();
        assert!(entry.is_some());
        assert_eq!(entry.unwrap().level, LogLevel::Error);
    }

    #[test]
    fn test_flush_exception_buffer_empty() {
        let mut session = create_test_session();

        // No pending exception
        let entry = session.flush_exception_buffer();
        assert!(entry.is_none());
    }

    #[test]
    fn test_normal_lines_after_exception() {
        let mut session = create_test_session();

        // Complete an exception block
        session.process_raw_line("══╡ EXCEPTION CAUGHT BY WIDGETS LIBRARY ╞═══════════");
        session.process_raw_line("Error");
        session.process_raw_line("════════════════════════════════════════════════════════");

        // Normal lines should work after
        let entries = session.process_raw_line("Normal log message");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].level, LogLevel::Info);
    }

    #[test]
    fn test_process_raw_line_with_ansi_codes() {
        let mut session = create_test_session();

        // ANSI codes should be stripped before processing
        let entries = session.process_raw_line("\x1b[38;5;244mflutter: Test message\x1b[0m");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].message, "Test message");
        assert!(!entries[0].message.contains('\x1b')); // No ANSI codes in message
    }

    #[test]
    fn test_process_raw_line_empty_after_strip() {
        let mut session = create_test_session();

        // Empty lines should return empty vec
        let entries = session.process_raw_line("");
        assert!(entries.is_empty());

        // Whitespace-only lines
        let entries = session.process_raw_line("   ");
        assert!(entries.is_empty());
    }
}
