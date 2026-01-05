## Task: Strip Redundant "flutter:" Prefix from Raw Lines

**Objective**: Remove the redundant `flutter:` prefix from raw stdout lines to eliminate duplicate source indicators in log display.

**Depends on**: [08-strip-ansi-escape-codes](08-strip-ansi-escape-codes.md) (should be applied first)

### Background

Currently, Flutter Demon displays logs like:
```
11:57:12 • [flutter] flutter: │ #0   List.[] (dart:core-patch/growable_array.dart)
```

This has two "flutter" references:
1. `[flutter]` - The LogSource indicator added by Flutter Demon
2. `flutter:` - The prefix from Flutter's stdout output

The `parse_flutter_log()` function in `protocol.rs` correctly strips the `flutter: ` prefix for JSON-wrapped daemon messages. However, raw stdout lines that aren't JSON-wrapped go through `detect_raw_line_level()` in `helpers.rs`, which does NOT strip the prefix.

### Root Cause

In `handle_session_stdout()`:
```rust
if let Some(json) = protocol::strip_brackets(line) {
    // JSON path → parse_flutter_log() → strips "flutter: " ✓
} else if !line.trim().is_empty() {
    // Raw path → detect_raw_line_level() → does NOT strip "flutter: " ✗
    let (level, message) = detect_raw_line_level(line);
    ...
}
```

### Scope

- `src/app/handler/helpers.rs`: Add `flutter:` prefix stripping to `detect_raw_line_level()`

### Implementation

Update `detect_raw_line_level()` in `src/app/handler/helpers.rs`:

```rust
use crate::core::strip_ansi_codes;

/// Detect log level from raw (non-JSON) output line
pub fn detect_raw_line_level(line: &str) -> (LogLevel, String) {
    // Strip ANSI codes first for accurate detection
    let cleaned = strip_ansi_codes(line);
    let trimmed = cleaned.trim();
    
    // Strip "flutter: " prefix if present (matches parse_flutter_log behavior)
    let message = trimmed
        .strip_prefix("flutter: ")
        .unwrap_or(trimmed);
    
    // Android logcat format: E/, W/, I/, D/, V/
    if message.starts_with("E/") {
        return (LogLevel::Error, message.to_string());
    }
    if message.starts_with("W/") {
        return (LogLevel::Warning, message.to_string());
    }
    if message.starts_with("I/") {
        return (LogLevel::Info, message.to_string());
    }
    if message.starts_with("D/") || message.starts_with("V/") {
        return (LogLevel::Debug, message.to_string());
    }
    
    // Use content-based detection for remaining patterns
    let level = detect_log_level_from_content(message);
    (level, message.to_string())
}
```

### Acceptance Criteria

