# Tasks: Device Cache Drops After 30s (Issue #33 follow-up)

Plan: [BUG.md](BUG.md)

---

## Wave 1 — Independent foundation tasks

| ID | Task | Depends on | Files Modified (Write) | Files Read |
|---|---|---|---|---|
| 01 | [Remove device cache TTL](tasks/01-remove-ttl.md) | — | `crates/fdemon-app/src/state.rs` | — |
| 02 | [Add refreshing state flags](tasks/02-refreshing-state-flags.md) | — | `crates/fdemon-app/src/new_session_dialog/target_selector_state.rs` | — |
| 03 | [Combined background refresh action](tasks/03-combined-bg-action.md) | — | `crates/fdemon-app/src/handler/mod.rs`, `crates/fdemon-app/src/actions/mod.rs` | `crates/fdemon-app/src/spawn.rs` |
| 05 | [Tab bar refreshing indicator](tasks/05-tab-bar-indicator.md) | — | `crates/fdemon-tui/src/widgets/new_session_dialog/tab_bar.rs` | — |

## Wave 2 — Wiring (sequential after Wave 1)

| ID | Task | Depends on | Files Modified (Write) | Files Read |
|---|---|---|---|---|
| 04 | [Dialog-open wiring](tasks/04-dialog-open-wiring.md) | 02, 03 | `crates/fdemon-app/src/handler/new_session/navigation.rs` | `state.rs`, `target_selector_state.rs`, `handler/mod.rs` |
| 06 | [Target selector wires flags into TabBar](tasks/06-target-selector-wiring.md) | 02, 05 | `crates/fdemon-tui/src/widgets/new_session_dialog/target_selector.rs` | `target_selector_state.rs`, `tab_bar.rs` |

---

## File Overlap Analysis

### Wave 1 — Overlap Matrix

| Pair | Shared Write Files | Strategy |
|---|---|---|
| 01 ↔ 02 | none | Parallel (worktree) |
| 01 ↔ 03 | none | Parallel (worktree) |
| 01 ↔ 05 | none | Parallel (worktree) |
| 02 ↔ 03 | none | Parallel (worktree) |
| 02 ↔ 05 | none | Parallel (worktree) |
| 03 ↔ 05 | none | Parallel (worktree) |

Wave 1 has zero write-file overlap → all four tasks (01, 02, 03, 05) can run in parallel
worktrees.

### Wave 2 — Overlap Matrix

| Pair | Shared Write Files | Strategy |
|---|---|---|
| 04 ↔ 06 | none | Parallel (worktree) |

Wave 2 has zero write-file overlap → tasks 04 and 06 can run in parallel worktrees once
Wave 1 has merged.

### Read-only overlap (informational, no conflict risk)
- Task 04 reads `target_selector_state.rs` (written by 02) and `handler/mod.rs`
  (written by 03) — Wave-2 ordering ensures these reads see the merged Wave-1 state.
- Task 06 reads `target_selector_state.rs` (written by 02) and `tab_bar.rs`
  (written by 05).

---

## Dispatch Order Summary

```
Wave 1 (parallel worktrees):  01, 02, 03, 05
        ↓
Wave 2 (parallel worktrees):  04, 06
```

All tasks: `Agent: implementor`. No core docs (`docs/ARCHITECTURE.md`,
`CODE_STANDARDS.md`, `DEVELOPMENT.md`) require updates — this is a localized bug fix
with no module/layer changes.
