# Review: Native Platform Logs — Phase 3 (Custom Sources + Docs)

**Date:** 2026-03-12
**Branch:** `feature/native-platform-logs`
**Reviewer:** Automated (4 agents)
**Verdict:** NEEDS WORK

---

## Change Summary

Phase 3 adds user-configurable custom log source processes (`[[native_logs.custom_sources]]` in `.fdemon/config.toml`), 4 format parsers (raw, JSON, logcat-threadtime, syslog), full documentation updates (CONFIGURATION.md, ARCHITECTURE.md), a new website docs page, and example project updates. This review covers the full branch diff (phases 1-3 + fixes, 18,712 lines across 124 files) with focus on the unstaged phase-3 work.

**9 tasks completed:** config types, format parsers, custom source runner, app integration, tests, example updates, docs, website page, architecture docs.

---

## Agent Verdicts

| Agent | Verdict | Critical | Major | Minor |
|-------|---------|----------|-------|-------|
| Architecture Enforcer | WARNING | 0 | 2 | 1 |
| Code Quality Inspector | NEEDS WORK | 0 | 4 | 5 |
| Logic & Reasoning Checker | CONCERNS | 1 | 3 | 3 |
| Risks & Tradeoffs Analyzer | CONCERNS | 0 | 4 | 3 |

**Consolidated Verdict: NEEDS WORK** — 1 critical bug (macOS min_level filtering missing), 4 unique major issues across agents, and several overlapping medium-severity concerns.

---

## Critical Issues (1)

### 1. macOS `run_log_stream_capture` does not apply event-level min_level filtering

- **Source:** Logic & Reasoning Checker
- **File:** `crates/fdemon-daemon/src/native_logs/macos.rs:152-225`
- **Severity:** CRITICAL

The Android backend applies a severity filter after parsing each event. The iOS backend (both simulator and physical) does the same. The macOS backend does NOT. It relies solely on the `--level` argument to `log stream`, but that argument only accepts `"default"`, `"info"`, or `"debug"` — there is no `"warning"` or `"error"` level.

The code itself documents this at line 122-126:
> "macOS `log stream` only accepts: 'default', 'info', 'debug'. There is no 'warning' or 'error' level — map those to 'default' so the command remains valid. Higher-level filtering is handled downstream by level comparison on parsed events."

The comment says "Higher-level filtering is handled downstream" but that downstream filter is **never implemented** in `run_log_stream_capture`. Setting `min_level = "warning"` correctly filters Android and iOS logs but has no effect on macOS logs.

---

## Major Issues (4)

### 2. Hot-restart guard misses custom-sources-only case — causes duplicate process spawning

- **Source:** Code Quality Inspector
- **File:** `crates/fdemon-app/src/handler/session.rs:301-309`
- **Severity:** MAJOR

The guard `handle.native_log_shutdown_tx.is_some()` prevents double-start of the platform capture on hot-restart. But for sessions using only custom sources (where `native_log_shutdown_tx` stays `None`), the guard never fires. Each hot-restart triggers `AppStart`, which re-enters `spawn_custom_sources`, spawning a new set of identical processes alongside the ones already running.

### 3. `NativeLogCaptureStopped` resets tag state while custom sources may still be running

- **Source:** Code Quality Inspector, Logic Checker, Risks Analyzer (all three)
- **File:** `crates/fdemon-app/src/handler/update.rs:2015-2023`
- **Severity:** MAJOR

When the platform capture exits (e.g., `adb logcat` crashes), `NativeLogCaptureStopped` unconditionally resets `native_tag_state` to default. Custom sources running independently lose their tag visibility preferences — any user-hidden tags reappear. The reset is also redundant since `handle_session_exited` and `handle_session_message_state` already reset it.

### 4. Custom source task handles not aborted on shutdown — potential zombie tasks

- **Source:** Logic Checker, Risks Analyzer
- **File:** `crates/fdemon-app/src/session/handle.rs:197-206`
- **Severity:** MAJOR

Platform capture cleanup both sends the shutdown signal AND aborts the task handle as a fallback. Custom source cleanup only sends the signal, then calls `clear()` which drops the `JoinHandle` — detaching the task without aborting it. This creates a resource leak window, especially with misbehaving custom source processes.

### 5. Debug scaffolding `[native-logs-debug]` left in production code at `info!` level

- **Source:** Architecture Enforcer, Code Quality Inspector
- **Files:** `crates/fdemon-app/src/actions/native_logs.rs:62-66`, `crates/fdemon-app/src/handler/session.rs:304-346`
- **Severity:** MAJOR

Four `tracing::info!("[native-logs-debug] ...")` calls are development artifacts that will appear in every user's log file for every session start. Should be `tracing::debug!` or removed entirely.

---

