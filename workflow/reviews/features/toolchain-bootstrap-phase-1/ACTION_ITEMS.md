# Action Items: Toolchain Bootstrap — Phase 1

**Review Date:** 2026-06-03
**Verdict:** ⚠️ NEEDS WORK
**Blocking Issues:** 0 (nothing panics/corrupts) · **Should-fix-before-merge:** 3 MAJOR + 5 MINOR

## Major Issues (Should Fix Before Merge)

### 1. Kill the timed-out `flutter doctor` child + cap output reads
- **Source:** code_quality_inspector, risks_tradeoffs_analyzer, security_reviewer, logic_reasoning_checker
- **File:** `crates/fdemon-daemon/src/toolchain/doctor.rs:50-103`
- **Problem:** Timeout arm returns `None` without killing `child` (comment claims it does; no `kill_on_drop` either) → orphaned process. `read_to_end` has no byte cap.
- **Required Action:** Restructure so `child` is reachable at the timeout arm and `let _ = child.kill().await;` (or `.kill_on_drop(true)` before spawn). Add `.take(MAX_DOCTOR_OUTPUT_BYTES)` (~1 MiB) to both stdout/stderr reads. Fix/remove the misleading comment.
- **Acceptance:** A timed-out doctor run leaves no lingering `flutter` process; oversized output is truncated, not buffered for 60s.

### 2. Open the wizard on the auto-launch + missing-SDK path
- **Source:** logic_reasoning_checker, risks_tradeoffs_analyzer
- **Files:** `crates/fdemon-tui/src/runner.rs:298-308`, `crates/fdemon-tui/src/startup.rs:55-58`, `crates/fdemon-app/src/handler/update.rs:1081-1084`
- **Problem:** With `auto_launch` config and no SDK, `StartAutoLaunch` logs a warning and no-ops — silent dead-end, the exact failure the wizard was meant to replace.
- **Required Action:** Dispatch `ShowInstallWizard` from the `StartAutoLaunch` no-SDK early-return, or gate the `AutoStart` decision on `flutter_executable().is_some()`.
- **Acceptance:** New handler test: `StartAutoLaunch` with `flutter_executable() == None` transitions to `UiMode::InstallWizard`.

### 3. Guard `r` re-run against re-entrancy
- **Source:** risks_tradeoffs_analyzer
- **File:** `crates/fdemon-app/src/handler/install_wizard/actions.rs:24-35`
- **Problem:** `handle_rerun_preflight` doesn't check `loading`; mashing `r` spawns N concurrent preflights (N × `flutter doctor`).
- **Required Action:** Early-return when `state.install_wizard_state.loading` is already true.
- **Acceptance:** Test: calling `handle_rerun_preflight` while `loading == true` returns no new `RunToolchainPreflight` action.

## Minor Issues (Fix Soon — recommended this pass)

### 4. Resolve the `fdemon-tui → fdemon-daemon` runtime dependency
- **Source:** architecture_enforcer, risks_tradeoffs_analyzer
- **Files:** `crates/fdemon-tui/Cargo.toml`, `widgets/install_wizard/{doctor_view,step_detail}.rs`
- **Action (preferred):** Re-export `DoctorLine`, `DoctorMarker`, `ComponentCheck`, `ComponentStatus` from `fdemon-app::install_wizard`; repoint widget imports; drop the runtime dep (keep dev-dep). **Or** document an approved exception in `docs/ARCHITECTURE.md` + `docs/REVIEW_FOCUS.md`.

### 5. JDK unparseable-major → `Partial`/`Error`, not `Ok`
- **Source:** logic_reasoning_checker
- **File:** `crates/fdemon-daemon/src/toolchain/checks.rs:214-218`
- **Action:** Change the `None` major-version arm from `Ok` to `Partial` (or `Error`).
- **Acceptance:** Test: `parse_jdk_output` on a line with version `"1"` does not return `Ok`.

### 6. Remove the dead `_effective` binding
- **Source:** architecture_enforcer, code_quality_inspector, logic_reasoning_checker
- **File:** `crates/fdemon-app/src/handler/install_wizard/navigation.rs:~100-111`
- **Action:** Either clamp `detail_scroll` using the hint + content length, or delete `_effective` and add a comment that the clamp is renderer-deferred in Phase 1.

### 7. Fix the flaky env-mutation test
- **Source:** architecture_enforcer, code_quality_inspector, risks_tradeoffs_analyzer
- **File:** `crates/fdemon-daemon/src/toolchain/checks.rs:~742-754`
- **Action:** Add `#[serial_test::serial]` to env-touching tests (or inject env lookup). Wrap `set_var`/`remove_var` in `unsafe` as required by Rust 1.77+.
- **Acceptance:** `cargo test --workspace` (default parallel) is deterministic.

### 8. Register the new Cell render-hint + remove duplicate test
- **Source:** code_quality_inspector
- **Action (a):** Add `InstallWizardState::last_known_visible_height` to `docs/REVIEW_FOCUS.md` "Current usage".
- **Action (b):** Remove the duplicate `test_host_platform_detect_matches_cfg` from `toolchain/mod.rs` (keep in `types.rs`).

## Tracked Follow-ups (not blocking)

- **9. Split `checks.rs` (962 LOC) into `toolchain/checks/android.rs`** — do before Phase 2 adds more Android probes. Correct the "~650 lines" claim in the task summary. [all agents]
- **10. Defense-in-depth (security):** cap `DoctorLine::indent` (~32); strip ANSI + truncate `ComponentCheck::detail`; sanitize `$SHELL` if ever logged.
- **11. Consolidate the duplicated `strip_ansi`** in `doctor.rs` with `flutter_sdk::diagnostics::strip_ansi`, or add a `// DUPLICATION:` cross-reference. [risks]
- **12. Remove unused `_selected_index` param** in `compute_corrected_scroll` (`step_detail.rs`); simplify redundant Doctor-step clamp math. [code_quality, logic]

## Re-review Checklist

After addressing issues:
- [ ] M1–M3 resolved (orphan kill + output cap, auto-launch wizard, re-run guard)
- [ ] m5–m8 (JDK classification, dead binding, flaky test, doc registry) resolved or justified
- [ ] m4 resolved or documented as an approved exception
- [ ] New tests added: auto-launch→wizard, re-run guard, JDK `"1"` classification
- [ ] Quality gate green: `cargo fmt --all -- --check && cargo check --workspace --all-targets && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings`
- [ ] `cargo test --workspace` deterministic under default parallel execution
