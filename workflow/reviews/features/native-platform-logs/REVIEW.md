# Code Review: Native Platform Log Capture (Phase 1)

**Date:** 2026-03-10
**Branch:** `feature/native-platform-logs`
**Reviewer Agents:** Architecture Enforcer, Code Quality Inspector, Logic & Reasoning Checker, Risks & Tradeoffs Analyzer

---

## Verdict: NEEDS WORK

| Agent | Verdict | Critical | Major | Minor |
|-------|---------|----------|-------|-------|
| Architecture Enforcer | CONCERNS | 0 | 3 warnings | 2 suggestions |
| Code Quality Inspector | NEEDS WORK | 0 | 3 | 6 |
| Logic & Reasoning Checker | CONCERNS | 2 | 4 | 2 |
| Risks & Tradeoffs Analyzer | CONCERNS | 2 blocking | 3 high | 4 medium |

---

## Summary

The implementation is architecturally sound. Layer boundaries are respected across all 4 crates, the TEA pattern is followed correctly (state mutations through `update()`, side effects via `UpdateAction`), and the native log lifecycle follows the established `watch::channel<bool>` + `JoinHandle` shutdown pattern used by perf/network monitoring. Test coverage of pure functions (parsing, filtering, priority mapping) is strong with 29+ new tests.

However, there are **2 blocking issues** that must be fixed before merge, plus several significant quality concerns.

---

## Blocking Issues

### 1. `check_macos_log()` always returns `false` on macOS

**File:** `crates/fdemon-daemon/src/tool_availability.rs:132-142`
**Found by:** Risks & Tradeoffs Analyzer, Logic Checker