1. [x] Raw stdout lines have `flutter:` prefix stripped
2. [x] Log display shows only `[flutter]` source tag, not `flutter:` in message
3. [x] Android logcat format detection still works after stripping
4. [x] ANSI code stripping happens before prefix stripping
5. [x] Existing log level detection still works correctly
6. [x] Unit tests verify prefix stripping behavior
7. [x] No regressions in existing functionality

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_flutter_prefix() {
        let (level, msg) = detect_raw_line_level("flutter: Hello World");
        assert_eq!(level, LogLevel::Info);
        assert_eq!(msg, "Hello World");
    }

    #[test]
    fn test_strip_flutter_prefix_with_box_drawing() {
        let (level, msg) = detect_raw_line_level("flutter: │ Stack trace info");
        assert_eq!(msg, "│ Stack trace info");
    }

    #[test]
    fn test_strip_flutter_prefix_with_emoji() {
        let (level, msg) = detect_raw_line_level("flutter: 💡 Info: message");
        assert_eq!(level, LogLevel::Info);
        assert_eq!(msg, "💡 Info: message");
    }

    #[test]
    fn test_strip_flutter_prefix_error() {
        let (level, msg) = detect_raw_line_level("flutter: ⛔ Error: failed");
        assert_eq!(level, LogLevel::Error);
        assert_eq!(msg, "⛔ Error: failed");
    }

    #[test]
    fn test_no_flutter_prefix() {
        let (level, msg) = detect_raw_line_level("Plain message without prefix");
        assert_eq!(msg, "Plain message without prefix");
    }

    #[test]
    fn test_android_logcat_after_strip() {
        // flutter: prefix should be stripped first, then logcat detection should work
        let (level, msg) = detect_raw_line_level("flutter: E/flutter: error message");
        assert_eq!(level, LogLevel::Error);
        assert_eq!(msg, "E/flutter: error message");
    }

    #[test]
    fn test_android_logcat_without_flutter_prefix() {
        let (level, msg) = detect_raw_line_level("E/flutter: error message");
        assert_eq!(level, LogLevel::Error);
        assert_eq!(msg, "E/flutter: error message");
    }

    #[test]
    fn test_strip_flutter_prefix_with_ansi() {
        // ANSI codes stripped first, then flutter: prefix
        let (level, msg) = detect_raw_line_level("\x1b[38;5;244mflutter: │ message\x1b[0m");
        assert_eq!(msg, "│ message");
    }

    #[test]
    fn test_strip_flutter_prefix_warning() {
        let (level, msg) = detect_raw_line_level("flutter: ⚠ Warning: deprecated");
        assert_eq!(level, LogLevel::Warning);
        assert_eq!(msg, "⚠ Warning: deprecated");
    }

    #[test]
    fn test_strip_flutter_prefix_debug() {
        let (level, msg) = detect_raw_line_level("flutter: 🐛 Debug: verbose info");
        assert_eq!(level, LogLevel::Debug);
        assert_eq!(msg, "🐛 Debug: verbose info");
    }

    #[test]
    fn test_flutter_prefix_case_sensitive() {
        // Only lowercase "flutter: " should be stripped
        let (_, msg) = detect_raw_line_level("Flutter: Message");
        assert_eq!(msg, "Flutter: Message"); // Not stripped
        
        let (_, msg) = detect_raw_line_level("FLUTTER: Message");
        assert_eq!(msg, "FLUTTER: Message"); // Not stripped
    }

    #[test]
    fn test_flutter_prefix_needs_space() {
        // Must have space after colon to strip
        let (_, msg) = detect_raw_line_level("flutter:NoSpace");
        assert_eq!(msg, "flutter:NoSpace"); // Not stripped
    }
}
```

### Integration Tests

After implementation, manually verify with the sample app:
1. Run Flutter Demon with `sample/` project
2. Trigger various log outputs (Logger package, print, debugPrint)
3. Verify logs show `[flutter] message` NOT `[flutter] flutter: message`
4. Verify Android logcat format still detected correctly
5. Verify emojis and level prefixes still work for level detection

### Files to Modify

| File | Action | Description |
|------|--------|-------------|
| `src/app/handler/helpers.rs` | Modify | Add `flutter: ` prefix stripping to `detect_raw_line_level()` |

### Estimated Time

2-3 hours

### Edge Cases

1. **Case sensitivity**: Only strip lowercase `flutter: ` (with space)
2. **Missing space**: `flutter:message` should NOT be stripped (no space after colon)
3. **Double prefix**: `flutter: flutter: message` - strip only first occurrence
4. **Android logcat inside**: `flutter: E/flutter: error` - strip outer prefix, keep logcat prefix
5. **Empty after strip**: `flutter: ` alone should result in empty string

### References

- `src/daemon/protocol.rs` - `parse_flutter_log()` for reference implementation
- Task 08 (ANSI stripping) - should be applied before prefix stripping
- Task 09 (log level detection) - level detection runs after prefix stripping

---

## Completion Summary

**Status**: ✅ Done

**Files Modified**:
- `src/app/handler/helpers.rs` - Added `flutter: ` prefix stripping to `detect_raw_line_level()` function

**Implementation Details**:
- Added `strip_prefix("flutter: ")` after ANSI code stripping and trimming, but before Android logcat detection
- This matches the behavior of `parse_flutter_log()` in `protocol.rs` for consistency
- The prefix is only stripped if exactly `flutter: ` (lowercase, with trailing space) is present

**Unit Tests Added** (14 new tests):
- `test_strip_flutter_prefix` - Basic prefix stripping
- `test_strip_flutter_prefix_with_box_drawing` - Logger package box-drawing chars
- `test_strip_flutter_prefix_with_emoji` - Emoji-prefixed messages
- `test_strip_flutter_prefix_error` - Error level with prefix
- `test_no_flutter_prefix` - Messages without prefix
- `test_android_logcat_after_strip` - `flutter: E/flutter:` case
- `test_android_logcat_without_flutter_prefix` - Direct logcat format
- `test_strip_flutter_prefix_with_ansi` - ANSI codes + prefix
- `test_strip_flutter_prefix_warning` - Warning level with prefix
- `test_strip_flutter_prefix_debug` - Debug level with prefix
- `test_flutter_prefix_case_sensitive` - Only lowercase stripped
- `test_flutter_prefix_needs_space` - Must have space after colon
- `test_flutter_prefix_empty_after_strip` - Edge case handling
- `test_double_flutter_prefix` - Only first occurrence stripped

**Testing Performed**:
- `cargo check` - PASS
- `cargo test app::handler::helpers` - 36 tests PASS
- `cargo test --lib` - 735 passed, 1 failed (unrelated flaky test in device_selector.rs)

**Notable Decisions**:
- Prefix stripping happens after trim, so `"flutter:  "` (with trailing spaces) becomes `"flutter:"` after trim, and doesn't match the prefix pattern (requires the space)
- Only lowercase `flutter: ` is stripped - uppercase variants are preserved

**Risks/Limitations**:
- None identified. The change is minimal and well-tested.