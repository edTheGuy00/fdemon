# Phase 4 Follow-up — iOS/macOS probe hardening (review findings) — Task Index

## Overview

Address the substantive findings from the Phase 4 code review
(`workflow/reviews/features/toolchain-platforms-submenu-phase-4/REVIEW.md` + `ACTION_ITEMS.md`).
Scope is **Blocking + Major + Medium**; the LOW/MINOR cleanups (test rename, bucket-passing, cap-helper
extraction, full pure-parser extraction, invariant test, nitpicks) are **deferred** and tracked at the bottom
of this file for a later pass.

Findings covered:

| ID | Sev | Finding | Task |
|----|-----|---------|------|
| H1 | HIGH (blocking) | Three new `ios.rs` probe spawns omit `kill_on_drop(true)` → orphaned `xcodebuild`/`pod` on timeout | 01 |
| M1 | MAJOR | `xcode-select -p` non-zero exit misclassified as `CltOnly(empty)` → false "Only CLT found ()" message | 01 |
| Md1 | MEDIUM | Docs claim `simctl`/license checks the probe never performs; `xcodebuild -version` Ok ≠ usable → **implement** the checks so `XcodeTools = Ok` genuinely means usable | 01 (code) + 03 (docs) |
| Md2 | MEDIUM | Guided `xcode-select -s /Applications/Xcode.app…` command has `note: None` — misleads non-standard installs | 02 |
| L1 | LOW (folded) | `CltOnly` path not `strip_and_truncate`'d — folded into M1's same-region edit | 01 |

### Decisions resolved (user, 2026-06-10)

1. **Md1 → implement the checks** (not just doc-correct). Add real, read-only, non-interactive license +
   first-launch + simctl probes so `XcodeTools = Ok` means *fully usable*, eliminating the false-positive Ok.
2. **Scope = Blocking + Major + Medium.** LOW/MINOR items deferred (see "Deferred" below).

### Research-confirmed probe sequence (external_researcher, sources in task 01)

All commands below are **read-only, non-interactive, no-sudo**, each wrapped in `PROBE_TIMEOUT` +
`kill_on_drop(true)`. Run **all** gates (do **not** short-circuit on first failure — a Mac can have a valid
license but incomplete first-launch, or vice-versa):

| Step | Command | Pass = | Remediation (guided, already emitted) |
|------|---------|--------|----------------------------------------|
| 1 | `xcode-select -p` | path under a full `Xcode.app/Contents/Developer` | Install Xcode |
| 2 | `xcodebuild -version` | exit 0 + parseable version | Install/repair Xcode |
| 3 | `xcodebuild -license check` | exit 0 (license accepted) | `sudo xcodebuild -license accept` |
| 4 | `xcodebuild -checkFirstLaunchStatus` | exit 0 (components installed) | `sudo xcodebuild -runFirstLaunch` |
| 5 | `xcrun simctl list devices booted` | exit 0 (simctl reachable) | `sudo xcodebuild -runFirstLaunch` |

`XcodeTools = Ok` **iff all five pass**. If full Xcode is present but a gate (3/4/5) fails, report a
non-`Ok` status with a `detail` naming the failed gate (the existing leaf guided command —
`xcode-select -s … && xcodebuild -runFirstLaunch && xcodebuild -license accept` — already remediates 3/4/5).
Exit code **69** on any `xcrun` call specifically signals an unaccepted license. `xcodebuild
-downloadPlatform iOS` is **Xcode 16+ only** (noted; gating it is a deferred nitpick).

### Why these task boundaries

- **All daemon probe work (H1 + M1 + Md1) lands in `checks/ios.rs`** as one task — the three findings touch
  the same `probe_xcode_tools` / `probe_xcode_select_path` region, so splitting them would force a same-file
  sequential chain anyway. Task 01 is daemon-only and compiles + tests green standalone.
- **Md2 is a one-field `state.rs` change** (guided-command `note`), write-disjoint from `ios.rs`, so Task 02
  runs in parallel with Task 01.
- **Docs (Task 03, `doc_maintainer`)** update `ARCHITECTURE.md` after the probe behavior lands.

**Total Tasks:** 3
**Estimated Hours:** 5–7 hours

## Task Dependency Graph

