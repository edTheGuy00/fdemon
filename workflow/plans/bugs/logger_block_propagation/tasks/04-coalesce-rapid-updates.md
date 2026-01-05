## Task: Coalesce Rapid Log Updates

**Objective**: Batch rapid log arrivals and throttle UI re-renders to reduce CPU usage during high-volume logging bursts.

**Depends on**: [01-stateful-block-tracking](01-stateful-block-tracking.md)

**Priority**: LOW

### Background

When a Flutter app outputs many logs rapidly (e.g., during hot reload, errors with stack traces, or verbose debugging), each log line currently triggers processing and potentially a UI re-render. Batching these updates and throttling renders to ~60fps can significantly reduce CPU overhead.

### Scope

- `src/app/handler/daemon.rs` or `src/app/session.rs`: Add batching logic for incoming logs
- `src/app/mod.rs` or main event loop: Add render throttling

### Implementation

#### 1. Add Log Batcher

```rust
use std::time::{Duration, Instant};

const BATCH_FLUSH_INTERVAL: Duration = Duration::from_millis(16); // ~60fps
const BATCH_MAX_SIZE: usize = 100;

pub struct LogBatcher {
    pending: Vec<LogEntry>,
    last_flush: Instant,
}

impl LogBatcher {
    pub fn new() -> Self {
        Self {
            pending: Vec::with_capacity(BATCH_MAX_SIZE),
            last_flush: Instant::now(),
        }
    }

    /// Add a log entry to the batch
    /// Returns true if batch should be flushed
    pub fn add(&mut self, entry: LogEntry) -> bool {
        self.pending.push(entry);
        self.should_flush()
    }

    /// Check if batch should be flushed
    pub fn should_flush(&self) -> bool {
        self.pending.len() >= BATCH_MAX_SIZE
            || self.last_flush.elapsed() >= BATCH_FLUSH_INTERVAL
    }

    /// Flush and return pending entries
    pub fn flush(&mut self) -> Vec<LogEntry> {
        self.last_flush = Instant::now();
        std::mem::take(&mut self.pending)
    }

    /// Check if there are pending entries
    pub fn has_pending(&self) -> bool {
        !self.pending.is_empty()
    }

    /// Time until next scheduled flush
    pub fn time_until_flush(&self) -> Duration {
        BATCH_FLUSH_INTERVAL.saturating_sub(self.last_flush.elapsed())
    }
}
```

#### 2. Integrate with Session

```rust
impl Session {
    /// Add multiple logs at once (batched)
    pub fn add_logs_batch(&mut self, entries: Vec<LogEntry>) {
        for entry in entries {
            self.add_log(entry);
        }
    }
}
```

#### 3. Update Event Handler

```rust
// In daemon handler or main event loop
pub struct DaemonHandler {
    log_batcher: LogBatcher,
    // ...
}

impl DaemonHandler {
    fn handle_log_event(&mut self, entry: LogEntry) {
        if self.log_batcher.add(entry) {
            self.flush_logs();
        }
    }

    fn flush_logs(&mut self) {
        let entries = self.log_batcher.flush();
        if !entries.is_empty() {
            self.session.add_logs_batch(entries);
            // Signal UI to re-render (if needed)
        }
    }

    /// Called from event loop tick
    fn tick(&mut self) {
        // Flush any pending logs if interval elapsed
        if self.log_batcher.should_flush() && self.log_batcher.has_pending() {
            self.flush_logs();
        }
    }
}
```

#### 4. Render Throttling (Optional)

If the TUI event loop doesn't already throttle, add render limiting:

```rust
const MIN_RENDER_INTERVAL: Duration = Duration::from_millis(16); // ~60fps

pub struct RenderThrottle {
    last_render: Instant,
    needs_render: bool,
}

impl RenderThrottle {
    pub fn new() -> Self {
        Self {
            last_render: Instant::now(),
            needs_render: false,
        }
    }

    /// Mark that a render is needed
    pub fn request_render(&mut self) {
        self.needs_render = true;
    }

    /// Check if we should render now
    pub fn should_render(&self) -> bool {
        self.needs_render && self.last_render.elapsed() >= MIN_RENDER_INTERVAL
    }

    /// Mark render complete
    pub fn did_render(&mut self) {
        self.last_render = Instant::now();
        self.needs_render = false;
    }
}
```

