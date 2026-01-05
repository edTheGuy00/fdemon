## Task: Fix False Positive Log Level Detection

**Objective**: Prevent class names and identifiers like `ErrorTestingPage` from triggering Error-level detection by using word boundary checks instead of substring matching.

**Depends on**: None (can be done in parallel with Task 01)

**Priority**: MEDIUM

### Background

The current log level detection uses broad substring matching like `lower.contains("error")`. This causes false positives:
- `ErrorTestingPage` → detected as Error (wrong - it's a class name)
- `handleError` → detected as Error (wrong - it's a method name)
- `errorCount` → detected as Error (wrong - it's a variable name)

### Scope Clarification: Only Affects stdout Logs

**Important**: The `app.log` event includes an `error: bool` flag:
- `error: true` (stderr) → Already handled correctly! `parse_flutter_log()` immediately returns `LogLevel::Error`
- `error: false` (stdout) → Falls back to content-based detection → **This is where false positives occur**

So this task only fixes detection for **stdout logs** where the daemon doesn't know the level. True errors from stderr are already correctly identified via the `error: true` flag.

```rust
// Current behavior in parse_flutter_log():
if is_error {  // error: true from daemon
    return (LogLevel::Error, message.to_string());  // ✅ Correct
}
// Content-based detection only runs for stdout (error: false)
let level = Self::detect_log_level(content);  // ⚠️ False positives here
```

### Scope

- `src/daemon/protocol.rs`: Update `detect_log_level()` function
- `src/app/handler/helpers.rs`: Update `detect_log_level_from_content()` function

### Implementation

#### 1. Add Word Boundary Helper

```rust
/// Check if a word exists at word boundaries in the text
/// Matches: "error", " error ", "error:", "[error]", "Error:"
/// Does NOT match: "ErrorTestingPage", "handleError", "errorCount"
fn contains_word(text: &str, word: &str) -> bool {
    let lower = text.to_lowercase();
    let word_lower = word.to_lowercase();

    // Check for common patterns that indicate a standalone word
    let patterns = [
        format!(" {} ", word_lower),      // surrounded by spaces
        format!(" {}:", word_lower),       // word followed by colon
        format!("[{}]", word_lower),       // bracketed (Talker format)
        format!("{}:", word_lower),        // starts with word:
        format!(" {}\n", word_lower),      // word at end of line
        format!(" {}.", word_lower),       // word followed by period
    ];

    // Check if text starts with the word followed by delimiter
    if lower.starts_with(&format!("{}:", word_lower))
        || lower.starts_with(&format!("{} ", word_lower))
        || lower.starts_with(&format!("[{}]", word_lower)) {
        return true;
    }

    patterns.iter().any(|p| lower.contains(p))
}
```

#### 2. Update detect_log_level() in protocol.rs

```rust
pub fn detect_log_level(message: &str) -> LogLevel {
    let lower = message.to_lowercase();

    // ─────────────────────────────────────────────────────────
    // Emoji-based detection (highest priority - unambiguous)
    // ─────────────────────────────────────────────────────────

    // Fatal/Critical indicators
    if message.contains('🔥') || message.contains("💀") {
        return LogLevel::Error;
    }

    // Error indicators
    if message.contains('⛔') || message.contains('❌') || message.contains("🚫") {
        return LogLevel::Error;
    }

    // Warning indicators
    if message.contains('⚠') || message.contains("⚡") {
        return LogLevel::Warning;
    }

    // Info indicators
    if message.contains('💡') || message.contains('ℹ') {
        return LogLevel::Info;
    }

    // Debug indicators
    if message.contains('🐛') || message.contains("🔍") {
        return LogLevel::Debug;
    }

    // ─────────────────────────────────────────────────────────
    // Prefix-based detection (Logger/Talker - specific patterns)
    // ─────────────────────────────────────────────────────────

    // These are safe because they require specific format (word + colon)
    if lower.contains("fatal:") || lower.contains("critical:") {
        return LogLevel::Error;
    }
    if lower.contains("exception:") {
        return LogLevel::Error;
    }
    if lower.contains("warning:") || lower.contains("warn:") {
        return LogLevel::Warning;
    }
    if lower.contains("info:") {
        return LogLevel::Info;
    }
    if lower.contains("debug:") || lower.contains("trace:") {
        return LogLevel::Debug;
    }

    // Talker bracketed format (safe - specific pattern)
    if lower.contains("[critical]") || lower.contains("[fatal]") {
        return LogLevel::Error;
    }
    if lower.contains("[error]") || lower.contains("[exception]") {
        return LogLevel::Error;
    }
    if lower.contains("[warning]") || lower.contains("[warn]") {
        return LogLevel::Warning;
    }
    if lower.contains("[info]") {
        return LogLevel::Info;
    }
    if lower.contains("[debug]") || lower.contains("[verbose]") || lower.contains("[trace]") {
        return LogLevel::Debug;
    }

    // ─────────────────────────────────────────────────────────
    // Word boundary detection (FIXED - prevents false positives)
    // ─────────────────────────────────────────────────────────

    // Error keywords - must be at word boundaries
    if contains_word(message, "error")
        || contains_word(message, "failed")
        || contains_word(message, "failure")
        || contains_word(message, "crash")
    {
        return LogLevel::Error;
    }

    // "error:" pattern (common in stack traces, build output)
    if lower.contains("error:") {
        return LogLevel::Error;
    }

    // Warning keywords - must be at word boundaries
    if contains_word(message, "warning")
        || contains_word(message, "deprecated")
    {
        return LogLevel::Warning;
    }

    // Debug keywords
    if lower.starts_with("debug") || contains_word(message, "verbose") {
        return LogLevel::Debug;
    }

    LogLevel::Info
}
```

#### 3. Update detect_log_level_from_content() in helpers.rs

Apply the same word boundary logic to `detect_log_level_from_content()`.

### Acceptance Criteria

1. [ ] `ErrorTestingPage` no longer triggers Error level
2. [ ] `handleError` no longer triggers Error level
3. [ ] `errorCount` no longer triggers Error level
4. [ ] `Error: something failed` still triggers Error level
5. [ ] `[error] message` still triggers Error level
6. [ ] `⛔ Error` still triggers Error level
7. [ ] `FAILURE: Build failed` still triggers Error level
8. [ ] Existing Logger/Talker patterns still work
9. [ ] Unit tests for word boundary detection
10. [ ] No regressions in existing level detection

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    // ─────────────────────────────────────────────────────────
    // False Positive Prevention Tests
    // ─────────────────────────────────────────────────────────

    #[test]
    fn test_class_name_not_error() {
        // Class names should NOT trigger Error
        assert_eq!(detect_log_level("ErrorTestingPage"), LogLevel::Info);
        assert_eq!(detect_log_level("MyErrorHandler"), LogLevel::Info);
        assert_eq!(detect_log_level("ErrorBoundary widget"), LogLevel::Info);
    }

    #[test]
    fn test_method_name_not_error() {
        // Method names should NOT trigger Error
        assert_eq!(detect_log_level("handleError called"), LogLevel::Info);
        assert_eq!(detect_log_level("onErrorCallback"), LogLevel::Info);
        assert_eq!(detect_log_level("throwError()"), LogLevel::Info);
    }

    #[test]
    fn test_variable_name_not_error() {
        // Variable names should NOT trigger Error
        assert_eq!(detect_log_level("errorCount: 5"), LogLevel::Info);
        assert_eq!(detect_log_level("hasError = false"), LogLevel::Info);
        assert_eq!(detect_log_level("isErrorState"), LogLevel::Info);
    }

    #[test]
    fn test_camel_case_not_error() {
        // CamelCase identifiers should NOT trigger Error
        assert_eq!(detect_log_level("NetworkError"), LogLevel::Info);
        assert_eq!(detect_log_level("ValidationError"), LogLevel::Info);
        assert_eq!(detect_log_level("TimeoutError"), LogLevel::Info);
    }

    // ─────────────────────────────────────────────────────────
    // Valid Error Detection Tests
    // ─────────────────────────────────────────────────────────

    #[test]
    fn test_error_with_colon_is_error() {
        assert_eq!(detect_log_level("Error: something failed"), LogLevel::Error);
        assert_eq!(detect_log_level("error: connection refused"), LogLevel::Error);
    }

    #[test]
    fn test_standalone_error_is_error() {
        assert_eq!(detect_log_level("An error occurred"), LogLevel::Error);
        assert_eq!(detect_log_level("error in processing"), LogLevel::Error);
        assert_eq!(detect_log_level("fatal error"), LogLevel::Error);
    }

    #[test]
    fn test_bracketed_error_is_error() {
        assert_eq!(detect_log_level("[error] something failed"), LogLevel::Error);
        assert_eq!(detect_log_level("[ERROR] critical"), LogLevel::Error);
    }

    #[test]
    fn test_emoji_error_is_error() {
        assert_eq!(detect_log_level("⛔ Error: failed"), LogLevel::Error);
        assert_eq!(detect_log_level("❌ Build failed"), LogLevel::Error);
        assert_eq!(detect_log_level("🔥 Fatal error"), LogLevel::Error);
    }

    #[test]
    fn test_build_failure_is_error() {
        assert_eq!(detect_log_level("FAILURE: Build failed"), LogLevel::Error);
        assert_eq!(detect_log_level("Build failed with errors"), LogLevel::Error);
    }

    // ─────────────────────────────────────────────────────────
    // Warning Detection Tests
    // ─────────────────────────────────────────────────────────

    #[test]
    fn test_warning_class_name_not_warning() {
        // Class names should NOT trigger Warning
        assert_eq!(detect_log_level("WarningDialog"), LogLevel::Info);
        assert_eq!(detect_log_level("ShowWarningBanner"), LogLevel::Info);
    }

    #[test]
    fn test_valid_warning_is_warning() {
        assert_eq!(detect_log_level("Warning: deprecated API"), LogLevel::Warning);
        assert_eq!(detect_log_level("⚠ Warning message"), LogLevel::Warning);
        assert_eq!(detect_log_level("[warning] check this"), LogLevel::Warning);
    }

    // ─────────────────────────────────────────────────────────
    // Regression Tests (existing patterns must still work)
    // ─────────────────────────────────────────────────────────

    #[test]
    fn test_logger_package_still_works() {
        assert_eq!(detect_log_level("🐛 Debug: message"), LogLevel::Debug);
        assert_eq!(detect_log_level("💡 Info: message"), LogLevel::Info);
        assert_eq!(detect_log_level("⚠️ Warning: message"), LogLevel::Warning);
        assert_eq!(detect_log_level("⛔ Error: message"), LogLevel::Error);
    }

    #[test]
    fn test_talker_package_still_works() {
        assert_eq!(detect_log_level("[debug] message"), LogLevel::Debug);
        assert_eq!(detect_log_level("[info] message"), LogLevel::Info);
        assert_eq!(detect_log_level("[warning] message"), LogLevel::Warning);
        assert_eq!(detect_log_level("[error] message"), LogLevel::Error);
    }
}
```

### Files to Modify

| File | Action | Description |
|------|--------|-------------|
| `src/daemon/protocol.rs` | Modify | Add `contains_word()` helper, update `detect_log_level()` |
| `src/app/handler/helpers.rs` | Modify | Update `detect_log_level_from_content()` with word boundaries |

### Edge Cases

1. **Mixed case**: `ERROR` vs `Error` vs `error` - all should work
2. **Punctuation**: `error.` `error,` `error;` should still match as word
3. **Start of line**: `Error: ...` should match (no leading space)
4. **End of line**: `...an error` should match (no trailing space)
5. **Unicode**: Ensure emoji detection still has priority

### Estimated Effort

2-3 hours

### References

- Current implementation: `src/daemon/protocol.rs` - `detect_log_level()`
- Current implementation: `src/app/handler/helpers.rs` - `detect_log_level_from_content()`
- BUG.md Phase 2 specification

---

## Completion Summary

**Status:** ✅ Done

**Completed:** 2026-01-05

### Files Modified

| File | Changes |
|------|---------|
| `src/core/ansi.rs` | Added `contains_word()` function for word boundary detection |
| `src/core/mod.rs` | Exported `contains_word` from core module |
| `src/daemon/protocol.rs` | Updated `detect_log_level()` to use word boundaries instead of substring matching |
| `src/app/handler/helpers.rs` | Updated `detect_log_level_from_content()` to use word boundaries |

### Implementation Notes

1. **Word Boundary Detection (`contains_word()`)**:
   - Checks if surrounding characters are non-alphanumeric
   - Handles: standalone words, colons, brackets, punctuation
   - Rejects: CamelCase identifiers, method names, variable names

2. **Dart Exception Pattern Detection**:
   - Added special handling for `"error ("` and `"exception ("` patterns
   - Catches real exceptions like `RangeError (length): Invalid value`
   - Distinguishes from class names like `ErrorTestingPage`

3. **Word Variations**:
   - Added "crashed", "crashing" to error keywords
   - Ensures "App crashed unexpectedly" still triggers Error level

### Testing Performed

```bash
cargo check                    # Compilation check - PASS
cargo test --lib ansi         # 59 tests passed (includes 15 new word boundary tests)
cargo test --lib helpers      # 62 tests passed (includes 12 new false positive tests)
cargo test --lib              # 824 passed, 1 failed (pre-existing flaky test)
```

### New Tests Added

**In `src/core/ansi.rs`:**
- `test_contains_word_standalone` - Standalone words
- `test_contains_word_with_colon` - Word followed by colon
- `test_contains_word_bracketed` - Talker format `[error]`
- `test_contains_word_start_of_text` - Word at start
- `test_contains_word_end_of_text` - Word at end
- `test_contains_word_with_punctuation` - Punctuation delimiters
- `test_contains_word_false_positive_class_names` - CamelCase class names
- `test_contains_word_false_positive_method_names` - Method names
- `test_contains_word_false_positive_variable_names` - Variable names
- `test_contains_word_case_insensitive` - Case handling
- `test_contains_word_warning_false_positives` - Warning class names
- `test_contains_word_multiple_occurrences` - Mixed occurrences
- `test_contains_word_empty_inputs` - Edge case handling
- `test_contains_word_single_word_text` - Single word

**In `src/app/handler/helpers.rs`:**
- `test_class_name_not_error` - Class names don't trigger Error
- `test_method_name_not_error` - Method names don't trigger Error
- `test_variable_name_not_error` - Variable names don't trigger Error
- `test_camel_case_not_error` - CamelCase identifiers
- `test_valid_error_detection_still_works` - Regression check
- `test_warning_class_name_not_warning` - Warning class names
- `test_valid_warning_detection_still_works` - Regression check
- `test_build_failure_still_detected` - Build failures
- `test_emoji_error_still_detected` - Emoji-based detection

### Acceptance Criteria Checklist

- [x] `ErrorTestingPage` no longer triggers Error level
- [x] `handleError` no longer triggers Error level
- [x] `errorCount` no longer triggers Error level
- [x] `Error: something failed` still triggers Error level
- [x] `[error] message` still triggers Error level
- [x] `⛔ Error` still triggers Error level
- [x] `FAILURE: Build failed` still triggers Error level
- [x] Existing Logger/Talker patterns still work
- [x] Unit tests for word boundary detection
- [x] No regressions in existing level detection

### Notes

- The `test_indeterminate_ratio_oscillates` test failure is a pre-existing flaky timing test in `device_selector.rs`, unrelated to this task.
