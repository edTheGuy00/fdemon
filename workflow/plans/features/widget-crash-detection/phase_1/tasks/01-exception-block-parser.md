## Task: Exception Block Parser

**Objective**: Create a line-by-line state machine parser in `fdemon-core` that detects Flutter framework exception blocks (══╡ EXCEPTION CAUGHT BY ╞══), accumulates their lines, and extracts the library name, error message, error-causing widget, and stack trace into a structured `ExceptionBlock` type.

**Depends on**: None

**Estimated Time**: 4-5 hours

### Scope

- `crates/fdemon-core/src/exception_block.rs` — **NEW** Exception block types and parser
- `crates/fdemon-core/src/lib.rs` — Export new public types

### Flutter Exception Block Format

The parser must handle this structure:

```
══╡ EXCEPTION CAUGHT BY WIDGETS LIBRARY ╞═════════════════════  ← HEADER
The following assertion was thrown building _CodeLine(...):      ← BODY (error description)
'package:flutter/.../container.dart': Failed assertion: ...     ← BODY (assertion detail)
                                                                ← BODY (blank line)
The relevant error-causing widget was:                          ← BODY (widget info marker)
  _CodeLine                                                     ← BODY (widget name)
  _CodeLine:file:///Users/.../ide_code_viewer.dart:72:22        ← BODY (widget location)
                                                                ← BODY (blank line)
When the exception was thrown, this was the stack:              ← STACK_TRACE marker
#2      new Container (package:flutter/.../container.dart:270:15)  ← STACK_TRACE
#3      _CodeLine.build (package:zabin_app/.../file.dart:141:16)   ← STACK_TRACE
...     Normal element mounting (131 frames)                       ← STACK_TRACE (elided)
(elided 2 frames from class _AssertionError)                      ← STACK_TRACE (elided)
════════════════════════════════════════════════════════════════  ← FOOTER
```

Also handle the compact follow-up format:
```
Another exception was thrown: RangeError (index): Invalid value: Not in inclusive range 0..2: 3
```

### Types

```rust
/// A parsed Flutter framework exception block
pub struct ExceptionBlock {
    /// The library that caught the exception (e.g., "WIDGETS LIBRARY", "RENDERING LIBRARY")
    pub library: String,

    /// The error description lines (between header and stack trace)
    pub description: String,

    /// The error-causing widget name, if identified
    pub widget_name: Option<String>,

    /// The error-causing widget source location, if identified
    pub widget_location: Option<String>,

    /// Raw stack trace text (all #N frame lines joined with newlines)
    pub stack_trace_text: String,

    /// Total lines in the original block (for diagnostics)
    pub line_count: usize,
}

/// Result of feeding a line to the parser
pub enum FeedResult {
    /// Line was consumed and buffered (part of an exception block)
    Buffered,

    /// Line is not part of an exception block — caller should handle it normally
    NotConsumed,

    /// An exception block is complete
    Complete(ExceptionBlock),

    /// A "Another exception was thrown:" one-liner was detected
    OneLineException(String),
}

/// Parser states
enum ParserState {
    /// Waiting for an exception block to start
    Idle,

    /// Inside the body (between header and stack trace)
    InBody,

    /// Inside the stack trace section
    InStackTrace,
}
```

### Parser State Machine

```
          ┌─────────┐
          │  Idle   │◄────────── default state
          └────┬────┘
               │ line contains "EXCEPTION CAUGHT BY" with ══╡ prefix
               ▼
          ┌─────────┐
          │ InBody  │◄────────── accumulate description lines
          └────┬────┘
               │ line matches "When the exception was thrown, this was the stack:"
               │ OR line starts with #<digit> (stack frame)
               ▼
        ┌────────────┐
        │InStackTrace│◄───────── accumulate stack trace frames
        └─────┬──────┘
              │ line is all ═ characters (footer) OR line count limit reached
              ▼
         ┌──────────┐
         │ Complete │──────────► emit ExceptionBlock, return to Idle
         └──────────┘
```

### Detection Patterns

```rust
/// Start marker: line contains "EXCEPTION CAUGHT BY" with ══╡ prefix
fn is_exception_header(line: &str) -> bool {
    // Strip ANSI codes first
    // Check for: ══╡ EXCEPTION CAUGHT BY <library> ╞══
    // Also handle: ═══ Exception caught by <library> ═══
}

/// End marker: line is entirely ═ characters (plus whitespace)
fn is_exception_footer(line: &str) -> bool {
    // Strip ANSI codes, trim whitespace
    // Check if remaining chars are all ═
    // Minimum length (e.g., 10) to avoid false positives on short lines
}

/// "Another exception was thrown:" one-liner
fn is_another_exception(line: &str) -> Option<String> {
    // Check for "Another exception was thrown: <message>"
    // Return the message portion
}

/// Stack trace section marker
fn is_stack_trace_marker(line: &str) -> bool {
    // "When the exception was thrown, this was the stack:"
}

/// Widget info section marker
fn is_widget_info_marker(line: &str) -> bool {
    // "The relevant error-causing widget was:"
}
```

