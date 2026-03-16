# Review: Pre-App Custom Sources (Phase 1)

**Date:** 2026-03-15
**Branch:** `feature/pre-app-custom-sources`
**Verdict:** NEEDS WORK

## Change Summary

Phase 1 implements pre-app custom source gating: custom sources configured with `start_before_app = true` spawn before the Flutter app, with five readiness check types (HTTP, TCP, command, stdout, delay) gating the Flutter launch until dependencies are healthy or timed out.

**Files Modified:** 15 source files, 2 doc files, 4 TUI snapshots, 1 new file
**Tests Added:** ~60 new unit tests across config, handler, actions, and daemon layers
**Tasks Completed:** 8/8 (all tasks marked Done)

---

## Agent Verdicts

| Agent | Verdict | Key Finding |
|-------|---------|-------------|
| Architecture Enforcer | WARNING | `pub mod ready_check` overly broad; pre-existing `config → daemon` boundary violation compounded |
| Code Quality Inspector | NEEDS WORK | HTTP buffer robustness gap; `spawn_pre_app_sources` exceeds 50-line limit |
| Logic & Reasoning Checker | CONCERNS | Auto-launch path bypasses pre-app source gating entirely |
| Risks & Tradeoffs Analyzer | CONCERNS | Coordinator task not cancellable on session close; `url` crate vs `parse_http_url` inconsistency |

---

## Critical Issues

### 1. Auto-launch path bypasses pre-app source gating

- **Source:** Logic & Reasoning Checker
- **File:** `crates/fdemon-app/src/handler/update.rs` (AutoLaunchResult handler, ~line 903)
- **Problem:** `Message::AutoLaunchResult` creates a session and returns `UpdateAction::SpawnSession` directly, without checking `has_pre_app_sources()`. Users with `auto_start = true` AND `start_before_app = true` custom sources will have Flutter launched without waiting for those sources.
- **Impact:** Directly violates the feature's core guarantee. The pre-app gate is only applied in `handle_launch()` (launch_context.rs), not in the auto-launch path.
- **Required Action:** Apply the same conditional gate in the `AutoLaunchResult` handler: check `native_logs.enabled && native_logs.has_pre_app_sources()` and return `SpawnPreAppSources` instead of `SpawnSession` when pre-app sources exist.

---

## Major Issues

### 2. `try_http_get` 256-byte buffer is insufficient for reliable HTTP checks

- **Source:** Code Quality Inspector, Risks & Tradeoffs Analyzer
- **File:** `crates/fdemon-app/src/actions/ready_check.rs:141-154`
- **Problem:** A single `read()` call with a 256-byte buffer is not guaranteed to deliver the complete status line. On slow or loaded servers, the first read may return as few as 1 byte, misclassifying a 2xx response as failure and forcing unnecessary retry cycles.
- **Suggested Fix:** Use `tokio::io::BufReader::read_line()` to read the complete status line regardless of TCP segmentation:
  ```rust
  let mut reader = BufReader::new(stream);
  let mut status_line = String::new();
  reader.read_line(&mut status_line).await?;
  ```

### 3. `pub mod ready_check` breaks `actions/` module visibility convention

- **Source:** Architecture Enforcer, Code Quality Inspector, Risks & Tradeoffs Analyzer
- **File:** `crates/fdemon-app/src/actions/mod.rs:25`
- **Problem:** All sibling modules use `pub(super)`. `pub mod` is broader than needed — only consumed by `native_logs.rs` within the same parent.
- **Required Fix:** Change to `pub(super) mod ready_check;` — no call-site changes needed.

---

## Minor Issues

### 4. `spawn_pre_app_sources` exceeds 50-line function limit (~237 lines)

- **Source:** Code Quality Inspector
- **File:** `crates/fdemon-app/src/actions/native_logs.rs:463-700`
- **Suggested Fix:** Extract the per-source spawn + forwarding task + readiness future registration into a private helper function.

### 5. `describe_ready_check` should be `Display` impl on `ReadyCheck`

