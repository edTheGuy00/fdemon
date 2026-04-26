# Tasks: PR #37 Copilot Review Fixes

Plan: [BUG.md](BUG.md)
Source review: [PR #37 Copilot review](https://github.com/edTheGuy00/fdemon/pull/37#pullrequestreview-4175487078)

**PR branch:** `fix/remove-cache-device-ttl` (commits land directly on this branch
so the open PR can be merged once all fixes are pushed).

---

## Wave 1 — All findings (single wave, all parallel)

| ID | Status | Task | Depends on | Files Modified (Write) | Files Read |
|---|---|---|---|---|---|
| 01 | [x] Done | [Stuck-loading + connected cache-miss foreground (F1+F2)](tasks/01-stuck-loading-and-cache-miss.md) | — | `crates/fdemon-app/src/handler/new_session/navigation.rs` | `target_selector_state.rs`, `handler/mod.rs`, `actions/mod.rs`, `state.rs` |
| 02 | [x] Done | [`set_error()` doc accuracy (F3)](tasks/02-set-error-doc-accuracy.md) | — | `crates/fdemon-app/src/new_session_dialog/target_selector_state.rs` | `handler/update.rs`, `handler/new_session/launch_context.rs`, `handler/new_session/target_selector.rs` (callers, read-only) |
| 03 | [x] Done | [Compact-mode glyph for inactive refreshing tabs (F4)](tasks/03-compact-mode-glyph-inactive-tab.md) | — | `crates/fdemon-tui/src/widgets/new_session_dialog/target_selector.rs` | `tab_bar.rs`, `theme/icons.rs` |
| 04 | [x] Done | [Thread `IconSet` from `NewSessionDialog` to `TargetSelector` (F5)](tasks/04-thread-iconset-to-target-selector.md) | — | `crates/fdemon-tui/src/widgets/new_session_dialog/mod.rs` | `target_selector.rs`, `theme/icons.rs` |

---

## File Overlap Analysis

### Wave 1 — Overlap Matrix

| Pair | Shared Write Files | Strategy |
|---|---|---|
| 01 ↔ 02 | none | Parallel (worktree) |
| 01 ↔ 03 | none | Parallel (worktree) |
| 01 ↔ 04 | none | Parallel (worktree) |
| 02 ↔ 03 | none | Parallel (worktree) |
| 02 ↔ 04 | none | Parallel (worktree) |
| 03 ↔ 04 | none | Parallel (worktree) |

Wave 1 has zero write-file overlap → all four tasks can run in parallel worktrees.

### Read-only overlap (informational, no conflict risk)

- Task 01 reads `handler/mod.rs` and `actions/mod.rs` (to confirm the existing
  `DiscoverDevicesAndBootable` and `RefreshDevicesAndBootableBackground` variants).
- Task 01 reads `target_selector_state.rs` (to understand `set_error()` semantics
  for the F1 fix). Task 02 writes that file — but only the doc comment for
  `set_error()`, not its behavior. Task 01 must use `set_error()` according to its
  current behavior (clears `loading` and `refreshing` only; bootable flags untouched),
  which is exactly what task 02's doc rewrite documents.
- Task 03 reads `tab_bar.rs` (to mirror its per-tab refreshing semantics).
- Task 04 reads `target_selector.rs` (to understand the `.icons()` builder). Task 03
  writes that file — but only `render_tabs_compact`, not the public API. Task 04
  uses the existing `.icons()` builder; no signature change.

### Cross-task informational note

- The PR branch (`fix/remove-cache-device-ttl`) already contains the
  `DiscoverDevicesAndBootable` and `RefreshDevicesAndBootableBackground`
  `UpdateAction` variants (added in device-cache-followup task 03 and
  device-cache-no-ttl task 03). Task 01 wires these into the new branching shape;
  it does **not** need to add new variants.

---

## Dispatch Order Summary

```
Wave 1 (parallel worktrees):  01, 02, 03, 04
```

All tasks: `Agent: implementor`. No core docs (`docs/ARCHITECTURE.md`,
`docs/CODE_STANDARDS.md`, `docs/DEVELOPMENT.md`) require updates — this is a localized
review-fix bundle on top of an open PR.
