## Task: Strip ANSI Escape Codes from Log Messages

**Objective**: Remove ANSI escape sequences from incoming Flutter log messages to ensure clean display in the TUI and enable accurate log level detection.

**Depends on**: None (can be done independently)

### Background

The Flutter `logger` package and other logging libraries output ANSI escape codes for terminal coloring:
- `\x1b[38;5;244m` - Set foreground to color 244 (gray)
- `\x1b[38;5;12m` - Set foreground to color 12 (blue)
- `\x1b[38;5;208m` - Set foreground to color 208 (orange)
- `\x1b[0m` - Reset all formatting

These codes appear as garbage in Flutter Demon's TUI (e.g., `^[[38;5;244m`) because:
1. The TUI applies its own styling via Ratatui
2. Raw escape codes are displayed as literal text instead of being interpreted

### Scope

- `src/core/ansi.rs`: **NEW** - Create utility module for ANSI code handling
- `src/core/mod.rs`: Add `ansi` module export
- `src/daemon/protocol.rs`: Apply stripping in `parse_flutter_log()`
- `src/app/handler/helpers.rs`: Apply stripping in `detect_raw_line_level()`

### Implementation

#### 1. Create ANSI Stripping Utility

```rust
// src/core/ansi.rs

use std::sync::LazyLock;
use regex::Regex;

/// Regex pattern for ANSI escape sequences
/// Matches: ESC [ (params) (command)
/// Examples: \x1b[0m, \x1b[38;5;244m, \x1b[1;31m
static ANSI_ESCAPE_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    // Comprehensive pattern covering:
    // - CSI sequences: ESC [ ... letter (colors, cursor, etc.)
    // - OSC sequences: ESC ] ... BEL or ST (hyperlinks, titles)
    // - Simple escapes: ESC letter
    Regex::new(r"\x1b\[[0-9;?]*[A-Za-z]|\x1b\][^\x07\x1b]*(?:\x07|\x1b\\)|\x1b[A-Za-z]")
        .expect("ANSI regex pattern is valid")
});

/// Strip all ANSI escape sequences from a string
pub fn strip_ansi_codes(input: &str) -> String {
    ANSI_ESCAPE_PATTERN.replace_all(input, "").into_owned()
}

/// Check if a string contains ANSI escape sequences
pub fn contains_ansi_codes(input: &str) -> bool {
    ANSI_ESCAPE_PATTERN.is_match(input)
}
```

#### 2. Apply to Log Processing

In `src/daemon/protocol.rs`:

```rust
use crate::core::strip_ansi_codes;

pub fn parse_flutter_log(raw: &str, is_error: bool) -> (LogLevel, String) {
    // Strip ANSI codes first
    let cleaned = strip_ansi_codes(raw);
    let message = cleaned.trim();
    
    // ... rest of existing logic using `message`
}
```

In `src/app/handler/helpers.rs`:

```rust
use crate::core::strip_ansi_codes;

pub fn detect_raw_line_level(line: &str) -> (LogLevel, String) {
    // Strip ANSI codes first
    let cleaned = strip_ansi_codes(line);
    let trimmed = cleaned.trim();
    
    // ... rest of existing logic using `trimmed`
}
```

### ANSI Escape Sequences Reference

| Pattern | Description | Example |
|---------|-------------|---------|
| `\x1b[0m` | Reset all attributes | Reset colors |
| `\x1b[1m` | Bold | |
| `\x1b[30-37m` | Foreground color (8 colors) | `\x1b[31m` = red |
| `\x1b[38;5;Nm` | 256-color foreground | `\x1b[38;5;244m` = gray |
| `\x1b[38;2;R;G;Bm` | 24-bit RGB foreground | |
| `\x1b[40-47m` | Background color (8 colors) | |
| `\x1b[48;5;Nm` | 256-color background | |
| `\x1b]8;;URL\x1b\\` | OSC 8 hyperlink | |

### What to Preserve

- Unicode box-drawing characters: `┌ │ └ ├ ┄ ─` (these are valid UTF-8, not escape codes)
- Unicode emojis: 🐛 💡 ⚠️ ⛔ 🔥 (used by Logger package for levels)
- All other visible text content

### Acceptance Criteria