- **Source:** Code Quality Inspector
- **File:** `crates/fdemon-app/src/actions/native_logs.rs:703-712`
- **Suggested Fix:** Implement `std::fmt::Display` for `ReadyCheck` in `config/types.rs`, remove the free function.

### 6. `run_command_check` timeout pattern inconsistent with siblings

- **Source:** Code Quality Inspector
- **File:** `crates/fdemon-app/src/actions/ready_check.rs:240`
- **Problem:** Uses `start.elapsed() >= timeout` while `run_http_check` and `run_tcp_check` use `remaining.is_zero()` after `saturating_sub`.
- **Suggested Fix:** Align to the `is_zero()` pattern for consistency.

### 7. `url` crate vs `parse_http_url` inconsistency

- **Source:** Risks & Tradeoffs Analyzer
- **File:** `crates/fdemon-app/Cargo.toml`, `config/types.rs:639`, `actions/ready_check.rs:161`
- **Problem:** `url::Url::parse()` is used for validation; manual `parse_http_url()` is used at runtime. The two parsers could disagree (e.g., IPv6 addresses, userinfo).
- **Suggested Fix:** Either use `url::Url::parse()` at runtime too, or replace the validation-time parse with `parse_http_url()` and remove the `url` dependency.

### 8. `test_tcp_check_timeout_on_closed_port` is environmentally fragile

- **Source:** Code Quality Inspector
- **File:** `crates/fdemon-app/src/actions/ready_check.rs:370-379`
- **Problem:** Uses port 1, which may be open on some CI environments.
- **Suggested Fix:** Bind a random port, immediately drop the listener, then test against the now-closed port.

---

## Warnings (Non-blocking)

### 9. Coordinator task not cancellable on session close

- **Source:** Risks & Tradeoffs Analyzer
- **Problem:** If a user closes a session during pre-app readiness, the coordinator task continues running readiness checks until each individually times out. Cleanup eventually happens but resources are consumed needlessly.
- **Recommendation:** Consider passing a `shutdown_rx` to the coordinator task for early abort.

### 10. Crashed post-app source re-spawn could duplicate platform capture

- **Source:** Logic & Reasoning Checker
- **Problem:** If a post-app custom source crashes (`CustomSourceStopped` removes it from handles), the guard in `maybe_start_native_log_capture` allows fall-through, which re-spawns platform capture (already running) alongside the missing post-app source.
- **Impact:** Low probability — requires a post-app source to crash, then a hot restart before it self-recovers. But could produce duplicate logcat/log-stream processes.

### 11. Pre-existing `config → daemon` boundary violation compounded

- **Source:** Architecture Enforcer
- **File:** `crates/fdemon-app/src/config/types.rs:867`
- **Problem:** `NativeLogsSettings::should_include_tag()` delegates to `fdemon_daemon`. The new helper methods (`has_pre_app_sources()`, etc.) are in the same impl block, making this violation more prominent.
- **Recommendation:** Move `should_include_tag` to `fdemon-core` in a future PR.

---

## What's Good

- TEA pattern compliance is strong: `handle_launch()` is pure, all three new message handlers are pure, side effects confined to action handlers
- Daemon-layer `spawn_with_readiness()` is cleanly designed — uses Tokio primitives, no app-layer coupling
- Double-spawn prevention uses three independent guard layers (defense in depth)
- Comprehensive test coverage: config deserialization, validation, message handling, guard logic, readiness checks, daemon stdout matching
- Backward compatibility preserved: `start_before_app` defaults to `false`, existing configs unaffected
- Timeout-then-proceed semantics correctly prevent indefinite blocking
- Well-documented in both ARCHITECTURE.md and CONFIGURATION.md

---

## Re-review Checklist

After addressing issues, the following must pass:

- [ ] Critical issue #1 resolved (auto-launch path gates on pre-app sources)
- [ ] Major issue #2 resolved (HTTP buffer robustness)
- [ ] Major issue #3 resolved (`pub(super)` visibility)
- [ ] Minor issues addressed or justified
- [ ] `cargo fmt --all && cargo check --workspace && cargo test --workspace && cargo clippy --workspace -- -D warnings`
