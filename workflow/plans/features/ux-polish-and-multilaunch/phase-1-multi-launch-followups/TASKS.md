# Phase 1 Follow-ups: Multi-Launch Review Remediation — Task Index

## Overview

Follow-up tasks addressing the Phase 1 (Multi-Device Launch Picker) review
(`workflow/reviews/features/ux-polish-and-multilaunch-phase-1/`). The review
returned **NEEDS WORK** on two converging concerns in the fan-out failure path
plus several minors and one user-requested UI improvement.

Research confirmed the fixes are small and well-localized:
- `SessionManager::remove_session` is already `pub` (`session_manager.rs:201`) — the M1 orphan fix is ~1 line, and the code's "no undo API" comment is factually wrong.
- The m4 bug is sharper than first reported: `save_last_selection` runs only for `i == 0` *and* after the skip-check, so if device 0 is skipped, **no** device persists the auto-launch default.
- m6 should use the public `fdemon_core::strip_ansi_codes` (the daemon's `strip_ansi` is `pub(crate)`).
- Removing the per-row platform icons makes `device_icon`, `bootable_device_icon`, the `icons` field, and `with_icons` fully dead; platform group headers always render, so no platform is left unlabeled.

**Total Tasks:** 4
**Estimated Hours:** 7–10h

## Task Dependency Graph

```
All four tasks are independent (no dependencies, no shared write files).

┌──────────────────────────────┐  ┌──────────────────────────────┐
│ 01-harden-multilaunch-fanout │  │ 02-validate-extra-args       │
│ (launch_context.rs)          │  │ (config/launch.rs)           │
└──────────────────────────────┘  └──────────────────────────────┘
┌──────────────────────────────┐  ┌──────────────────────────────┐
│ 03-remove-platform-icons     │  │ 04-document-multilaunch-res. │
│ (device_list.rs)             │  │ (docs/KEYBINDINGS.md)        │
└──────────────────────────────┘  └──────────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Est. Hours | Priority | Items |
|---|------|--------|------------|------------|----------|-------|
| 1 | [01-harden-multilaunch-fanout](tasks/01-harden-multilaunch-fanout.md) | ✅ Done (Validated: PASS) | - | 3–4h | High | M1, M2, m3, m4, m6 |
| 2 | [02-validate-extra-args](tasks/02-validate-extra-args.md) | ✅ Done (Validated: PASS) | - | 1–2h | Low (optional) | m7 |
| 3 | [03-remove-platform-icons](tasks/03-remove-platform-icons.md) | ✅ Done (Validated: CONCERN) | - | 2–3h | Medium | UI #8 |
| 4 | [04-document-multilaunch-resources](tasks/04-document-multilaunch-resources.md) | ✅ Done (Validated: PASS) | - | 0.5h | Low | m5 |

### Validation Concern (Task 03)

Task 03 functional removal is correct and complete (icons gone, dead code removed,
all builds/tests green), but validation flagged a **test-quality gap**: AC #5 asks for
an explicit assertion that the `[M]`/`[W]`/`[D]` glyphs are **absent** from rendered
output, and no such "assert absence" test was added. Existing checkbox tests pass
because they never asserted icon presence. Impact: a future regression re-introducing
icons would not be caught by the suite. Recommend adding an absence assertion in a
cleanup pass or during deep review.

## File Overlap Analysis

| Task | Files Modified (Write) | Files Read (Dependencies) |
|------|------------------------|---------------------------|
| 01-harden-multilaunch-fanout | `crates/fdemon-app/src/handler/new_session/launch_context.rs` | `crates/fdemon-app/src/session_manager.rs`, `crates/fdemon-core/src/ansi.rs` |
| 02-validate-extra-args | `crates/fdemon-app/src/config/launch.rs` | `crates/fdemon-app/src/config/types.rs` |
| 03-remove-platform-icons | `crates/fdemon-tui/src/widgets/new_session_dialog/device_list.rs` | `crates/fdemon-tui/src/widgets/new_session_dialog/target_selector.rs`, `crates/fdemon-tui/src/theme/icons.rs` |
| 04-document-multilaunch-resources | `docs/KEYBINDINGS.md` | - |

### Overlap Matrix

| Task Pair | Shared Write Files | Isolation Strategy |
|-----------|-------------------|--------------------|
| 01 + 02 | None | Parallel (worktree) |
| 01 + 03 | None | Parallel (worktree) |
| 01 + 04 | None | Parallel (worktree) |
| 02 + 03 | None | Parallel (worktree) |
| 02 + 04 | None | Parallel (worktree) |
| 03 + 04 | None | Parallel (worktree) |

**Waves:** Single wave — all four tasks write disjoint files in (mostly) different crates and have no inter-dependencies, so all run in parallel worktrees.

## Success Criteria

This follow-up phase is complete when:

- [ ] No orphaned session remains in `SessionManager` when a multi-launch device fails after session creation (M1); the misleading "no undo API" comment is removed.
- [ ] A dedicated test covers the cap-hit-mid-loop path: partial launch + warn toast + no panic (M2).
- [ ] The eviction-policy coupling is documented in `spawn_one` (m3).
- [ ] `save_last_selection` persists the first *successfully launched* device, never a skipped one (m4).
- [ ] Daemon-sourced device names/reasons are ANSI-stripped before appearing in toasts/errors (m6).
- [ ] `extra_args` are validated at a single chokepoint before reaching `Command` (m7) — or the task is explicitly deferred with rationale.
- [ ] Platform-letter icons (`[M]`/`[W]`/`[D]`) no longer render on connected or bootable device rows; freed width goes to the device name; dead code removed (UI #8).
- [ ] User docs note that confirming N checked devices launches up to N concurrent Flutter processes (m5).
- [ ] `cargo test --workspace` and `cargo clippy --workspace --all-targets -- -D warnings` pass.

## Notes

- **Source review:** `workflow/reviews/features/ux-polish-and-multilaunch-phase-1/REVIEW.md` and `ACTION_ITEMS.md`.
- Task 02 (m7) is **optional** — the review rated `extra_args` HIGH in isolation but MINOR under the local-developer trust model (args reach `Command::args()` as separate, non-shell-evaluated elements). Include it for defense-in-depth; defer if scope is tight.
- The NITPICK / pre-existing cleanup items from the review (auto-save duplication, `calculate_scroll_offset` dedup, magic-number comments, etc.) are intentionally **not** scheduled here — track them in a separate cleanup pass to keep this remediation focused.
