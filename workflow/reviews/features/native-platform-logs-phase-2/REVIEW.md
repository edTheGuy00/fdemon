# Review: Native Platform Logs Phase 2

**Date:** 2026-03-10
**Branch:** `feature/native-platform-logs`
**Verdict:** NEEDS WORK

## Summary

Phase 2 adds iOS simulator/physical device native log capture, per-session tag discovery/filtering, per-tag config, and a tag filter overlay UI. The architecture is sound — all layer boundaries are respected, TEA patterns are followed correctly, and the code is well-structured with thorough test coverage (684 new tests). However, there is one **critical bug** causing the reported issue (no native logs on iOS Simulator) and several major issues that must be addressed.

**Build:** Compiles cleanly. 3,209 unit tests pass (4 pre-existing snapshot failures from version bump, unrelated).

---

## Critical Issues

### 1. BUG: iOS process name derivation is incorrect — root cause of reported issue

**File:** `crates/fdemon-app/src/actions/native_logs.rs:265-277`
**Severity:** CRITICAL

`derive_ios_process_name` delegates to `derive_macos_process_name`, which takes the last component of the bundle identifier (e.g., `"com.example.flutterDeamonSample"` -> `"flutterDeamonSample"`). On iOS, Flutter apps always use `"Runner"` as the process name (the Xcode target name), NOT the last component of the bundle ID. This is confirmed by `example/app2/ios/Runner.xcodeproj/project.pbxproj` which shows `PRODUCT_NAME = "$(TARGET_NAME)"` (resolves to `Runner`).

The command produced is:
```
xcrun simctl spawn <UDID> log stream --predicate 'process == "flutterDeamonSample"' --style syslog
```

This predicate matches **nothing**, so the log stream produces zero output. The capture appears to work (no errors), but no logs are ever forwarded.

**Fix:** `derive_ios_process_name` should always return `"Runner"` for iOS apps regardless of bundle ID. The macOS convention (process name = last bundle component) does not apply to iOS.

```rust
fn derive_ios_process_name(_app_id: &Option<String>) -> String {
    // iOS Flutter apps always use "Runner" as the Xcode target/process name.
    // Unlike macOS, the process name does not correspond to the bundle ID.
    "Runner".to_string()
}
```

---

## Major Issues

### 2. `effective_min_level()` is never called at runtime

**File:** `crates/fdemon-app/src/config/types.rs:630` and `crates/fdemon-app/src/handler/update.rs:1937-1961`
**Severity:** MAJOR

Task 08's per-tag minimum log level override (`[native_logs.tags.GoLog] min_level = "debug"`) is implemented in `NativeLogsSettings::effective_min_level()` with 8 unit tests, but it is **never called** in the production `NativeLog` handler. The per-tag level configuration is dead code — user settings are silently ignored.

**Fix:** Wire into the `NativeLog` handler in `update.rs` after `observe_tag()` and before `is_tag_visible()`:
```rust
let effective_min = state.settings.native_logs.effective_min_level(&event.tag);
if let Some(min_level) = parse_min_level(effective_min) {
    if event.level.severity() < min_level.severity() {
        return UpdateResult::none();
    }
}
```

### 3. Simulator capture missing min_level event-level filter

**File:** `crates/fdemon-daemon/src/native_logs/ios.rs:281-361`
**Severity:** MAJOR

`run_simctl_log_capture` applies tag filtering but does NOT apply `min_level` filtering at the event level. Compare with `run_idevicesyslog_capture` which correctly filters by severity. When `min_level` is `"warning"` or `"error"`, the `--level` flag maps to `"default"` (accepting all levels), but no client-side severity filtering is performed. This means debug/info logs leak through when the user has set `min_level = "warning"`.

### 4. `scroll_offset` is tracked but never applied in rendering

**File:** `crates/fdemon-tui/src/widgets/tag_filter.rs:82-130` and `crates/fdemon-app/src/state.rs:838`
**Severity:** MAJOR

`TagFilterUiState.scroll_offset` is declared, documented, reset on open, and tested — but the `render_tag_filter` function builds all items and passes them to `List::new(items)` without applying any scroll offset. When there are more than 15 tags, the selected row scrolls off screen with no visible feedback. The `move_down` handler updates `selected_index` but nothing updates `scroll_offset`.

**Fix:** Either implement proper scroll tracking using `ratatui::widgets::ListState` or remove `scroll_offset` and document the 15-tag cap.

