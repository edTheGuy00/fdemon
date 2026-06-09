# Phase 2 Followup — Platforms Submenu Review Findings — Task Index

## Overview

Address the 13 findings from the Phase 2 review
(`workflow/reviews/features/toolchain-platforms-submenu-phase-2/ACTION_ITEMS.md`). The review verdict was
**NEEDS WORK** — no crashes or layer violations, but a cluster of cross-confirmed correctness/UX defects
(M1–M3 must-fix), maintainability cleanups (S1–S4 should-fix), and minor polish (N1–N6).

Research collapsed the 13 findings into **3 implementor tasks + 1 doc task**:

- **M1 + M2 + M3 + S3** are all in `handler/install_wizard/navigation.rs` (+ message/keys/dispatch wiring)
  and are intertwined around the expand/collapse handlers. They land as one **navigation-correctness**
  task: extract a single shared `set_platforms_expanded` helper (borrow-split, no clone → S3) that
  rebuilds, re-anchors the cursor to the Platforms parent when collapsing from a leaf (M2), clamps, and
  resets `selected_command_index` (M3); then add directional `Expand`/`Collapse` messages + handlers (M1).
- **S2 + S4** are both in `install_wizard/state.rs` `build_steps`/`rollup_step_statuses` → one **state
  rollup-cleanup** task.
- **S1 + N1 + N2 + N4 + N5 + N6** are all in `widgets/install_wizard/{step_list,mod,step_detail}.rs` → one
  **TUI-polish** task.
- **Task 04** refreshes `docs/ARCHITECTURE.md` for the new directional messages/handlers (doc_maintainer).

**Decisions (confirmed with the user):**
- **M1** → **directional split**: add `InstallWizardExpand` + `InstallWizardCollapse` (set, not flip);
  `l`/`Right`→expand, `h`/`Left`→collapse; `Enter` stays toggle (on parent) / run (else).
- **N3** (selected leaf can scroll off-screen on short terminals when expanded) → **deferred to Phase 3**.
  It is pre-existing and needs a real scroll-offset render-hint (like the existing
  `last_known_visible_height` pattern). Tracked in the Deferred section below, not in this followup.

**Total Tasks:** 4
**Estimated Hours:** 5–7 hours

## Task Dependency Graph

