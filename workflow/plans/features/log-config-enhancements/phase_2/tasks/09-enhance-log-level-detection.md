## Task: Enhance Log Level Detection for Logger/Talker Packages

**Objective**: Improve log level detection to correctly identify log levels from popular Flutter logging packages (Logger, Talker), enabling accurate filtering by log level.

**Depends on**: [08-strip-ansi-escape-codes](08-strip-ansi-escape-codes.md) (ANSI codes must be stripped first for accurate detection)

### Background

Flutter Demon's current log level detection misses patterns from popular logging packages:

**Logger Package** uses:
- Level prefixes: `Trace:`, `Debug:`, `Info:`, `Warning:`, `Error:`, `Fatal:`
- Emojis: 🐛 (debug), 💡 (info), ⚠️ (warning), ⛔ (error), 🔥 (fatal)
- Note: Logger uses "Trace" for most verbose level, which Dart doesn't have natively

**Talker Package** uses:
- Level prefixes: `[verbose]`, `[debug]`, `[info]`, `[warning]`, `[error]`, `[critical]`
- Emojis vary by configuration

**Current Detection Gaps:**
1. `detect_log_level()` doesn't recognize `Trace:` prefix
2. `detect_log_level()` doesn't detect emoji indicators
3. `Fatal:` and `Critical:` aren't mapped to Error level
4. Logger's pretty-printed format has the level inside box-drawing structure

### Scope

- `src/daemon/protocol.rs`: Enhance `detect_log_level()` function
- `src/app/handler/helpers.rs`: Enhance `detect_raw_line_level()` function

### Implementation

#### 1. Enhanced Log Level Detection

Update `detect_log_level()` in `src/daemon/protocol.rs`:

```rust
/// Detect log level from message content
/// Supports standard patterns plus Logger/Talker package formats
pub fn detect_log_level(message: &str) -> LogLevel {
    let lower = message.to_lowercase();
    
    // ─────────────────────────────────────────────────────────
    // Emoji-based detection (Logger package uses these)
    // ─────────────────────────────────────────────────────────
    
    // Fatal/Critical indicators (check first - highest priority)
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
    // Prefix-based detection (Logger/Talker package formats)
    // ─────────────────────────────────────────────────────────
    
    // Logger package prefixes (with colon)
    if lower.contains("fatal:") || lower.contains("critical:") {
        return LogLevel::Error;
    }
    if lower.contains("error:") || lower.contains("exception:") {
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
    
    // Talker package format (bracketed)
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
    // General keyword detection (existing logic)
    // ─────────────────────────────────────────────────────────
    
    // Error keywords
    if lower.contains("error") 
        || lower.contains("exception") 
        || lower.contains("failed") 
        || lower.contains("fatal")
        || lower.contains("crash")
    {
        return LogLevel::Error;
    }
    
    // Warning keywords
    if lower.contains("warning") 
        || lower.contains("deprecated") 
        || lower.contains("caution")
    {
        return LogLevel::Warning;
    }
    
    // Debug keywords
    if lower.starts_with("debug") || lower.contains("verbose") {
        return LogLevel::Debug;
    }
    
    LogLevel::Info
}
```

#### 2. Update Raw Line Level Detection

Update `detect_raw_line_level()` in `src/app/handler/helpers.rs`:

