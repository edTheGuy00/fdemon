## Task: Wire `effective_min_level()` into NativeLog Handler

**Objective**: Connect the per-tag minimum log level configuration (`[native_logs.tags.X] min_level = "warning"`) to the actual event processing pipeline. Currently `NativeLogsSettings::effective_min_level()` is implemented and tested (8 tests) but never called — user config is silently ignored.

**Depends on**: 01-fix-ios-process-name (process name fix should land first so iOS logs flow, making this testable)

**Review Issue:** #2 (Major)

### Scope

- `crates/fdemon-app/src/handler/update.rs`: Add `effective_min_level` call in the `NativeLog` handler
- `crates/fdemon-daemon/src/native_logs/mod.rs`: Potentially promote a `parse_min_level` helper to the shared module

### Details

#### Problem

The `NativeLog` handler in `update.rs` (lines 1937-1962) processes events through this flow:

```
NativeLog { session_id, event }
  → observe_tag(&event.tag)
  → is_tag_visible(&event.tag)?
  → LogEntry::new(...)
  → queue_log(entry)
```

There is **no level-based filter** in this flow. `NativeLogsSettings::effective_min_level()` (defined in `config/types.rs:626-635`) returns the per-tag override or the global `min_level`, but nobody calls it.

#### Fix

Insert the level filter between `observe_tag` and `is_tag_visible` in the `NativeLog` handler:

```rust
handle.native_tag_state.observe_tag(&event.tag);

// Filter by effective per-tag (or global) minimum level
let effective_min = state.settings.native_logs.effective_min_level(&event.tag);
if let Some(min_level) = parse_min_level(effective_min) {
    if event.level.severity() < min_level.severity() {
        return UpdateResult::none();
    }
}

if !handle.native_tag_state.is_tag_visible(&event.tag) {
    return UpdateResult::none();
}
```

#### `parse_min_level` location

A `parse_min_level` function already exists in `fdemon-daemon/src/native_logs/ios.rs:125-133` (maps `"debug"` → `LogLevel::Debug`, etc.). However, it's `#[cfg(target_os = "macos")]`-gated with the ios module. Two approaches:

**Option A (preferred):** Promote `parse_min_level` to `fdemon-daemon/src/native_logs/mod.rs` as a `pub fn` so it's available cross-platform. The ios module can call `super::parse_min_level` instead of its local copy. This is consistent with how `should_include_tag` was promoted in phase-1-fixes (task 04).

**Option B:** Inline the trivial match directly in the handler. This avoids the cross-crate dependency but duplicates logic.

#### Severity mapping

`LogLevel::severity()` is defined in `fdemon-core/src/types.rs:149-156`:
- `Debug` → 0
- `Info` → 1
- `Warning` → 2
- `Error` → 3

The comparison `event.level.severity() < min_level.severity()` correctly drops events below the threshold.

### Acceptance Criteria

1. `effective_min_level()` is called in the `NativeLog` handler for every event
2. Events with `level.severity() < effective_min.severity()` are dropped before being queued
3. Per-tag config `[native_logs.tags.GoLog] min_level = "warning"` correctly filters Debug/Info events for that tag while passing Warning/Error
4. Tags without per-tag config fall back to the global `min_level`
5. `observe_tag()` is still called before the filter (so the tag still appears in the T-overlay even if its events are filtered)
6. New handler tests cover the level filtering behavior

### Testing

Add handler tests in `crates/fdemon-app/src/handler/tests.rs`:

```rust
#[test]
fn test_native_log_filtered_by_effective_min_level() {
    // Setup: global min_level = "info", per-tag "GoLog" = "warning"
    // Send Debug event for "GoLog" → should be filtered
    // Send Warning event for "GoLog" → should pass
    // Send Debug event for "OtherTag" → should be filtered (global "info")
    // Send Info event for "OtherTag" → should pass
}

#[test]
fn test_native_log_tag_observed_even_when_level_filtered() {
    // Send Debug event for "GoLog" with per-tag min_level = "warning"
    // Event should be filtered, but tag should still appear in native_tag_state
}
```

### Notes

- The filter placement (after `observe_tag`, before `is_tag_visible`) ensures tags are still discovered even if their events are level-filtered. This matches the intent: the user should see all tags in the T-overlay and be able to toggle visibility, even for tags whose events are currently below the level threshold.
- If Option A is chosen for `parse_min_level`, the ios module's local copy should be removed and replaced with a call to `super::parse_min_level`.

---

## Completion Summary

**Status:** Not Started
