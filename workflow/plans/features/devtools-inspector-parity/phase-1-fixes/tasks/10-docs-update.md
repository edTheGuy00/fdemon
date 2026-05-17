## Task: Update Documentation for Phase 1.5 Changes

**Agent:** doc_maintainer

**Objective**: Bring `docs/ARCHITECTURE.md` and `docs/KEYBINDINGS.md` in line with the Phase 1.5 changes. Reflect the `InspectorState::selected_row()` + `reset_details_and_groups()` helpers, the `UpdateAction::PersistSettings` infrastructure, and the renamed `DevToolsEscape` message variant. Fix the KEYBINDINGS.md Up/Down doc drift in the Details mode table.

**Depends on**: 01–09 (consumes the final state of all preceding implementation tasks)

**Estimated Time**: 0.5–1 hour

### Scope

**Files Modified (Write):**
- `docs/ARCHITECTURE.md` — minor updates to the DevTools Subsystem section: mention the new `selected_row()` helper, the `reset_details_and_groups()` lifecycle helper, and the `UpdateAction::PersistSettings` flow.
- `docs/KEYBINDINGS.md` — fix the Up/Down entry in the Details-mode table (clarifying the handler-level freeze, not a keys-level unbind).

**Files Read (Dependencies):**
- `~/.claude/skills/doc-standards/schemas.md` — content boundary rules.
- All Phase 1.5 implementation task files in `workflow/plans/features/devtools-inspector-parity/phase-1-fixes/tasks/` for change context.
- The merged source of truth in `crates/fdemon-app/src/state.rs` (final helper signatures) and `crates/fdemon-app/src/handler/mod.rs` (final UpdateAction enum).

### Review Items Resolved

- **m6 part 2** — KEYBINDINGS.md Up/Down doc drift in Details-mode table
- **n1** — Task 11 (Phase 1) completion summary still says "Not Started" — update for traceability
- Documentation completeness for the Phase 1.5 deliverables

### Change Context

1. **ARCHITECTURE.md — DevTools Subsystem → Panel State Model**: The Inspector state now exposes `selected_row()` returning an `InspectorRow<'_>` with full `RowGroup` info. The `reset_details_and_groups()` helper is the canonical reset point for state that does not survive a tree refresh or hot restart. Add 1–2 sentences in the existing `InspectorState` paragraph; do not expand into algorithmic detail.

2. **ARCHITECTURE.md — Engine Architecture → UpdateAction**: A new variant `UpdateAction::PersistSettings { settings, project_path }` joins the async-side-effect family alongside `AutoSaveConfig`. Add one row to the variant list (if the doc maintains one) or one sentence in the surrounding prose.

3. **KEYBINDINGS.md — Widget Inspector Panel → Details mode table**: The Up/Down row currently reads `**No-op** — selection frozen while details is open`. This is true at the *handler* level, but technically the keys.rs layer still emits `InspectorNavigate(Up/Down)` — the handler returns early. The user-facing behaviour is unchanged; the docs entry is accurate from a user perspective. **No change required to this row** unless the doc maintainer prefers to add a footnote explaining the handler-side freeze for power users. Leave it as is.

   The real KEYBINDINGS.md change for m6 is to ensure consistency with the keys.rs comment (which task 09 corrects). If the implementor of task 09 changed the keys.rs comment to clarify "emitted by keys, swallowed by handler," the KEYBINDINGS.md text should match that nuance — though for user-facing docs, "No-op" is already the right summary. The doc_maintainer should read task 09's completion summary and decide whether a footnote is warranted.

4. **Task 11 (Phase 1) completion summary fix**: The doc maintainer should also update `workflow/plans/features/devtools-inspector-parity/phase-1/tasks/11-docs-update.md` to mark its status as `Done ✅` retroactively, with a note in the completion summary explaining the docs were updated during Phase 1 task 11 even though the status field was never flipped. (This is n1 from the review.)

### Acceptance Criteria

1. `docs/ARCHITECTURE.md` mentions `selected_row()` and `reset_details_and_groups()` in the InspectorState description.
2. `docs/ARCHITECTURE.md` mentions `UpdateAction::PersistSettings` alongside `AutoSaveConfig`.
3. `docs/KEYBINDINGS.md` Details-mode table is consistent with the keys.rs comment fix from task 09 (or explicitly leaves the row unchanged if "No-op" remains the most user-friendly summary).
4. The Phase 1 task 11 completion summary is updated to `Done ✅` retroactively, with a brief note acknowledging the gap.
5. No content boundary violations:
   - Architectural content stays in ARCHITECTURE.md (no implementation details that belong in code comments).
   - Key bindings stay in KEYBINDINGS.md (no architecture leaking in).
6. Cross-references valid (the `selected_row()` reference should point at the right file path: `crates/fdemon-app/src/state.rs`).
7. `cargo doc --workspace --no-deps` still produces a clean doc tree.

### Notes

- Follow content boundaries strictly — see `~/.claude/skills/doc-standards/schemas.md`.
- Make targeted edits, do NOT rewrite either document end-to-end.
- The `Message::DevToolsEscape` rename does not need a docs callout — it's an internal symbol name, not a user-facing concept.
- The `UpdateAction::PersistSettings` flow may be too implementation-detail for ARCHITECTURE.md. Use judgment: if the doc lists every UpdateAction variant, add a row; if it summarises the family, a single sentence in the surrounding paragraph is enough.
- The `_visible` parameter removal (task 09 / M2) is an internal cleanup with no doc impact.

---

## Completion Summary

**Status:** Not Started
**Branch:** —

### Files Modified

| File | Changes |
|------|---------|

### Notable Decisions/Tradeoffs

### Testing Performed

### Risks/Limitations
