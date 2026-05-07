## Task: ARCHITECTURE.md Mouse Subsystem — Finalize

**Agent:** doc_maintainer

**Objective**: Verify the existing Mouse Region Registry section in `docs/ARCHITECTURE.md` reflects Phase 5 + 5.5 final state, and add a short "Modal Precedence and Sub-Modal Gates" subsection documenting the renderer-level base-region suppression introduced by Phase 5.5 Task 01.

**Depends on**: None

**Estimated Time**: 0.5h

### Scope

**Files Modified (Write):**
- `docs/ARCHITECTURE.md`: Add a "Modal Precedence and Sub-Modal Gates" subsection inside the existing Mouse Region Registry section (currently around lines 787–825). Verify the surrounding paragraphs are still accurate against current source; correct any drift.

**Files Read (Dependencies):**
- `~/.claude/skills/doc-standards/schemas.md` — content boundary rules.
- `crates/fdemon-app/src/mouse_regions.rs` — registry types, builder API, hit-test semantics.
- `crates/fdemon-app/src/handler/mouse/mod.rs` — dispatcher gate ordering (tag-filter route first, then mode dispatch).
- `crates/fdemon-app/src/handler/mouse/settings.rs` — sub-modal gate (returns `None` when `dart_defines_modal` or `extra_args_modal` is open).
- `crates/fdemon-tui/src/render/mod.rs` — renderer-level base-region suppression (does not pass `MouseCtx` to base-UI widgets when a modal is up).

### Change Context

Phase 5.5 Task 01 changed how modal precedence is enforced:

- **Before Phase 5.5:** Per-mode dispatchers in `handler/mouse/{confirm_dialog,settings,new_session,tag_filter}.rs` were expected to filter hits by `z_index >= 1`. The implementation did not, leaking base-UI clicks (e.g. clicking header `[r]` while `ConfirmDialog` was up fired a hot reload).
- **After Phase 5.5:** `render::view()` skips threading `Some(&mut MouseCtx)` to `MainHeader` / `LogView` / tabs when in a modal `UiMode` or when `tag_filter_visible`. Base-UI z=0 regions are simply not registered while a modal is up; per-mode dispatchers using `regions.hit_test(x, y, button)` are therefore correct without `z_index` filtering.
- **Sub-modal gate:** `settings::handle_press` short-circuits to `None` when the dart-defines or extra-args sub-modal is open inside the Settings panel.

The existing Mouse Region Registry section (~lines 787–825) was written for the per-dispatcher-filter model. Verify the prose still matches; add a short subsection to document the renderer-level approach explicitly.

### Acceptance Criteria

1. A new subsection named "Modal Precedence and Sub-Modal Gates" exists inside the "Mouse Region Registry" section. It documents:
   - When a modal `UiMode` is active (`Startup`, `NewSessionDialog`, `ConfirmDialog`, `Settings`, `LinkHighlight`, `FlutterVersion`) or when `tag_filter_visible`, the renderer does not pass a `MouseCtx` to base-UI widgets, so no base-UI z=0 regions are registered.
   - Per-mode dispatchers therefore see only modal-owned regions when they call `regions.hit_test(...)`; explicit `z_index` filtering is unnecessary at the dispatcher level.
   - Sub-modals (`dart_defines_modal`, `extra_args_modal` inside Settings) gate `settings::handle_press`, which returns `None` while either is open.
2. Existing prose in the Mouse Region Registry section accurately describes the current source — drift, if found, is corrected in this task.
3. No content is moved out of the Mouse Region Registry section into other sections; no architecture content is moved into CODE_STANDARDS.md (boundary respected).
4. The TEA exception cross-reference to `docs/CODE_STANDARDS.md` Principle 3 still resolves; the citation to `docs/REVIEW_FOCUS.md` "Approved TEA Exception → Current usage" still resolves (verify by `grep`).
5. No new sections, no new diagrams. Edits are targeted to the Mouse Region Registry section only.

### Testing

```bash
# Verify cross-references still resolve:
grep -n "Cell<usize>" docs/CODE_STANDARDS.md
grep -n "Approved TEA Exception" docs/REVIEW_FOCUS.md

# Verify the new subsection exists:
grep -n "Modal Precedence" docs/ARCHITECTURE.md
```

### Notes

- This is **architecture content only**. Coding-standard prose (the "you should pattern-match this exception …") belongs in Task 03 (CODE_STANDARDS.md), not here.
- Do not duplicate the per-mode dispatcher code into the doc. Reference the file paths.
- If you find that `crates/fdemon-tui/src/render/mod.rs` does not in fact suppress base-UI MouseCtx as Phase 5.5 Task 01 promised, halt the task and surface a defect — do not paper over it in docs.
- Follow content boundaries strictly — see `~/.claude/skills/doc-standards/schemas.md`.
- Make targeted edits, do not rewrite the surrounding sections.
