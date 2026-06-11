## Task: Daemon — fix gate-1 timeout visibility + cap composed detail strings (review round-1 fixes)

**Objective**: Resolve the two blocking Major findings from the Phase 4 follow-up review, folding in the
two Minor cleanups, all within `crates/fdemon-daemon/src/toolchain/checks/ios.rs`:

- **AI-1 (Major):** a timed-out/failed `xcode-select -p` currently reports `ComponentStatus::Unknown`,
  which the app's leaf rollup ignores — the iOS/macOS leaf can show **Ok** while the probe never ran.
  Re-map it to `ComponentStatus::Error` so it surfaces as a visible non-blocking `Partial`.
- **AI-2 (Major):** the non-`Ok` details composed in `classify_xcode_gates` (and the two
  `probe failed: {e}` arms) bypass the `strip_and_truncate` / `MAX_DETAIL_LEN` convention.
- **AI-3/AI-4 (Minor fold-ins):** two doc-comment defects; two test-pattern gaps.

**Depends on**: Phase 4 follow-up Wave 1 (merged). Daemon-only; compiles and tests green standalone.

**Agent:** implementor

**Complexity:** medium

**Estimated Time**: 1–2 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-daemon/src/toolchain/checks/ios.rs`

**Files Read (Dependencies):**
- `crates/fdemon-daemon/src/toolchain/checks/mod.rs` — `strip_and_truncate`, `MAX_DETAIL_LEN`.
- `crates/fdemon-app/src/install_wizard/state.rs` — `rollup_status` (read-only; the
  `ComponentStatus::Unknown` no-op at ~line 502 is the reason `Error` is required — do **not** edit
  this file).
- `crates/fdemon-daemon/src/toolchain/types.rs` — `ComponentStatus` variants.

### Details

> Locate by symbol, not line — line numbers below are current as of commit `ade2f7bc` and will drift.

#### 1. AI-1 — `XcodeSelectResult::Unknown` arm → `ComponentStatus::Error`

In `probe_xcode_tools`, the arm (currently ~`ios.rs:160-166`):

```rust
XcodeSelectResult::Unknown => {
    return ComponentCheck {
        kind: ComponentKind::XcodeTools,
        status: ComponentStatus::Unknown,   // BUG: rollup treats Unknown as a no-op → leaf can show Ok
        detail: "xcode-select -p timed out or failed".to_string(),
    };
}
```

Change `status` to `ComponentStatus::Error`. This matches the existing gate-2 timeout arm (the
`xcodebuild -version` timeout already reports `Error`) and makes the leaf roll up `Error → Partial`
(visible, non-blocking) instead of `Unknown → Ok` (invisible). Keep the detail message (optionally
extend it, e.g. "…— could not determine the active developer directory").

**Preserve unchanged:** the `check_ios(&HostPlatform::Unknown)` path that returns **two
`ComponentStatus::Unknown` checks** (host platform undetectable — that semantics is intentional and
tested), and the non-macOS empty-`Vec` path. Update the `//!` module header / fn docs if they mention
the Unknown mapping.

#### 2. AI-2 — `strip_and_truncate` every composed external-content detail

- In `classify_xcode_gates` (~`ios.rs:448-518`): wrap the **four** non-`Ok` composed details
  (license-fail, first-launch-fail, simctl-fail, unknown-gate) in `strip_and_truncate(&format!(...))`.
  The all-pass `Ok` arm's `version_detail.to_string()` is already cap-respecting (pre-truncated at
  origin) — leave it.
- In `probe_xcode_tools`'s spawn-error arm (~`ios.rs:337`): `strip_and_truncate(&format!("xcodebuild
  probe failed: {e}"))`.
- In `probe_cocoapods`'s spawn-error arm (~`ios.rs:568`): same for `"pod probe failed: {e}"`.

`classify_xcode_gates` stays a pure function — `strip_and_truncate` is itself pure, so purity and the
Linux-runnable tests are unaffected.

#### 3. AI-3 — doc-comment fixes

- `probe_xcodebuild_version_detail` (~`ios.rs:286`): the doc links to **itself**
  ("Separating it from [`probe_xcodebuild_version_detail`]"). Rewrite, e.g.: "Returning
  `Result<String, ComponentCheck>` lets [`probe_xcode_tools`] early-return on a gate-2 failure and
  thread the version string into [`classify_xcode_gates`]."
- `probe_simctl` (~`ios.rs:405-406`): replace the inverted prose with: "Exit 0 = reachable (`Pass`).
  Non-zero exit = unreachable (`Fail`). Timeout or spawn error = `Unknown`."

#### 4. AI-4 — test additions

- `test_classify_xcode_gates_simctl_fail_is_missing_with_simctl_detail`: add
  `assert!(check.detail.contains("sudo xcodebuild -runFirstLaunch"))` (matching the license /
  first-launch test pattern).
- New `test_classify_xcode_gates_fail_beats_unknown_across_gates`: `(license=Unknown,
  first_launch=Fail, simctl=Pass)` → `Missing` with the **first-launch** detail (not "could not verify
  license"). Optionally also `(Pass, Unknown, Fail)` → simctl detail.
- New `test_classify_xcode_gates_detail_respects_max_len`: feed a `version_detail` of length
  `MAX_DETAIL_LEN` (e.g. `"x".repeat(MAX_DETAIL_LEN)`) into a failing-gate arm and assert the resulting
  detail length does not exceed `strip_and_truncate`'s output contract (check how `MAX_DETAIL_LEN` +
  ellipsis is asserted in the existing `strip_and_truncate` tests in `checks/mod.rs` and mirror it).
