# Phase 5 Follow-up: Runnable-Filtering Review Fixes — Task Index

## Overview

Addresses the findings from the Phase 5 code review
(`workflow/reviews/features/phase-5-runnable-filtering/REVIEW.md` and `ACTION_ITEMS.md`).
Phase 5 itself is functionally complete and its acceptance criteria are met; this follow-up
closes the review's two MAJOR items plus the agreed minor/UX cleanups.

**Decisions carried in from review triage:**
- **M2 scope = dialog-only.** The `is_supported` filter is *intentionally* scoped to the
  new-session dialog's Connected tab (matching the Phase 5 "Connected tab only" scope).
  `find_auto_launch_target` / headless / cached-selection paths remain exempt **by design**.
  This follow-up *documents* that boundary rather than extending the filter — so it is not
  later "fixed" as a regression. No production code change for M2.
- **Coverage = everything**, including the MEDIUM UX "N hidden" footer.

**Total Tasks:** 4
**Estimated Hours:** 3–5h

## Task Dependency Graph

```
   (no dependencies — all four are independent, disjoint file sets)

   ┌───────────────────────────┐   ┌───────────────────────────┐
   │ 01-app-test-and-cleanups   │   │ 02-daemon-cleanups         │
   │   (fdemon-app)            │   │   (fdemon-daemon)         │
   └───────────────────────────┘   └───────────────────────────┘

   ┌───────────────────────────┐   ┌───────────────────────────┐
   │ 03-connected-hidden-footer │   │ 04-document-filter-scope   │
   │   (fdemon-tui)            │   │   (docs/REVIEW_FOCUS.md)  │
   └───────────────────────────┘   └───────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Est. Hours | Modules |
|---|------|--------|------------|------------|---------|
| 1 | [01-app-test-and-cleanups](tasks/01-app-test-and-cleanups.md) | Not Started | - | 1–2h | `fdemon-app/src/new_session_dialog/target_selector_state.rs`, `device_groups.rs` |
| 2 | [02-daemon-cleanups](tasks/02-daemon-cleanups.md) | Not Started | - | 0.5–1h | `fdemon-daemon/src/lib.rs`, `devices.rs` |
| 3 | [03-connected-hidden-footer](tasks/03-connected-hidden-footer.md) | Not Started | - | 1–2h | `fdemon-tui/src/widgets/new_session_dialog/device_list.rs` |
| 4 | [04-document-filter-scope](tasks/04-document-filter-scope.md) | Not Started | - | 0.5h | `docs/REVIEW_FOCUS.md` |

## File Overlap Analysis

| Task | Files Modified (Write) | Files Read (Dependencies) |
|------|------------------------|---------------------------|
| 01-app-test-and-cleanups | `crates/fdemon-app/src/new_session_dialog/target_selector_state.rs`, `crates/fdemon-app/src/new_session_dialog/device_groups.rs` | - |
| 02-daemon-cleanups | `crates/fdemon-daemon/src/lib.rs`, `crates/fdemon-daemon/src/devices.rs` | - |
| 03-connected-hidden-footer | `crates/fdemon-tui/src/widgets/new_session_dialog/device_list.rs` | `crates/fdemon-app/src/new_session_dialog/device_groups.rs` (read-only) |
| 04-document-filter-scope | `docs/REVIEW_FOCUS.md` | `crates/fdemon-app/src/spawn.rs` (read-only) |

### Overlap Matrix

| Task Pair | Shared Write Files | Isolation Strategy |
|-----------|-------------------|--------------------|
| 01 + 02 | None (different crates) | Parallel (worktree) |
| 01 + 03 | None (01 = app, 03 = tui; 03 only *reads* device_groups.rs) | Parallel (worktree) |
| 01 + 04 | None (code vs doc) | Parallel (worktree) |
| 02 + 03 | None (different crates) | Parallel (worktree) |
| 02 + 04 | None (code vs doc) | Parallel (worktree) |
| 03 + 04 | None (code vs doc) | Parallel (worktree) |

**Waves:** Wave 1 = `01` + `02` + `03` + `04`, all in parallel worktrees. There is zero
write-file overlap across the four tasks, and no task depends on another's output (task 03's
footer math is self-contained in `device_list.rs`; it only *reads* the already-merged
`group_connected_devices` behavior from Phase 5).

## Success Criteria

This follow-up is complete when:

- [ ] (M1) `toggle_checked_cursor_skips_unsupported` actually invokes `toggle_checked_cursor()` and asserts the checked-set is unmodified — the production guard path is covered.
- [ ] (M2) `docs/REVIEW_FOCUS.md` records that `is_supported` filtering is dialog-scoped by design and `find_auto_launch_target` is exempt.
- [ ] (m1/UX) When the Connected tab has ≥1 supported device AND ≥1 hidden unsupported device, a muted "(N hidden: not runnable for this project)" footer is shown; absent when nothing is hidden.
- [ ] (minors) `.any()` idiom, direct-`BTreeSet` collect, `DeviceCapabilities` re-export, defensive `.get()`/`.last()` indexing, and `debug!`→`trace!` stdout demotion are applied.
- [ ] `cargo test --workspace`, `cargo fmt --all -- --check`, and `cargo clippy --workspace --all-targets -- -D warnings` pass.

## Keyboard Shortcuts

No new keybindings.

## Notes

- **Deferred / not actioned this follow-up** (recorded for traceability):
  - **n1 — `capabilities` dead field:** It was *just* added in Phase 5 as forward-compat for
    future UI (hot-reload/restart greying). Removing it now would be churn; revisit only if
    no consumer lands within ~2 phases. Leave in place.
  - **m7 — `Device` lacks `Default` (the ~27-literal churn tax):** Real tech debt, but adding
    `#[derive(Default)]` (empty-`platform` Device) or a test-only builder is a broader
    test-infra refactor with its own ripple/merge surface. Tracked as a standalone
    cleanup, intentionally out of scope here to keep this follow-up low-risk and the four
    tasks conflict-free. Consider a dedicated task if the next `Device` field lands.
- **M2 is documentation-only by decision.** If a future product call flips the invariant to
  system-wide, that becomes a *new* feature task (shared `Device::is_runnable()` +
  `find_auto_launch_target` fallback filter + tests), not a bug fix against this follow-up.
- All four tasks are independent; if any single task is blocked in review, the other three
  are unaffected.
