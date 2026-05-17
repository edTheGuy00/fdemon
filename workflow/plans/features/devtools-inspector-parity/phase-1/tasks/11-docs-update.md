## Task: Update Documentation for Phase 1 Inspector parity changes

**Agent:** doc_maintainer

**Objective**: Update core project documentation to reflect the Phase 1 implementation: new `DetailsTab` state, `expanded_groups` row-folding model, hide-implementation toggle, tabbed details view, new key bindings, and tiered Esc semantics.

**Depends on**: 01-core-diagnostics-and-row-builder, 02-state-inspector-extensions, 03-settings-hide-implementation, 04-message-variants, 05-handlers-details-and-toggle, 06-key-bindings, 07-tui-tree-rendering, 08-tui-details-tabs, 09-tui-inspector-mode-switch, 10-tui-footer-hints

**Estimated Time**: 1–2 hours

### Scope

**Files Modified (Write):**
- `docs/ARCHITECTURE.md` — DevTools Subsystem section: add `DetailsTab` to the state model description; mention the row-builder algorithm at a high level (1 paragraph); note that `visible_nodes()` is now a backward-compat shim over `inspector_rows()`.
- `docs/KEYBINDINGS.md` — Widget Inspector Panel section (lines 445–457): update key table; add Details mode bindings.

**Files Read (Dependencies):**
- `~/.claude/skills/doc-standards/schemas.md` — Content boundary rules.
- All Phase 1 implementation task files (for change context).
- `workflow/plans/features/devtools-inspector-parity/PLAN.md` — Reference plan.

### Change Context

Summarize what implementation changes require doc updates:

1. **State shape change (ARCHITECTURE.md)** — `InspectorState` gained `expanded_groups`, `hide_implementation_widgets`, `details_open`, `details_tab`, `details_node_id`, `properties`, `render_properties`. A new `DetailsTab` enum exists. `visible_nodes()` is now a thin shim over `inspector_rows()`. The DevTools Subsystem section ("Panel State Model" subsection at ARCHITECTURE.md:887–893) needs to mention these.
2. **Row-builder algorithm (ARCHITECTURE.md)** — Add a 1-paragraph note under the "Inspector Widget Tree Fetch" subsection (line ~909) describing that the rendered tree applies DevTools' `_alwaysVisible` heuristic to fold contiguous chains of non-local-project wrapper widgets, gated by the `hide_implementation_widgets` flag.
3. **New keys (KEYBINDINGS.md)** — Inspector Panel section currently lists only `Up`, `Down`, `Enter`, `Right`, `Left`, `r`. Phase 1 adds: `Enter` (now opens Details), `Shift+H` (toggle hide-implementation), `Tab`/`Shift+Tab` (cycle tabs in details mode), tiered Esc semantics. Document the two modes (tree mode vs details mode) and which keys are active in each.

### Acceptance Criteria

1. `docs/ARCHITECTURE.md` accurately describes the new state fields and the row-builder behavior — without expanding into implementation detail that belongs in code comments.
2. `docs/KEYBINDINGS.md` enumerates all Phase 1 key bindings; tree mode and details mode are clearly separated.
3. No content boundary violations: keep architectural content in ARCHITECTURE.md, key bindings in KEYBINDINGS.md, coding patterns in CODE_STANDARDS.md.
4. Cross-references valid — any link from ARCHITECTURE.md to the new types references their actual file paths.
5. `cargo doc --workspace --no-deps` still produces a valid doc tree (no broken doc-links).

### Specific Edits Needed

#### ARCHITECTURE.md

**Section: DevTools Subsystem → Panel State Model (line ~887):**

Replace or expand the bullet for `InspectorState` (line 893) with:

> - **Inspector state** (`InspectorState` within `DevToolsViewState`): Holds the widget tree, layout data, selected node, the `has_ever_rendered_tree` flag, the `hide_implementation_widgets` toggle, and the Details view fields (`details_open`, `details_tab: DetailsTab`, `details_node_id`, `properties`, `render_properties`). `hide_implementation_widgets` survives `reset()` because it is a user preference; the Details fields are cleared on reset. The active row list is produced by `inspector_rows()`, which folds contiguous chains of non-local-project wrapper widgets into a leader row when `hide_implementation_widgets == true`. `visible_nodes()` is kept as a backwards-compatible flat-tuple shim over the row builder.

**Section: DevTools Subsystem → (NEW subsection or appended to Inspector Widget Tree Fetch):**

Add a 1-paragraph block:

> **Tree row builder.** The rendered tree is built by `build_inspector_rows()` in `fdemon-core/widget_tree.rs`. The algorithm computes per-row metadata (`ticks` for ancestor guideline columns, `line_to_parent` for `├─`/`└─` branch ticks, `RowGroup` for chain-fold leaders and members) and folds contiguous chains of non-local-project wrapper widgets behind a `+ N more widgets` leader row when the user's `hide_implementation_widgets` toggle is on. This mirrors DevTools' `_alwaysVisible` heuristic (`createdByLocalProject || has >1 children || has siblings || is root`).

#### KEYBINDINGS.md

Replace the Widget Inspector Panel block (lines 445–457) with two side-by-side mode tables:

```markdown
### Widget Inspector Panel

The Inspector panel has two modes: **tree mode** (default) and **details mode**
(after pressing `Enter` on a selected widget). Key bindings differ between
modes.

#### Tree mode

| Key | Action |
|-----|--------|
| `Up` / `k` | Move selection up |
| `Down` / `j` | Move selection down |
| `Right` / `l` | Expand node (or expand collapsed group) |
| `Left` / `h` | Collapse node |
| `Enter` | Open Details view for selected widget |
| `Shift+H` | Toggle "Hide implementation widgets" (chain collapsing) |
| `r` | Refresh widget tree |
| `b` | Open Flutter DevTools in browser |
| `Esc` | Exit DevTools → Logs |

#### Details mode

| Key | Action |
|-----|--------|
| `Tab` / `Right` / `l` | Cycle to next tab (Widget properties → Render object → Flex explorer → wrap) |
| `Shift+Tab` / `Left` / `h` | Cycle to previous tab |
| `Esc` | Close Details (return to tree mode) |
| `r` | Refresh details |
| `b` | Open Flutter DevTools in browser |
| `Up` / `Down` / `j` / `k` | **No-op** — selection frozen while details is open |

Press `Esc` from Details to return to tree mode; press `Esc` again to exit
DevTools to the log view.

Chain collapsing: when "Hide implementation widgets" is on (default,
`[devtools] hide_implementation_widgets = true` in `.fdemon/config.toml`),
long single-child chains of non-local-project wrapper widgets (e.g. nested
`BlocProvider`s) fold into a single `+ N more widgets` row. Press `Right` on
the leader to expand the chain in place.
```

### Notes

- Follow content boundaries strictly. Anything about WHY or HOW the row builder works algorithmically belongs in code comments / the implementation file — ARCHITECTURE.md should only state that the row builder exists and what role it plays.
- Make targeted edits — do NOT rewrite either document end-to-end. The existing layout, sectioning, and tone should be preserved.
- Phase 2 (Render object + Flex explorer real content) and Phase 3 (conditional tab visibility) will require further doc updates — this task covers only Phase 1.

---

## Completion Summary

**Status:** Not Started
**Branch:** feat/devtools-inspector-parity

### Files Modified

| File | Changes |
|------|---------|

### Notable Decisions/Tradeoffs

### Testing Performed

### Risks/Limitations