### Acceptance Criteria

1. [ ] `LogBatcher` struct implemented with time and size-based flushing
2. [ ] Logs batched during rapid output bursts
3. [ ] Batch flushed at minimum interval (16ms) or max size (100)
4. [ ] Session supports batch log insertion
5. [ ] UI re-renders throttled to ~60fps during high volume
6. [ ] No visible delay for normal log output
7. [ ] Unit tests for batching logic
8. [ ] Performance improvement measured during burst logging

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;

    #[test]
    fn test_batch_size_trigger() {
        let mut batcher = LogBatcher::new();

        // Add entries up to max size
        for i in 0..99 {
            let should_flush = batcher.add(LogEntry::new(
                LogSource::Flutter,
                LogLevel::Info,
                format!("Log {}", i)
            ));
            assert!(!should_flush, "Should not flush before max size");
        }

        // This should trigger flush
        let should_flush = batcher.add(LogEntry::new(
            LogSource::Flutter,
            LogLevel::Info,
            "Log 100".to_string()
        ));
        assert!(should_flush, "Should flush at max size");
    }

    #[test]
    fn test_batch_time_trigger() {
        let mut batcher = LogBatcher::new();

        batcher.add(LogEntry::new(LogSource::Flutter, LogLevel::Info, "Log 1"));
        assert!(!batcher.should_flush());

        // Wait for flush interval
        sleep(Duration::from_millis(20));

        assert!(batcher.should_flush(), "Should flush after time interval");
    }

    #[test]
    fn test_flush_returns_entries() {
        let mut batcher = LogBatcher::new();

        batcher.add(LogEntry::new(LogSource::Flutter, LogLevel::Info, "Log 1"));
        batcher.add(LogEntry::new(LogSource::Flutter, LogLevel::Error, "Log 2"));

        let entries = batcher.flush();

        assert_eq!(entries.len(), 2);
        assert!(!batcher.has_pending());
    }

    #[test]
    fn test_flush_resets_timer() {
        let mut batcher = LogBatcher::new();

        sleep(Duration::from_millis(20));
        batcher.add(LogEntry::new(LogSource::Flutter, LogLevel::Info, "Log 1"));

        assert!(batcher.should_flush()); // Time elapsed before add

        batcher.flush();

        assert!(!batcher.should_flush()); // Timer reset
    }
}
```

### Integration Test

```rust
#[test]
fn test_high_volume_batching() {
    let mut handler = DaemonHandler::new(/* ... */);

    let start = Instant::now();

    // Simulate rapid log burst (1000 logs)
    for i in 0..1000 {
        handler.handle_log_event(LogEntry::new(
            LogSource::Flutter,
            LogLevel::Info,
            format!("Rapid log {}", i)
        ));
    }

    // Force final flush
    handler.tick();

    let elapsed = start.elapsed();

    // Should complete quickly (batching reduces overhead)
    assert!(elapsed < Duration::from_millis(100), "Batching should be fast");

    // All logs should be in session
    assert_eq!(handler.session.log_count(), 1000);
}
```

### Files to Modify

| File | Action | Description |
|------|--------|-------------|
| `src/app/handler/daemon.rs` | Modify | Add `LogBatcher`, integrate with log handling |
| `src/app/session.rs` | Modify | Add `add_logs_batch()` method |
| `src/app/mod.rs` | Modify | Add `RenderThrottle` if not already present |

### Configuration Options

Consider making these configurable:
- `batch_flush_interval_ms`: Default 16 (60fps)
- `batch_max_size`: Default 100
- `render_throttle_ms`: Default 16 (60fps)

### Edge Cases

1. **Final flush**: Ensure pending logs are flushed when session ends
2. **Block boundaries**: Blocks spanning batch boundaries should still propagate correctly
3. **Error highlighting**: Errors should still be visible promptly (don't over-batch)
4. **Single logs**: Normal low-volume logging should feel instant

### Estimated Effort

3-4 hours

### References

- VS Code terminal throttling ([Issue #283056](https://github.com/microsoft/vscode/issues/283056))
- xterm.js rate-limited viewport refresh
- BUG.md Phase 3E specification
