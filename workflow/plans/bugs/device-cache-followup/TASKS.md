# Tasks: Device Cache No-TTL Review Follow-up

Plan: [BUG.md](BUG.md)
Source review: [REVIEW.md](../../../reviews/bugs/device-cache-no-ttl/REVIEW.md)

---

## Wave 1 — Independent foundation tasks

| ID | Task | Depends on | Files Modified (Write) | Files Read |
|---|---|---|---|---|
| 01 | [Clear `refreshing` on background failure](tasks/01-clear-refreshing-on-bg-failure.md) | — | `crates/fdemon-app/src/handler/update.rs`, `crates/fdemon-app/src/handler/tests.rs`, `workflow/plans/bugs/device-cache-no-ttl/BUG.md` | `target_selector_state.rs`, `state.rs` |
| 02 | [Reference-returning bootable accessor](tasks/02-bootable-accessor-refs.md) | — | `crates/fdemon-app/src/state.rs`, `crates/fdemon-app/src/handler/new_session/navigation.rs` | `target_selector_state.rs` |

## Wave 2 — Cache-miss wiring (sequential after Wave 1)

| ID | Task | Depends on | Files Modified (Write) | Files Read |
|---|---|---|---|---|
| 03 | [`DiscoverDevicesAndBootable` action + cache-miss wiring](tasks/03-discover-and-bootable-action.md) | 02 | `crates/fdemon-app/src/handler/mod.rs`, `crates/fdemon-app/src/actions/mod.rs`, `crates/fdemon-app/src/handler/new_session/navigation.rs` | `spawn.rs`, `state.rs` |

## Wave 3 — Polish (parallel after Wave 2)

| ID | Task | Depends on | Files Modified (Write) | Files Read |
|---|---|---|---|---|
| 04 | [Icon routing + compact-mode glyph](tasks/04-icon-routing-and-compact.md) | — | `crates/fdemon-tui/src/widgets/new_session_dialog/tab_bar.rs`, `crates/fdemon-tui/src/widgets/new_session_dialog/target_selector.rs` | `theme/icons.rs`, `widgets/new_session_dialog/mod.rs` |
| 05 | [Polish bundle](tasks/05-polish-bundle.md) | — | `crates/fdemon-app/src/handler/tests.rs`, `crates/fdemon-app/src/handler/mod.rs`, `crates/fdemon-app/src/handler/new_session/navigation.rs`, `crates/fdemon-app/src/new_session_dialog/target_selector_state.rs` | — |

---

## File Overlap Analysis

### Wave 1 — Overlap Matrix

| Pair | Shared Write Files | Strategy |
|---|---|---|
| 01 ↔ 02 | none | Parallel (worktree) |

Wave 1 has zero write-file overlap → tasks 01 and 02 can run in parallel worktrees.

### Wave 2 — Single Task

Wave 2 contains a single task (03), so there is no parallelism question. Task 03 writes
`navigation.rs`, which is also written by task 02; this is why 03 depends on 02 and runs
sequentially after Wave 1 has merged.

### Wave 3 — Overlap Matrix

| Pair | Shared Write Files | Strategy |
|---|---|---|
| 04 ↔ 05 | none | Parallel (worktree) |

Wave 3 has zero write-file overlap → tasks 04 and 05 can run in parallel worktrees once
Waves 1 and 2 have merged.

### Cross-wave file overlap (informational)

These files are written across multiple tasks. Wave ordering ensures sequential merges,
so there is no merge-conflict risk:

- `handler/new_session/navigation.rs`: written by 02 (Wave 1) → 03 (Wave 2) → 05 (Wave 3)
- `handler/mod.rs`: written by 03 (Wave 2) → 05 (Wave 3)
- `handler/tests.rs`: written by 01 (Wave 1) → 05 (Wave 3)
- `target_selector.rs`: written by 04 (Wave 3) only

### Read-only overlap (informational, no conflict risk)

- Task 01 reads `target_selector_state.rs` and `state.rs` (informational).
- Task 03 reads `state.rs` (modified by 02 — Wave 2 ordering ensures the post-02 state).
- Task 04 reads `theme/icons.rs` (unmodified) and `widgets/new_session_dialog/mod.rs` (for
  the existing `&'a IconSet` pattern).

---

## Dispatch Order Summary

```
Wave 1 (parallel worktrees):  01, 02
        ↓
Wave 2 (single task, sequential): 03
        ↓
Wave 3 (parallel worktrees):  04, 05
```

All tasks: `Agent: implementor`. No core docs (`docs/ARCHITECTURE.md`,
`docs/CODE_STANDARDS.md`, `docs/DEVELOPMENT.md`) require updates — this is a localized
follow-up bug fix. The only documentation change is a one-line correction to the parent
plan's `BUG.md`, applied as part of task 01 (no `doc_maintainer` routing needed for plan
docs under `workflow/`).
