## Task: Add min_level Event-Level Filter to Simulator Capture

**Objective**: Fix the iOS simulator capture path so it applies per-event `min_level` filtering, matching the physical device capture path. Currently `run_simctl_log_capture` relies solely on the `--level` CLI flag which cannot express `"warning"` or `"error"` as a floor.

**Depends on**: None

**Review Issue:** #3 (Major)

### Scope

- `crates/fdemon-daemon/src/native_logs/ios.rs`: Add `parse_min_level` + severity guard to `run_simctl_log_capture`

### Details

#### Problem

The two iOS capture paths have asymmetric min_level filtering:

**Physical device** (`run_idevicesyslog_capture`, lines 164-240):
```rust
let min_level = parse_min_level(&config.min_level);  // line 169
// ...
// Inside read loop:
if let Some(min) = min_level {                        // line 216
    if event.level.severity() < min.severity() {      // line 217
        continue;                                     // line 218
    }
}
```

**Simulator** (`run_simctl_log_capture`, lines 281-361):
- Never calls `parse_min_level`
- No per-event severity guard in the read loop
- Only filtering is the `--level` flag in `build_simctl_log_stream_command`, which maps:
  - `"warning"` → `"default"` (includes Notice/Info — too permissive)
  - `"error"` → `"default"` (same issue)

When the user sets `min_level = "warning"`, the CLI flag passes through Debug/Info events, and nothing filters them out.

#### Fix

Add the same two-line pattern from the physical path to the simulator path:

1. At the top of `run_simctl_log_capture`, after config is received:
```rust
let min_level = parse_min_level(&config.min_level);
```

2. Inside the read loop, after the tag filter check and before `event_tx.send`:
```rust
if let Some(min) = min_level {
    if event.level.severity() < min.severity() {
        continue;
    }
}
```

The `--level` CLI flag can remain as a best-effort pre-filter at the process level; the per-event guard provides the precise floor that the CLI flag cannot.

### Acceptance Criteria

1. `run_simctl_log_capture` calls `parse_min_level(&config.min_level)` at function entry
2. Events with `level.severity() < min_level.severity()` are dropped in the simulator read loop
3. Existing tests pass: `cargo test -p fdemon-daemon -- ios`
4. New test: simulator path with `min_level = "warning"` filters Debug/Info events
5. `cargo clippy -p fdemon-daemon -- -D warnings` passes

### Testing

Add a test that verifies the simulator path filters events by severity:

```rust
#[test]
fn test_simctl_capture_filters_by_min_level() {
    // Verify that when min_level is "warning", the capture path
    // drops events with Debug and Info levels.
    // This can test the filter logic directly without spawning a process.
}
```

### Notes

- The `--level` CLI flag mapping in `build_simctl_log_stream_command` (lines 260-267) should NOT be changed — it provides best-effort pre-filtering at the OS level. The per-event guard is the precise filter.
- If task 02 promotes `parse_min_level` to `native_logs/mod.rs`, this task should use the shared version. Otherwise, the local `parse_min_level` already exists in `ios.rs:125-133`.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-daemon/src/native_logs/ios.rs` | Added `parse_min_level` call at top of `run_simctl_log_capture`; added severity guard in read loop after tag filter; added 3 new tests |

### Notable Decisions/Tradeoffs

1. **Local `parse_min_level`**: Task 02 did not promote `parse_min_level` to `mod.rs`, so the existing local function in `ios.rs` (line 125) was used directly — exactly as noted in the task's "Notes" section.
2. **Test approach**: Tests exercise `parse_min_level` + `LogLevel::severity()` directly without spawning a process, making them fast, hermetic, and consistent with the existing test style in the module. Three tests were added: one verifying warning/error pass and debug/info are dropped (`test_simctl_capture_filters_by_min_level`), one verifying `None` min_level passes all events (`test_simctl_capture_no_min_level_passes_all`), and one verifying debug floor passes all levels (`test_simctl_capture_min_level_debug_passes_all_levels`).

### Testing Performed

- `cargo test -p fdemon-daemon -- ios` — PASS (35 tests, 0 failed)
- `cargo clippy -p fdemon-daemon -- -D warnings` — PASS (no warnings)

### Risks/Limitations

1. **No integration test**: The severity guard in the async loop path is not exercised by a live process test; however, the logic is identical to the physical device path (which had the same test approach), and the unit tests verify the filter semantics correctly.
