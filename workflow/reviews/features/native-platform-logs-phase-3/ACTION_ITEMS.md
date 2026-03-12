# Action Items: Native Platform Logs — Phase 3

**Review Date:** 2026-03-12
**Verdict:** NEEDS WORK
**Blocking Issues:** 5 (1 critical + 4 major)

## Critical Issues (Must Fix)

### 1. Add min_level severity filtering to macOS `run_log_stream_capture`

- **Source:** Logic & Reasoning Checker
- **File:** `crates/fdemon-daemon/src/native_logs/macos.rs:152-225`
- **Problem:** macOS backend does not apply event-level min_level filtering. Setting `min_level = "warning"` filters Android and iOS logs correctly but has no effect on macOS. The code's own comment says "Higher-level filtering is handled downstream" but the downstream filter was never implemented.
- **Required Action:** Add `parse_min_level(&config.min_level)` at the start of `run_log_stream_capture` and apply the `event.level.severity() < min.severity()` guard after `syslog_line_to_event`, exactly as done in `run_idevicesyslog_capture` (`ios.rs:162,209-213`) and `run_simctl_log_capture` (`ios.rs:280,339-344`).
- **Acceptance:** `min_level = "error"` in config produces only error-level macOS logs (not info/warning).

## Major Issues (Should Fix)

### 2. Extend hot-restart guard to prevent duplicate custom source spawning

- **Source:** Code Quality Inspector
- **File:** `crates/fdemon-app/src/handler/session.rs:301-309`
- **Problem:** The guard `handle.native_log_shutdown_tx.is_some()` prevents double-start of platform capture on hot-restart. But for sessions using only custom sources, the guard never fires. Each hot-restart spawns duplicate custom source processes.
- **Required Action:** Extend the guard:
  ```rust
  if handle.native_log_shutdown_tx.is_some()
      || !handle.custom_source_handles.is_empty()
  {
      return None;
  }
  ```
- **Acceptance:** Hot-restart does not spawn duplicate custom source processes.

### 3. Fix `NativeLogCaptureStopped` tag state reset

- **Source:** Code Quality Inspector, Logic Checker, Risks Analyzer
- **File:** `crates/fdemon-app/src/handler/update.rs:2015-2023`
- **Problem:** `NativeLogCaptureStopped` unconditionally resets `native_tag_state` to default. Custom sources running independently lose their tag visibility preferences (hidden tags reappear). The reset is also redundant since `handle_session_exited` already resets it.
- **Required Action:** Either guard the reset behind `handle.custom_source_handles.is_empty()`, or remove the reset entirely from the `NativeLogCaptureStopped` handler (the session exit/stop paths already handle it).
- **Acceptance:** When `adb logcat` exits while custom sources are running, tag filter selections are preserved.

### 4. Abort custom source task handles in `shutdown_native_logs`

- **Source:** Logic Checker, Risks Analyzer
- **File:** `crates/fdemon-app/src/session/handle.rs:197-206`
- **Problem:** Platform capture cleanup calls `handle.abort()` as a fallback. Custom source cleanup only sends the shutdown signal. Dropping `JoinHandle` detaches the task without aborting it, creating potential zombie tasks.
- **Required Action:** Add abort to the custom source shutdown loop:
  ```rust
  for handle in &mut self.custom_source_handles {
      let _ = handle.shutdown_tx.send(true);
      if let Some(h) = handle.task_handle.take() {
          h.abort();
      }
  }
  self.custom_source_handles.clear();
  ```
- **Acceptance:** No detached Tokio tasks after session close.

### 5. Remove debug scaffolding from production code

- **Source:** Architecture Enforcer, Code Quality Inspector
- **Files:** `crates/fdemon-app/src/actions/native_logs.rs:62-66`, `crates/fdemon-app/src/handler/session.rs:304-346`
- **Problem:** Four `tracing::info!("[native-logs-debug] ...")` calls pollute every user's log file for every session start.
- **Required Action:** Downgrade all four to `tracing::debug!` and remove the `[native-logs-debug]` prefix. Or remove entirely if not needed.
- **Acceptance:** No `[native-logs-debug]` strings in `tracing::info!` calls.

## Minor Issues (Consider Fixing)

### 6. Move `parse_min_level` to `fdemon-core`

- **Source:** Architecture Enforcer
- The function operates entirely on `&str` and `LogLevel` (both core types). Should be `LogLevel::from_level_str()` or similar in `fdemon-core::types`.

### 7. Call `CustomSourceConfig::validate()` from spawn path

- **Source:** Code Quality Inspector
- Replace the inline guard in `spawn_custom_sources` with `source_config.validate()` so platform tag shadowing warnings are not silently skipped.

### 8. Normalize tag case for per-tag config lookup

- **Source:** Logic Checker, Risks Analyzer
- Make `effective_min_level` case-insensitive: `self.tags.get(&tag.to_ascii_lowercase())` or store keys as lowercase during deserialization.

### 9. Promote tag column width to named constant

- **Source:** Code Quality Inspector
- Replace `let tag_col_width: usize = 20` in `tag_filter.rs:95` with a named constant and derivation comment.

### 10. Warn when syslog format used on non-macOS

- **Source:** Risks Analyzer
- Add a `tracing::warn!` in `parse_syslog` on non-macOS, or validate at config parse time.

### 11. Add duplicate custom source name validation

- **Source:** Risks Analyzer
- Warn at config parse time, or use a unique index for handle removal instead of name.

## Re-review Checklist

After addressing issues, the following must pass:
- [ ] All 5 critical/major issues resolved
- [ ] `cargo fmt --all` — formatted
- [ ] `cargo check --workspace` — compiles
- [ ] `cargo test --workspace` — all tests pass
- [ ] `cargo clippy --workspace -- -D warnings` — no warnings
- [ ] macOS min_level filtering has a test
- [ ] Hot-restart with custom sources has a test
- [ ] No `[native-logs-debug]` strings remain in info-level tracing