```
        ┌───────────────────────────────────────┐   ┌──────────────────────────────┐
        │ 01-daemon-harden-ios-probe (ios.rs)    │   │ 02-app-xcode-guided-path-     │  Wave 1
        │  H1 kill_on_drop + M1 misclassification │   │    caveat (state.rs)          │  (parallel
        │  + Md1 license/first-launch/simctl      │   │  Md2 note on xcode-select cmd │   worktrees)
        └────────────────────┬────────────────────┘   └──────────────┬───────────────┘
                             │ (depends on 01)                        │
                             ▼                                        │
        ┌───────────────────────────────────────┐                    │
        │ 03-update-docs (doc_maintainer)        │◄───────────────────┘            Wave 2
        │  ARCHITECTURE.md probe-sequence update  │
        └───────────────────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Est. Hours | Modules |
|---|------|--------|------------|------------|---------|
| 1 | [01-daemon-harden-ios-probe](tasks/01-daemon-harden-ios-probe.md) | Not Started | - | 3–4h | `fdemon-daemon/src/toolchain/checks/ios.rs` |
| 2 | [02-app-xcode-guided-path-caveat](tasks/02-app-xcode-guided-path-caveat.md) | Not Started | - | 0.5–1h | `fdemon-app/src/install_wizard/state.rs` |
| 3 | [03-update-docs](tasks/03-update-docs.md) | Not Started | 1 | 1h | `docs/ARCHITECTURE.md` |

## File Overlap Analysis

| Task | Files Modified (Write) | Files Read (Dependencies) |
|------|----------------------|--------------------------|
| 01 | `crates/fdemon-daemon/src/toolchain/checks/ios.rs` | `checks/mod.rs` (`PROBE_TIMEOUT`, `strip_and_truncate`), `toolchain/doctor.rs` + `toolchain/process_stream.rs` (`kill_on_drop` convention), `toolchain/types.rs` (`ComponentStatus`) |
| 02 | `crates/fdemon-app/src/install_wizard/state.rs` | `install_wizard/types.rs` (`GuidedCommand`) |
| 03 | `docs/ARCHITECTURE.md` | task 01 result, `~/.claude/skills/doc-standards/schemas.md` |

### Overlap Matrix

| Task Pair | Shared Write Files | Isolation Strategy |
|-----------|-------------------|-------------------|
| **01 + 02** | **none** (`ios.rs` vs `state.rs`) | **Parallel (worktree)** |
| 03 vs 01 | none (`ARCHITECTURE.md` vs `ios.rs`) — 03 depends on 01 | Sequential (after 01) |
| 03 vs 02 | none | Sequential (after 01; 02 may merge first or after, no conflict) |

> Tasks 01 and 02 are write-disjoint and have no dependency between them → they run in parallel worktrees.
> Task 03 is documentation and depends on Task 01's merged probe behavior.

## Success Criteria

Phase 4 follow-up is complete when:

- [ ] **H1:** all process spawns in `ios.rs` (`xcode-select`, `xcodebuild -version`, `xcodebuild -license
      check`, `xcodebuild -checkFirstLaunchStatus`, `xcrun simctl …`, `pod --version`) set
      `.kill_on_drop(true)`, matching the `doctor.rs` / `process_stream.rs` convention. No orphaned child on
      timeout.
- [ ] **M1:** `xcode-select -p` exiting non-zero (no developer tools active) no longer renders "Only Xcode
      Command Line Tools found ()". It classifies as a no-tools/unknown state with an accurate `detail`. The
      `CltOnly` path (when genuinely CLT) is `strip_and_truncate`'d (L1).
- [ ] **Md1:** `XcodeTools = Ok` **only when** full Xcode + `xcodebuild -version` + license accepted +
      first-launch complete + `simctl` reachable; any present-but-misconfigured gate yields a non-`Ok`
      status with a `detail` naming the failed gate. All gates run (no short-circuit). Each gate is
      `PROBE_TIMEOUT`-wrapped and `kill_on_drop`. A pure gate→status/detail classifier is unit-tested on
      Linux CI.
- [ ] **Md2:** the "Select Xcode & accept license" guided command carries a `note` warning that
      `/Applications/Xcode.app` is an assumption to adjust for non-standard installs.
- [ ] `cargo test --workspace --lib` green (modulo the pre-existing, unrelated
      `test_run_preflight_nonexistent_sdk_path_does_not_panic` environment failure); `cargo fmt --all` +
      `cargo clippy --workspace --all-targets -- -D warnings` clean.
- [ ] `docs/ARCHITECTURE.md` describes the real five-gate probe sequence (license/first-launch/simctl now
      implemented), replacing the prior over-claim.

## Notes

- **No new `ComponentKind`, no new config field, no new keybindings.** This is hardening of the existing
  Phase 4 probe only. The non-blocking semantics are unchanged: the app still caps `Missing → Partial` at the
  iOS/macOS leaves and handback still reads only `FlutterSdk`.
- **Status encoding for present-but-misconfigured Xcode:** report `ComponentStatus::Missing` with a precise
  `detail` (e.g. "Xcode present but license not accepted — run sudo xcodebuild -license accept"). Reusing
  `Missing` avoids a new `ComponentStatus` variant and a cross-crate match churn; the app's existing
  `Missing → Partial` cap turns it into a non-blocking `Partial` leaf whose existing guided commands already
  remediate license/first-launch/simctl. (Distinguishing present-but-broken from absent at the
  `ComponentStatus` level is a deferred nitpick — see below.)
- **Locate by symbol, not line.** Line numbers drift; find by function / variant / test name.
- **Pre-existing failure** `test_run_preflight_nonexistent_sdk_path_does_not_panic` is out of scope
  (environment artifact, present before Phase 4).

## Deferred (LOW / MINOR — not in this follow-up)

Tracked from the review for a later cleanup pass:

- **L2** — rename `test_ios_macos_leaves_absent_when_no_xcode_components` (asserts *present*, not absent).
- **L3** — pass the leaf's own component bucket into `xcode_guided_commands` instead of re-reading
  `report.components`.
- **L4** — extract `cap_missing_to_partial(&[ComponentCheck]) -> StepStatus` (three inline repetitions;
  before Phase 5 adds a fourth).
- **L5** — extract the full `Output → ComponentCheck` mapping for `xcodebuild`/`pod` into pure helpers with
  Linux-CI unit tests (Task 01 already extracts a gate classifier; the full extraction is broader).
- **L6** — regression test asserting `ios_status == macos_status` for a mixed-status report.
- **Nitpicks** — distinguish present-but-broken Xcode (`Error`/new variant) from absent (`Missing`) so guided
  text doesn't say "Install Xcode" to an installed user; gate `xcodebuild -downloadPlatform iOS` on Xcode 16+;
  use `XcodeTools`/`CocoaPods` in the TUI test fixtures instead of `Prerequisites`; move the `all_components_ok`
  note off `xcode_guided_commands`'s doc comment.
