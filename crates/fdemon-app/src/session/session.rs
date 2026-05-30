//! Per-device session state — logs, filters, search, and lifecycle.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use chrono::{DateTime, Local};

use crate::config::LaunchConfig;
use crate::handler::helpers::{detect_raw_line_level, is_block_end, is_block_start};
use crate::hyperlinks::LinkHighlightState;
use crate::log_view_state::LogViewState;
use fdemon_core::url::percent_encode_uri;
use fdemon_core::{
    strip_ansi_codes, AppPhase, ExceptionBlockParser, FeedResult, FilterState, LogEntry, LogLevel,
    LogSource, SearchState,
};

use super::block_state::LogBlockState;
use super::collapse::CollapseState;
use super::debug_state::DebugState;
use super::log_batcher::LogBatcher;
use super::memory::MemoryState;
use super::network::NetworkState;
use super::next_session_id;
use super::performance::PerformanceState;

// ─────────────────────────────────────────────────────────────────────────────
// DevTools Endpoint (browser DevTools integration)
// ─────────────────────────────────────────────────────────────────────────────

/// The DevTools server endpoint associated with a Flutter session.
///
/// Populated from the `app.devTools` event that the Flutter daemon emits
/// automatically during `flutter run --machine` startup (Flutter ≥ 1.22.0),
/// or from the `devtools.serve` RPC response as a fallback.
///
/// The `base_url` is the raw DevTools server URL with NO query parameters.
/// Two formats exist:
/// - Standalone DevTools (older Flutter): `http://127.0.0.1:9100`
/// - DDS-integrated DevTools (Flutter ≥ 3.24): `http://127.0.0.1:59123/<auth-token>/devtools`
#[derive(Debug, Clone)]
pub struct DevToolsEndpoint {
    /// Base DevTools server URL without trailing `?uri=` parameter.
    pub base_url: String,
}

impl DevToolsEndpoint {
    /// Construct the full browser URL by appending `?uri=<encoded_ws_uri>`.
    ///
    /// The `ws_uri` is the VM Service WebSocket URI (e.g.
    /// `ws://127.0.0.1:1234/abc=/ws`). It is percent-encoded and appended as
    /// the `uri` query parameter per the Flutter DevTools convention.
    ///
    /// # Example
    ///
    /// ```
    /// # use fdemon_app::session::DevToolsEndpoint;
    /// let ep = DevToolsEndpoint {
    ///     base_url: "http://127.0.0.1:9100".into(),
    /// };
    /// let url = ep.url("ws://127.0.0.1:1234/abc=/ws");
    /// assert_eq!(url, "http://127.0.0.1:9100?uri=ws%3A%2F%2F127.0.0.1%3A1234%2Fabc%3D%2Fws");
    /// ```
    pub fn url(&self, ws_uri: &str) -> String {
        let encoded = percent_encode_uri(ws_uri);
        format!("{}?uri={}", self.base_url, encoded)
    }
}

/// A single Flutter app session
#[derive(Debug)]
pub struct Session {
    /// Unique session identifier
    pub id: super::SessionId,

    /// Display name for this session (device name or config name)
    pub name: String,

    /// Current phase of this session
    pub phase: AppPhase,

    /// Latest human-readable launch progress line (Flutter `app.progress`
    /// build messages, or pre-app source readiness updates). `None` once
    /// the app is running or when there is nothing in flight.
    pub current_progress: Option<String>,

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

    /// DevTools server endpoint (populated from `app.devTools` event or
    /// `devtools.serve` RPC response). `None` until the daemon reports that
    /// DevTools is ready.
    ///
    /// Use [`DevToolsEndpoint::url`] to obtain the full browser URL with the
    /// VM Service URI query parameter appended.
    pub devtools_endpoint: Option<DevToolsEndpoint>,

    /// True between sending a `ServeDevTools` command and receiving the
    /// corresponding response. Used to debounce duplicate serve requests when
    /// the eager-serve path fires before the `app.devTools` event arrives.
    pub devtools_serve_pending: bool,

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
    /// Performance monitoring state (frame timing, stats).
    pub performance: PerformanceState,

