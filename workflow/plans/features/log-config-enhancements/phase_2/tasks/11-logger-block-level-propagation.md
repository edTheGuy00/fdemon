## Task: Logger Package Block-Level Propagation

**Objective**: Propagate log levels across multi-line Logger package structured output so that entire error/warning blocks are styled consistently, not just individual lines containing level indicators.

**Depends on**: 
- [08-strip-ansi-escape-codes](08-strip-ansi-escape-codes.md)
- [09-enhance-log-level-detection](09-enhance-log-level-detection.md)

### Background

The Logger package outputs multi-line structured logs with box-drawing characters:

```
┌───────────────────────────────────────────────────────────────
│ RangeError (length): Invalid value: Not in inclusive range 0..2: 10   ← ERROR detected (✗ red)
├┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄
│ #0   List.[] (dart:core-patch/growable_array.dart)                    ← INFO (• white) - should be red!
│ #1   triggerRangeError (package:flutter_deamon/errors/...)            ← ERROR (✗ red)
│ #2   ErrorTestingPage._buildErrorButton.<anonymous closure>           ← ERROR (✗ red)
│ #3   _InkResponseState.handleTap (package:flutter/...)                ← INFO (• white) - should be red!
├┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄
│ 11:57:11.960 (+0:05:46.971300)                                        ← INFO (• white) - should be red!
├┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄
│ ⛔┄ Error triggered: Range Error                                      ← ERROR (✗ red)
└───────────────────────────────────────────────────────────────
```

**Current Behavior**: Each line is processed independently, so only lines containing error indicators (like "RangeError", `⛔`, or "Error") are marked as errors. The box-drawing lines and stack traces remain white.

**Desired Behavior**: The entire block (from `┌` to `└`) should inherit the highest severity level detected within it.

### Scope

- `src/app/handler/session.rs`: Add stateful log processing for block detection
- `src/app/handler/helpers.rs`: Add box-drawing detection utility
- `src/core/types.rs`: Potentially add block tracking state

### Implementation Approach

#### Option A: Post-Processing Block Detection (Recommended)

After adding log entries, scan backwards to detect Logger blocks and update levels:

```rust
/// Detect if a line is part of a Logger package structured block
pub fn is_logger_block_line(message: &str) -> bool {
    let trimmed = message.trim_start();
    trimmed.starts_with('┌') ||
    trimmed.starts_with('│') ||
    trimmed.starts_with('├') ||
    trimmed.starts_with('└') ||
    trimmed.starts_with('┄')
}

/// Detect block boundary characters
pub fn is_block_start(message: &str) -> bool {
    message.trim_start().starts_with('┌')
}

pub fn is_block_end(message: &str) -> bool {
    message.trim_start().starts_with('└')
}
```

In `Session::add_log()` or a new `Session::finalize_log_block()`:

```rust
pub fn add_log(&mut self, entry: LogEntry) {
    self.logs.push(entry);
    
    // Check if this completes a Logger block
    if is_block_end(&self.logs.last().unwrap().message) {
        self.propagate_block_level();
    }
}

fn propagate_block_level(&mut self) {
    // Find the block start (scan backwards for ┌)
    let block_end = self.logs.len() - 1;
    let mut block_start = block_end;
    let mut highest_level = LogLevel::Info;
    
    for i in (0..=block_end).rev() {
        let entry = &self.logs[i];
        
        // Track highest severity in block
        if entry.level > highest_level {
            highest_level = entry.level;
        }
        
        // Found block start
        if is_block_start(&entry.message) {
            block_start = i;
            break;
        }
        
        // Safety: don't scan more than 50 lines back
        if block_end - i > 50 {
            break;
        }
    }
    
    // Apply highest level to all entries in block
    if block_start < block_end && highest_level > LogLevel::Info {
        for i in block_start..=block_end {
            self.logs[i].level = highest_level;
        }
    }
}
```

#### Option B: Stateful Line Processing

Track block context during log processing:

```rust
pub struct LogBlockContext {
    in_block: bool,
    block_level: LogLevel,
    block_start_index: usize,
}

impl LogBlockContext {
    pub fn process_line(&mut self, message: &str, detected_level: LogLevel) -> LogLevel {
        if is_block_start(message) {
            self.in_block = true;
            self.block_level = detected_level;
            return detected_level;
        }
        
        if self.in_block {
            // Update block level if we find higher severity
            if detected_level > self.block_level {
                self.block_level = detected_level;
            }
            
            if is_block_end(message) {
                self.in_block = false;
                // Return block level for final line
                return self.block_level;
            }
            
            // Return current block level for continuation lines
            return self.block_level;
        }
        
        detected_level
    }
}
```