The check runs `log --help` and verifies `status.success()`. On macOS, `log --help` exits with code 64 (non-zero). This means `macos_log` will **always** be `false`, even on valid macOS systems. The bug is currently masked because `native_logs_available()` is never called in the spawn pipeline (see issue #2), but it is a latent defect that will break macOS capture the moment the tool check is wired in.

**Fix:** Use `Command::new("log").arg("help")` (no `--` prefix, which macOS `log` accepts) or `Command::new("which").arg("log")`.

### 2. `ToolAvailability::native_logs_available()` is dead code

**File:** `crates/fdemon-app/src/handler/session.rs` (missing call), `crates/fdemon-daemon/src/tool_availability.rs` (defined but unused)
**Found by:** All 4 agents

Task 03 explicitly created `native_logs_available()` with the stated intent that "the app layer (task 07) will read `tools.adb` before attempting to spawn `adb logcat`". However, neither `maybe_start_native_log_capture()` nor `spawn_native_log_capture()` ever calls it. Without `adb` installed, every Android session start will attempt to spawn `adb logcat`, fail, log a warning, and silently exit -- defeating the purpose of the tool availability system.

**Fix:** Add guard in `maybe_start_native_log_capture()`:
```rust
if !state.tool_availability.native_logs_available(platform) {
    return None;
}
```

---

## Major Issues

### 3. macOS `log stream --level error` is invalid

**File:** `crates/fdemon-daemon/src/native_logs/macos.rs:135-140`
**Found by:** Code Quality Inspector, Logic Checker

The `build_log_stream_command` maps `min_level = "warning"` or `"error"` to `--level error`. But macOS `log stream --level` only accepts `"default"`, `"info"`, and `"debug"`. Passing `"error"` will cause `log stream` to fail to start, silently breaking macOS native log capture for any user who sets `min_level = "warning"` or `"error"`.

**Fix:** Map `"warning" | "error"` to `"info"` and rely on the existing severity filtering in the parse loop to discard lower-priority messages.

### 4. No double-start guard for native log capture

**File:** `crates/fdemon-app/src/handler/session.rs:265-298`
**Found by:** Logic Checker

Unlike `maybe_connect_vm_service` which guards with `!handle.session.vm_connected && handle.vm_shutdown_tx.is_none()`, `maybe_start_native_log_capture` has no equivalent guard checking `handle.native_log_shutdown_tx.is_some()`. If `AppStart` fires twice for the same session, two capture processes would be spawned. The second `NativeLogCaptureStarted` would overwrite the first's handles, leaking the first capture task.

**Fix:** Add `handle.native_log_shutdown_tx.is_none()` guard.

### 5. Late `NativeLogCaptureStarted` after session closure leaks the capture task

**File:** `crates/fdemon-app/src/handler/update.rs:1958`
**Found by:** Logic Checker

If the session is closed between `StartNativeLogCapture` dispatch and `NativeLogCaptureStarted` arrival, the handler finds no session, drops the `shutdown_tx` Arc without sending `true`, and the capture task runs indefinitely (sending messages for a non-existent session).

**Fix:** When session is not found in the `NativeLogCaptureStarted` handler, explicitly send `true` on `shutdown_tx` before dropping it.

---

## Minor Issues

### 6. Operator precedence hazard in `needs_capture`

**File:** `crates/fdemon-app/src/handler/session.rs:293`
**Found by:** All 4 agents

```rust
let needs_capture = platform == "android" || cfg!(target_os = "macos") && platform == "macos";
```

This is technically correct (`&&` binds tighter than `||`), but the unparenthesized form is a maintenance hazard. Every agent flagged this independently.

**Fix:** Add explicit parentheses: `platform == "android" || (cfg!(target_os = "macos") && platform == "macos")`

### 7. Triplicated tag-filtering logic

**Files:** `config/types.rs:602`, `android.rs:92`, `macos.rs:111`
**Found by:** Architecture Enforcer, Code Quality Inspector, Risks Analyzer

The same include-overrides-exclude, case-insensitive tag filter is implemented three times. If one copy is updated and others are not, platforms would filter inconsistently.

**Fix:** Extract to a shared function in `native_logs/mod.rs`.

### 8. Triple-parse of daemon messages in Stdout path

**File:** `crates/fdemon-app/src/handler/daemon.rs:47-75`
**Found by:** Code Quality Inspector, Logic Checker, Risks Analyzer

`parse_daemon_message` is called up to 3 times for the same stdout line: once for `AppDebugPort`, once for `AppStart` boolean check, once to re-extract `AppStart` data. Each call involves JSON deserialization.

**Fix:** Parse once and branch on the result.

### 9. Magic number `256` in macos.rs

**File:** `crates/fdemon-daemon/src/native_logs/macos.rs:254`
**Found by:** Architecture Enforcer, Code Quality Inspector

`android.rs` defines `const EVENT_CHANNEL_CAPACITY: usize = 256`, but `macos.rs` hardcodes `256` directly.

**Fix:** Share the constant from `mod.rs` or define it locally.

### 10. Manual field-clone instead of `derive(Clone)` on config structs

**Files:** `android.rs:243-249`, `macos.rs:248-253`
**Found by:** Code Quality Inspector

Both `AndroidLogConfig` and `MacOsLogConfig` are cloned field-by-field in `spawn()`. If a new field is added, it will cause a compile error (safe) but is unnecessarily verbose.

**Fix:** Derive `Clone` on both config structs.

### 11. No handler tests for new Message variants

**File:** `crates/fdemon-app/src/handler/tests.rs`
**Found by:** Code Quality Inspector

Zero tests for `Message::NativeLog`, `Message::NativeLogCaptureStarted`, `Message::NativeLogCaptureStopped`, or `maybe_start_native_log_capture()`.

**Fix:** Add integration tests for TEA message handling.

---

## Accepted Risks (Phase 1)

These were documented in task summaries and accepted as Phase 1 limitations:

| Risk | Impact | Phase 2? |
|------|--------|----------|
| PID changes on hot restart | Old capture gets EOF, new AppStart triggers fresh capture | No action needed |
| No restart on capture crash | User loses native logs until session restart | Yes |
| Platform timestamp discarded | Cannot correlate with device time | Yes |
| `derive_macos_process_name` fragile | May not match actual Xcode product name | Yes |
| Non-UTF-8 logcat output breaks loop | Loses subsequent logs | Yes |
| `min_level` not validated at parse time | Silent misconfiguration | Yes (add warning) |
| Header line count in macOS log stream | Fragile if `log` emits extra diagnostic lines | Yes |
| Engine shutdown doesn't explicitly clean up native log tasks | Tasks killed by runtime drop | Low priority |

---

## Strengths

- **Clean layer boundaries**: All crate dependencies verified correct. `fdemon-core` remains dependency-free.
- **Consistent TEA pattern**: Message variants, UpdateAction, stateless action dispatch all follow established patterns.
- **Strong test coverage**: 29+ new unit tests covering parsing, filtering, priority mapping, event conversion.
- **Proper `#[cfg(target_os = "macos")]` gating** throughout the macOS codepath.
- **Good defensive coding**: `kill_on_drop(true)`, `biased` select for shutdown priority, graceful PID resolution fallback.
- **`LogBatcher` integration** prevents render loop saturation from high-volume native logs.
- **Shutdown lifecycle** covered in 3 session exit paths (process exit, app stop, session close).
- **Well-documented task summaries** with explicit tradeoff reasoning.

---

## Verification

```bash
cargo fmt --all && cargo check --workspace && cargo clippy --workspace -- -D warnings && cargo test --workspace --lib
```

Pre-existing: 4 snapshot test failures in `fdemon-tui::render::tests` (version string mismatch `v0.1.0` vs `v0.2.1`) -- unrelated to this feature.