    /// Memory monitoring state (heap snapshots, GC events, allocation profile).
    pub memory: MemoryState,

    // ─────────────────────────────────────────────────────────
    // Network Monitoring (Phase 4, Task 03)
    // ─────────────────────────────────────────────────────────
    /// Network monitoring state (HTTP profile, sockets).
    pub network: NetworkState,

    // ─────────────────────────────────────────────────────────
    // Debug Adapter Protocol state (DAP feature, Phase 1, Task 04)
    // ─────────────────────────────────────────────────────────
    /// Per-session debug state (pause status, breakpoints, exception mode).
    pub debug: DebugState,

    // ─────────────────────────────────────────────────────────
    // Jump-to-latest indicator (Phase 4, Task 01)
    // ─────────────────────────────────────────────────────────
    /// Count of log entries appended while the view was scrolled away from the
    /// tail (i.e., `log_view_state.auto_scroll == false`). Advisory only — used
    /// by the log view to render a "↓ N new · G to jump" indicator. Reset to
    /// zero whenever auto-scroll re-engages via `mark_tail_followed()`.
    ///
    /// Ring-buffer eviction does not decrement this counter: evicted entries
    /// are old (front), unseen entries are new (back). The two are independent.
    ///
    /// Filter-gated: only entries that pass `filter_state.matches(&entry)` at
    /// the time of insertion are counted. This ensures the pill number matches
    /// what the user actually sees when they jump to the tail. Note: changing
    /// the filter while scrolled away does **not** retroactively recompute this
    /// counter — the value reflects the filter state at insertion time only.
    pub unseen_log_count: usize,
}