### ExceptionBlock to LogEntry Conversion

```rust
impl ExceptionBlock {
    /// Convert to a LogEntry with parsed stack trace
    pub fn to_log_entry(&self) -> LogEntry {
        // Message format: "<LIBRARY>: <first line of description>"
        // If widget_name present: "<LIBRARY>: <description> — <widget_name>"
        let message = self.format_summary();

        // Parse stack trace using existing infrastructure
        let stack_trace = if !self.stack_trace_text.is_empty() {
            let parsed = ParsedStackTrace::parse(&self.stack_trace_text);
            if parsed.has_frames() { Some(parsed) } else { None }
        } else {
            None
        };

        // Create LogEntry at Error level
        if let Some(trace) = stack_trace {
            LogEntry::with_stack_trace(LogLevel::Error, LogSource::Flutter, message, trace)
        } else {
            LogEntry::error(LogSource::Flutter, message)
        }
    }

    /// Format a compact summary for the log message
    fn format_summary(&self) -> String {
        // Extract the first meaningful line from description
        // Append widget name if available
        // Keep it under ~120 chars for readability
    }
}
```

### Safety Limits

```rust
/// Maximum lines to buffer before force-flushing as incomplete
const MAX_EXCEPTION_BLOCK_LINES: usize = 500;
```

If the parser accumulates 500 lines without seeing a footer, it force-completes with whatever was collected. This prevents unbounded memory growth from malformed or truncated output.

### Acceptance Criteria