```rust
use crate::core::strip_ansi_codes;

/// Detect log level from raw (non-JSON) output line
pub fn detect_raw_line_level(line: &str) -> (LogLevel, String) {
    // Strip ANSI codes first for accurate detection
    let cleaned = strip_ansi_codes(line);
    let trimmed = cleaned.trim();
    
    // Android logcat format: E/, W/, I/, D/
    if trimmed.starts_with("E/") {
        return (LogLevel::Error, trimmed.to_string());
    }
    if trimmed.starts_with("W/") {
        return (LogLevel::Warning, trimmed.to_string());
    }
    if trimmed.starts_with("I/") {
        return (LogLevel::Info, trimmed.to_string());
    }
    if trimmed.starts_with("D/") || trimmed.starts_with("V/") {
        return (LogLevel::Debug, trimmed.to_string());
    }
    
    // Use shared detection logic for content-based detection
    let level = detect_log_level_from_content(trimmed);
    (level, trimmed.to_string())
}

/// Shared content-based log level detection
/// Used by both JSON and raw line processing
fn detect_log_level_from_content(message: &str) -> LogLevel {
    // Check emojis first (Logger package)
    if message.contains('🔥') || message.contains('⛔') || message.contains('❌') {
        return LogLevel::Error;
    }
    if message.contains('⚠') {
        return LogLevel::Warning;
    }
    if message.contains('💡') || message.contains('ℹ') {
        return LogLevel::Info;
    }
    if message.contains('🐛') {
        return LogLevel::Debug;
    }
    
    let lower = message.to_lowercase();
    
    // Gradle/build errors
    if lower.contains("failure:") || lower.contains("build failed") || lower.contains("error:") {
        return LogLevel::Error;
    }
    
    // Fatal/critical
    if lower.contains("fatal:") || lower.contains("critical:") {
        return LogLevel::Error;
    }
    
    // Warnings
    if lower.contains("warning:") || lower.contains("warn:") {
        return LogLevel::Warning;
    }
    
    // Trace/Debug
    if lower.contains("trace:") || lower.contains("debug:") {
        return LogLevel::Debug;
    }
    
    // Build progress (often noise, show as debug)
    if message.starts_with("Running ") 
        || message.starts_with("Building ") 
        || message.starts_with("Compiling ")
        || message.contains("...")
    {
        return LogLevel::Debug;
    }
    
    LogLevel::Info
}
```

### Logger Package Level Mapping

| Logger Level | Emoji | Prefix | Maps To |
|--------------|-------|--------|---------|
| `logger.t()` | (none) | `Trace:` | `LogLevel::Debug` |
| `logger.d()` | 🐛 | `Debug:` | `LogLevel::Debug` |
| `logger.i()` | 💡 | `Info:` | `LogLevel::Info` |
| `logger.w()` | ⚠️ | `Warning:` | `LogLevel::Warning` |
| `logger.e()` | ⛔ | `Error:` | `LogLevel::Error` |
| `logger.f()` | 🔥 | `Fatal:` | `LogLevel::Error` |

### Talker Package Level Mapping

| Talker Level | Prefix | Maps To |
|--------------|--------|---------|
| `talker.verbose()` | `[verbose]` | `LogLevel::Debug` |
| `talker.debug()` | `[debug]` | `LogLevel::Debug` |
| `talker.info()` | `[info]` | `LogLevel::Info` |
| `talker.warning()` | `[warning]` | `LogLevel::Warning` |
| `talker.error()` | `[error]` | `LogLevel::Error` |
| `talker.critical()` | `[critical]` | `LogLevel::Error` |

### Acceptance Criteria

