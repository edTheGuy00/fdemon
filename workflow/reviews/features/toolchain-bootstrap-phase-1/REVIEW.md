# Code Review: Toolchain Bootstrap — Phase 1

**Review Date:** 2026-06-03
**Branch:** `feat/toolchain-bootstrap`
**Diff Base:** `7f5b022e9c2268c1aeeaa4eb4ce3ed1758b78b89..HEAD`
**Change Type:** Feature implementation (read-only toolchain preflight + InstallWizard diagnostics modal)
**Scope:** 32 files, +4857 / −17 across `fdemon-daemon`, `fdemon-app`, `fdemon-tui`, docs

## Overall Verdict: ⚠️ NEEDS WORK

No agent returned REJECTED, and nothing in the diff panics, corrupts state, or fails the quality gate (fmt/check/test/clippy all pass). However, **four reviewer agents independently returned CONCERNS/NEEDS WORK**, converging on a small set of genuine should-fix-before-merge defects — most notably an orphaned-process leak on `flutter doctor` timeout (with a comment that claims the cleanup it never performs), a UX dead-end for auto-launch users with no SDK, and a missing re-entrancy guard on preflight re-run. None are blockers, but together they warrant a revision pass.

### Agent Verdicts

| Agent | Verdict |
|-------|---------|
| architecture_enforcer | ⚠️ WARNING (0 critical, 2 warnings) |
| code_quality_inspector | ⚠️ NEEDS WORK |
| logic_reasoning_checker | ⚠️ CONCERNS |
| risks_tradeoffs_analyzer | ⚠️ CONCERNS (0 blockers, 2 HIGH) |
| security_reviewer | ⚠️ CONCERNS (0 critical, 3 medium) |

### What's Good

- **Scope discipline is excellent.** Verified across multiple agents: no Phase 2+ leakage — no `reqwest`/`zip`/`tar`/`sha2` deps, no `[toolchain]` config keys, no `RunWizardStep`/download messages, no `Enter` step-execution binding. The READ-ONLY constraint holds.
- **`run_preflight` never-fails contract** is correctly implemented; all probe failures encode as component statuses.
- **`parse_doctor_output` is genuinely pure/total** with strong edge-case coverage (empty, garbage, ANSI/OSC, ASCII fallbacks).
- **TEA compliance is clean** — pure handlers, side effects via `UpdateAction::RunToolchainPreflight`, view purity preserved except the correctly-annotated `Cell` render-hint. Modal precedence registered. `try_send` used to avoid blocking startup.
- **No command-injection surface** — all external commands use fixed argument vectors, never shell strings. Env-var-derived paths are used only for filesystem existence checks, never as command arguments.
- **`apply_report` defensively clamps `selected_index`**, so the concurrency issues below cause no panics.

---

## Consolidated Findings

Findings are deduplicated across agents; `[Source: ...]` credits each agent that flagged the issue. Severity is the highest assigned by any agent.

### 🟠 MAJOR (should fix before merge)

#### M1. Orphaned `flutter doctor` process on timeout; comment claims a kill that never happens
- **Source:** code_quality_inspector, risks_tradeoffs_analyzer, security_reviewer, logic_reasoning_checker
- **File:** `crates/fdemon-daemon/src/toolchain/doctor.rs:50-103`
- **Confirmed by direct read.** `child` is moved into the timeout future (line 50). On timeout the `Err(_)` arm (lines 95-102) logs a warning, carries the comment `// Kill the lingering process on timeout`, and returns `None` — **but never calls `child.kill()`**, and `exe.command()` does not set `kill_on_drop(true)`. Dropping a `tokio::process::Child` only detaches it; the OS process keeps running. `flutter doctor -v` does network + SDK-cache I/O and can linger for minutes. On the wizard's target audience (broken toolchain), repeated `r` re-runs multiply the leak (see M3).
- **Aggravating factor (security):** the two `read_to_end` calls (lines 53-54, 62-63) have **no byte cap** — only the 60s wall-clock timeout. A misbehaving/replaced Flutter binary streaming output could grow memory unbounded for the full 60s before the future is dropped.
- **Fix:** Restructure so `child` is accessible at the timeout arm and call `let _ = child.kill().await;` (or set `.kill_on_drop(true)` on the command before spawn). Add `.take(MAX_DOCTOR_OUTPUT_BYTES)` (e.g. 1 MiB) to both reads. Correct or remove the misleading comment.

