//! Log batching for performance — coalesces rapid log arrivals.

use std::time::{Duration, Instant};

use fdemon_core::LogEntry;

/// Default batch flush interval (~60fps)
pub(crate) const BATCH_FLUSH_INTERVAL: Duration = Duration::from_millis(16);

/// Maximum batch size before forced flush
pub(crate) const BATCH_MAX_SIZE: usize = 100;

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