1. [x] Logger package trace logs detected as Debug level
2. [x] Logger package debug logs detected as Debug level (emoji 🐛)
3. [x] Logger package info logs detected as Info level (emoji 💡)
4. [x] Logger package warning logs detected as Warning level (emoji ⚠️)
5. [x] Logger package error logs detected as Error level (emoji ⛔)
6. [x] Logger package fatal logs detected as Error level (emoji 🔥)
7. [x] Talker package all levels detected correctly
8. [x] Filtering by log level correctly includes/excludes Logger output
9. [x] Filtering by log level correctly includes/excludes Talker output
10. [x] Existing Android logcat detection still works
11. [x] Existing Gradle/Xcode error detection still works
12. [x] Unit tests for each Logger package pattern
13. [x] Unit tests for each Talker package pattern
14. [x] No regressions in existing level detection

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    // ─────────────────────────────────────────────────────────
    // Logger Package Tests
    // ─────────────────────────────────────────────────────────
    
    #[test]
    fn test_logger_trace_prefix() {
        assert_eq!(detect_log_level("Trace: Very detailed info"), LogLevel::Debug);
        assert_eq!(detect_log_level("│  Trace: message"), LogLevel::Debug);
    }

    #[test]
    fn test_logger_debug_emoji() {
        assert_eq!(detect_log_level("🐛 Debug: Debugging info"), LogLevel::Debug);
        assert_eq!(detect_log_level("│ 🐛  Debug: message"), LogLevel::Debug);
    }

    #[test]
    fn test_logger_info_emoji() {
        assert_eq!(detect_log_level("💡 Info: General info"), LogLevel::Info);
        assert_eq!(detect_log_level("│ 💡  Info: message"), LogLevel::Info);
    }

    #[test]
    fn test_logger_warning_emoji() {
        assert_eq!(detect_log_level("⚠️ Warning: Something wrong"), LogLevel::Warning);
        assert_eq!(detect_log_level("│ ⚠  Warning: message"), LogLevel::Warning);
    }

    #[test]
    fn test_logger_error_emoji() {
        assert_eq!(detect_log_level("⛔ Error: Something failed"), LogLevel::Error);
        assert_eq!(detect_log_level("│ ⛔  Error: message"), LogLevel::Error);
        assert_eq!(detect_log_level("❌ Error: failure"), LogLevel::Error);
    }

    #[test]
    fn test_logger_fatal_emoji() {
        assert_eq!(detect_log_level("🔥 Fatal: Critical failure"), LogLevel::Error);
        assert_eq!(detect_log_level("│ 🔥  Fatal: message"), LogLevel::Error);
    }

    // ─────────────────────────────────────────────────────────
    // Talker Package Tests
    // ─────────────────────────────────────────────────────────
    
    #[test]
    fn test_talker_verbose() {
        assert_eq!(detect_log_level("[verbose] Detailed message"), LogLevel::Debug);
    }

    #[test]
    fn test_talker_debug() {
        assert_eq!(detect_log_level("[debug] Debug message"), LogLevel::Debug);
    }

    #[test]
    fn test_talker_info() {
        assert_eq!(detect_log_level("[info] Info message"), LogLevel::Info);
    }

    #[test]
    fn test_talker_warning() {
        assert_eq!(detect_log_level("[warning] Warning message"), LogLevel::Warning);
        assert_eq!(detect_log_level("[warn] Warning message"), LogLevel::Warning);
    }

    #[test]
    fn test_talker_error() {
        assert_eq!(detect_log_level("[error] Error message"), LogLevel::Error);
        assert_eq!(detect_log_level("[exception] Exception occurred"), LogLevel::Error);
    }

    #[test]
    fn test_talker_critical() {
        assert_eq!(detect_log_level("[critical] Critical failure"), LogLevel::Error);
        assert_eq!(detect_log_level("[fatal] Fatal error"), LogLevel::Error);
    }

    // ─────────────────────────────────────────────────────────
    // Existing Patterns (Regression Tests)
    // ─────────────────────────────────────────────────────────
    
    #[test]
    fn test_android_logcat_still_works() {
        let (level, _) = detect_raw_line_level("E/flutter: error message");
        assert_eq!(level, LogLevel::Error);
        
        let (level, _) = detect_raw_line_level("W/flutter: warning");
        assert_eq!(level, LogLevel::Warning);
        
        let (level, _) = detect_raw_line_level("I/flutter: info");
        assert_eq!(level, LogLevel::Info);
        
        let (level, _) = detect_raw_line_level("D/flutter: debug");
        assert_eq!(level, LogLevel::Debug);
    }

    #[test]
    fn test_gradle_errors_still_work() {
        assert_eq!(detect_log_level("FAILURE: Build failed"), LogLevel::Error);
        assert_eq!(detect_log_level("BUILD FAILED"), LogLevel::Error);
    }

    #[test]
    fn test_xcode_errors_still_work() {
        let (level, _) = detect_raw_line_level("❌ Build failed");
        assert_eq!(level, LogLevel::Error);
    }

    // ─────────────────────────────────────────────────────────
    // Edge Cases
    // ─────────────────────────────────────────────────────────
    
    #[test]
    fn test_plain_message_is_info() {
        assert_eq!(detect_log_level("Just a regular message"), LogLevel::Info);
    }

    #[test]
    fn test_box_drawing_with_level() {
        // Logger package wraps messages in boxes
        assert_eq!(detect_log_level("│ 💡  Info: Login successful"), LogLevel::Info);
        assert_eq!(detect_log_level("│ 🐛  Debug: User data loaded"), LogLevel::Debug);
    }

    #[test]
    fn test_case_insensitive_prefixes() {
        assert_eq!(detect_log_level("ERROR: something failed"), LogLevel::Error);
        assert_eq!(detect_log_level("Warning: be careful"), LogLevel::Warning);
        assert_eq!(detect_log_level("DEBUG: verbose output"), LogLevel::Debug);
    }
}
```

### Integration Tests

After implementation, manually verify with the sample app:
1. Run Flutter Demon with `sample/` project
2. Tap "All Levels" button in "Logger Package" section
3. Press `f` to cycle through level filters
4. Verify "Errors only" filter shows only error/fatal logs
5. Verify "Warnings" filter shows warning and above
6. Verify filtering works correctly for both Logger and Talker output

### Files to Modify

| File | Action | Description |
|------|--------|-------------|
| `src/daemon/protocol.rs` | Modify | Enhance `detect_log_level()` with emoji and prefix patterns |
| `src/app/handler/helpers.rs` | Modify | Enhance `detect_raw_line_level()` to use shared detection |

### Estimated Time

3-4 hours

### References

- [Logger Package](https://pub.dev/packages/logger) - Pretty printing with levels
- [Talker Package](https://pub.dev/packages/talker) - Alternative logging library
- Task 08 (ANSI stripping) - Required dependency
- Phase 1 log filtering implementation

---

## Completion Summary

**Status:** ✅ Done

**Files Modified:**
- `src/daemon/protocol.rs` - Enhanced `detect_log_level()` with emoji and prefix pattern detection
- `src/app/handler/helpers.rs` - Enhanced `detect_raw_line_level()` with comprehensive content-based detection via `detect_log_level_from_content()`

**Implementation Details:**

1. **Emoji-based detection** (Logger package):
   - 🔥💀 → Error (fatal)
   - ⛔❌🚫 → Error
   - ⚠️⚡ → Warning
   - 💡ℹ → Info
   - 🐛🔍 → Debug

2. **Prefix-based detection** (Logger/Talker):
   - `fatal:`, `critical:` → Error
   - `error:`, `exception:` → Error
   - `warning:`, `warn:` → Warning
   - `info:` → Info
   - `debug:`, `trace:` → Debug

3. **Bracketed prefixes** (Talker):
   - `[critical]`, `[fatal]` → Error
   - `[error]`, `[exception]` → Error
   - `[warning]`, `[warn]` → Warning
   - `[info]` → Info
   - `[debug]`, `[verbose]`, `[trace]` → Debug

4. **Preserved existing detection**:
   - Android logcat (E/, W/, I/, D/, V/)
   - Gradle/Xcode build errors
   - General keyword matching

**Testing Performed:**
- `cargo test daemon::protocol` - 57 tests passed (17 new Logger/Talker tests)
- `cargo test app::handler::helpers` - 22 tests passed (16 new Logger/Talker tests)
- `cargo check` - Compiles successfully
- `cargo clippy` - No warnings

**Notable Decisions/Tradeoffs:**
- Emoji detection runs first (before lowercase conversion) for better performance
- Detection order: emojis → specific prefixes → talker brackets → general keywords
- Created shared `detect_log_level_from_content()` helper to avoid code duplication
- Box-drawing characters (from Logger package pretty printing) are handled correctly

**Risks/Limitations:**
- Detection is case-insensitive for text patterns, which may rarely cause false positives
- New keywords added (crash, caution) for improved detection coverage