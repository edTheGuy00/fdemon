# Action Items: toolchain-platforms-submenu — Phase 4 Follow-up (iOS/macOS probe hardening)

**Review Date:** 2026-06-10
**Verdict:** ⚠️ NEEDS WORK
**Blocking Issues:** 2

## Critical Issues (Must Fix)

None.

## Major Issues (Must Fix for approval)

### 1. Gate-1 timeout silently produces a passing leaf (timeout-mapping inconsistency)

- **Source:** logic_reasoning_checker
- **File:** `crates/fdemon-daemon/src/toolchain/checks/ios.rs`
- **Locate by symbol:** `probe_xcode_tools`, the `XcodeSelectResult::Unknown` match arm (~lines 160-166)
- **Problem:** `probe_xcode_select_path` maps timeout/spawn-error to `XcodeSelectResult::Unknown`, which
  `probe_xcode_tools` turns into `ComponentStatus::Unknown`. `rollup_status`
  (`fdemon-app/src/install_wizard/state.rs:502`) treats `Unknown` as a no-op, so on macOS (CocoaPods Ok)
  the iOS/macOS leaf rolls up to `StepStatus::Ok` — no Partial, no guided command. Meanwhile a gate-2
  timeout yields `Error` (→ visible Partial) and gates 3–5 Unknown yield `Missing` (→ visible Partial).
  The same root condition (probe timed out) produces three different leaf outcomes, and only the gate-1
  one disappears entirely — masking a hung Xcode as fully configured, which is the false-positive class
  this phase exists to eliminate.
- **Required Action:** In the `XcodeSelectResult::Unknown` arm, report `ComponentStatus::Error`
  (preferred — matches gate-2's timeout arm; `Missing` also acceptable) with the existing
  "could not run xcode-select…"-style detail, so the leaf caps to a visible non-blocking `Partial`.
  Daemon-only change; no app/TUI edits.
- **Acceptance:** A unit test (pure or arm-level) asserting the gate-1 unknown outcome maps to a non-`Ok`,
  non-`Unknown` `ComponentStatus`; existing non-macOS / `HostPlatform::Unknown` tests unchanged
  (`check_ios(&HostPlatform::Unknown)` still returns two `Unknown` checks — that path is distinct and
  intentional); `cargo test -p fdemon-daemon --lib toolchain` green.

### 2. Composite detail strings bypass the `MAX_DETAIL_LEN` convention

- **Source:** security_reviewer
- **File:** `crates/fdemon-daemon/src/toolchain/checks/ios.rs`
- **Locate by symbol:** `classify_xcode_gates` non-Ok arms; `probe_xcode_tools`'s
  `format!("xcodebuild probe failed: {e}")` arm; `probe_cocoapods`'s `format!("pod probe failed: {e}")` arm
- **Problem:** `version_detail` is `strip_and_truncate`'d (≤256 chars) at origin, but the classifier then
  appends ~50–60-char remediation suffixes, so the final `ComponentCheck.detail` can exceed
  `MAX_DETAIL_LEN`. The two `probe failed: {e}` arms embed unbounded, unsanitized `std::io::Error`
  strings (pattern pre-existing, but the surrounding region was rewritten this phase).
- **Required Action:** Pass each composed non-Ok detail through `strip_and_truncate` before storing it in
  `ComponentCheck.detail` (classifier arms + both `probe failed` arms).
- **Acceptance:** A unit test feeding a max-length `version_detail` into `classify_xcode_gates` asserts
  the resulting detail length ≤ `MAX_DETAIL_LEN` (plus ellipsis convention, matching
  `strip_and_truncate`'s contract); fmt/clippy/test gates green.

## Minor Issues (Fix opportunistically in the same round)

### 3. Doc-comment defects in `ios.rs`

- **Source:** architecture_enforcer, code_quality_inspector, security_reviewer
- (a) `probe_xcodebuild_version_detail` doc (~line 286) links to **itself** — rewrite to reference
  `probe_xcode_tools` or drop the sentence.
- (b) `probe_simctl` doc (~lines 405-406) inverts the mapping — should read: "Exit 0 = `Pass`. Non-zero
  exit = `Fail`. Timeout or spawn error = `Unknown`."

### 4. Test-pattern gaps in the gate classifier suite

- **Source:** code_quality_inspector, security_reviewer
- (a) Add `assert!(check.detail.contains("sudo xcodebuild -runFirstLaunch"))` to
  `test_classify_xcode_gates_simctl_fail_is_missing_with_simctl_detail` (matches the license /
  first-launch test pattern).
- (b) Add one cross-gate Fail-beats-Unknown test, e.g. `(license=Unknown, first_launch=Fail, simctl=Pass)`
  → first-launch detail, and/or `(Pass, Unknown, Fail)` → simctl detail.

## Tracked (no code change this round)

- **Release notes:** worst-case macOS probe latency is ~30s (gates 1→2 sequential before the concurrent
  3–5 join); previously-Ok Macs with incomplete first-launch / unreachable simctl now show a non-blocking
  `Partial` Xcode leaf. (Source: risks_tradeoffs_analyzer)
- **Deferred refinement:** capture stderr on the `-license check` gate to distinguish "not accepted" from
  "could not run check" (route the latter to `Unknown`); fold gates 3–5 into the same join as gate 2 to
  recover slow-path latency. Add to the phase's deferred list alongside L2–L6.
- **Resolved during consolidation:** the risks analyzer's question about the `Missing → Partial` cap is
  confirmed wired (iOS/macOS leaf cap in `state.rs`, exercised by
  `test_xcode_select_command_has_path_caveat_note`); no action.

## Re-review Checklist

After addressing issues, the following must pass:
- [ ] Major issues 1 and 2 resolved with the listed acceptance tests
- [ ] Minor issues 3–4 resolved or explicitly deferred with justification
- [ ] `cargo fmt --all -- --check && cargo check --workspace --all-targets && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings` (modulo the pre-existing `test_run_preflight_nonexistent_sdk_path_does_not_panic` environment failure)