## Minor Issues (7)

### 6. `parse_min_level` placed in daemon layer but is a pure core utility

- **Source:** Architecture Enforcer
- **File:** `crates/fdemon-daemon/src/native_logs/mod.rs:145`, called from `crates/fdemon-app/src/handler/update.rs:1941`
- Logically belongs in `fdemon-core` alongside `LogLevel`.

### 7. `CustomSourceConfig::validate()` is dead code — spawn path duplicates validation inline

- **Source:** Code Quality Inspector
- **File:** `crates/fdemon-app/src/config/types.rs:596-627`, `crates/fdemon-app/src/actions/native_logs.rs:259-265`
- The spawn path skips the platform tag shadowing warning that `validate()` provides.

### 8. Case-sensitivity mismatch in tag handling

- **Source:** Logic Checker, Risks Analyzer
- `should_include_tag` (daemon) is case-insensitive; `effective_min_level` (config) and `is_tag_visible` (handler) are case-sensitive.
- Per-tag config keys silently fail to match if casing differs from the emitted tag.

### 9. Magic number `20` for tag column width in `tag_filter.rs`

- **Source:** Code Quality Inspector
- **File:** `crates/fdemon-tui/src/widgets/tag_filter.rs:95`
- Should be a named constant per CODE_STANDARDS.md.

### 10. Syslog format silently produces no output on non-macOS

- **Source:** Risks Analyzer
- **File:** `crates/fdemon-daemon/src/native_logs/formats.rs:152-159`
- `parse_syslog` on non-macOS always returns `None` — no warning or error.

### 11. `std::env::set_var` in tests is unsound for parallel test execution

- **Source:** Code Quality Inspector
- **File:** `crates/fdemon-daemon/src/tool_availability.rs:280-292`
- Global env mutation without synchronization can corrupt parallel test state.

### 12. Duplicate custom source names cause orphaned processes

- **Source:** Risks Analyzer
- `CustomSourceStopped` removes handles by name — if two sources share a name, both handles are removed when one stops.

---

## Strengths

- **Clean trait abstraction**: `NativeLogCapture` scales elegantly from platform backends to custom sources
- **Format parser delegation**: Reuses existing android/macos parsers — no code duplication
- **Unified event pipeline**: Custom sources flow through `Message::NativeLog`, inheriting all tag filtering and UI treatment
- **Proper `kill_on_drop(true)`** on child processes
- **Comprehensive tests**: `custom.rs` has 11 tests, `formats.rs` has 23 tests, `handler/tests.rs` adds 7 lifecycle tests
- **Good error handling**: Spawn failures are warnings, closed session races handled in both `CustomSourceStarted` and `NativeLogCaptureStarted`
- **Layer boundaries respected**: Core has zero internal deps, daemon depends only on core, app orchestrates without parsing
- **TEA pattern compliance**: All side effects via `UpdateAction`, all events via `Message` enum
- **Backwards compatible**: Existing configs without `custom_sources` deserialize correctly

---

## Quality Metrics

| Metric | Score | Notes |
|--------|-------|-------|
| Architecture | 4/5 | Layer boundaries correct; `parse_min_level` placement is the only concern |
| Rust Idioms | 4/5 | Good iterators, borrows, pattern matching; `Arc<Mutex<Option<JoinHandle>>>` is acceptable |
| Error Handling | 3/5 | macOS min_level bug, validate() dead code, silent syslog failure |
| Testing | 4/5 | Thorough unit tests; `set_var` race and missing handler tests for custom lifecycle |
| Documentation | 5/5 | All public items documented; module headers present; architecture docs updated |
| Maintainability | 3/5 | Debug logs, hot-restart double-spawn, tag state reset bugs are subtle to diagnose |

---

## Recommendations

1. **Fix macOS min_level filtering** — Add the same severity check that exists in Android and iOS capture loops
2. **Fix hot-restart guard** — Extend to check `custom_source_handles.is_empty()`
3. **Fix tag state reset** — Only reset when ALL capture sources have stopped, or remove the redundant reset from `NativeLogCaptureStopped`
4. **Abort custom source task handles** — Add `handle.task_handle.take().map(|h| h.abort())` in shutdown loop
5. **Clean up debug logs** — Downgrade to `tracing::debug!` and remove `[native-logs-debug]` prefix
6. **Move `parse_min_level` to core** — It operates on core types only
7. **Call `validate()` from spawn path** — Replace inline guard with `source_config.validate()`
8. **Normalize tag case** — Make `effective_min_level` case-insensitive

---

**Reviewed by:** Architecture Enforcer, Code Quality Inspector, Logic & Reasoning Checker, Risks & Tradeoffs Analyzer
**Files Analyzed:** 124 files, 18,712 lines changed across 4 crates + docs + examples + website