#### M2. Auto-launch + missing SDK is a silent dead-end (feature's headline benefit doesn't fire)
- **Source:** logic_reasoning_checker, risks_tradeoffs_analyzer
- **Files:** `crates/fdemon-tui/src/runner.rs:298-308`, `crates/fdemon-tui/src/startup.rs:55-58`, `crates/fdemon-app/src/handler/update.rs:1081-1084`
- The wizard-on-missing-SDK hook was added **only** to the `StartupAction::Ready` branch. `startup_flutter` chooses `AutoStart` based purely on config presence (`auto_launch`/cache opt-in), independent of whether Flutter exists. When `AutoStart` is taken with no SDK, `StartAutoLaunch`'s handler logs `"no Flutter SDK — cannot auto-launch"` and returns `UpdateResult::none()` — leaving the user on an empty Normal screen with no wizard and no error UI. This is exactly the dead-end the feature was meant to eliminate, and it hits configured users (the most affected cohort) while sparing unconfigured ones.
- This is a **requirement-alignment gap, not a regression** of prior behavior (the `Ready` branch is correct and `DeviceDiscoveryFailed` machinery is preserved).
- **Fix:** Dispatch `ShowInstallWizard` from the `StartAutoLaunch` no-SDK early-return, or gate the `AutoStart` decision on `flutter_executable().is_some()`. Add a handler test asserting `StartAutoLaunch` with no SDK → `UiMode::InstallWizard`.

#### M3. No re-entrancy guard on `r` re-run → unbounded concurrent preflights
- **Source:** risks_tradeoffs_analyzer
- **File:** `crates/fdemon-app/src/handler/install_wizard/actions.rs:24-35` (+ spawn at `actions/mod.rs:800-815`)
- `handle_rerun_preflight` sets `loading = true` and returns `RunToolchainPreflight` without checking whether `loading` is already true. Each `r` press fire-and-forget-spawns a fresh `run_preflight` (which spawns `git`/`java`/`adb` + a 60s `flutter doctor`). Mashing `r` (natural when a check looks stuck) spawns N concurrent preflights — compounding M1's orphan leak and causing out-of-order `ToolchainPreflightCompleted` UI flicker. No panic (`apply_report` clamps), but a real resource/UX issue.
- **Fix:** Early-return in `handle_rerun_preflight` when `state.install_wizard_state.loading` is already true (mirrors the existing `StartAutoLaunch` `UiMode::Loading` guard).

### 🟡 MINOR (fix soon)

#### m4. `fdemon-tui` gains a new **runtime** dependency on `fdemon-daemon` (layer-boundary deviation)
- **Source:** architecture_enforcer, risks_tradeoffs_analyzer
- **Files:** `crates/fdemon-tui/Cargo.toml`, `widgets/install_wizard/doctor_view.rs:22`, `step_detail.rs:25`
- `docs/ARCHITECTURE.md` (dependency matrix) and `docs/REVIEW_FOCUS.md` (layer table) state `fdemon-tui` depends only on `fdemon-core` + `fdemon-app`, and `tui/` should NOT import from `daemon/`. These are the first production (non-`#[cfg(test)]`) `use fdemon_daemon::...` imports in the crate. No dependency cycle exists, but it widens the presentation layer's documented contract. The "couldn't touch `fdemon-app`" justification is a task-split artifact, not a design constraint (task 02 already edited `fdemon-app/src/lib.rs`).
- **Fix (preferred):** Re-export `DoctorLine`, `DoctorMarker`, `ComponentCheck`, `ComponentStatus` from `fdemon-app` (e.g. `install_wizard/mod.rs`); point the widget imports at `fdemon_app::...`; drop the runtime dep (keep daemon as dev-dep). **Or** keep the dep and add an explicit approved-exception entry to `docs/ARCHITECTURE.md` + `docs/REVIEW_FOCUS.md` (mirroring the `version_check` precedent).

#### m5. JDK with a parseable-quote-but-unparseable major version classifies as `Ok`
- **Source:** logic_reasoning_checker
- **File:** `crates/fdemon-daemon/src/toolchain/checks.rs:214-218` (confirmed by direct read)
- When `extract_quoted_version` succeeds but `parse_java_major_version` returns `None` (e.g. a bare `"1"`), the `None` arm returns `ComponentStatus::Ok`. An *unknown* major version is treated as *good* — an inverted safety condition. Real JDKs rarely report this way (so MINOR), but it's a latent correctness trap that could mask a misreported JDK 8.
- **Fix:** Change the `None` arm to `Partial` (or `Error`), consistent with conservative classification.

#### m6. Dead `_effective` binding in `handle_down` implies incomplete logic
- **Source:** architecture_enforcer, code_quality_inspector, logic_reasoning_checker
- **File:** `crates/fdemon-app/src/handler/install_wizard/navigation.rs:~100-111`
- The `Detail`-pane branch computes `_effective` from the render-hint, discards it, and advances `detail_scroll` by 1 unbounded; the render-time clamp is the only guard. State value diverges from displayed offset, which will mislead any future direct reader of `detail_scroll` (scrollbar, page-down multiplier). The dead binding reads as wiring that was never finished.
- **Fix:** Either consume the hint + content length to clamp `detail_scroll` in the handler, or remove `_effective` entirely and add a one-line comment that the upper-bound clamp is intentionally deferred to the renderer in Phase 1.

