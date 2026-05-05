# Task 10: PLAN Drift Documentation + `docs/MOUSE.md` Phase 4 Updates

## Goal

Two documentation updates:
1. Document the two PLAN.md ↔ implementation deviations from Phase 4 directly in `workflow/plans/features/mouse-support/PLAN.md` so future readers don't get confused (Minor #14).
2. Update or extend `docs/MOUSE.md` to cover Phase 4 click semantics: the per-row registration model, double-click without spatial constraint, single-click visual inertness (deliberate choice), and the network filter-input click suppression with the sub-tab carve-out (after Task 08 lands) (Minor #12).

## Background

**Drift A — log-view click registration model:**
- PLAN.md sketched: `MouseAction::EmitWithCoord(|x, y| Message::FocusLogEntryAtRow { row: y - origin_y })` — one coordinate-aware region covering the whole log area, with the handler computing `entry_id` from the clicked row via a `LogViewState`-maintained row→entry map.
- As shipped: one `MouseAction::Emit(Message::ClickLogRow { entry_id, frame_index })` per visible row. Each row's `entry_id` is captured at render time so the handler doesn't consult any auxiliary map.
- Why it changed: wrap-mode pixel-row → entry mapping was complex; per-row registration is cleaner and works correctly across wrap modes. Phase 4 task 06's notes section documents this rationale, but PLAN.md was never updated.

**Drift B — double-click position constraint:**
- PLAN.md sketched: "double-click = two consecutive clicks within 400ms **AND within 1 cell of previous click**."
- As shipped: double-click = two consecutive clicks within 400ms **on the same `entry_id`** (no spatial constraint). Position is no longer tracked in `LogClickStamp`.
- Why it changed: clicking a row, scrolling so it moves on screen, then clicking the same row again within 400ms still counts as a double-click. PLAN's "within 1 cell" would have rejected this. Entry-id matching is more robust to scrolling.

**`docs/MOUSE.md` updates:**
- This file was created in Phase 2.5 task 03 to cover scroll behavior, modifier handling, Win11 caveats, etc. Phase 4 added click handling for log/inspector/performance/network and the file has not been updated. Add a "Phase 4: Click Surfaces" section.

## Files

**Modify:**
- `workflow/plans/features/mouse-support/PLAN.md`
- `docs/MOUSE.md`

**Read (reference):**
- `workflow/reviews/features/mouse-support-phase-4/REVIEW.md` — review summary
- `workflow/plans/features/mouse-support/phase-4-log-view-devtools-clicks/TASKS.md` — Phase 4 design notes section (especially the rationale notes at the bottom)

## Plan

1. **Update PLAN.md.** Find the relevant sections describing the original click design (likely under "Interaction Map" or "Edge Cases" or task-level sketches). Add a "Drift Notes (post Phase 4 implementation)" subsection summarizing both deviations. Sample text:

   ```markdown
   ### Drift Notes (Phase 4 implementation)

   The Phase 4 implementation deviated from this PLAN's original sketch in two
   places. Both deviations were made during implementation for sound technical
   reasons; they are recorded here so future readers don't get confused.

   **Drift A — log-view click registration uses per-row `Emit` instead of
   `EmitWithCoord`:** the original sketch called for one coordinate-aware
   `MouseAction::EmitWithCoord(|x, y| Message::FocusLogEntryAtRow { ... })`
   covering the whole log area. The implementation registers one
   `MouseAction::Emit(Message::ClickLogRow { entry_id, frame_index })` per
   visible row. This is cleaner across wrap modes (where pixel-row → entry
   mapping is non-trivial) and avoids the need for a `LogViewState`-maintained
   row→entry auxiliary map. The cost is one `Box<Message>` allocation per
   visible row per frame (~200 entries × 20 fps = 4k allocs/sec at peak,
   acceptable). See Phase 4 task 06 notes for the full rationale.

   **Drift B — double-click detection uses entry_id matching, not position
   matching:** the original sketch called for "two consecutive clicks within
   400ms AND within 1 cell of previous click." The implementation drops the
   position constraint and matches on `entry_id`. This is more robust to
   scrolling between clicks (clicking row 5, scrolling so the row moves to
   row 3, clicking again still counts as a double-click on the same entry).
   The cost is that two clicks on different rows within 400ms are correctly
   treated as separate single clicks (handled by the entry_id mismatch).
   ```

   Place this after the relevant interaction-map section, or in an Appendix if PLAN.md has one.

2. **Update `docs/MOUSE.md`.** Add a Phase 4 section describing click semantics for users. Sample:

   ```markdown
   ## Phase 4: Click Behavior

   ### Log View

   - **Single click on a log row**: no visible action. The row is registered
     for double-click detection but not scrolled or highlighted.
   - **Double click on the same row within 400ms**: toggles the entry's stack
     trace expansion (if the entry has a stack trace).
   - **Double click on a different row within 400ms**: treated as two separate
     single clicks; no toggle.
   - **Double click on the same row after a session switch**: treated as a
     fresh single click (the previous click stamp is cleared on session change).

   ### DevTools Sub-tab Bar

   - Click `[i] Inspector` / `[p] Performance` / `[n] Network` to switch
     active panel. Equivalent to pressing `i` / `p` / `n` keys.

   ### Inspector Tree

   - Click a tree row to select it (equivalent to `↑/↓` keyboard navigation).
   - Click the `▶`/`▼` glyph at the row's left edge to expand/collapse the
     node (equivalent to `→/←` keyboard expand/collapse).
   - Both clicks dispatch a layout fetch under the same debounce / cache rules
     as keyboard navigation.

   ### Performance Frame Chart

   - Click a frame's bar in the chart to select it. Equivalent to `Tab`/`Shift+Tab`
     in the frames view.
   - Clicking outside any frame bar (e.g., on the budget-line area) is a no-op.

   ### Network Table

   - Click a row in the request table to select it; details appear in the side
     panel (or below in narrow mode).
   - Click `[g] [h] [q] [s] [t]` in the detail-tab bar to switch detail tabs.

   ### Network Filter Input Mode

   - When typing in the network filter input, clicks in the table area are
     suppressed (the user is typing).
   - **Exception:** clicks on the DevTools sub-tab bar (`[i]/[p]/[n]`) escape
     the filter input — they switch panels AND exit filter input mode. (This
     prevents a mouse-only user from being trapped in the filter.)
   ```

3. **Review `docs/MOUSE.md` for currency.** If older sections (Phase 2 scroll, Phase 3 region registry) reference behavior that has since changed, fix in passing — but do not expand scope significantly.

## Acceptance Criteria

- [ ] PLAN.md contains a "Drift Notes (Phase 4 implementation)" section covering both deviations.
- [ ] `docs/MOUSE.md` contains a "Phase 4: Click Behavior" section covering log view, sub-tab bar, inspector tree, performance frame chart, network table, and network filter input mode (including the sub-tab carve-out).
- [ ] No source code is modified.
- [ ] `cargo fmt --all -- --check`, `cargo check --workspace --all-targets`, `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings` still pass (this is a docs-only change so should be a no-op for these checks).

## Notes

- **No code changes.** This is a docs-only task. The implementor's commit should only touch the two markdown files.
- The "Network Filter Input Mode" section in `docs/MOUSE.md` describes the post-Task-08 behavior (sub-tab carve-out). If Task 08 has not yet merged when this task runs, that's fine — the docs describe the intended end-of-phase behavior. The orchestrator will merge tasks in number order.
- If `docs/MOUSE.md` does not yet exist, create it. Phase 2.5 task 03 was supposed to create it; verify before this task starts. If missing, scaffold a minimal file (similar in tone to other `docs/*.md` files) and add the Phase 4 section.
