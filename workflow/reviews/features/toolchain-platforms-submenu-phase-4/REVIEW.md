# Code Review: Phase 4 — iOS + macOS install-wizard leaves

**Feature:** toolchain-platforms-submenu / Phase 4 (iOS + macOS leaves, shared Xcode/CocoaPods)
**Review Date:** 2026-06-10
**Diff Base:** `2e229e9357577e3b7f9c67395a3c982defb2686d..27579b67654a54efe145ce77c52bb3d2c0b49008`
**Tasks:** 01–05 (all merged on `feat/toolchain-platforms-submenu`)
**Reviewers:** architecture_enforcer, code_quality_inspector, logic_reasoning_checker, risks_tradeoffs_analyzer, security_reviewer

## Verdict: ⚠️ NEEDS WORK

The feature is architecturally clean, logically correct on all five core invariants (non-blocking
handback, local Missing→Partial cap, no double-penalty rollup, Partial-only guided commands, CLT-vs-full-Xcode
discrimination), and free of security issues. However, three convergent findings should be addressed before
this is considered done:

1. **[HIGH / blocking] Process leak** — the three new probe spawns in `ios.rs` omit `kill_on_drop(true)`,
   diverging from the subsystem's own convention; a hung `xcodebuild`/`pod` is orphaned on timeout.
2. **[MAJOR] Misleading diagnostic** — `xcode-select -p` exiting non-zero (no tools at all) is misclassified
   as `CltOnly`, telling the user "Only Xcode Command Line Tools found ()" when nothing is installed.
3. **[MEDIUM] Doc/code contract drift** — TASKS.md, `ARCHITECTURE.md`, and the `ios.rs` module docs claim
   `simctl` reachability and license/EULA checks that the implementation does not perform; `xcodebuild
   -version` often succeeds with an unaccepted license, yielding a false-positive `Ok`.

### Per-Agent Verdicts

| Agent | Verdict |
|-------|---------|
| architecture_enforcer | ✅ PASS (0 violations, 2 suggestions) |
| code_quality_inspector | ⚠️ APPROVED WITH CONCERNS (1 major, several minor) |
| logic_reasoning_checker | ✅ PASS (2 minor notes) |
| risks_tradeoffs_analyzer | ⚠️ CONCERNS (1 blocking, 2 medium) |
| security_reviewer | ✅ PASS (5 low, all defense-in-depth) |

## What's Strong

