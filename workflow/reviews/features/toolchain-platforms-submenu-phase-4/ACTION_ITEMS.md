# Action Items: Phase 4 — iOS + macOS install-wizard leaves

**Review Date:** 2026-06-10
**Verdict:** ⚠️ NEEDS WORK
**Blocking Issues:** 1

## Critical Issues (Must Fix)

### 1. Add `kill_on_drop(true)` to the three new probe spawns
- **Source:** risks_tradeoffs_analyzer
- **File:** `crates/fdemon-daemon/src/toolchain/checks/ios.rs`
- **Functions:** `probe_xcode_select_path`, `probe_xcodebuild_version`, `probe_cocoapods`
- **Problem:** All three `Command` builders lack `.kill_on_drop(true)`. On `PROBE_TIMEOUT` the `output()`
  future is dropped and Tokio detaches the child instead of killing it, orphaning a hung `xcodebuild`/`pod`.
  Diverges from the subsystem convention (`process_stream.rs`, `doctor.rs`, `flutter_install.rs` all set it).
- **Required Action:** Add `.kill_on_drop(true)` to each `Command` builder.
- **Acceptance:** All three spawns set `kill_on_drop(true)`; behavior matches the rest of the toolchain
  subsystem.

## Major Issues (Should Fix)

### 2. Fix `xcode-select -p` non-zero-exit misclassification
- **Source:** code_quality_inspector
- **File:** `crates/fdemon-daemon/src/toolchain/checks/ios.rs` (~line 169)
- **Problem:** `Ok(Ok(_)) => XcodeSelectResult::CltOnly(String::new())` classifies "no developer tools at all"
  as CLT-only, producing the false message "Only Xcode Command Line Tools found (). Install full Xcode…".
- **Suggested Action:** Route the non-zero-exit arm to `XcodeSelectResult::Unknown` (or add a
  `NoToolsInstalled` variant) so the message is accurate; add a unit test for the no-tools message path.

### 3. Reconcile the `simctl` / license-check contract drift
- **Source:** risks_tradeoffs_analyzer, architecture_enforcer
- **Files:** `phase-4/TASKS.md`, `docs/ARCHITECTURE.md` (~line 280),
  `crates/fdemon-daemon/src/toolchain/checks/ios.rs` (module + fn doc comments)
- **Problem:** Docs claim `simctl` reachability and license/EULA acceptance checks; the code performs only
  `xcode-select -p` + `xcodebuild -version`. `xcodebuild -version` can succeed with an unaccepted license,
  yielding a false-positive `Ok` and hiding the `xcodebuild -license accept` guided command.
- **Suggested Action:** Either implement the `simctl`/license probes, or correct all three doc sources to
  describe the actual two-step detection. Pick one — do not leave docs claiming unimplemented checks.

## Minor Issues (Consider Fixing)

1. **`strip_and_truncate` the `CltOnly` path** before storing it in `detail` (`ios.rs` ~line 159) — match the
   handling of all other external-process output in the file. [security_reviewer]
2. **Rename `test_ios_macos_leaves_absent_when_no_xcode_components`** — it asserts the leaves are *present*
   (Pending), not absent. [code_quality_inspector]
3. **Pass the leaf's own component bucket into `xcode_guided_commands`** instead of re-reading
   `report.components`, removing the redundant coupling. [logic_reasoning_checker, architecture_enforcer]
4. **Extract `cap_missing_to_partial(&[ComponentCheck])`** before Phase 5 adds a fourth non-blocking leaf.
   [code_quality_inspector]
5. **Extract `Output → ComponentCheck` mapping into pure helpers** and add Linux-CI unit tests for the
   xcodebuild/pod success / non-zero / not-found branches. [risks_tradeoffs_analyzer]
6. **Add a regression test** asserting `ios_status == macos_status` for a mixed-status report. [risks]
7. **Nitpicks:** distinguish present-but-broken Xcode (`Error`) from absent (`Missing`); use
   `XcodeTools`/`CocoaPods` in the TUI test fixtures instead of `Prerequisites`; move the `all_components_ok`
   note off `xcode_guided_commands`'s doc comment.

## Re-review Checklist

After addressing issues, the following must pass:
- [ ] Item 1 (kill_on_drop) resolved — no orphaned probe processes on timeout
- [ ] Item 2 (misclassification) resolved or justified
- [ ] Item 3 (doc drift) resolved — docs and code agree on what is probed
- [ ] `cargo test --workspace --lib` green (modulo the pre-existing, unrelated
      `test_run_preflight_nonexistent_sdk_path_does_not_panic` environment failure)
- [ ] `cargo fmt --all -- --check` + `cargo clippy --workspace --all-targets -- -D warnings` clean
