## Task: Session Exception Buffer Integration

**Objective**: Add an `ExceptionBlockParser` to the `Session` struct and provide methods that route raw stderr/stdout lines through exception detection before creating `LogEntry` items.

**Depends on**: [01-exception-block-parser](01-exception-block-parser.md)

**Estimated Time**: 2-3 hours

### Scope

- `crates/fdemon-app/src/session.rs` — Add `ExceptionBlockParser` field and processing methods

### Session Integration

Add the parser as a field alongside the existing `LogBlockState`:

```rust
use fdemon_core::ExceptionBlockParser;

pub struct Session {
    // ... existing fields ...

    /// Block state for Logger package block level propagation
    block_state: LogBlockState,

    /// Exception block parser for multi-line Flutter exception detection
    exception_parser: ExceptionBlockParser,

    // ... rest of fields ...
}
```

### Processing Methods

```rust
impl Session {
    /// Process a raw line (from stderr or non-JSON stdout) through exception detection.
    ///
    /// Returns zero or more LogEntry items to be queued:
    /// - If the line is part of an exception block: returns empty (buffered)
    /// - If the line completes an exception block: returns the exception LogEntry
    /// - If the line is not part of an exception: returns a normal LogEntry
    /// - If the line is a "Another exception was thrown:" one-liner: returns an Error entry
    pub fn process_raw_line(&mut self, line: &str) -> Vec<LogEntry> {
        let cleaned = strip_ansi_codes(line);
        match self.exception_parser.feed_line(&cleaned) {
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
                let (level, message) = detect_raw_line_level(&cleaned);
                if message.is_empty() {
                    vec![]
                } else {
                    vec![LogEntry::new(level, LogSource::Flutter, message)]
                }
            }
        }
    }

    /// Flush any pending exception buffer (e.g., on session exit).
    ///
    /// Returns a LogEntry if there was a partial exception block being accumulated.
    pub fn flush_exception_buffer(&mut self) -> Option<LogEntry> {
        self.exception_parser.flush().map(|block| block.to_log_entry())
    }
}
```

### Constructor Update

Initialize the parser in `Session::new()`:

```rust
impl Session {
    pub fn new(/* existing params */) -> Self {
        Self {
            // ... existing fields ...
            block_state: LogBlockState::default(),
            exception_parser: ExceptionBlockParser::new(),
            // ... rest ...
        }
    }
}
```

### Interaction with Existing LogBlockState

The `process_raw_line()` method handles exception detection **before** the line reaches `add_log()`. The existing `LogBlockState` (Logger package ┌─┘ block detection) continues to work as-is inside `add_log()`, because:

1. Exception block lines are **consumed** by the parser and never reach `add_log()` individually
2. When the exception block completes, a **single** `LogEntry` is emitted (with stack trace), which goes through `queue_log()` → `add_log()` normally
3. Normal lines pass through as `NotConsumed` and follow the existing path

There is no conflict between the two mechanisms.

### Acceptance Criteria

1. [ ] `Session` struct has `exception_parser: ExceptionBlockParser` field
2. [ ] `Session::new()` initializes the parser
3. [ ] `process_raw_line()` routes lines through exception detection
4. [ ] `process_raw_line()` returns normal `LogEntry` for non-exception lines (using existing `detect_raw_line_level`)
5. [ ] `process_raw_line()` returns empty `Vec` for buffered exception lines
6. [ ] `process_raw_line()` returns exception `LogEntry` when block completes
7. [ ] `flush_exception_buffer()` returns partial block on session exit
8. [ ] No changes to existing `add_log()`, `queue_log()`, or `flush_batched_logs()` methods
9. [ ] Existing `LogBlockState` and Logger block propagation continue to work

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

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

        let entries = session.process_raw_line(
            "══╡ EXCEPTION CAUGHT BY WIDGETS LIBRARY ╞═══════════"
        );
        assert!(entries.is_empty()); // buffered, not emitted yet
    }

    #[test]
    fn test_process_raw_line_exception_complete() {
        let mut session = create_test_session();

        // Feed exception block
        assert!(session.process_raw_line(
            "══╡ EXCEPTION CAUGHT BY WIDGETS LIBRARY ╞═══════════"
        ).is_empty());
        assert!(session.process_raw_line("Error description").is_empty());
        assert!(session.process_raw_line(
            "#0      main (package:app/main.dart:15:3)"
        ).is_empty());

        // Footer completes the block
        let entries = session.process_raw_line(
            "════════════════════════════════════════════════════════"
        );
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].level, LogLevel::Error);
        assert!(entries[0].stack_trace.is_some());
    }

    #[test]
    fn test_process_raw_line_another_exception() {
        let mut session = create_test_session();

        let entries = session.process_raw_line(
            "Another exception was thrown: RangeError (index): Invalid value"
        );
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].level, LogLevel::Error);
    }

    #[test]
    fn test_flush_exception_buffer_on_exit() {
        let mut session = create_test_session();

        // Start an exception block but don't finish it
        session.process_raw_line(
            "══╡ EXCEPTION CAUGHT BY WIDGETS LIBRARY ╞═══════════"
        );
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
}
```

### Notes

- `process_raw_line()` encapsulates both exception detection AND the existing `detect_raw_line_level()` logic, providing a single entry point for the handler to use
- The method returns `Vec<LogEntry>` (not a single entry) to handle edge cases where the parser might emit multiple entries (e.g., a partial flush + normal line)
- ANSI stripping happens inside `process_raw_line()` once, avoiding double-stripping
- The existing `detect_raw_line_level` import needs to be accessible from `session.rs` (it's currently in `handler/helpers.rs`)

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/session.rs` | Added `exception_parser` field to Session struct, initialized in `Session::new()`, added `process_raw_line()` and `flush_exception_buffer()` methods, added 9 test cases |
| `crates/fdemon-core/src/exception_block.rs` | Added `#[derive(Debug)]` to `ExceptionBlockParser` struct to support Session's Debug derive |

