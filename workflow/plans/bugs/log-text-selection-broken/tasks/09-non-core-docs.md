# Task 09 — Update non-core docs

**Agent:** implementor
**Wave:** 4
**Depends on:** Tasks 01–08 (all functional changes landed first)
**Files written:**
- `docs/MOUSE.md`
- `docs/KEYBINDINGS.md`
- `docs/CONFIGURATION.md`
- `workflow/plans/features/mouse-support/PLAN.md` *(cross-reference note only)*

---

## Goal

Bring user-facing documentation in line with the bug fix. Specifically:

- **`docs/MOUSE.md`:**
  - Replace the "Out of scope — drag-to-select" framing with the truth: Shift+drag now works natively on every supported terminal because fdemon no longer sets `?1003` (any-motion tracking).
  - Add a new section "Selecting and copying log text" describing: (a) Shift+drag for arbitrary substrings, (b) right-click on a log row for full-line copy with toast confirmation, (c) `Alt+m` to fully suspend capture if Shift+drag still misbehaves.
  - Update the Future Work list: remove "Drag-to-select for log lines" if right-click + Shift+drag is judged to cover the use case (recommended); keep "Drag-to-resize panel splits", "Hover tooltips", "Project-selector mouse support", "Right-click context menus" (now contextual: the doc should note right-click currently has a fixed action, no menu).
  - Add a "Runtime toggle" section explaining `Alt+m`, the status-bar `[mouse]` / `[mouse-off]` badge, and that the toggle is in-process only (not persisted to `config.toml`).
  - Adjust the "Disabling Mouse Capture" section to point at the runtime toggle for ad-hoc suspends and reserve the `[ui] enable_mouse = false` config for permanent opt-out.

- **`docs/KEYBINDINGS.md`:**
  - Add a row for `Alt+m` — "Toggle mouse capture (runtime, in-process)" — in the global / always-on bindings cluster.
  - Cross-link to MOUSE.md as it already does for mouse interactions.

- **`docs/CONFIGURATION.md`:**
  - Clarify the `[ui] enable_mouse` description: this is the *initial* state at startup; the in-app `Alt+m` toggle changes runtime state without persisting it. The setting still controls the default at every restart.
  - Mention the `[mouse]` / `[mouse-off]` status-bar badge briefly so users searching the config doc can find it.

- **`workflow/plans/features/mouse-support/PLAN.md`:**
  - Append a single-paragraph "Bugfix follow-up" note at the bottom of the file with a link to `workflow/plans/bugs/log-text-selection-broken/BUG.md`. Explain in one sentence that the v1 "Shift+drag passthrough suffices" assumption was incorrect under the `?1003` mode crossterm enables by default, and that the bugfix dropped `?1003` and added `Alt+m`.

## Constraints

- This task touches **only the four files listed above**. Do not modify `docs/ARCHITECTURE.md` (that is Task 10, routed to `doc_maintainer`).
- Do not touch `docs/CODE_STANDARDS.md` — the fix did not introduce a new pattern; the existing Region Registry / `Cell<usize>` exception notes already cover the involved patterns.
- Do not touch `docs/IDEAS.md`, `docs/TESTING.md`, `docs/REVIEW_FOCUS.md`, or `docs/EXTENSION_API.md` — none of them describe behavior changed by this fix.

## Acceptance Criteria

- [ ] `docs/MOUSE.md` accurately describes the three new affordances (Shift+drag, right-click-copy, `Alt+m` toggle).
- [ ] `docs/KEYBINDINGS.md` lists `Alt+m`.
- [ ] `docs/CONFIGURATION.md` reflects the "initial state vs runtime" semantics of `[ui] enable_mouse`.
- [ ] `workflow/plans/features/mouse-support/PLAN.md` has a Bugfix follow-up note with a working relative link to `BUG.md`.
- [ ] No code changes in this task.

## Notes for Reviewer

- The implementor should regenerate any markdown TOC sections in the affected files if the project convention auto-generates them; otherwise manually update inline anchors.
- Keep the prose terse. Existing `MOUSE.md` lines tend to be ≤ 90 chars; match the local convention.