1. [ ] `ExceptionBlockParser` detects `══╡ EXCEPTION CAUGHT BY` headers (with ANSI stripping)
2. [ ] Parser accumulates body lines (description, widget info)
3. [ ] Parser transitions to stack trace mode on "When the exception was thrown..." or `#N` frame
4. [ ] Parser completes on `════...════` footer line
5. [ ] `ExceptionBlock` extracts: library name, description, widget name/location, stack trace text
6. [ ] `to_log_entry()` creates `LogEntry` with `LogLevel::Error` and parsed `ParsedStackTrace`
7. [ ] `FeedResult::OneLineException` handles "Another exception was thrown:" pattern
8. [ ] `FeedResult::NotConsumed` returns lines that aren't part of any exception block
9. [ ] Safety limit (500 lines) prevents unbounded buffering
10. [ ] `reset()` method to clear parser state (for session exit flush)
11. [ ] All detection functions strip ANSI codes before matching
12. [ ] Types exported from `fdemon-core/src/lib.rs`

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    // ─────────────────────────────────────────────
    // Header Detection
    // ─────────────────────────────────────────────

    #[test]
    fn test_detects_exception_header() {
        assert!(is_exception_header(
            "══╡ EXCEPTION CAUGHT BY WIDGETS LIBRARY ╞═══════════"
        ));
    }

    #[test]
    fn test_detects_exception_header_with_ansi() {
        assert!(is_exception_header(
            "\x1b[38;5;196m══╡ EXCEPTION CAUGHT BY RENDERING LIBRARY ╞═══\x1b[0m"
        ));
    }

    #[test]
    fn test_non_exception_header() {
        assert!(!is_exception_header("Just a normal log line"));
        assert!(!is_exception_header("══════════════════════")); // no EXCEPTION CAUGHT BY
    }

    // ─────────────────────────────────────────────
    // Footer Detection
    // ─────────────────────────────────────────────

    #[test]
    fn test_detects_exception_footer() {
        assert!(is_exception_footer(
            "════════════════════════════════════════════════════════"
        ));
    }

    #[test]
    fn test_short_equals_not_footer() {
        assert!(!is_exception_footer("═══")); // too short
    }

    // ─────────────────────────────────────────────
    // Another Exception One-Liner
    // ─────────────────────────────────────────────

    #[test]
    fn test_detects_another_exception() {
        let result = is_another_exception(
            "Another exception was thrown: RangeError (index): Invalid value"
        );
        assert_eq!(result, Some("RangeError (index): Invalid value".to_string()));
    }

    // ─────────────────────────────────────────────
    // Full Block Parsing
    // ─────────────────────────────────────────────

    #[test]
    fn test_parse_complete_exception_block() {
        let mut parser = ExceptionBlockParser::new();

        let lines = vec![
            "══╡ EXCEPTION CAUGHT BY WIDGETS LIBRARY ╞═══════════",
            "The following assertion was thrown building _CodeLine:",
            "'package:flutter/src/widgets/container.dart': Failed assertion: line 270",
            "",
            "The relevant error-causing widget was:",
            "  _CodeLine",
            "  _CodeLine:file:///Users/ed/.../ide_code_viewer.dart:72:22",
            "",
            "When the exception was thrown, this was the stack:",
            "#0      new Container (package:flutter/src/widgets/container.dart:270:15)",
            "#1      _CodeLine.build (package:zabin_app/.../ide_code_viewer.dart:141:16)",
            "(elided 2 frames from class _AssertionError)",
            "════════════════════════════════════════════════════════",
        ];

        let mut result = None;
        for line in &lines {
            match parser.feed_line(line) {
                FeedResult::Complete(block) => {
                    result = Some(block);
                    break;
                }
                FeedResult::Buffered => {}
                _ => panic!("unexpected result for line: {}", line),
            }
        }

        let block = result.expect("should have completed");
        assert_eq!(block.library, "WIDGETS LIBRARY");
        assert!(block.description.contains("Failed assertion"));
        assert_eq!(block.widget_name.as_deref(), Some("_CodeLine"));
        assert!(block.stack_trace_text.contains("#0"));
        assert!(block.stack_trace_text.contains("#1"));
    }

    #[test]
    fn test_parse_exception_block_to_log_entry() {
        // Parse a block, convert to LogEntry, verify stack trace is parsed
        let block = ExceptionBlock {
            library: "WIDGETS LIBRARY".to_string(),
            description: "Failed assertion: 'margin.isNonNegative'".to_string(),
            widget_name: Some("_CodeLine".to_string()),
            widget_location: Some("file:///Users/ed/.../file.dart:72:22".to_string()),
            stack_trace_text: "#0      new Container (package:flutter/src/widgets/container.dart:270:15)\n#1      _CodeLine.build (package:zabin_app/.../file.dart:141:16)".to_string(),
            line_count: 13,
        };

        let entry = block.to_log_entry();
        assert_eq!(entry.level, LogLevel::Error);
        assert!(entry.stack_trace.is_some());
        assert_eq!(entry.stack_trace.as_ref().unwrap().frame_count(), 2);
    }

    #[test]
    fn test_safety_limit_forces_completion() {
        let mut parser = ExceptionBlockParser::new();

        // Feed header
        parser.feed_line("══╡ EXCEPTION CAUGHT BY WIDGETS LIBRARY ╞═══════════");

        // Feed 500 body lines without footer
        for i in 0..MAX_EXCEPTION_BLOCK_LINES {
            let result = parser.feed_line(&format!("line {}", i));
            if matches!(result, FeedResult::Complete(_)) {
                // Should force-complete at the limit
                return;
            }
        }

        panic!("parser should have force-completed at line limit");
    }

    #[test]
    fn test_non_exception_lines_pass_through() {
        let mut parser = ExceptionBlockParser::new();

        assert!(matches!(
            parser.feed_line("Just a normal log line"),
            FeedResult::NotConsumed
        ));
        assert!(matches!(
            parser.feed_line("flutter: Hello World"),
            FeedResult::NotConsumed
        ));
    }

    #[test]
    fn test_parser_resets_after_completion() {
        let mut parser = ExceptionBlockParser::new();

        // Parse one block
        parser.feed_line("══╡ EXCEPTION CAUGHT BY WIDGETS LIBRARY ╞═══════════");
        parser.feed_line("Error description");
        let result = parser.feed_line("════════════════════════════════════════════════════════");
        assert!(matches!(result, FeedResult::Complete(_)));

        // Parser should be back to Idle
        assert!(matches!(
            parser.feed_line("Normal line after exception"),
            FeedResult::NotConsumed
        ));
    }

    #[test]
    fn test_flush_incomplete_returns_partial_block() {
        let mut parser = ExceptionBlockParser::new();

        parser.feed_line("══╡ EXCEPTION CAUGHT BY WIDGETS LIBRARY ╞═══════════");
        parser.feed_line("Error description");
        // Don't send footer — simulate session exit

        let partial = parser.flush();
        assert!(partial.is_some());
        let block = partial.unwrap();
        assert_eq!(block.library, "WIDGETS LIBRARY");
    }
}
```

### Notes

- The parser is designed to be used per-session, not globally. Each session gets its own `ExceptionBlockParser` instance.
- ANSI code stripping uses the existing `strip_ansi_codes()` from `fdemon-core/src/ansi.rs`.
- The `to_log_entry()` method reuses `ParsedStackTrace::parse()` — no new parsing logic needed for the stack frames.
- The message summary should be concise enough to be readable in the log view's single message line, with the full details available in the expanded stack trace.
- Flutter uses `debugPrint()` which may word-wrap long lines. The parser should handle lines that are continuations of previous lines (no `#N` prefix, no `══` markers).

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-core/src/exception_block.rs` | NEW - Complete exception block parser with state machine, detection functions, and 24 tests |
| `crates/fdemon-core/src/lib.rs` | Added `exception_block` module and re-exported `ExceptionBlock`, `ExceptionBlockParser`, `FeedResult` |

### Implementation Details

**ExceptionBlock struct** - Fully implemented with:
- Library name, description, widget name/location, stack trace text, line count
- `to_log_entry()` conversion that integrates with existing `ParsedStackTrace::parse()`
- `format_summary()` for compact display with truncation (~120 chars)

**ExceptionBlockParser state machine** - Implements 3-state parser (Idle → InBody → InStackTrace):
- `feed_line()` method strips ANSI codes and processes line-by-line
- Detects exception headers and extracts library name
- Accumulates description lines between header and stack trace
- Detects and extracts widget name/location after "The relevant error-causing widget was:" marker
- Transitions to stack trace mode on "When the exception was thrown..." or direct `#N` frame
- Completes on footer detection (all `═` characters)
- Force-completes at MAX_EXCEPTION_BLOCK_LINES (500) safety limit
- Auto-resets to Idle after completion
- `flush()` method for partial block recovery on session exit
- `reset()` method for manual state reset