1. [x] `strip_ansi_codes()` function removes all ANSI escape sequences
2. [x] Box-drawing characters (┌│└├┄─) are preserved
3. [x] Emoji characters are preserved
4. [x] `parse_flutter_log()` applies stripping before processing
5. [x] `detect_raw_line_level()` applies stripping before processing
6. [x] Log messages no longer show `^[[38;5;244m` style garbage
7. [x] Unit tests cover common ANSI patterns
8. [x] Unit tests verify box-drawing and emoji preservation
9. [x] No regressions in existing log display

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_simple_color_codes() {
        let input = "\x1b[31mred text\x1b[0m";
        assert_eq!(strip_ansi_codes(input), "red text");
    }

    #[test]
    fn test_strip_256_color_codes() {
        let input = "\x1b[38;5;244m│ Trace: message\x1b[0m";
        assert_eq!(strip_ansi_codes(input), "│ Trace: message");
    }

    #[test]
    fn test_strip_multiple_codes() {
        let input = "\x1b[1m\x1b[38;5;12mBold blue\x1b[0m";
        assert_eq!(strip_ansi_codes(input), "Bold blue");
    }

    #[test]
    fn test_preserve_box_drawing() {
        let input = "┌─────────┐\n│ Message │\n└─────────┘";
        assert_eq!(strip_ansi_codes(input), input);
    }

    #[test]
    fn test_preserve_emojis() {
        let input = "🐛 Debug: message";
        assert_eq!(strip_ansi_codes(input), input);
    }

    #[test]
    fn test_mixed_content() {
        let input = "\x1b[38;5;244m┌───────────\x1b[0m\n\x1b[38;5;244m│ 🐛 Debug\x1b[0m";
        assert_eq!(strip_ansi_codes(input), "┌───────────\n│ 🐛 Debug");
    }

    #[test]
    fn test_no_codes() {
        let input = "Plain text with no codes";
        assert_eq!(strip_ansi_codes(input), input);
    }

    #[test]
    fn test_empty_string() {
        assert_eq!(strip_ansi_codes(""), "");
    }

    #[test]
    fn test_contains_ansi_codes() {
        assert!(contains_ansi_codes("\x1b[31mred\x1b[0m"));
        assert!(!contains_ansi_codes("plain text"));
    }

    #[test]
    fn test_logger_package_output() {
        // Real Logger package output sample
        let input = "\x1b[38;5;244m│  Trace: Very detailed debugging info\x1b[0m";
        let result = strip_ansi_codes(input);
        assert_eq!(result, "│  Trace: Very detailed debugging info");
        assert!(result.contains("Trace:"));
    }
}
```

### Integration Tests

After implementation, manually verify with the sample app:
1. Run Flutter Demon with `sample/` project
2. Tap "All Levels" button in "Logger Package" section
3. Verify logs display cleanly without `^[[...m` garbage
4. Verify box-drawing characters still appear (┌│└├)
5. Verify emojis still appear (🐛💡⚠️⛔🔥)

### Files to Create/Modify

| File | Action | Description |
|------|--------|-------------|
| `src/core/ansi.rs` | Create | New module with ANSI stripping utilities |
| `src/core/mod.rs` | Modify | Add `pub mod ansi;` and re-export |
| `src/core/types.rs` | Modify | Apply `strip_ansi_codes` in `LogEntry::new()` |
| `src/core/stack_trace.rs` | Modify | Apply `strip_ansi_codes` in `ParsedStackTrace::parse()` |
| `src/daemon/protocol.rs` | Modify | Apply `strip_ansi_codes` in `parse_flutter_log` |
| `src/app/handler/helpers.rs` | Modify | Apply `strip_ansi_codes` in `detect_raw_line_level` |

### Estimated Time

2-3 hours

### References

- [ANSI Escape Code Wikipedia](https://en.wikipedia.org/wiki/ANSI_escape_code)
- [Logger Package Source](https://github.com/simc/logger) - see PrettyPrinter output
- `regex` crate documentation

---

## Completion Summary

**Status:** ✅ Done

**Files Modified:**
- `src/core/ansi.rs` (NEW) - Created ANSI stripping utilities with `strip_ansi_codes()` and `contains_ansi_codes()` functions
- `src/core/mod.rs` - Added `pub mod ansi;` and re-exported functions
- `src/core/types.rs` - Applied ANSI stripping in `LogEntry::new()` to ensure ALL log entries are cleaned
- `src/core/stack_trace.rs` - Applied ANSI stripping in `ParsedStackTrace::parse()` for stack trace content
- `src/daemon/protocol.rs` - Applied `strip_ansi_codes` in `parse_flutter_log()` before any processing
- `src/app/handler/helpers.rs` - Applied `strip_ansi_codes` in `detect_raw_line_level()` before any processing

**Notable Decisions/Tradeoffs:**
- Used `LazyLock` for compiled regex pattern to avoid recompilation on each call
- Comprehensive regex pattern covers CSI sequences (colors, cursor), OSC sequences (hyperlinks, titles), and simple escape sequences
- **Critical fix:** Applied stripping at `LogEntry::new()` level to ensure ALL log entries are cleaned regardless of entry point (stdout, stderr, direct creation). This was essential because stderr logs were bypassing the protocol-level stripping.
- Also applied stripping to stack trace parsing to ensure parsed frames don't contain ANSI codes

**Testing Performed:**
- `cargo test core::ansi` - 21 tests passed covering:
  - Simple color codes (8-color, 256-color, RGB)
  - Background colors
  - Bold and combined attributes
  - Cursor movement codes
  - OSC hyperlinks and window titles
  - Box-drawing character preservation
  - Emoji preservation
  - Logger package real-world output samples
- `cargo test core::types` - 72 tests passed, including new `test_log_entry_strips_ansi_codes`
- `cargo test core::stack_trace` - 56 tests passed, no regressions
- `cargo test daemon::protocol` - 40 tests passed, no regressions
- `cargo test app::handler` - 94 tests passed, no regressions
- `cargo check` - Compiles successfully
- `cargo clippy` - No warnings

**Risks/Limitations:**
- The regex pattern covers common ANSI sequences but may not catch all edge cases (extremely rare sequences)
- Performance is optimal due to LazyLock - regex is compiled once and reused
- Stripping happens multiple times for some paths (e.g., protocol + LogEntry), but this is intentional for robustness and has negligible performance impact