#### m7. `checks.rs` is 962 lines (≈2× the 500-line standard); completion summary understated it as ~650
- **Source:** all five agents
- **File:** `crates/fdemon-daemon/src/toolchain/checks.rs`
- The Android probes (`check_android_*`, `android_sdk_root`, `count_subdirs`, helpers) form a clearly bounded group (~380 LOC + tests). Extracting `toolchain/checks/android.rs` brings both files under the limit. The task explicitly deferred this; track it before Phase 2 adds more Android probes.
- **Fix:** Follow-up refactor task. Also correct the inaccurate line-count note in the task summary.

#### m8. Flaky env-mutation test introduced by this PR (mislabeled "pre-existing")
- **Source:** architecture_enforcer, code_quality_inspector, risks_tradeoffs_analyzer
- **File:** `crates/fdemon-daemon/src/toolchain/checks.rs:~742-754`
- `test_android_sdk_root_from_env_android_home` uses `std::env::set_var`/`remove_var` (process-global, and `unsafe` since Rust 1.77) and races other tests reading those vars under the default parallel `cargo test`. The summary says it passes with `--test-threads=1` — i.e. CI's default parallel run is non-deterministic. This test is **new in this PR**, so it owns the defect.
- **Fix:** Add `#[serial_test::serial]` (already a workspace dev-dep) to the env-touching tests, or refactor `android_sdk_root` to accept an injectable env lookup.

#### m9. New `Cell<usize>` render-hint not registered in `docs/REVIEW_FOCUS.md`
- **Source:** code_quality_inspector
- **File:** `docs/REVIEW_FOCUS.md` "Approved TEA Exception → Current usage"
- `InstallWizardState::last_known_visible_height` is correctly annotated at the write site (`step_detail.rs`) but missing from the authoritative registry list. Add a bullet so the list stays exhaustive.

#### m10. Duplicate test `test_host_platform_detect_matches_cfg`
- **Source:** code_quality_inspector
- **Files:** `crates/fdemon-daemon/src/toolchain/types.rs:~224-235` and `toolchain/mod.rs:~144-155`
- Identical test body in both modules. Keep it in `types.rs` (home of `HostPlatform::detect`); remove the `mod.rs` copy.

### 🔵 NITPICK / Defense-in-depth

- **n11. Unbounded `indent` allocation** [security] — `DoctorLine::indent` (count of leading spaces from untrusted doctor output) feeds `" ".repeat(indent)` per frame. Cap to e.g. `MIN(.., 32)` in `parse_single_line` (`doctor.rs`).
- **n12. ANSI not stripped from `ComponentCheck::detail`** [security] — `check_git`/`check_jdk` stderr paths store raw stderr into `detail`. Not a terminal-injection risk (ratatui renders escapes as literal cells), but cosmetically ugly. Apply the existing `strip_ansi` and truncate to ~256 cols as defense-in-depth.
- **n13. Duplicated `strip_ansi`** [risks] — `doctor.rs` forks a second ANSI stripper (for OSC handling) instead of extending the shared `flutter_sdk::diagnostics::strip_ansi`. Will drift. Consolidate, or add a `// DUPLICATION:` cross-reference.
- **n14. Unused `_selected_index` param** [code_quality] — `compute_corrected_scroll` in `step_detail.rs` accepts a `_`-prefixed param used at no call site. Remove until needed.
- **n15. Redundant Doctor-step clamp math** [logic] — `step_detail.rs` `unwrap_or(1)` is dead in the `None` branch and `start.min(lines.len())` is already guaranteed by the clamp; simplify or comment the defensive intent.
- **n16. `$SHELL` basename used without validation** [security] — display-only, `file_name()` strips traversal; harmless today but sanitize if ever logged/exported.
- **n17. `count_subdirs` collapses "missing" and "unreadable" to `Missing`** [logic] — a permission error arguably warrants `Error`/`Unknown`. Low impact in read-only diagnostics.

---

## Documentation Freshness

Phase 1 docs were updated by task 06 (`docs/ARCHITECTURE.md`, `docs/CODE_STANDARDS.md`) and task 03 (`docs/KEYBINDINGS.md`), and validated. Two doc gaps surfaced during review:
- `docs/REVIEW_FOCUS.md` — missing the new `Cell` render-hint entry (m9) and, if m4 is resolved via "keep the dep", a new approved-exception entry for the `tui → daemon` dependency.

---

## Recommendation

Address the three MAJOR items (M1–M3) and the quick MINOR cleanups (m5, m6, m8, m9, m10) before merge — they are all low-effort and several share files. Track m4, m7, and the nitpicks as follow-ups (m7 explicitly before Phase 2). See `ACTION_ITEMS.md` for the actionable checklist.

The feature is well-built and scope-disciplined; this is a polish pass, not a redesign.