```
        ┌─────────────────────────────┐  ┌─────────────────────────────┐  ┌─────────────────────────────┐
        │ 01-navigation-correctness   │  │ 02-state-rollup-cleanup     │  │ 03-tui-polish               │   Wave 1
        │ (fdemon-app: msg/keys/nav)  │  │ (fdemon-app: state.rs)      │  │ (fdemon-tui: 3 widgets)     │   (parallel
        │ M1 + M2 + M3 + S3           │  │ S2 + S4                     │  │ S1 + N1 + N2 + N4 + N5 + N6 │    worktree)
        └──────────────┬──────────────┘  └─────────────────────────────┘  └─────────────────────────────┘
                       │  (new Expand/Collapse messages + handlers)
                       ▼                                                                                     Wave 2
        ┌─────────────────────────────┐
        │ 04-update-architecture-docs │
        │ (doc_maintainer)            │
        └─────────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Est. Hours | Modules |
|---|------|--------|------------|------------|---------|
| 1 | [01-navigation-correctness](tasks/01-navigation-correctness.md) | Not Started | - | 2–3h | `message.rs`, `handler/update.rs`, `handler/keys.rs`, `handler/install_wizard/navigation.rs`, `docs/KEYBINDINGS.md` |
| 2 | [02-state-rollup-cleanup](tasks/02-state-rollup-cleanup.md) | Not Started | - | 1h | `install_wizard/state.rs` |
| 3 | [03-tui-polish](tasks/03-tui-polish.md) | Not Started | - | 1.5–2h | `widgets/install_wizard/{step_list,mod,step_detail}.rs` |
| 4 | [04-update-architecture-docs](tasks/04-update-architecture-docs.md) | Not Started | 1 | 0.5h | `docs/ARCHITECTURE.md` |

## File Overlap Analysis

| Task | Files Modified (Write) | Files Read (Dependencies) |
|------|----------------------|--------------------------|
| 01 | `crates/fdemon-app/src/message.rs`, `crates/fdemon-app/src/handler/update.rs`, `crates/fdemon-app/src/handler/keys.rs`, `crates/fdemon-app/src/handler/install_wizard/navigation.rs`, `docs/KEYBINDINGS.md` | `crates/fdemon-app/src/install_wizard/{state,types}.rs` (build_steps, WizardStepKind, is_platform_leaf — read only) |
| 02 | `crates/fdemon-app/src/install_wizard/state.rs` | `crates/fdemon-daemon/src/toolchain/types.rs` (HostPlatform, StepStatus — read only) |
| 03 | `crates/fdemon-tui/src/widgets/install_wizard/step_list.rs`, `crates/fdemon-tui/src/widgets/install_wizard/mod.rs`, `crates/fdemon-tui/src/widgets/install_wizard/step_detail.rs` | `crates/fdemon-app/src/install_wizard/{state,types}.rs` (read only) |
| 04 | `docs/ARCHITECTURE.md` | task 01 files, `~/.claude/skills/doc-standards/schemas.md` |

### Overlap Matrix

| Task Pair | Shared Write Files | Isolation Strategy |
|-----------|-------------------|-------------------|
| 01 + 02 | **none** (navigation.rs vs state.rs; both fdemon-app, different files) | **Parallel (worktree)** |
| 01 + 03 | **none** (fdemon-app vs fdemon-tui) | **Parallel (worktree)** |
| 02 + 03 | **none** (state.rs vs widgets) | **Parallel (worktree)** |
| 04 vs all | none | Sequential (after 01) |

> **Critical isolation constraint for Task 01:** the shared `set_platforms_expanded` helper MUST live in
> `navigation.rs` as a private free function — **not** as a method on `InstallWizardState` in `state.rs` —
> so Task 01 and Task 02 stay on disjoint files and parallelize. Task 01 must not write `state.rs`.
> Task 02's changes to `rollup_step_statuses` (private) and the `build_steps` parent-status computation are
> behavior-preserving (identical observable parent status), so Task 01's tests that call `build_steps` are
> unaffected.

## Success Criteria

The followup is complete when:

- [ ] **M1**: `l`/`Right` expands (set true), `h`/`Left` collapses (set false), `Enter` on the parent
      toggles; doc-comments and `docs/KEYBINDINGS.md` match the behavior.
- [ ] **M2**: Esc-collapse (and `h`/`Left` collapse) from any platform leaf re-anchors the cursor to the
      Platforms parent on every host; a test asserts the landing `kind`, not just `selected_index < len`.
- [ ] **M3**: `selected_command_index` is reset to 0 on every collapse path; both collapse entry points go
      through one shared helper that cannot diverge.
- [ ] **S1**: the contradictory caret/fill comments in `step_list.rs` are corrected.
- [ ] **S2**: the Platforms parent status is rolled up over the actually-emitted leaf steps (single
      host-gating source); no parallel `vec![…]` status list.
- [ ] **S3**: no full `ToolchainReport` clone on the toggle/collapse path (borrow-split in the helper).
- [ ] **S4**: `rollup_step_statuses` is a single-pass scan; a `[Ok]`/`[Ok, Pending] → Ok` unit test exists.
- [ ] **N1–N6**: named height constant, softened placeholder copy, constant-derived test coordinates,
      `make_steps()` note, `step_caption` exhaustiveness note.
- [ ] `docs/ARCHITECTURE.md` reflects the directional messages/handlers (Task 04).
- [ ] `cargo fmt --all -- --check`, `cargo test --workspace --lib`, and
      `cargo clippy --workspace --all-targets -- -D warnings` are all clean.

## Deferred (not in this followup)

- **N3 — step-list scroll-to-selection on short terminals (expanded).** Pre-existing; the expanded list
  (up to ~9 rows on macOS) makes a selected leaf reachable past the clamped pane height with no scroll
  offset. Needs a scroll-offset field + render-time clamp keyed to `selected_index`, mirroring the
  approved `Cell<usize>` render-hint pattern (`InstallWizardState::last_known_visible_height`). Fold into
  the Phase 3 step-list work or raise as a standalone task.

## Notes

- Snapshot line numbers in the task files are from a research pass and **will drift** — locate by
  symbol/test-name/variant, not absolute line.
- No new dependencies; no new crates; layer boundaries unchanged.
- `docs/KEYBINDINGS.md` is implementor-editable (Task 01 owns it); only `docs/ARCHITECTURE.md` requires
  `doc_maintainer` (Task 04).
