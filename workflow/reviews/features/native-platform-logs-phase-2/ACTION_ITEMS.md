# Action Items: Native Platform Logs Phase 2

**Review Date:** 2026-03-10
**Verdict:** NEEDS WORK
**Blocking Issues:** 6

## Critical Issues (Must Fix)

### 1. iOS process name derivation returns wrong value
- **Source:** Logic Reasoning Checker + Bug Investigation
- **File:** `crates/fdemon-app/src/actions/native_logs.rs:265-277`
- **Problem:** `derive_ios_process_name` returns last component of bundle ID (e.g., `"flutterDeamonSample"`), but iOS Flutter apps always use `"Runner"` as the process name. The `xcrun simctl log stream --predicate 'process == "flutterDeamonSample"'` matches nothing.
- **Required Action:** Change `derive_ios_process_name` to always return `"Runner"`. Update tests accordingly.
- **Acceptance:** Running `example/app2` on iOS Simulator produces native log entries in the TUI.

## Major Issues (Should Fix)

### 2. Per-tag `effective_min_level()` is dead code
- **Source:** Code Quality Inspector + Risks/Tradeoffs Analyzer
- **File:** `crates/fdemon-app/src/handler/update.rs` (NativeLog handler, ~line 1937)
- **Problem:** `NativeLogsSettings::effective_min_level()` is implemented and tested but never called. Per-tag level config from `[native_logs.tags.X]` is silently ignored.
- **Suggested Action:** Call `effective_min_level(&event.tag)` in the `NativeLog` handler after `observe_tag()`, filter events below the threshold. Add handler-level test.

### 3. Simulator capture missing min_level event-level filter
- **Source:** Logic Reasoning Checker
- **File:** `crates/fdemon-daemon/src/native_logs/ios.rs:281-361` (`run_simctl_log_capture`)
- **Problem:** Physical device capture path applies `parse_min_level` filter, but simulator capture path does not. When `min_level = "warning"`, debug/info logs leak through on simulator.
- **Suggested Action:** Add the same `parse_min_level` + severity check to `run_simctl_log_capture` that exists in `run_idevicesyslog_capture`.

### 4. `scroll_offset` is dead state in tag filter UI
- **Source:** Architecture Enforcer + Code Quality Inspector
- **File:** `crates/fdemon-app/src/state.rs:838` and `crates/fdemon-tui/src/widgets/tag_filter.rs:82-130`
- **Problem:** `TagFilterUiState.scroll_offset` is declared, documented, and reset but never read during rendering. Lists with 15+ tags have selected row scroll off screen.
- **Suggested Action:** Either implement scroll tracking with `ratatui::widgets::ListState` or remove `scroll_offset` and document the tag cap.

### 5. `Ctrl+C` swallowed by tag filter overlay
- **Source:** Risks/Tradeoffs Analyzer
- **File:** `crates/fdemon-app/src/handler/keys.rs:104-122`
- **Problem:** The `_ => None` catch-all intercepts `Ctrl+C` (`InputKey::CharCtrl('c')`). Every other overlay/dialog handles `CharCtrl('c') => Some(Message::Quit)`. User cannot force-quit while overlay is open.
- **Suggested Action:** Add `InputKey::CharCtrl('c') => Some(Message::Quit)` before the catch-all.

### 6. `truncate_tag()` panics on multi-byte UTF-8
- **Source:** Risks/Tradeoffs Analyzer
- **File:** `crates/fdemon-tui/src/widgets/tag_filter.rs:153`
- **Problem:** `&tag[..max_len - 3]` is byte-level slicing. Multi-byte UTF-8 chars (CJK subsystem names) cause panic in render path.
- **Suggested Action:** Use `tag.chars().take(max_len - 3).collect::<String>()` for character-based truncation.

## Minor Issues (Consider Fixing)

### 7. Malformed doc comments
- `crates/fdemon-app/src/state.rs:958` — `/ ` instead of `/// `

### 8. `IDEVICESYSLOG_RE` regex fails on device names with spaces
- `crates/fdemon-daemon/src/native_logs/ios.rs:49` — `\S+` for device name drops logs from "Ed's iPhone" etc.

### 9. Unnecessary clones in hot path
- `crates/fdemon-daemon/src/native_logs/ios.rs:109-120` — `idevicesyslog_line_to_event` borrows but clones all fields; should take ownership

### 10. `idevicesyslog --help` availability check unreliable
- `crates/fdemon-daemon/src/tool_availability.rs:188-196` — Some versions exit non-zero on `--help`

## Re-review Checklist

After addressing issues, the following must pass:
- [ ] Issue 1: `derive_ios_process_name` returns `"Runner"`
- [ ] Issue 2: `effective_min_level()` wired into NativeLog handler
- [ ] Issue 3: Simulator capture applies min_level event filter
- [ ] Issue 4: scroll_offset wired or removed
- [ ] Issue 5: Ctrl+C handled in tag filter overlay
- [ ] Issue 6: truncate_tag uses char-based slicing
- [ ] `cargo check --workspace` passes
- [ ] `cargo test --workspace --lib` passes
- [ ] `cargo clippy --workspace` passes
- [ ] Manual test: native logs from iOS Simulator appear in TUI
