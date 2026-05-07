## Task: MOUSE.md Phase 5 Coverage + Compact-Dialog Caveat

**Objective**: Bring `docs/MOUSE.md` up to date with Phase 5 click semantics (NewSessionDialog, ConfirmDialog, TagFilter overlay, LinkHighlight badges, Settings panel), document modal precedence and the sub-modal gates, and add a "Compact NewSessionDialog" caveat under Platform Caveats.

**Depends on**: None

**Estimated Time**: 1.25h

### Scope

**Files Modified (Write):**
- `docs/MOUSE.md`: Add Phase 5 click sections; add "Modal Precedence and Sub-Modal Gates" subsection; add "Compact NewSessionDialog" caveat under Platform Caveats; trim "Future Work" entries shipped in Phase 5.

**Files Read (Dependencies):**
- `workflow/plans/features/mouse-support/phase-5-dialogs-overlays/TASKS.md` — interaction map and z-index policy for Phase 5 surfaces.
- `workflow/plans/features/mouse-support/phase-5.5-followup/TASKS.md` — modal precedence renderer-level approach (Task 01); sub-modal gate (settings dart_defines / extra_args) from `handle_press`.
- `crates/fdemon-app/src/handler/mouse/{confirm_dialog,settings,new_session,link_highlight,tag_filter}.rs` — interaction reference for what each press handler does (only as a cross-check; do not embed code).

### Details

The current `docs/MOUSE.md` covers:

- Wheel scroll (per-mode table + modifier rules) — keep as-is.
- Phase 3: Header brackets, session tabs, device pill — keep as-is.
- Phase 4: Log view single/double-click, DevTools sub-tabs, Inspector tree, Performance frame chart, Network table — keep as-is.
- Future Work list at the bottom.

Add new sections after Phase 4 and before Future Work:

#### Section: "Phase 5: Dialogs and Overlays"

Cover, with the same heading depth and prose style as the Phase 3 / Phase 4 sections:

- **NewSessionDialog**
  - Click `[1] Connected` / `[2] Bootable` tab headers → switch tab.
  - Click a device row → select device (single click selects + activates; subsequent click on `Launch` button starts the session).
  - Click a launch-context field (`Configuration` / `Mode` / `Flavor` / `Entry Point` / `Dart Defines`) → focus + activate (mirrors keyboard `Enter` on the field).
  - Click `Launch` button → launch.
  - Inside the fuzzy modal, click any visible result row → select + confirm.
  - The dart-defines modal inside `NewSessionDialog` is keyboard-only (no clickable rows in v1).

- **ConfirmDialog**
  - Click `[y] Yes` or `[n] No` button → emit the action stored at the corresponding index. The clickable rect covers the bracket + label only; clicks elsewhere on the modal are no-ops.

- **TagFilter overlay** (open with `T`)
  - Click a tag row → set selected index AND toggle the tag's visibility in a single click.
  - Click `[a] All` / `[n] None` action labels → show all / hide all.

- **LinkHighlight badges** (visible after `Shift+L`)
  - Click any badge `[<char>]` → emit `Message::SelectLink(<char>)`. The clickable rect is exactly the three-cell badge span.

- **Settings panel**
  - Click a tab header (`1. PROJECT` / `2. USER` / `3. LAUNCH` / `4. VSCODE`) → switch tab.
  - Click a setting row → select (single click sets `selected_index`; double-click within 400 ms enters edit mode).
  - The Settings dart-defines / extra-args sub-modals are keyboard-only.

#### Section: "Modal Precedence and Sub-Modal Gates"

Document the rule (introduced in Phase 5 task 11 success criteria, hardened in Phase 5.5 task 01):

> When a modal is open (NewSessionDialog, ConfirmDialog, TagFilter, FlutterVersion, Settings, LinkHighlight), the renderer does not register base-UI click regions for the underlying header/log-view/tabs. Clicks that land outside the modal's own rects are silently dropped — they do **not** activate the underlying base-UI region. This guarantees, for example, that clicking on header `[r]` while a `ConfirmDialog` is shown does not fire a hot reload.

> Sub-modals (Settings dart-defines, Settings extra-args) gate the Settings dispatcher: when a sub-modal is open, `settings::handle_press` returns `None` for any click, preventing leaks to the underlying Settings rows.

#### Section: "Compact NewSessionDialog"

Add under "Platform Caveats" (or at the end of "Coordinate-Free Routing"):

> When the terminal is between 40–69 columns wide and 20–21 rows tall, the New Session Dialog falls back to a compact-vertical layout that does not register device-row click regions. In this size range, fdemon shows a small hint line (e.g. `"Resize for mouse"`); device selection remains fully functional via the keyboard. Resize the terminal wider than 70 columns to restore mouse coverage.

#### Update the "Future Work" list

Remove entries that have shipped: NewSessionDialog rows, ConfirmDialog buttons, TagFilter rows, LinkHighlight badges, Settings rows. Keep: drag-to-select, drag-to-resize splits, hover tooltips, project-selector mouse, right-click context menus.

### Acceptance Criteria

1. `docs/MOUSE.md` has new sections for Phase 5 surfaces in the order documented above.
2. The "Modal Precedence and Sub-Modal Gates" subsection is present and accurate.
3. The "Compact NewSessionDialog" caveat is present.
4. The "Future Work" list no longer claims dialogs/overlays are deferred.
5. No section was deleted that should remain (Phase 3 / Phase 4 content survives untouched; only Future Work entries are trimmed).
6. Cross-references to `CONFIGURATION.md` (existing) survive intact.
7. Markdown lint clean: no broken links, table column alignment preserved.

### Testing

Manual review only (no test suite for markdown). After the edit:

```bash
# Render check (any markdown previewer; the project does not commit a renderer).
# Cross-link check:
grep -n "MOUSE.md\|enable_mouse" docs/CONFIGURATION.md docs/MOUSE.md
```

### Notes

- The PLAN.md's "Mouse Interaction Summary" table at lines 442–465 is the canonical reference for what to document. Use it as a checklist; do not duplicate the table verbatim into MOUSE.md.
- Keep section headings stable — Tasks 07/08/09 (website) read MOUSE.md to mirror content. If you rename a heading, leave a note for the website tasks (or land Task 01 first; the orchestrator may run all in parallel).
- Do not document anything that is not yet implemented. If a check against the source reveals a gap, file a follow-up — do not paper over it in docs.
- This task does **not** edit `docs/ARCHITECTURE.md`, `docs/CODE_STANDARDS.md`, `docs/CONFIGURATION.md`, `docs/IDEAS.md`, or `docs/KEYBINDINGS.md`. Those are separate Phase 6 tasks.