**Detection functions**:
- `extract_library_from_header()` - Extracts library name from `══╡ EXCEPTION CAUGHT BY <LIBRARY> ╞══`
- `is_exception_footer()` - Detects all-equals footer lines (min 10 chars)
- `is_another_exception()` - Detects and extracts "Another exception was thrown:" one-liners
- `is_stack_trace_marker()` - Detects "When the exception was thrown, this was the stack:"
- `is_widget_info_marker()` - Detects "The relevant error-causing widget was:"

**FeedResult enum**:
- `Buffered` - Line consumed, continue parsing
- `NotConsumed` - Line not part of exception block, caller should handle normally
- `Complete(ExceptionBlock)` - Block finished, contains parsed data
- `OneLineException(String)` - "Another exception" one-liner detected

### Notable Decisions/Tradeoffs

1. **State machine over regex**: Used explicit state machine for better control over multi-line parsing and widget extraction. This makes the parser more maintainable and easier to debug.

2. **ANSI stripping at entry**: Strip ANSI codes in `feed_line()` at the start, ensuring all detection logic works on clean text. Reuses existing `strip_ansi_codes()` infrastructure.

3. **Widget name/location extraction**: Used simple heuristic - first non-empty line after marker is widget name, next line with colon is location. This handles the common case without complex parsing.

4. **Direct stack frame detection**: Parser transitions to InStackTrace on either the marker line OR a line starting with `#<digit>`. This handles cases where the marker is missing.

5. **Safety limit**: MAX_EXCEPTION_BLOCK_LINES = 500 prevents unbounded memory growth from malformed output. Force-completes with whatever has been accumulated.

6. **Auto-reset on completion**: Parser automatically resets to Idle after returning `FeedResult::Complete`, enabling continuous parsing of multiple exception blocks.

7. **Summary truncation**: `format_summary()` truncates to ~120 chars for readability in single-line log view, with full details accessible in expanded stack trace.

### Testing Performed

- `cargo test -p fdemon-core exception_block` - 24 tests passed
- `cargo test -p fdemon-core` - All 267 tests passed (no regressions)
- `cargo test --workspace --lib` - All 751 workspace tests passed
- `cargo clippy -p fdemon-core -- -D warnings` - Clean (0 warnings)

**Test coverage**:
- Header detection with/without ANSI codes
- Footer detection with/without ANSI codes
- Library name extraction
- "Another exception" one-liner detection
- Complete block parsing with description, widget info, and stack trace
- Widget name and location extraction
- Multi-line descriptions
- Direct stack frame transition (without marker)
- Exception blocks without stack traces
- Safety limit force-completion
- Parser reset after completion
- Incomplete block flushing
- Non-exception lines pass-through
- `to_log_entry()` conversion with stack trace parsing
- `format_summary()` with/without widget, with truncation

### Risks/Limitations

1. **Widget extraction heuristic**: Simple pattern matching may not handle all edge cases (e.g., multi-line widget names, unusual formatting). Should be tested with real-world exception output.

2. **No validation of library names**: Parser accepts any text between "EXCEPTION CAUGHT BY" and " ╞". Could add validation if specific library names are known.

3. **Stack frame detection**: Relies on `#<digit>` pattern at start of line. May miss frames with unusual formatting.

4. **Memory usage**: Buffers up to 500 lines per exception block. For extremely large stack traces, this could be significant. The limit is defensive but may need tuning based on real-world usage.

5. **No backtracking**: State machine doesn't backtrack on ambiguous input. If a line looks like a stack frame but isn't, it will be treated as one. This is by design for simplicity and performance.