- **Non-blocking handback is correct and well-tested.** `flutter_now_live()` /
  `close_wizard_and_dispatch_discovery` read only `FlutterSdk`/`flutter_executable`; the Missing→Partial cap
  is local to the iOS/macOS leaf bindings and leaves `rollup_status` (and Android's true `Missing`) untouched.
  [Source: logic_reasoning_checker, architecture_enforcer, risks_tradeoffs_analyzer]
- **Shared-probe / dual-bucket model is sound.** Cloning the two checks into both buckets yields two equal
  leaf statuses, so `rollup_step_statuses` does not double-penalize the Platforms parent. [Source: all]
- **Layer boundaries respected.** Probe in `fdemon-daemon`, routing/builder in `fdemon-app`, rendering in
  `fdemon-tui` via the re-export gateway (no direct tui→daemon compile dep). TEA purity maintained.
  [Source: architecture_enforcer]
- **`is_full_xcode_path` pure-function extraction** with strong edge-case tests (versioned bundles, external
  mounts, CLT rejection, empty) is exemplary and the right testability call. [Source: risks, code_quality]
- **No injection surface.** All process spawns use fixed argument arrays; guided commands (incl. `sudo`) are
  display-only clipboard text, never executed — invariant upheld by the guided-only handler arms.
  [Source: security_reviewer]

## Consolidated Findings

### HIGH (blocking)

**H1. Missing `kill_on_drop(true)` on the three new probe spawns — orphaned processes on timeout**
- **Source:** risks_tradeoffs_analyzer
- **File:** `crates/fdemon-daemon/src/toolchain/checks/ios.rs` — `probe_xcode_select_path`,
  `probe_xcodebuild_version`, `probe_cocoapods`
- All three wrap `Command::...output().await` in `tokio::time::timeout` **without** `.kill_on_drop(true)`.
  Every other spawn in this subsystem (`process_stream.rs:78,190`, `doctor.rs:57`, `flutter_install.rs:802`)
  sets it. On timeout the `output()` future is dropped and Tokio detaches (does not SIGKILL) the child. A hung
  `xcodebuild` (license prompt, first-launch component install, disk contention) is left running. Re-checks
  (`r` key) compound the leak.
- **Fix:** Add `.kill_on_drop(true)` to all three `Command` builders.

### MAJOR

**M1. `xcode-select -p` non-zero exit misclassified as `CltOnly`, producing a false diagnostic**
- **Source:** code_quality_inspector (major); risks_tradeoffs_analyzer (low)
- **File:** `crates/fdemon-daemon/src/toolchain/checks/ios.rs` — `Ok(Ok(_)) =>
  XcodeSelectResult::CltOnly(String::new())` (~line 169)
- A non-zero exit from `xcode-select -p` means *no developer tools active*, but it is classified `CltOnly`
  with an empty path, rendering "Only Xcode Command Line Tools found (). Install full Xcode from the App
  Store." — telling the user they have CLT when they have nothing.
- **Fix:** Route this arm to `XcodeSelectResult::Unknown` (or a new `NoToolsInstalled` variant) so the
  message is accurate. The `CltOnly(String)` variant currently double-serves two distinct states — splitting
  them removes the ambiguity. Add a regression test for the no-tools message.

### MEDIUM

**Md1. Doc/code contract drift — `simctl` + license checks claimed but not implemented**
- **Source:** risks_tradeoffs_analyzer (medium); architecture_enforcer (suggestion)
- **Files:** `phase-4/TASKS.md` (lines ~9, 12, 26, 143, 146, 195–197), `docs/ARCHITECTURE.md` (~line 280),
  `crates/fdemon-daemon/src/toolchain/checks/ios.rs` (module/fn doc comments)
- `probe_xcode_tools` runs only `xcode-select -p` + `xcodebuild -version`. There is no `simctl` probe and no
  explicit license check; license state is inferred from `xcodebuild -version`'s exit code, which frequently
  succeeds even when the license is unaccepted → false-positive `Ok`, and the user who most needs
  `xcodebuild -license accept` never sees the guided command (it only emits when `Partial`).
- **Fix:** Either implement the documented `simctl`/license probes, or correct all three doc sources to state
  plainly that detection is `xcode-select` + `xcodebuild -version` only. Do not leave the contract claiming
  checks that aren't performed.

**Md2. Hardcoded `/Applications/Xcode.app` in a copy-pasteable `sudo` command with no caveat**
- **Source:** risks_tradeoffs_analyzer (medium); security_reviewer (low)
- **File:** `crates/fdemon-app/src/install_wizard/state.rs` — `xcode_guided_commands`, "Select Xcode & accept
  license" command (~line 1119, `note: None`)
- The probe accepts versioned/external-volume Xcode bundles, but the remediation command always points at the
  canonical path. A user with `Xcode_15.2.app`, `Xcode-beta.app`, or an external install copy-pastes a command
  that misconfigures `xcode-select`. The `c`-to-copy affordance actively encourages running it verbatim.
- **Fix:** Add a `note` to that command, e.g. "Adjust the path if Xcode is not in /Applications."

### LOW / MINOR

**L1. `strip_and_truncate` not applied to the `CltOnly` path** before embedding in `detail`
(`ios.rs` ~line 159) — inconsistent with every other external-output string in the file; ANSI in a crafted
developer-dir path lands in the detail row. One-line fix. [Source: security_reviewer]

**L2. Misleading test name** — `test_ios_macos_leaves_absent_when_no_xcode_components` actually asserts the
leaves are **present** (Pending) on macOS with no components. Rename to reflect the real scenario.
[Source: code_quality_inspector]

**L3. `xcode_guided_commands` re-derives missingness from `report.components`** rather than the leaf's own
(cloned) bucket. Provably equivalent today; a redundant coupling that would silently desync if the bucket
ever diverged. [Source: logic_reasoning_checker, architecture_enforcer, code_quality_inspector]

**L4. Missing→Partial cap duplicated three times** (`web_status`, `ios_status`, `macos_status`). Extract a
`cap_missing_to_partial(&[ComponentCheck]) -> StepStatus` helper before Phase 5 (Windows) adds a fourth.
[Source: code_quality_inspector]

**L5. Output-parsing branches of `probe_xcodebuild_version`/`probe_cocoapods` have no Linux-CI coverage.**
Extract the `Output → ComponentCheck` mapping into pure helpers (mirroring `is_full_xcode_path`) and add
Linux-runnable unit tests for the success / non-zero / not-found branches. [Source: risks_tradeoffs_analyzer]

**L6. No regression test for the `ios_status == macos_status` invariant** (currently comment-only). Add one
assertion over a mixed-status report. [Source: risks_tradeoffs_analyzer]

**Nitpicks:** `xcodebuild -version` non-zero → `Missing` (vs. present-but-broken `Error`) yields "Install
Xcode" guidance to a user who has it; TUI test fixtures use `ComponentKind::Prerequisites` instead of
`XcodeTools`/`CocoaPods`; the `all_components_ok` note is misplaced on `xcode_guided_commands`'s doc comment.

## Documentation Freshness

`docs/ARCHITECTURE.md` was updated by Task 05 and accurately reflects the new module/types/model — **except**
it inherits the `simctl`/license over-claim (see Md1). No new crate/dependency/build-step, so DEVELOPMENT.md
and CODE_STANDARDS.md need no change. CONFIGURATION.md and KEYBINDINGS.md correctly untouched (no new config
field, no new keybindings).

## Note: Pre-existing Test Failure

`test_run_preflight_nonexistent_sdk_path_does_not_panic` (fdemon-daemon) fails on this dev machine. Confirmed
present verbatim at the base commit `2e229e93` and **unrelated** to Phase 4 (environment artifact: Flutter
resolvable via `PATH`). Out of scope for this review.

See `ACTION_ITEMS.md` for the prioritized fix list.
