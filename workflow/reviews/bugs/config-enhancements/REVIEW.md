# Code Review: Config Enhancements (Issues #17, #18)

**Review Date:** 2026-03-13
**Branch:** `fix/config-enhancements`
**Verdict:** :warning: **APPROVED WITH CONCERNS**
**Changes:** 674 insertions, 65 deletions across 16 files

---

## Summary

Two configuration bugs fixed by wiring existing infrastructure that was never connected:

- **Issue #17**: `settings.watcher.paths` and `settings.watcher.extensions` from `config.toml` silently ignored — `WatcherConfig` builder calls were missing in `start_file_watcher()`
- **Issue #18**: `auto_start = true` in `launch.toml` / `config.toml` had no effect — startup sequence never checked for auto-start configuration

Both fixes are minimal, surgically correct, and low-regression-risk. The root causes were correctly diagnosed. Test coverage is thorough (18 new tests). Several minor concerns were raised across reviewers that should be addressed before merge.

---

## Agent Verdicts

| Agent | Verdict | Critical | Warnings | Notes |
|-------|---------|----------|----------|-------|
| Bug Fix Reviewer | :white_check_mark: APPROVED | 0 | 1 | Root cause correct, fix complete, coverage adequate |
| Architecture Enforcer | :warning: APPROVED WITH CONCERNS | 0 | 2 | Layer boundaries respected; direct state mutation in startup + synchronous `process_message` before loop noted |
| Code Quality Inspector | :warning: NEEDS WORK | 0 | 3 | Unnecessary clone, stale doc comment, hardcoded `/tmp/test` in tests |
| Logic Reasoning Checker | :white_check_mark: PASS | 0 | 2 | All paths logically sound; empty `paths = []` silent behavior + runner duplication noted |
| Risks/Tradeoffs Analyzer | :white_check_mark: Acceptable | 0 | 3 | Low risk overall; TESTING.md path mismatch, runner duplication, empty paths UX |

---

## Issues To Address

### Must Fix (before merge)

**1. Unnecessary `configs.clone()` on Ready path**
- **File:** `crates/fdemon-tui/src/startup.rs:44`
- **Source:** Code Quality Inspector
- **Problem:** `configs` is not used after `show_new_session_dialog()` on the `Ready` branch, so the `.clone()` is gratuitous. `LoadedConfigs` contains a `Vec<SourcedConfig>` — this is a heap allocation for no reason.
- **Fix:** Change `state.show_new_session_dialog(configs.clone())` to `state.show_new_session_dialog(configs)`

**2. Stale module doc comment in watcher**
- **File:** `crates/fdemon-app/src/watcher/mod.rs:2-3`
- **Source:** Code Quality Inspector
- **Problem:** Doc comment says "Watches the `lib/` directory" — this is now incorrect after the fix. The watcher watches configurable paths.
- **Fix:** Update to reflect configurable paths (e.g., "Watches one or more configured directories for file changes...")

**3. Hardcoded `/tmp/test` path in two startup tests**
- **File:** `crates/fdemon-tui/src/startup.rs:61,75`
- **Source:** Code Quality Inspector
- **Problem:** Two tests use `Path::new("/tmp/test")` instead of `tempfile::tempdir()` as the other 5 tests in the same file do. Non-portable and inconsistent with project standards.
- **Fix:** Replace with `let dir = tempfile::tempdir().unwrap(); let project_path = dir.path();`

### Should Fix (recommended)

**4. Empty `paths = []` produces silent no-watch behavior**
- **File:** `crates/fdemon-app/src/watcher/mod.rs` (in `run_watcher`)
- **Source:** Logic Reasoning Checker, Risks/Tradeoffs Analyzer
- **Problem:** If a user writes `paths = []` in config.toml, the watcher starts successfully but watches nothing, with no log output. This is technically correct but confusing.
- **Fix:** Add a `warn!("No watch paths configured — file watcher will not trigger reloads")` when `config.paths` is empty, before the `resolve_watch_paths` loop.

