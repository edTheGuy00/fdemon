# Action Items: Native Platform Log Capture (Phase 1)

**Review Date:** 2026-03-10
**Verdict:** NEEDS WORK
**Blocking Issues:** 2

---

## Critical Issues (Must Fix)

### 1. Fix `check_macos_log()` — always returns `false`

- **Source:** Risks & Tradeoffs Analyzer, Logic Checker
- **File:** `crates/fdemon-daemon/src/tool_availability.rs:132-142`
- **Problem:** `log --help` exits with code 64 on macOS. `check_macos_log()` checks `status.success()`, so it always returns `false`.
- **Required Action:** Change to `Command::new("log").arg("help")` (no `--` prefix) or use `Command::new("which").arg("log")`.
- **Acceptance:** `cargo test -p fdemon-daemon -- check_macos_log` passes on macOS; `ToolAvailability::check().macos_log` returns `true` on macOS.

### 2. Wire `native_logs_available()` into spawn guard

- **Source:** All 4 agents
- **File:** `crates/fdemon-app/src/handler/session.rs:265-298`
- **Problem:** `ToolAvailability::native_logs_available()` was created for this purpose but is never called. Without `adb`, every Android session start attempts to spawn `adb logcat` and fails with a warning.
- **Required Action:** Add `if !state.tool_availability.native_logs_available(platform) { return None; }` to `maybe_start_native_log_capture()`. Note: fix issue #1 first so the macOS tool check works.
- **Acceptance:** On a system without `adb`, no `tracing::warn!("Failed to spawn adb logcat")` appears. Instead, the skip is logged at `debug` level.

---

## Major Issues (Should Fix)

### 3. Fix macOS `log stream --level error` invalid argument

- **Source:** Code Quality Inspector, Logic Checker
- **File:** `crates/fdemon-daemon/src/native_logs/macos.rs:135-140`
- **Problem:** `"error"` is not a valid `log stream --level` value. Users with `min_level = "warning"` or `"error"` get no macOS log output.
- **Suggested Action:** Map `"warning" | "error"` to `"info"` in `build_log_stream_command`. The severity filtering in the parse loop already handles the rest.

### 4. Add double-start guard

- **Source:** Logic Checker
- **File:** `crates/fdemon-app/src/handler/session.rs:265-298`
- **Problem:** No guard against starting native log capture twice for the same session.
- **Suggested Action:** Add `if handle.native_log_shutdown_tx.is_some() { return None; }` check.

### 5. Handle late `NativeLogCaptureStarted` for closed sessions

- **Source:** Logic Checker
- **File:** `crates/fdemon-app/src/handler/update.rs:1958`
- **Problem:** If session is gone when `NativeLogCaptureStarted` arrives, the capture task leaks.
- **Suggested Action:** When session not found, send `true` on `shutdown_tx` before dropping it.

---

## Minor Issues (Consider Fixing)

### 6. Add explicit parentheses to `needs_capture`
- `crates/fdemon-app/src/handler/session.rs:293` — Add `(...)` around the macOS condition.

### 7. Consolidate tag-filtering logic
- Extract `should_include_tag(include: &[String], exclude: &[String], tag: &str) -> bool` to `native_logs/mod.rs`.

### 8. Parse daemon message once in Stdout path
- `crates/fdemon-app/src/handler/daemon.rs:47-75` — Parse once, branch on result.

### 9. Share `EVENT_CHANNEL_CAPACITY` constant
- `crates/fdemon-daemon/src/native_logs/macos.rs:254` — Use named constant instead of `256`.

### 10. Derive `Clone` on config structs
- Add `#[derive(Clone)]` to `AndroidLogConfig` and `MacOsLogConfig`.

### 11. Add TEA handler tests
- Test `Message::NativeLog`, `NativeLogCaptureStarted`, `NativeLogCaptureStopped`, and `maybe_start_native_log_capture()`.

---

## Re-review Checklist

After addressing issues, the following must pass:

- [ ] Issues #1 and #2 resolved (blocking)
- [ ] Issue #3 resolved (macOS capture works with `min_level = "error"`)
- [ ] Issue #4 resolved (no double-start possible)
- [ ] Issue #5 resolved (no task leak on late message)
- [ ] `cargo fmt --all` — no changes
- [ ] `cargo check --workspace` — no errors
- [ ] `cargo clippy --workspace -- -D warnings` — no warnings
- [ ] `cargo test --workspace --lib` — all pass (excluding pre-existing snapshot failures)