### Notable Decisions/Tradeoffs

1. **ANSI Stripping**: The `ExceptionBlockParser::feed_line()` already strips ANSI codes internally, so `process_raw_line()` passes the raw line directly to the parser. For `NotConsumed` paths, we strip ANSI codes before calling `detect_raw_line_level()` to ensure consistent behavior.

2. **Import Organization**: Added `detect_raw_line_level` to the imports from `handler::helpers`, along with `ExceptionBlockParser`, `FeedResult`, and `strip_ansi_codes` from `fdemon_core`.

3. **Debug Derive**: Had to add `#[derive(Debug)]` to `ExceptionBlockParser` in fdemon-core because `Session` derives Debug and needs all its fields to implement Debug.

### Testing Performed

- `cargo test -p fdemon-app session::tests::test_process_raw_line` - Passed (6 tests)
- `cargo test -p fdemon-app session::tests::test_flush_exception_buffer` - Passed (2 tests)
- `cargo test -p fdemon-app session::tests::test_normal_lines_after_exception` - Passed (1 test)
- `cargo test -p fdemon-app` - Passed (755 tests, 5 ignored)
- `cargo clippy -p fdemon-app -- -D warnings` - Passed (no warnings)
- `cargo test --workspace --lib` - Passed (1604 tests total: 755 fdemon-app, 267 fdemon-core, 136 fdemon-daemon, 446 fdemon-tui)

### Test Coverage

Added 9 test cases covering:
- Normal line processing with level detection
- Exception header buffering
- Complete exception block with stack trace
- "Another exception" one-liner detection
- Flush on exit with partial block
- Empty flush when no pending exception
- Normal lines after exception completion
- ANSI code handling
- Empty line handling

### Acceptance Criteria Status

All acceptance criteria met:

1. ✅ `Session` struct has `exception_parser: ExceptionBlockParser` field
2. ✅ `Session::new()` initializes the parser
3. ✅ `process_raw_line()` routes lines through exception detection
4. ✅ `process_raw_line()` returns normal `LogEntry` for non-exception lines
5. ✅ `process_raw_line()` returns empty `Vec` for buffered exception lines
6. ✅ `process_raw_line()` returns exception `LogEntry` when block completes
7. ✅ `flush_exception_buffer()` returns partial block on session exit
8. ✅ No changes to existing `add_log()`, `queue_log()`, or `flush_batched_logs()` methods
9. ✅ Existing `LogBlockState` and Logger block propagation continue to work

### Risks/Limitations

1. **Parser State Persistence**: The exception parser maintains state across calls to `process_raw_line()`. If the handler doesn't properly flush on session exit, partial exception blocks could be lost. This is mitigated by providing the `flush_exception_buffer()` method.

2. **Memory Usage**: The parser buffers exception block lines up to MAX_EXCEPTION_BLOCK_LINES (500 lines). In pathological cases with extremely long exception blocks, this could use significant memory. However, the parser force-completes blocks that exceed this limit.

3. **No Breaking Changes**: The implementation is additive only - existing code paths remain unchanged. The new methods provide an optional enhanced path for exception detection that must be explicitly called by the handler.
