# Phase 4 Follow-up — Fix Round 1 (review findings) — Task Index

## Overview

Address the 2 blocking Major findings (plus the 2 fold-in Minor cleanups) from the Phase 4 follow-up
review (`workflow/reviews/features/toolchain-platforms-submenu-phase-4-followup/REVIEW.md` +
`ACTION_ITEMS.md`, round 1, 2026-06-10). All edits land in a single file
(`crates/fdemon-daemon/src/toolchain/checks/ios.rs`), so this round is **one task**.

Root causes verified in code (planner, 2026-06-10):

- **AI-1 (Major):** `ios.rs:160-166` — the `XcodeSelectResult::Unknown` arm reports
  `ComponentStatus::Unknown`; `rollup_status` (`fdemon-app/src/install_wizard/state.rs:502`) treats
  `Unknown` exactly like `Ok` (no-op), so a timed-out/hung `xcode-select -p` lets the iOS/macOS leaf
  roll up to `Ok` with no guided command. Gate-2's timeout arm (`ios.rs:341`) already uses
  `ComponentStatus::Error` (→ `any_partial` → visible `Partial`); gates 3–5 `Unknown` map to `Missing`.
  Fix = align the gate-1 arm with gate-2: `Error`.
- **AI-2 (Major):** `classify_xcode_gates` (`ios.rs:448-518`) — all four non-`Ok` arms compose
  `format!("{version_detail} — …")` without re-applying `strip_and_truncate`, so the stored detail can
  exceed `MAX_DETAIL_LEN` (version_detail is already ≤cap; the suffix adds ~50–60 chars). Same gap in
  `format!("xcodebuild probe failed: {e}")` (`ios.rs:337`) and `format!("pod probe failed: {e}")`
  (`ios.rs:568`), which embed unbounded `std::io::Error` strings.
- **AI-3 (Minor, fold-in):** self-referential doc link on `probe_xcodebuild_version_detail`
  (`ios.rs:286`); inverted `Fail`/`Unknown` prose in the `probe_simctl` doc (`ios.rs:405-406`).
- **AI-4 (Minor, fold-in):** simctl gate test lacks the remediation-command assertion the other two gate
  tests make; no cross-gate Fail-beats-Unknown test.

No design decision changes: the status encoding stays within existing `ComponentStatus` variants, the
`Missing`-for-misconfigured rule from the approved plan is untouched, and the intentional
`HostPlatform::Unknown` → two `ComponentStatus::Unknown` checks path is explicitly preserved.

**Total Tasks:** 1
**Estimated Hours:** 1–2 hours

## Task Dependency Graph

```
┌────────────────────────────────────────────────────────┐
│ 01-daemon-fix-gate1-timeout-and-detail-caps (ios.rs)   │  Wave 1 (single task,
│  AI-1 Unknown→Error + AI-2 strip_and_truncate caps     │   current branch)
│  + AI-3 doc comments + AI-4 test-pattern gaps          │
└────────────────────────────────────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Est. Hours | Modules |
|---|------|--------|------------|------------|---------|
| 1 | [01-daemon-fix-gate1-timeout-and-detail-caps](tasks/01-daemon-fix-gate1-timeout-and-detail-caps.md) | ✅ Done (validated PASS) | - | 1–2h | `fdemon-daemon/src/toolchain/checks/ios.rs` |

## File Overlap Analysis

| Task | Files Modified (Write) | Files Read (Dependencies) |
|------|----------------------|--------------------------|
| 01 | `crates/fdemon-daemon/src/toolchain/checks/ios.rs` | `checks/mod.rs` (`strip_and_truncate`, `MAX_DETAIL_LEN`), `fdemon-app/src/install_wizard/state.rs` (`rollup_status` — read-only, to confirm visibility semantics), `toolchain/types.rs` (`ComponentStatus`) |

### Overlap Matrix

| Task Pair | Shared Write Files | Isolation Strategy |
|-----------|-------------------|-------------------|
| (single task) | n/a | Sequential on current branch (no worktree) |

## Success Criteria

- [ ] **AI-1:** the `XcodeSelectResult::Unknown` arm reports `ComponentStatus::Error` (not `Unknown`),
      so a timed-out `xcode-select -p` surfaces as a visible non-blocking `Partial` leaf. The
      `HostPlatform::Unknown` path still returns two `ComponentStatus::Unknown` checks (unchanged).
- [ ] **AI-2:** every non-`Ok` detail composed in `classify_xcode_gates`, plus the
      `xcodebuild probe failed: {e}` and `pod probe failed: {e}` details, passes through
      `strip_and_truncate` before storage; a max-length `version_detail` test proves the cap holds.
- [ ] **AI-3:** both doc-comment defects fixed.
- [ ] **AI-4:** simctl test asserts the remediation command; at least one cross-gate
      Fail-beats-Unknown test added.
- [ ] `cargo test --workspace --lib` green (modulo the pre-existing
      `test_run_preflight_nonexistent_sdk_path_does_not_panic` environment failure);
      `cargo fmt --all` + `cargo clippy --workspace --all-targets -- -D warnings` clean.

## Notes

- **Deferred (recorded, no tasks):** release-note items — worst-case macOS probe latency ~30s (gates
  1→2 sequential before the concurrent 3–5 join) and the intentional behavior change (previously-`Ok`
  Macs with incomplete first-launch / unreachable simctl now show a non-blocking `Partial`); license-gate
  stderr capture to distinguish "not accepted" from "could not run check"; folding gates 3–5 into the
  same join as gate 2. These join the original follow-up's deferred list (L2–L6 + nitpicks).
- **Resolved without action:** the risks analyzer's `Missing → Partial` cap question — the cap is wired
  in `state.rs` (iOS/macOS leaf cap) and exercised by `test_xcode_select_command_has_path_caveat_note`.