---

### 5. `Ctrl+C` swallowed by tag filter overlay

**File:** `crates/fdemon-app/src/handler/keys.rs:104-122`
**Severity:** MAJOR

The tag filter overlay's `_ => None` catch-all intercepts ALL keys including `Ctrl+C` (`InputKey::CharCtrl('c')`). Every other overlay/dialog in the codebase explicitly handles `CharCtrl('c') => Some(Message::Quit)`. The user cannot force-quit while the overlay is open — they must press `Esc` or `T` first.

**Fix:** Add `InputKey::CharCtrl('c') => Some(Message::Quit)` before the catch-all.

### 6. `truncate_tag()` panics on multi-byte UTF-8

**File:** `crates/fdemon-tui/src/widgets/tag_filter.rs:153`
**Severity:** MAJOR

`&tag[..max_len - 3]` performs byte-level slicing. If a tag contains multi-byte UTF-8 characters (CJK subsystem names, emoji), slicing at a non-character boundary causes a panic in the render path. Use `tag.chars()` for character-based truncation.

---

## Minor Issues

### 7. `IDEVICESYSLOG_RE` regex uses `\S+` for device name

**File:** `crates/fdemon-daemon/src/native_logs/ios.rs:49`

The regex uses `\S+` for the device name field. iOS device names frequently contain spaces (e.g., "Ed's iPhone", "My iPad Pro"). The `\S+` stops at the first space, causing the regex to fail silently. All log lines from such devices are dropped with no error.

### 8. Malformed doc comments

**File:** `crates/fdemon-app/src/state.rs:958`

Two doc comments use `/ ` instead of `/// `, making them invisible to `cargo doc`.

### 9. Unnecessary clones in `idevicesyslog_line_to_event`

**File:** `crates/fdemon-daemon/src/native_logs/ios.rs:109-120`

Takes `&IdevicesyslogLine` but clones every field. Should take ownership since the parsed line is not reused after conversion.

### 10. `idevicesyslog --help` exit code may be non-zero

**File:** `crates/fdemon-daemon/src/tool_availability.rs:188-196`

`check_idevicesyslog()` checks `status.success()`, but `idevicesyslog --help` exits with non-zero on some libimobiledevice versions. Could falsely report the tool as unavailable.

---

## Architecture Review

**Verdict:** PASS

All layer boundaries are respected. No violations found:
- fdemon-daemon depends only on fdemon-core
- fdemon-app depends only on fdemon-core + fdemon-daemon
- fdemon-tui depends only on fdemon-core + fdemon-app
- iOS capture backend correctly gated with `#[cfg(target_os = "macos")]`
- TEA pattern followed: all state changes go through Message variants
- New modules have proper documentation headers

---

## Test Coverage

| Crate | Tests | Status |
|-------|-------|--------|
| fdemon-core | 367 | PASS |
| fdemon-daemon | 527 (3 ignored) | PASS |
| fdemon-app | 1,511 (4 ignored) | PASS |
| fdemon-tui | 814 (4 snapshot failures) | PASS (snapshot failures are pre-existing version bump) |

New tests added: ~684 across native log capture, tag state, tag filter UI, per-tag config, iOS tool availability.

---

## Agent Verdicts

| Agent | Verdict |
|-------|---------|
| Architecture Enforcer | CONCERNS (scroll_offset dead state, malformed doc comments) |
| Logic & Reasoning | CONCERNS (process name bug, missing min_level filter on simulator path) |
| Code Quality Inspector | NEEDS WORK (scroll_offset not wired, effective_min_level dead code) |
| Risks & Tradeoffs | CONCERNS (per-tag config not wired, Ctrl+C swallowed, truncate_tag panic risk) |

---

## Re-review Checklist

After addressing issues, the following must pass:
- [ ] `derive_ios_process_name` returns `"Runner"` (not last bundle component)
- [ ] `effective_min_level()` called in `NativeLog` handler
- [ ] Simulator capture applies min_level event-level filter
- [ ] `scroll_offset` either wired or removed
- [ ] `Ctrl+C` quit works while tag filter overlay is open
- [ ] `truncate_tag` uses character-based slicing (not byte slicing)
- [ ] `cargo test --workspace --lib` passes
- [ ] Manual test: native logs appear when running `example/app2` on iOS Simulator
