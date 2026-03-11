## Task: Minor Cleanups — Doc Comments and Unnecessary Clones

**Objective**: Fix two minor code quality issues: malformed doc comments in `state.rs` and unnecessary `String` clones in `idevicesyslog_line_to_event`.

**Depends on**: None

**Review Issues:** #7 (Minor), #9 (Minor)

### Scope

- `crates/fdemon-app/src/state.rs`: Fix malformed doc comments (around line 958)
- `crates/fdemon-daemon/src/native_logs/ios.rs`: Remove unnecessary clones in `idevicesyslog_line_to_event` (lines 109-120)

### Details

#### Issue #7: Malformed doc comments

The review identified two doc comments near line 958 of `state.rs` that use `/ ` (single slash + space) instead of `/// ` (triple slash + space), making them invisible to `cargo doc`. Search for any single-slash comments preceding `pub` items in the `TagFilterUiState` region and fix them to use `///`.

#### Issue #9: Unnecessary clones in `idevicesyslog_line_to_event`

Current implementation (lines 109-120):
```rust
fn idevicesyslog_line_to_event(line: &IdevicesyslogLine) -> NativeLogEvent {
    let tag = line.framework.clone();
    let level = bsd_syslog_level_to_log_level(&line.level_str);
    NativeLogEvent {
        tag,
        level,
        message: line.message.clone(),
        timestamp: Some(line.timestamp.clone()),
    }
}
```

Takes `&IdevicesyslogLine` and clones all three `String` fields (`.framework`, `.message`, `.timestamp`). The parsed line is never reused after conversion — the call site (line 203-204):

```rust
if let Some(parsed) = parse_idevicesyslog_line(&line) {
    let event = idevicesyslog_line_to_event(&parsed);
    // parsed is never used again
```

**Fix:** Change the function to take `IdevicesyslogLine` by value and move fields:

```rust
fn idevicesyslog_line_to_event(line: IdevicesyslogLine) -> NativeLogEvent {
    let level = bsd_syslog_level_to_log_level(&line.level_str);
    NativeLogEvent {
        tag: line.framework,          // move, no clone
        level,
        message: line.message,        // move, no clone
        timestamp: Some(line.timestamp),  // move, no clone
    }
}
```

Update the call site to pass by value:
```rust
let event = idevicesyslog_line_to_event(parsed);  // remove &
```

This eliminates 3 heap allocations per log line on a hot streaming path. Follows the project's code standard: "Unnecessary clones, missing borrows" from `docs/CODE_STANDARDS.md`.

### Acceptance Criteria

1. All doc comments near `TagFilterUiState` use `///` (not `/ `)
2. `idevicesyslog_line_to_event` takes `IdevicesyslogLine` by value (no `&`)
3. No `.clone()` calls in `idevicesyslog_line_to_event`
4. `cargo test --workspace --lib` passes
5. `cargo clippy --workspace -- -D warnings` passes

### Testing

No new tests needed — existing tests cover both areas. Run:
- `cargo test -p fdemon-app -- tag_filter` (doc comment area)
- `cargo test -p fdemon-daemon -- idevicesyslog` (clone removal)

### Notes

- Both fixes are low-risk mechanical changes.
- The clone removal may require updating test call sites if any tests call `idevicesyslog_line_to_event` with a borrow. Check and update accordingly.

---

## Completion Summary

**Status:** Not Started