/// Duration of the post-reload success flash in milliseconds. The header tint
/// fades from full intensity to none over this window.
const RELOAD_FLASH_DURATION_MS: i64 = 500;

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
            current_progress: None,
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
            devtools_endpoint: None,
            devtools_serve_pending: false,
            launch_config: None,
            created_at: Local::now(),
            started_at: None,
            reload_start_time: None,
            last_reload_time: None,
            reload_count: 0,
            error_count: 0,
            log_batcher: LogBatcher::new(),
            performance: PerformanceState::default(),
            memory: MemoryState::default(),
            network: NetworkState::default(),
            debug: DebugState::default(),
            unseen_log_count: 0,
        }
    }

    /// Create session with a launch configuration
    pub fn with_config(mut self, config: LaunchConfig) -> Self {
        self.name = config.name.clone();
        self.launch_config = Some(config);
        self
    }

    /// Apply network configuration from DevTools settings.
    ///
    /// Sets `max_entries` and initial `recording` state on the session's
    /// `NetworkState`. Call this after `Session::new()` when you have access
    /// to [`crate::config::DevToolsSettings`].
    pub fn with_network_config(mut self, max_entries: usize, auto_record: bool) -> Self {
        self.network = NetworkState::with_config(max_entries, auto_record);
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

        // Evaluate filter match BEFORE pushing (entry not yet moved).
        // Used later to gate the unseen_log_count increment.
        let passes_filter = self.filter_state.matches(&entry);

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

        // Track unseen logs for the jump-to-latest indicator (issue #31).
        // Only count entries that are (a) arriving while scrolled away from the tail
        // AND (b) visible under the active filter — so the pill matches what `G` reveals.
        // Ring-buffer eviction is intentionally independent of this counter.
        if !self.log_view_state.auto_scroll && passes_filter {
            self.unseen_log_count = self.unseen_log_count.saturating_add(1);
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

    /// Reset the unseen log counter, called when the view re-engages tail-follow
    /// (either via `Message::ScrollToBottom` or by scrolling down to the natural
    /// bottom). Idempotent — safe to call when `unseen_log_count` is already 0
    /// or when `auto_scroll` is already true.
    pub fn mark_tail_followed(&mut self) {
        self.unseen_log_count = 0;
    }

    /// Clear all logs and reset error count
    pub fn clear_logs(&mut self) {
        self.logs.clear();
        self.log_view_state.offset = 0;
        self.error_count = 0;
        // M2: no unseen entries remain after a wipe
        self.unseen_log_count = 0;
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

    /// Mark the session as launching: the `app.start` daemon event captured the
    /// app id, but the app is still building/starting. Phase flips to `Running`
    /// only when [`mark_running`](Self::mark_running) is called from the
    /// `app.started` event.
    ///
    /// Note: `started_at` is intentionally set here (at `app.start` / Launching)
    /// rather than at `app.started` / Running, so `session_duration` counts from
    /// the beginning of the launch, not from the first frame.
    pub fn mark_started(&mut self, app_id: String) {
        self.app_id = Some(app_id);
        self.started_at = Some(Local::now());
        self.phase = AppPhase::Launching;
    }

    /// Mark the session as actually running (the `app.started` daemon event).
    /// Clears any in-flight build/readiness progress text.
    pub fn mark_running(&mut self) {
        self.phase = AppPhase::Running;
        self.current_progress = None;
    }

    /// Mark session as stopped. Clears any in-flight progress text.
    pub fn mark_stopped(&mut self) {
        self.phase = AppPhase::Stopped;
        self.current_progress = None;
    }

    /// Set the current launch progress line (shown next to a transient phase label).
    pub fn set_progress(&mut self, message: impl Into<String>) {
        self.current_progress = Some(message.into());
    }

    /// Clear the current launch progress line.
    pub fn clear_progress(&mut self) {
        self.current_progress = None;
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

    /// Intensity of the reload-success flash at wall-clock `now`, in `[0.0, 1.0]`.
    ///
    /// Returns `1.0` at the instant of `complete_reload()` and decays linearly to
    /// `0.0` over [`RELOAD_FLASH_DURATION_MS`], staying `0.0` afterwards.
    /// Returns `0.0` when the session never reloaded or is not in a steady
    /// `Running` phase (so the flash never bleeds into `Stopped`/`Quitting`/error
    /// states — a failed reload leaves the phase at `Running` and does not stamp
    /// `last_reload_time`, so only successful reloads can trigger it).
    ///
    /// `now` is injected (rather than read internally) to keep the helper pure and
    /// unit-testable; the render path passes `Local::now()`.
    pub fn reload_flash_alpha(&self, now: DateTime<Local>) -> f32 {
        if self.phase != AppPhase::Running {
            return 0.0;
        }
        let Some(reloaded_at) = self.last_reload_time else {
            return 0.0;
        };
        let elapsed_ms = (now - reloaded_at).num_milliseconds();
        if !(0..RELOAD_FLASH_DURATION_MS).contains(&elapsed_ms) {
            return 0.0; // future timestamp (clock skew) or window elapsed
        }
        1.0 - (elapsed_ms as f32 / RELOAD_FLASH_DURATION_MS as f32)
    }

    /// Check if session is running
    pub fn is_running(&self) -> bool {
        matches!(self.phase, AppPhase::Running | AppPhase::Reloading)
    }

    /// Check if session is in a busy state (reload/restart in progress)
    pub fn is_busy(&self) -> bool {
        matches!(self.phase, AppPhase::Reloading)
    }

    /// Check if session is actively in use (not stopped/quitting).
    ///
    /// Unlike `is_running()` which only matches `Running | Reloading`,
    /// this also includes `Initializing`, `Preparing`, and `Launching` —
    /// phases where the session is alive but the app is not yet interactive.
    pub fn is_active(&self) -> bool {
        !matches!(self.phase, AppPhase::Stopped | AppPhase::Quitting)
    }

    /// Get status indicator character
    pub fn status_icon(&self) -> &'static str {
        match self.phase {
            AppPhase::Initializing => "○",
            AppPhase::Preparing => "◌",
            AppPhase::Launching => "◐",
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

// ─────────────────────────────────────────────────────────────────────────────
// Tests — unseen_log_count and mark_tail_followed (Phase 4, Task 01)
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use fdemon_core::{LogEntry, LogSource};

    fn make_session() -> Session {
        Session::new("d".into(), "Device".into(), "android".into(), false)
    }

    fn make_log_entry(msg: &str) -> LogEntry {
        LogEntry::info(LogSource::Flutter, msg)
    }

    #[test]
    fn unseen_log_count_does_not_increment_while_following() {
        let mut s = make_session();
        assert!(s.log_view_state.auto_scroll);
        s.add_log(make_log_entry("a"));
        s.add_log(make_log_entry("b"));
        assert_eq!(s.unseen_log_count, 0);
    }

    #[test]
    fn unseen_log_count_increments_while_scrolled_up() {
        let mut s = make_session();
        s.log_view_state.auto_scroll = false;
        s.add_log(make_log_entry("a"));
        s.add_log(make_log_entry("b"));
        s.add_log(make_log_entry("c"));
        assert_eq!(s.unseen_log_count, 3);
    }

    #[test]
    fn unseen_log_count_unaffected_by_ring_buffer_eviction() {
        let mut s = make_session();
        s.max_logs = 2; // tight buffer
        s.log_view_state.auto_scroll = false;
        for i in 0..5 {
            s.add_log(make_log_entry(&format!("log {i}")));
        }
        assert_eq!(s.logs.len(), 2);
        assert_eq!(s.unseen_log_count, 5); // all 5 appends counted
    }

    #[test]
    fn mark_tail_followed_resets_counter() {
        let mut s = make_session();
        s.log_view_state.auto_scroll = false;
        s.add_log(make_log_entry("a"));
        s.add_log(make_log_entry("b"));
        assert_eq!(s.unseen_log_count, 2);
        s.mark_tail_followed();
        assert_eq!(s.unseen_log_count, 0);
    }

    #[test]
    fn unseen_log_count_saturates_at_max() {
        let mut s = make_session();
        s.log_view_state.auto_scroll = false;
        s.unseen_log_count = usize::MAX;
        s.add_log(make_log_entry("overflow"));
        assert_eq!(s.unseen_log_count, usize::MAX);
    }

    #[test]
    fn unseen_log_count_zero_by_default() {
        let s = make_session();
        assert_eq!(s.unseen_log_count, 0);
    }

    // ─────────────────────────────────────────────────────────
    // Tests — M2: clear_logs resets unseen_log_count
    // ─────────────────────────────────────────────────────────

    #[test]
    fn clear_logs_resets_unseen_log_count() {
        let mut s = make_session();
        s.log_view_state.auto_scroll = false;
        s.add_log(make_log_entry("a"));
        s.add_log(make_log_entry("b"));
        assert!(s.unseen_log_count > 0);
        s.clear_logs();
        assert_eq!(s.unseen_log_count, 0);
        assert!(s.logs.is_empty());
    }

    // ─────────────────────────────────────────────────────────
    // Tests — m1: filter-gated unseen_log_count increment
    // ─────────────────────────────────────────────────────────

    #[test]
    fn unseen_log_count_skips_filtered_out_entries() {
        let mut s = make_session();
        s.log_view_state.auto_scroll = false;
        // Errors-only filter: info entries (from make_log_entry) are filtered out
        s.filter_state.level_filter = fdemon_core::LogLevelFilter::Errors;
        s.add_log(make_log_entry("info line that is filtered out"));
        assert_eq!(s.unseen_log_count, 0);
    }

    #[test]
    fn unseen_log_count_counts_filter_matching_entries() {
        let mut s = make_session();
        s.log_view_state.auto_scroll = false;
        // Default match-all filter: info entries pass
        s.add_log(make_log_entry("visible line"));
        assert_eq!(s.unseen_log_count, 1);
    }

    #[test]
    fn unseen_log_count_counts_only_matching_entries_mixed() {
        let mut s = make_session();
        s.log_view_state.auto_scroll = false;
        // Errors-only filter
        s.filter_state.level_filter = fdemon_core::LogLevelFilter::Errors;
        s.add_log(make_log_entry("info — filtered out"));
        s.add_log(LogEntry::error(LogSource::Flutter, "error — passes filter"));
        s.add_log(make_log_entry("another info — filtered out"));
        s.add_log(LogEntry::error(
            LogSource::Flutter,
            "another error — passes filter",
        ));
        assert_eq!(s.unseen_log_count, 2);
    }

    // ─────────────────────────────────────────────────────────
    // Tests — reload_flash_alpha (Phase 6, Task 01)
    // ─────────────────────────────────────────────────────────

    fn make_running_session_with_reload() -> (Session, DateTime<Local>) {
        let mut s = make_session();
        // Stamp a reload at a fixed wall-clock instant
        let reloaded_at = Local::now();
        s.phase = AppPhase::Running;
        s.last_reload_time = Some(reloaded_at);
        (s, reloaded_at)
    }

    #[test]
    fn flash_alpha_full_at_reload_instant() {
        let (s, reloaded_at) = make_running_session_with_reload();
        // now == last_reload_time → elapsed = 0 ms → alpha = 1.0
        let alpha = s.reload_flash_alpha(reloaded_at);
        assert!(
            (alpha - 1.0).abs() < 1e-6,
            "expected 1.0 at reload instant, got {alpha}"
        );
    }

    #[test]
    fn flash_alpha_half_at_midpoint() {
        let (s, reloaded_at) = make_running_session_with_reload();
        // +250 ms → elapsed = 250 / 500 = 0.5 → alpha ≈ 0.5
        let now = reloaded_at + chrono::Duration::milliseconds(250);
        let alpha = s.reload_flash_alpha(now);
        assert!(
            (alpha - 0.5).abs() < 1e-4,
            "expected ~0.5 at midpoint, got {alpha}"
        );
    }

    #[test]
    fn flash_alpha_zero_after_window() {
        let (s, reloaded_at) = make_running_session_with_reload();
        // +500 ms (equal to RELOAD_FLASH_DURATION_MS) is outside the half-open
        // range [0, 500), so alpha must be 0.0
        let at_boundary = reloaded_at + chrono::Duration::milliseconds(500);
        assert_eq!(s.reload_flash_alpha(at_boundary), 0.0);

        // +1 s is well past the window
        let past = reloaded_at + chrono::Duration::milliseconds(1_000);
        assert_eq!(s.reload_flash_alpha(past), 0.0);
    }

    #[test]
    fn flash_alpha_zero_when_never_reloaded() {
        let mut s = make_session();
        s.phase = AppPhase::Running;
        // last_reload_time is None (default)
        assert_eq!(s.reload_flash_alpha(Local::now()), 0.0);
    }

    #[test]
    fn flash_alpha_suppressed_when_not_running() {
        let (mut s, reloaded_at) = make_running_session_with_reload();
        // Override phase to something other than Running
        for phase in [
            AppPhase::Stopped,
            AppPhase::Reloading,
            AppPhase::Quitting,
            AppPhase::Initializing,
            AppPhase::Launching,
            AppPhase::Preparing,
        ] {
            s.phase = phase;
            // Use the reload instant itself — would be 1.0 if Running
            assert_eq!(
                s.reload_flash_alpha(reloaded_at),
                0.0,
                "expected 0.0 for phase {phase:?}"
            );
        }
    }

    #[test]
    fn flash_alpha_zero_for_past_now() {
        let (s, reloaded_at) = make_running_session_with_reload();
        // now is 1 ms BEFORE the reload stamp → elapsed_ms = -1, outside [0,500)
        let before = reloaded_at - chrono::Duration::milliseconds(1);
        let alpha = s.reload_flash_alpha(before);
        assert_eq!(alpha, 0.0, "expected 0.0 for clock-skew case, got {alpha}");
    }

    #[test]
    fn flash_alpha_always_in_unit_interval() {
        let (s, reloaded_at) = make_running_session_with_reload();
        // Probe every 50 ms from -100 to +1000 ms
        for offset_ms in (-100i64..=1000).step_by(50) {
            let now = reloaded_at + chrono::Duration::milliseconds(offset_ms);
            let alpha = s.reload_flash_alpha(now);
            assert!(
                (0.0..=1.0).contains(&alpha),
                "alpha {alpha} out of [0,1] at offset {offset_ms} ms"
            );
        }
    }
}
