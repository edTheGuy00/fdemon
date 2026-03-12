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

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-daemon/src/native_logs/macos.rs` | Added `let min_level = super::parse_min_level(&config.min_level);` at top of `run_log_stream_capture`; added severity guard (`if let Some(min) = min_level { if event.level.severity() < min.severity() { continue; } }`) after `syslog_line_to_event` call and before `event_tx.send`; added `use super::super::parse_min_level;` import in test module; added 3 new tests |

### Notable Decisions/Tradeoffs

1. **Placement of severity guard**: The guard is placed after the tag filter and after `syslog_line_to_event`, identical to the iOS simulator path in `ios.rs:339-344`. This is consistent and avoids redundant event construction for filtered-out tags.
2. **Comment added**: Added inline comment explaining why the event-level guard is needed (the `--level` flag has no `"warning"` or `"error"` value), fulfilling the promise made in the existing `build_log_stream_command` comment ("Higher-level filtering is handled downstream").
3. **Three tests added**: `test_run_log_stream_capture_filters_below_min_level` (warning floor — filters debug/info, passes error), `test_run_log_stream_capture_min_level_error_drops_info_and_warning` (error floor — filters info, passes fault/error), `test_run_log_stream_capture_no_min_level_passes_all` (None path — no filter applied). These follow the same pattern as the iOS simulator tests.

### Testing Performed

- `cargo fmt --all` - Passed
- `cargo check -p fdemon-daemon` - Passed
- `cargo test -p fdemon-daemon --lib` - Passed (577 passed, 0 failed, 3 ignored)
- `cargo clippy -p fdemon-daemon -- -D warnings` - Passed (no warnings)
- `cargo test -p fdemon-daemon --lib native_logs::macos` - Passed (20 tests, including 3 new)

### Risks/Limitations

1. **No async integration test**: The async `run_log_stream_capture` function is not exercised end-to-end (it requires a real `log stream` process). Tests use the same `parse_min_level` + `LogLevel::severity()` pairing that the loop depends on, which is sufficient to verify the logic without spawning a process. This matches the approach used for iOS simulator tests.
