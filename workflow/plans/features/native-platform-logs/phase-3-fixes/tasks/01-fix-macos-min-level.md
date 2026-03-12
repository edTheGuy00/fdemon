## Task: Fix macOS `run_log_stream_capture` min_level Filtering

**Objective**: Add event-level severity filtering to the macOS log capture loop, matching the pattern used by Android and iOS backends.

**Depends on**: None

**Review Issue**: #1 (CRITICAL)

### Scope

- `crates/fdemon-daemon/src/native_logs/macos.rs`: Add `parse_min_level` call and severity guard in `run_log_stream_capture`

### Details

The macOS `run_log_stream_capture` function (lines 177-225) applies tag filtering after parsing syslog lines but never applies a `min_level` severity guard. The `--level` argument passed to `log stream` only accepts `"default"`, `"info"`, `"debug"` — there is no `"warning"` or `"error"` level. When `min_level = "warning"` or `"error"`, the code maps to `"default"` (lines 122-126), so the process-level flag provides no filtering and the event-level guard was never implemented.

The comment at line 122-126 says "Higher-level filtering is handled downstream" but no downstream filter exists.

**Reference implementations:**

iOS simulator (`ios.rs:280, 339-344`):
```rust
let min_level = super::parse_min_level(&config.min_level);
// ... after tag filter, before send:
if let Some(min) = min_level {
    if event.level.severity() < min.severity() {
        continue;
    }
}
```

iOS physical device (`ios.rs:162, 209-213`) uses the same pattern.

Android (`android.rs:131-183`) uses `parse_min_priority` + `min_severity` u8 comparison.

**Fix:** At the top of `run_log_stream_capture`, add:
```rust
let min_level = super::parse_min_level(&config.min_level);
```

Then after `let event = syslog_line_to_event(&parsed);` (before the `event_tx.send`), add:
```rust
if let Some(min) = min_level {
    if event.level.severity() < min.severity() {
        continue;
    }
}
```

### Acceptance Criteria

1. `min_level = "error"` in config produces only error-level macOS logs (not info/warning)
2. `min_level = "warning"` filters out info/debug but passes warning and error
3. The severity check is structurally identical to the iOS simulator pattern
4. Existing tests pass; new test covers the min_level filtering

### Testing

Add a test in the macOS native_logs test module that verifies events below `min_level` are filtered out. Follow the pattern used in iOS tests for `min_level` filtering.

```rust
#[test]
fn test_run_log_stream_capture_filters_below_min_level() {
    // Configure min_level = "warning"
    // Feed info-level and error-level syslog lines
    // Assert only error-level events are emitted
}
```

### Notes

- If task 06 (move `parse_min_level` to core) is done first, call `LogLevel::from_level_str()` instead of `super::parse_min_level()`
- The `--level` flag on `log stream` is still useful as a coarse pre-filter for `"debug"` and `"info"` levels — keep it
