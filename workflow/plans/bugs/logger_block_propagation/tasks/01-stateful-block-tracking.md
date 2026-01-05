## Task: Stateful Block Tracking

**Objective**: Replace backward-scanning block propagation with incremental state tracking to fix O(N*M) performance issue and ensure correct block-level propagation.

**Depends on**: None

**Priority**: HIGH

### Background

The current `propagate_block_level()` implementation scans backwards up to 50 lines on every block end. This causes O(N*M) complexity where N is total logs and M is scan depth. With high-volume logging, this creates significant CPU overhead.

### Scope

- `src/app/session.rs`: Add `LogBlockState` struct, modify `add_log()` to track block state incrementally

### Implementation

#### 1. Add Block State Tracking

```rust
/// Tracks state for Logger package block detection
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
```

#### 2. Add State to Session

```rust
pub struct Session {
    // ... existing fields
    block_state: LogBlockState,
}
```

#### 3. Modify add_log() for Incremental Tracking

```rust
pub fn add_log(&mut self, entry: LogEntry) {
    let idx = self.logs.len();

    // Check for block boundaries BEFORE pushing
    let is_start = is_block_start(&entry.message);
    let is_end = is_block_end(&entry.message);

    // Track block state as we go
    if is_start {
        self.block_state.block_start = Some(idx);
        self.block_state.block_max_level = entry.level;
    } else if self.block_state.block_start.is_some() {
        // Inside a block - update max level
        if entry.level.is_more_severe_than(&self.block_state.block_max_level) {
            self.block_state.block_max_level = entry.level;
        }
    }

    // Push the entry
    self.logs.push(entry);

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

        // Reset block state
        self.block_state = LogBlockState::default();
    }
}
```

#### 4. Remove Old Backward-Scanning Code

Remove or deprecate the existing `propagate_block_level()` method that scans backwards.

### Acceptance Criteria

1. [ ] `LogBlockState` struct added to track block boundaries
2. [ ] `add_log()` tracks block state incrementally (no backward scanning)
3. [ ] Block level propagation applies highest severity to all lines in block
4. [ ] Error count updated correctly when levels change
5. [ ] Old `propagate_block_level()` backward-scanning code removed
6. [ ] Incomplete blocks (no end) don't cause issues
7. [ ] Nested or interleaved blocks handled gracefully
8. [ ] Unit tests for stateful block tracking
9. [ ] Performance improvement verified (no O(N*M) scanning)

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stateful_error_block_propagation() {
        let mut session = Session::new(/* ... */);

        // Simulate Logger error block arriving line-by-line
        session.add_log(LogEntry::new(LogSource::Flutter, LogLevel::Info, "┌───────────"));
        session.add_log(LogEntry::new(LogSource::Flutter, LogLevel::Error, "│ ⛔ Error: failed"));
        session.add_log(LogEntry::new(LogSource::Flutter, LogLevel::Info, "│ #0 stack trace"));
        session.add_log(LogEntry::new(LogSource::Flutter, LogLevel::Info, "└───────────"));

        // All lines should now be Error level
        assert!(session.logs.iter().all(|e| e.level == LogLevel::Error));
    }

    #[test]
    fn test_stateful_incomplete_block_no_propagation() {
        let mut session = Session::new(/* ... */);

        // Block starts but never ends
        session.add_log(LogEntry::new(LogSource::Flutter, LogLevel::Info, "┌───────────"));
        session.add_log(LogEntry::new(LogSource::Flutter, LogLevel::Error, "│ ⛔ Error"));
        session.add_log(LogEntry::new(LogSource::Flutter, LogLevel::Info, "│ More content"));
        // No └ line

        // First line should still be Info (no propagation without block end)
        assert_eq!(session.logs[0].level, LogLevel::Info);
    }

    #[test]
    fn test_stateful_info_only_block_no_change() {
        let mut session = Session::new(/* ... */);

        session.add_log(LogEntry::new(LogSource::Flutter, LogLevel::Info, "┌───────────"));
        session.add_log(LogEntry::new(LogSource::Flutter, LogLevel::Info, "│ 💡 Info message"));
        session.add_log(LogEntry::new(LogSource::Flutter, LogLevel::Info, "└───────────"));

        // All should remain Info
        assert!(session.logs.iter().all(|e| e.level == LogLevel::Info));
    }

    #[test]
    fn test_stateful_multiple_blocks_independent() {
        let mut session = Session::new(/* ... */);

        // First block (error)
        session.add_log(LogEntry::new(LogSource::Flutter, LogLevel::Info, "┌───────────"));
        session.add_log(LogEntry::new(LogSource::Flutter, LogLevel::Error, "│ ⛔ Error"));
        session.add_log(LogEntry::new(LogSource::Flutter, LogLevel::Info, "└───────────"));

        // Regular log between blocks
        session.add_log(LogEntry::new(LogSource::Flutter, LogLevel::Info, "Plain message"));

        // Second block (warning)
        session.add_log(LogEntry::new(LogSource::Flutter, LogLevel::Info, "┌───────────"));
        session.add_log(LogEntry::new(LogSource::Flutter, LogLevel::Warning, "│ ⚠ Warning"));
        session.add_log(LogEntry::new(LogSource::Flutter, LogLevel::Info, "└───────────"));

        // First block should be Error
        assert_eq!(session.logs[0].level, LogLevel::Error);
        assert_eq!(session.logs[1].level, LogLevel::Error);
        assert_eq!(session.logs[2].level, LogLevel::Error);

        // Middle log unchanged
        assert_eq!(session.logs[3].level, LogLevel::Info);

        // Second block should be Warning
        assert_eq!(session.logs[4].level, LogLevel::Warning);
        assert_eq!(session.logs[5].level, LogLevel::Warning);
        assert_eq!(session.logs[6].level, LogLevel::Warning);
    }
}
```

### Files to Modify

| File | Action | Description |
|------|--------|-------------|
| `src/app/session.rs` | Modify | Add `LogBlockState`, update `add_log()` for incremental tracking |
| `src/app/session.rs` | Remove | Delete old `propagate_block_level()` backward-scanning method |

### Edge Cases

1. **Incomplete blocks**: Block starts but app crashes before `└` - don't propagate
2. **Back-to-back blocks**: Two blocks with no gap - handle as separate blocks
3. **Interleaved sources**: Non-Flutter logs between block lines - skip them in propagation
4. **Empty blocks**: `┌` immediately followed by `└` - handle gracefully

### Estimated Effort

3-4 hours

### References

- Current implementation: `src/app/session.rs` - `propagate_block_level()`
- Block detection helpers: `src/app/handler/helpers.rs` - `is_block_start()`, `is_block_end()`
- BUG.md Phase 1 specification
