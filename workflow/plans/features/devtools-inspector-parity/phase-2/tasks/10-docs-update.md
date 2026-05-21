## Task: Update Documentation for Phase 2 (Render Object + Flex Explorer tabs)

**Agent:** doc_maintainer

**Objective**: Update `docs/ARCHITECTURE.md` to reflect the Phase 2 additions: the new `ext.flutter.inspector.getProperties` VM Service extension call, the new flex-layout types in `fdemon-core` (`FlexChild`, `FlexFit`, `Axis`, `MainAxisAlignment`, `CrossAxisAlignment`, `MainAxisSize`), the extended `LayoutInfo` shape, the new `FetchInspectorProperties` `UpdateAction`, and the new `InspectorState` cache fields (`last_fetched_properties_node_id`, `pending_properties_node_id`).

**Depends on**: 01, 02, 03, 04, 05, 06, 07, 08, 09 (all implementation must be complete before docs are updated)

**Estimated Time**: 1–2 hours

### Scope

**Files Modified (Write):**
- `docs/ARCHITECTURE.md` — DevTools Subsystem section (add `getProperties` to extension list; add new types; document the two-stage fetch pipeline; document the new state cache fields)

**Files NOT updated:**
- `docs/KEYBINDINGS.md` — no key binding changes in Phase 2; Phase 1.5 already documented the Inspector keys.
- `docs/CODE_STANDARDS.md` — no new patterns or conventions introduced.
- `docs/DEVELOPMENT.md` — no new build steps, commands, or dependencies.

**Files Read (Dependencies):**
- `~/.claude/skills/doc-standards/schemas.md` — content boundary rules
- All Phase 2 implementation task files (`tasks/01` through `tasks/09`) — change context
- Current `docs/ARCHITECTURE.md` — to make targeted edits, not a rewrite
- Phase 1 doc update at `phase-1/tasks/11-docs-update.md` — for the established style and section locations

### Change Context

Phase 2 implementation introduces five distinct architectural changes that need to be documented:

1. **New VM Service extension call**: `ext.flutter.inspector.getProperties` joins the existing list of inspector RPCs (`getRootWidgetTree`, `getDetailsSubtree`, `getSelectedWidget`, `disposeGroup`, `getLayoutExplorerNode`, `isWidgetTreeReady`). Document in the section that enumerates VM Service extensions.

2. **Two-stage fetch pipeline**: `FetchInspectorProperties` action issues one `getProperties` call for the widget, partitions the response by `propertyType == "RenderObject"`, then issues one further `getProperties` per render-object property to fetch its sub-properties. Document this pipeline in the DevTools subsystem flow description.

3. **New `fdemon-core` types**: `FlexChild`, `FlexFit`, `Axis`, `MainAxisAlignment`, `CrossAxisAlignment`, `MainAxisSize` joined `DiagnosticsNode`/`LayoutInfo` in `widget_tree.rs`. Document in the section that enumerates core domain types.

4. **Extended `LayoutInfo` shape**: New fields `children: Vec<FlexChild>`, `direction: Option<Axis>`, `main_axis_alignment: Option<MainAxisAlignment>`, `cross_axis_alignment: Option<CrossAxisAlignment>`, `main_axis_size: Option<MainAxisSize>`. These are populated from the existing `getLayoutExplorerNode` response (no new RPC for flex children).

5. **New `InspectorState` cache fields**: `last_fetched_properties_node_id: Option<String>` and `pending_properties_node_id: Option<String>`. These complete a pattern already established for layout fetching (`last_fetched_node_id`, `pending_node_id`). Document the cache + stale-guard policy alongside the existing description of `InspectorState` lifecycle.

### Acceptance Criteria

1. `docs/ARCHITECTURE.md` lists `getProperties` in its enumeration of VM Service inspector extensions.
2. The DevTools Inspector data-flow description includes the two-stage `getProperties` round-trip (widget call → split → per-render-object sub-call).
3. The `fdemon-core` types section enumerates the six new flex/axis types.
4. The `LayoutInfo` field list (if present in current ARCHITECTURE.md) is updated to include the five new fields.
5. The `InspectorState` state-management description mentions the two new cache fields and how they interact with the existing `reset()` / `reset_details_and_groups()` reset surfaces.
6. No content boundary violations — only architecture content goes in ARCHITECTURE.md; no code standards, build steps, or keybindings content.
7. All required ARCHITECTURE.md sections per `~/.claude/skills/doc-standards/schemas.md` remain valid.
8. Cross-references (if any) to ARCHITECTURE.md sections from other docs remain valid.
9. The edit is **targeted** — diff should show small additions and updates, not a rewrite. Existing prose for Phase 1 / Phase 1.5 stays put.

### Notes

- Follow content boundaries strictly per `~/.claude/skills/doc-standards/schemas.md`.
- Do NOT add prose about the renderer (`flex_explorer_tab.rs` ASCII visualization, render-object property table). The TUI rendering layer is the consumer; ARCHITECTURE.md documents the architecture, not specific widget rendering details.
- Do NOT add prose about per-tab UX (the tab strip, key bindings, "Coming soon" stub retirement). Those are Phase 1 / Phase 1.5 KEYBINDINGS.md territory.
- If the current ARCHITECTURE.md has a "Phase 1 / Phase 1.5 history" subsection, optionally add a "Phase 2 history" entry per the existing pattern; otherwise weave the updates into the existing structural sections.
- `docs/KEYBINDINGS.md` does NOT need updating — Phase 2 introduces no new key bindings.

---

## Completion Summary

**Status:** Done
**Branch:** feat/devtools-inspector-parity

### Files Modified

| File | Changes |
|------|---------|
| `docs/ARCHITECTURE.md` | Added `getProperties` extension table; added "Inspector Properties Fetch (Two-Stage Pipeline)" subsection; updated `InspectorState` description with two new cache fields and their stale-guard/reset semantics; extended `UpdateResult` with `extra_actions` field description; added `FetchLayoutData` and `FetchInspectorProperties` to `UpdateAction` variants list; updated `widget_tree.rs` description in project structure and fdemon-core public API section to list the six new flex/axis types; added `properties.rs` to the extensions directory listing |

### Content Boundary Compliance

- All updates within correct document boundaries: YES
- Cross-contamination detected and fixed: YES/NO/N/A — N/A (no violations found)

### Notable Decisions/Tradeoffs

1. **No LayoutInfo field list added**: The current ARCHITECTURE.md has no existing field-by-field table for `LayoutInfo`. The acceptance criterion says "if present in current ARCHITECTURE.md" — it was not present, so only the type-level mentions in the project structure and API surface sections were updated to name the five new fields.
2. **VM Service extension table placed in new subsection**: Rather than appending `getProperties` to a prose sentence, a proper table of all inspector VM Service extensions was added in the new "Inspector Properties Fetch" subsection. This is more discoverable and easier to maintain as further extensions are added.
3. **`FetchLayoutData` documented alongside `FetchInspectorProperties`**: The two actions work in tandem and are dispatched together, so both were added to the `UpdateAction` variants list for completeness — `FetchLayoutData` was previously undocumented there.
