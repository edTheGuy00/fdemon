## Task: Ring Buffer for Log Storage

**Objective**: Replace unbounded `Vec<LogEntry>` with a ring buffer (`VecDeque`) to cap memory usage and prevent RAM growth during long debugging sessions.

**Depends on**: [01-stateful-block-tracking](01-stateful-block-tracking.md) (block state tracking needs adjustment for ring buffer indices)

**Priority**: LOW

### Background

Currently, `Session.logs` is a `Vec<LogEntry>` that grows unbounded. During long debugging sessions or apps with high log volume, this can consume significant RAM. Industry standard (VS Code, xterm.js) is to use ring buffers with configurable scrollback limits.

### Scope

- `src/app/session.rs`: Change `logs: Vec<LogEntry>` to `logs: VecDeque<LogEntry>`, add capacity limiting
- `src/core/config.rs` (or similar): Add configurable max log entries setting

### Implementation

#### 1. Change Storage Type

```rust
use std::collections::VecDeque;

const DEFAULT_MAX_LOG_ENTRIES: usize = 10_000;

pub struct Session {
    logs: VecDeque<LogEntry>,
    max_log_entries: usize,
    // ... other fields
}

impl Session {
    pub fn new(/* ... */) -> Self {
        Self {
            logs: VecDeque::with_capacity(DEFAULT_MAX_LOG_ENTRIES),
            max_log_entries: DEFAULT_MAX_LOG_ENTRIES,
            // ...
        }
    }
}
```

#### 2. Update add_log() with Capacity Limiting

```rust
pub fn add_log(&mut self, entry: LogEntry) {
    // Evict oldest entry if at capacity
    if self.logs.len() >= self.max_log_entries {
        if let Some(evicted) = self.logs.pop_front() {
            // Update error count if evicting an error
            if evicted.level == LogLevel::Error {
                self.error_count = self.error_count.saturating_sub(1);
            }
        }

        // Adjust block_start index if we're in a block
        if let Some(ref mut start) = self.block_state.block_start {
            if *start > 0 {
                *start -= 1;
            } else {
                // Block start was evicted - cancel block tracking
                self.block_state.block_start = None;
            }
        }
    }

    let idx = self.logs.len();

    // ... rest of block tracking logic from Task 01

    self.logs.push_back(entry);
}
```

#### 3. Update Index-Based Access

Replace `self.logs[i]` with `self.logs.get(i)` or `self.logs[i]` (VecDeque supports indexing).

```rust
// VecDeque supports Index trait, so this still works:
for i in start..=end {
    self.logs[i].level = max_level;
}

// For iteration, can use:
for entry in self.logs.iter() { }
for entry in self.logs.iter_mut() { }
```

#### 4. Update Log Retrieval Methods

```rust
impl Session {
    pub fn logs(&self) -> &VecDeque<LogEntry> {
        &self.logs
    }

    pub fn log_count(&self) -> usize {
        self.logs.len()
    }

    pub fn get_log(&self, index: usize) -> Option<&LogEntry> {
        self.logs.get(index)
    }

    /// Get logs in a range (for virtualized rendering)
    pub fn get_logs_range(&self, start: usize, end: usize) -> impl Iterator<Item = &LogEntry> {
        self.logs.range(start..end.min(self.logs.len()))
    }
}
```

### Acceptance Criteria