- New (or extended) test for AI-1: assert the gate-1 unknown outcome maps to `ComponentStatus::Error`.
  The arm is inside async `probe_xcode_tools` (macOS-gated I/O), so test at whatever seam is practical —
  if no pure seam exists, a small pure helper mapping `XcodeSelectResult` → early-return
  `ComponentCheck` may be extracted, or assert via the existing match-arm structure with a
  `#[cfg(target_os = "macos")]` smoke test plus a non-cfg unit test on the extracted helper. Keep the
  extraction minimal — do not restructure the probe flow.

### Acceptance Criteria

1. **AI-1:** `XcodeSelectResult::Unknown` yields `ComponentStatus::Error`; `HostPlatform::Unknown`
   still yields two `ComponentStatus::Unknown` checks; non-macOS still yields an empty `Vec`.
2. **AI-2:** all four non-`Ok` `classify_xcode_gates` details + both `probe failed: {e}` details pass
   through `strip_and_truncate`; the max-length test proves the cap.
3. **AI-3:** no self-referential doc link; `probe_simctl` doc matches the code's Pass/Fail/Unknown
   mapping.
4. **AI-4:** the simctl remediation assertion, the cross-gate Fail-beats-Unknown test, the max-len
   test, and the AI-1 mapping test all exist and pass on Linux (except any explicitly macOS-gated
   smoke test).
5. `cargo test -p fdemon-daemon --lib toolchain` green; `cargo test --workspace --lib` green (modulo
   the pre-existing `test_run_preflight_nonexistent_sdk_path_does_not_panic` environment failure);
   `cargo fmt --all` + `cargo clippy --workspace --all-targets -- -D warnings` clean.

### Testing

```bash
cargo test -p fdemon-daemon --lib toolchain
cargo test --workspace --lib
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
```

### Notes

- **Why `Error` and not `Missing` for AI-1:** `Missing` semantically claims "Xcode/CLT not present",
  which a timeout cannot establish; `Error` is the existing convention for "probe could not complete"
  (gate-2 timeout arm, `pod` timeout arm) and rolls up to the same visible `Partial`. This stays within
  the approved status-encoding decision (no new `ComponentStatus` variant).
- Do **not** touch `fdemon-app` — `rollup_status` semantics are out of scope; the fix is entirely on
  the daemon side.
- Single task on the current branch (no worktree) — there is no parallel peer in this round.

---

## Completion Summary

**Status:** Done
**Branch:** feat/toolchain-platforms-submenu

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-daemon/src/toolchain/checks/ios.rs` | AI-1: Change `XcodeSelectResult::Unknown` arm to `ComponentStatus::Error`; extract pure `xcode_select_result_to_check` helper for testability. AI-2: Wrap four non-Ok `classify_xcode_gates` details + two `probe failed: {e}` arms through `strip_and_truncate`. AI-3: Fix self-referential doc in `probe_xcodebuild_version_detail`; fix inverted prose in `probe_simctl`. AI-4: Add 7 new tests covering remediation assertion, fail-beats-unknown, max-len cap, and gate-1 Unknown→Error mapping. |

### Notable Decisions/Tradeoffs

1. **Pure helper extraction for AI-1**: Extracted `xcode_select_result_to_check(XcodeSelectResult) -> Option<ComponentCheck>` as a minimal pure function. `None` signals "proceed to gate 2" (FullXcode); `Some(check)` is the early-return check. This avoids restructuring the probe flow while enabling host-agnostic unit tests.
2. **MAX_DETAIL_LEN in test**: `MAX_DETAIL_LEN` is `const` (not `pub`) in `checks/mod.rs` and not visible from `ios::tests`. Used a local `const MAX_LEN: usize = 256` with a comment referencing the source of truth. A clippy warning about `const` visibility in test modules would be the prompt to revisit if the value ever changes.
3. **`GateResult::clone()` in loop**: The test loop iterates over a `[(GateResult, GateResult, GateResult); 4]` array; since `GateResult` derives `Clone` but not `Copy`, cloned the three values before passing them to `classify_xcode_gates` to keep the values available for error messages.

### Testing Performed

- `cargo test -p fdemon-daemon --lib toolchain::checks::ios` — Passed (22 tests, all new tests included)
- `cargo test -p fdemon-daemon --lib toolchain` — 431 passed; 1 pre-existing failure (`test_run_preflight_nonexistent_sdk_path_does_not_panic` — environment-specific, explicitly noted in task acceptance criteria)
- `cargo test --workspace --lib` — 1209 passed; 1 pre-existing failure (same as above)
- `cargo fmt --all -- --check` — Passed
- `cargo clippy --workspace --all-targets -- -D warnings` — Passed (no warnings)

### Risks/Limitations

1. **Pre-existing test failure**: `test_run_preflight_nonexistent_sdk_path_does_not_panic` fails on this machine because `flutter` is on PATH (the test expects it to be absent). This is documented in the acceptance criteria as an expected environment failure, not introduced by this task.