### Box-Drawing Characters Reference

| Character | Unicode | Name | Usage in Logger |
|-----------|---------|------|-----------------|
| `┌` | U+250C | Box Drawings Light Down and Right | Block start |
| `└` | U+2514 | Box Drawings Light Up and Right | Block end |
| `│` | U+2502 | Box Drawings Light Vertical | Block content |
| `├` | U+251C | Box Drawings Light Vertical and Right | Section divider |
| `┄` | U+2504 | Box Drawings Light Triple Dash Horizontal | Dashed divider |
| `─` | U+2500 | Box Drawings Light Horizontal | Horizontal line |

### Log Level Priority

When propagating levels, use the highest severity found:
1. Error (highest)
2. Warning
3. Info
4. Debug (lowest)

```rust
impl Ord for LogLevel {
    fn cmp(&self, other: &Self) -> Ordering {
        self.severity().cmp(&other.severity())
    }
}

impl LogLevel {
    fn severity(&self) -> u8 {
        match self {
            LogLevel::Debug => 0,
            LogLevel::Info => 1,
            LogLevel::Warning => 2,
            LogLevel::Error => 3,
        }
    }
}
```

### Acceptance Criteria

1. [x] Logger package blocks (┌ to └) have consistent styling
2. [x] Highest severity in block propagates to all lines
3. [x] Box-drawing detection works for all Logger characters
4. [x] Block detection doesn't affect non-Logger output
5. [x] Performance: block scanning limited to reasonable depth (50 lines)
6. [x] Error blocks display all lines in red
7. [x] Warning blocks display all lines in yellow
8. [x] Unit tests for block detection
9. [x] Unit tests for level propagation
10. [x] No regressions in existing log processing

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_logger_block_line() {
        assert!(is_logger_block_line("┌───────────"));
        assert!(is_logger_block_line("│ Message"));
        assert!(is_logger_block_line("├┄┄┄┄┄┄┄┄┄┄"));
        assert!(is_logger_block_line("└───────────"));
        assert!(!is_logger_block_line("Regular message"));
    }

    #[test]
    fn test_block_start_detection() {
        assert!(is_block_start("┌───────────────────"));
        assert!(!is_block_start("│ Content"));
        assert!(!is_block_start("└───────────────────"));
    }

    #[test]
    fn test_block_end_detection() {
        assert!(is_block_end("└───────────────────"));
        assert!(!is_block_end("│ Content"));
        assert!(!is_block_end("┌───────────────────"));
    }

    #[test]
    fn test_error_block_propagation() {
        let mut session = Session::new(...);
        
        // Simulate Logger error block
        session.add_log(LogEntry::info(LogSource::Flutter, "┌───────────"));
        session.add_log(LogEntry::error(LogSource::Flutter, "│ ⛔ Error: failed"));
        session.add_log(LogEntry::info(LogSource::Flutter, "│ #0 stack trace"));
        session.add_log(LogEntry::info(LogSource::Flutter, "└───────────"));
        
        // All lines should be Error level now
        assert!(session.logs.iter().all(|e| e.level == LogLevel::Error));
    }

    #[test]
    fn test_non_block_lines_unchanged() {
        let mut session = Session::new(...);
        
        session.add_log(LogEntry::info(LogSource::Flutter, "Regular info"));
        session.add_log(LogEntry::error(LogSource::Flutter, "Standalone error"));
        session.add_log(LogEntry::info(LogSource::Flutter, "Another info"));
        
        // Levels should remain as originally set
        assert_eq!(session.logs[0].level, LogLevel::Info);
        assert_eq!(session.logs[1].level, LogLevel::Error);
        assert_eq!(session.logs[2].level, LogLevel::Info);
    }

    #[test]
    fn test_warning_block_propagation() {
        let mut session = Session::new(...);
        
        session.add_log(LogEntry::info(LogSource::Flutter, "┌───────────"));
        session.add_log(LogEntry::warn(LogSource::Flutter, "│ ⚠ Warning: deprecated"));
        session.add_log(LogEntry::info(LogSource::Flutter, "│ Additional info"));
        session.add_log(LogEntry::info(LogSource::Flutter, "└───────────"));
        
        // All lines should be Warning level
        assert!(session.logs.iter().all(|e| e.level == LogLevel::Warning));
    }
}
```

### Integration Tests

After implementation, manually verify with the sample app:
1. Run Flutter Demon with `sample/` project
2. Trigger an error (e.g., "Range Error" button)
3. Verify entire Logger block displays in red, not just error lines
4. Trigger a warning and verify entire block is yellow
5. Verify regular (non-Logger) logs are unaffected

### Files to Create/Modify

| File | Action | Description |
|------|--------|-------------|
| `src/app/handler/helpers.rs` | Modify | Add box-drawing detection utilities |
| `src/app/session.rs` | Modify | Add block level propagation in `add_log()` |
| `src/core/types.rs` | Modify | Add `Ord` impl for LogLevel if needed |

### Estimated Time

4-5 hours

### Edge Cases

1. **Incomplete blocks**: Block start without end (process timeout) - don't propagate
2. **Nested blocks**: Logger doesn't nest, but handle gracefully
3. **Very long blocks**: Limit scan depth to prevent performance issues
4. **Mixed sources**: Only apply to Flutter/FlutterError sources
5. **Interleaved output**: Other log sources between block lines - skip non-Flutter lines

### References

- [Logger Package Source](https://github.com/simc/logger) - PrettyPrinter implementation
- Box Drawing Unicode Block: U+2500–U+257F
- Phase 2 Task 09 (log level detection)

---

## Completion Summary

**Status**: ✅ Done

**Files Modified**:
- `src/app/handler/helpers.rs` - Added box-drawing detection utilities (`is_logger_block_line()`, `is_block_start()`, `is_block_end()`)
- `src/core/types.rs` - Added severity comparison methods to `LogLevel` (`severity()`, `is_more_severe_than()`, `max_severity()`)
- `src/app/session.rs` - Added block level propagation in `add_log()` method with new `propagate_block_level()` helper

**Implementation Details**:
- Block detection uses box-drawing characters: `┌` (start), `└` (end), `│`, `├`, `┄`, `─` (content)
- When a block end (`└`) is detected, scan backwards (max 50 lines) to find block start (`┌`)
- Highest severity level in block is applied to all lines
- Error count is updated correctly when levels change
- Only propagates if highest level > Info (no point changing all to Info)

**Unit Tests Added**:
- `helpers.rs`: 4 tests for box-drawing detection (`is_logger_block_line`, `is_block_start`, `is_block_end`, `test_block_detection_with_logger_output`)
- `types.rs`: 3 tests for LogLevel severity methods (`test_log_level_severity`, `test_log_level_is_more_severe_than`, `test_log_level_max_severity`)
- `session.rs`: 10 tests for block propagation (`test_error_block_propagation`, `test_warning_block_propagation`, `test_non_block_lines_unchanged`, `test_block_propagation_error_count`, `test_info_only_block_not_propagated`, `test_incomplete_block_not_propagated`, `test_block_end_without_start_not_propagated`, `test_multiple_blocks_independent`, `test_mixed_content_between_blocks`, `test_block_with_leading_whitespace`)

**Testing Performed**:
- `cargo check` - PASS
- `cargo test app::handler::helpers` - 40 tests PASS
- `cargo test app::session` - 76 tests PASS
- `cargo test core::types` - 75 tests PASS

**Notable Decisions**:
- Used Option A (Post-Processing Block Detection) as recommended in the task spec
- Scan limit of 50 lines prevents performance issues with very long output
- Incomplete blocks (no start or no end) are not propagated to prevent incorrect level assignment
- Error count is tracked correctly by counting promotions and demotions

**Edge Cases Handled**:
- Incomplete blocks (missing start or end) - not propagated
- Info-only blocks - no propagation needed
- Blocks with leading whitespace - handled via `trim_start()`
- Multiple independent blocks - each processed separately
- Mixed content between blocks - regular logs unaffected

**Risks/Limitations**:
- Mixed sources within a block are not specially handled (all Flutter lines in block are affected)
- Very long blocks (>50 lines) won't be fully processed (conservative choice for performance)