1. [ ] `logs` field changed from `Vec<LogEntry>` to `VecDeque<LogEntry>`
2. [ ] Oldest entries evicted when capacity reached
3. [ ] Error count updated correctly when evicting error entries
4. [ ] Block state tracking adjusted for index shifts on eviction
5. [ ] Configurable max entries (default 10,000)
6. [ ] All existing log access patterns still work
7. [ ] Memory usage capped during high-volume logging
8. [ ] Unit tests for capacity limiting and eviction

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ring_buffer_capacity() {
        let mut session = Session::new_with_capacity(5);

        for i in 0..10 {
            session.add_log(LogEntry::new(
                LogSource::Flutter,
                LogLevel::Info,
                format!("Message {}", i)
            ));
        }

        // Should only have last 5 entries
        assert_eq!(session.logs.len(), 5);
        assert!(session.logs[0].message.contains("Message 5"));
        assert!(session.logs[4].message.contains("Message 9"));
    }

    #[test]
    fn test_error_count_on_eviction() {
        let mut session = Session::new_with_capacity(3);

        session.add_log(LogEntry::new(LogSource::Flutter, LogLevel::Error, "Error 1"));
        session.add_log(LogEntry::new(LogSource::Flutter, LogLevel::Info, "Info 1"));
        session.add_log(LogEntry::new(LogSource::Flutter, LogLevel::Error, "Error 2"));

        assert_eq!(session.error_count, 2);

        // Add more - should evict Error 1
        session.add_log(LogEntry::new(LogSource::Flutter, LogLevel::Info, "Info 2"));

        assert_eq!(session.error_count, 1); // Only Error 2 remains
        assert_eq!(session.logs.len(), 3);
    }

    #[test]
    fn test_block_tracking_survives_eviction() {
        let mut session = Session::new_with_capacity(5);

        // Fill with some logs
        session.add_log(LogEntry::new(LogSource::Flutter, LogLevel::Info, "Old 1"));
        session.add_log(LogEntry::new(LogSource::Flutter, LogLevel::Info, "Old 2"));

        // Start a block
        session.add_log(LogEntry::new(LogSource::Flutter, LogLevel::Info, "┌───────────"));
        session.add_log(LogEntry::new(LogSource::Flutter, LogLevel::Error, "│ ⛔ Error"));

        // This should evict Old 1, shifting indices
        session.add_log(LogEntry::new(LogSource::Flutter, LogLevel::Info, "└───────────"));

        // Block should still be properly propagated despite eviction
        // Find the block entries and verify they're all Error level
        let block_entries: Vec<_> = session.logs.iter()
            .filter(|e| e.message.contains('┌') || e.message.contains('│') || e.message.contains('└'))
            .collect();

        assert!(block_entries.iter().all(|e| e.level == LogLevel::Error));
    }

    #[test]
    fn test_block_start_evicted_cancels_tracking() {
        let mut session = Session::new_with_capacity(3);

        // Start a block
        session.add_log(LogEntry::new(LogSource::Flutter, LogLevel::Info, "┌───────────"));
        session.add_log(LogEntry::new(LogSource::Flutter, LogLevel::Error, "│ ⛔ Error"));
        session.add_log(LogEntry::new(LogSource::Flutter, LogLevel::Info, "│ Content"));

        // This evicts the block start
        session.add_log(LogEntry::new(LogSource::Flutter, LogLevel::Info, "│ More"));

        // Block tracking should be cancelled (start was evicted)
        // End line should NOT trigger propagation
        session.add_log(LogEntry::new(LogSource::Flutter, LogLevel::Info, "└───────────"));

        // Entries should NOT be propagated to Error since block start was lost
        assert_eq!(session.logs.back().unwrap().level, LogLevel::Info);
    }
}
```

### Files to Modify

| File | Action | Description |
|------|--------|-------------|
| `src/app/session.rs` | Modify | Change `Vec` to `VecDeque`, add capacity limiting |
| `src/core/config.rs` | Modify | Add `max_log_entries` configuration option |
| Any file using `session.logs` | Modify | Update if using Vec-specific APIs |

### Edge Cases

1. **Block start evicted**: If `┌` line is evicted while block is open, cancel block tracking
2. **Error eviction**: Decrement error count when evicting error entries
3. **Zero capacity**: Handle gracefully (probably should enforce minimum)
4. **Capacity change at runtime**: If supported, handle shrinking gracefully

### Estimated Effort

2-3 hours

### References

- [VecDeque documentation](https://doc.rust-lang.org/std/collections/struct.VecDeque.html)
- [log_buffer crate](https://github.com/whitequark/rust-log_buffer) - ring buffer pattern
- VS Code terminal scrollback limiting
- BUG.md Phase 3B specification