**5. TESTING.md path reference mismatch**
- **File:** `example/TESTING.md`
- **Source:** Risks/Tradeoffs Analyzer
- **Problem:** Test C references `../../shared_lib` but the actual config in `app4/.fdemon/config.toml` uses `../shared_lib`. A developer following TESTING.md literally would see a mismatch.
- **Fix:** Update TESTING.md Test C to match the actual config path.

### Consider (optional improvements)

**6. Consolidate startup state mutations**
- **File:** `crates/fdemon-tui/src/startup.rs:44-45`
- **Source:** Architecture Enforcer
- **Problem:** The `Ready` path makes two sequential mutations to `AppState` directly (outside TEA update cycle): `show_new_session_dialog()` sets `ui_mode = NewSessionDialog`, then it's immediately overridden to `Startup`. This is a documented exception for TUI startup, but the two-step pattern is fragile.
- **Suggestion:** Consolidate into a single `AppState` method (e.g., `state.prepare_startup_dialog(configs)`) that encapsulates both mutations.

**7. Duplicated startup dispatch in runner.rs**
- **File:** `crates/fdemon-tui/src/runner.rs` (lines 33-57 and 119-143)
- **Source:** Logic Reasoning Checker, Risks/Tradeoffs Analyzer
- **Problem:** The identical 13-line `match startup_result { AutoStart => ..., Ready => ... }` block appears in both `run_with_project()` and `run_with_project_and_dap()`. Any future startup flow change must be applied to both.
- **Suggestion:** Extract a shared helper function. Track for extraction when a third call site or variant is added.

**8. `pending_watcher_errors` unbounded growth**
- **File:** `crates/fdemon-app/src/state.rs`
- **Source:** Bug Fix Reviewer, Architecture Enforcer
- **Problem:** `Vec<String>` with no capacity limit. If many watcher errors fire before a session starts and the user quits without starting one, the buffer grows unboundedly.
- **Suggestion:** Add a capacity cap (e.g., truncate to last N errors) or drain on quit.

**9. Synchronous `process_message()` before event loop**
- **File:** `crates/fdemon-tui/src/runner.rs:51,137`
- **Source:** Architecture Enforcer
- **Problem:** `engine.process_message(Message::StartAutoLaunch { configs })` is called synchronously before `run_loop()`. This is consistent with the existing `StartDapServer` pattern, but if the handler ever returns a follow-up message, ordering guarantees could break.
- **Suggestion:** Either switch to `engine.msg_sender().try_send()` or document the ordering assumption explicitly.

---

## Strengths

- **Minimal, surgical fixes** — 2 lines of wiring in `engine.rs`, ~30 lines of path resolution, ~30 lines of startup detection
- **Excellent test coverage** — 18 new tests covering all branches, edge cases (non-existent paths, empty paths, macOS symlink tempdir, mixed absolute/relative), and config combinations
- **Clean extraction of `resolve_watch_paths()`** — Pure function, independently testable, well-documented, `pub(crate)` visibility
- **TEA pattern compliance** — Auto-start flows correctly through `Message::StartAutoLaunch` → `handler::update()` → `UpdateAction::DiscoverDevicesAndAutoLaunch`
- **No regression risk** — Default values in `Settings` match `WatcherConfig::default()`, so behavior is identical when no config is present
- **Comprehensive example fixtures** — app3 (multi-config auto-start) and app4 (watcher path edge cases) exercise the exact scenarios from the bug reports

---

## Verification

```bash
cargo fmt --all                          # Format check
cargo clippy --workspace -- -D warnings  # Lint check
cargo test -p fdemon-app -- watcher      # Watcher path tests
cargo test -p fdemon-app -- engine       # Engine settings tests
cargo test -p fdemon-tui -- startup      # Startup auto-start tests
cargo test -p fdemon-app -- auto_launch  # Downstream auto-launch tests
cargo test --workspace                   # Full suite
```

**Known pre-existing failures:** 4 snapshot tests in `fdemon-tui` fail due to version string mismatch (`v0.1.0` vs `v0.2.1`). Unrelated to this change.

---

## Sign-off

- **Reviewed by:** 5 specialized agents (bug fix, architecture, code quality, logic, risks)
- **Files analyzed:** 7 source files + task plans + example fixtures
- **Total issues:** 0 critical, 0 major, 5 minor, 4 suggestions
