## Task: Document Phase 3 conditional tab visibility in `docs/ARCHITECTURE.md` and `docs/KEYBINDINGS.md`

**Objective**: Update the project documentation to reflect Phase 3's data model and behavior changes. ARCHITECTURE.md gains a description of the `DetailsContext` value type, the `visible_tabs()` predicate, and the open-time computation pattern. KEYBINDINGS.md notes that `Tab` / `Shift+Tab` (and `Right` / `Left` in details mode) skip hidden tabs.

**Depends on**: Tasks 01–04 (implementation must be complete so the docs reflect what shipped)

**Estimated Time**: 1–2 hours

**Agent: doc_maintainer**

### Scope

**Files Modified (Write):**
- `docs/ARCHITECTURE.md` (DevTools Subsystem section — additive)
- `docs/KEYBINDINGS.md` (Inspector Panel section — additive)

**Files Read (Dependencies):**
- Task 01 completion summary — `compute_details_context`, `parent_of`, `DetailsContext` signatures
- Task 02 completion summary — `InspectorState::visible_tabs`, `clamp_details_tab` signatures, `details_context` field
- Task 03 completion summary — handler-level wiring (where context populates, where clamp runs)
- Task 04 completion summary — renderer-level dispatch / strip iteration
- `~/.claude/skills/doc-standards/schemas.md` — content-boundary rules
- Current `docs/ARCHITECTURE.md` DevTools Subsystem section (post Phase 2 follow-up task 05)
- Current `docs/KEYBINDINGS.md` Inspector Panel section

### Details

#### Background

The `doc_maintainer` agent is the only agent permitted to edit `docs/ARCHITECTURE.md` (per `claude-code` skill rules). `docs/KEYBINDINGS.md` is unmanaged and is normally edited by the implementor, but for this phase bundle it under the same task to keep the Phase 3 doc update atomic.

ARCHITECTURE.md content-boundary rules:
- Describe **what** the system is, not how it was implemented.
- Surface domain types, layer boundaries, data flow.
- AVOID step-by-step implementation prose, code snippets longer than ~5 lines, or task-level commentary.

KEYBINDINGS.md content-boundary rules:
- Lists keys, contexts, and effects.
- Avoid implementation references.

#### 1. ARCHITECTURE.md — DevTools Subsystem update

Locate the DevTools Subsystem section. After Phase 2 follow-up task 05 it should already mention `getProperties`, `FlexChild`, `LayoutInfo.children`, and the stale-guard pattern. Add a new sub-section (or extend the existing Details-view paragraph) covering:

**Suggested heading: "Inspector Details Tab Visibility"** or add to the existing "Inspector Details View" sub-section if one exists.

Content to capture (interface-level, no Rust syntax):

- The Inspector Details view shows up to three tabs: Widget Properties, Render Object, Flex Explorer.
- Tab visibility is data-driven, mirroring DevTools' `DetailsTable` predicate:
  - Widget Properties: always visible.
  - Render Object: visible only when the selected widget's `getProperties` response contained a node with `propertyType == "RenderObject"` (equivalently, `inspector.render_properties` is non-empty).
  - Flex Explorer: visible only when the selected widget OR its tree parent is `Row`, `Column`, or `Flex`.
- The `DetailsContext` value type (in `fdemon-core`) holds the tree-derived visibility predicates for one details session. It is computed once when the user opens the details view and cached on `InspectorState`. The cached context is cleared when the details view closes, when the inspector state resets, or overwritten when the user opens details on a new node.
- The `visible_tabs` accessor on `InspectorState` derives the visible-tab list from `DetailsContext` plus the current `render_properties` length. The handler and TUI renderer both consume this accessor — single source of truth.
- When a properties fetch settles (success or failure), the active tab may no longer be visible. A `clamp_details_tab` mutator on `InspectorState` is invoked from `handle_inspector_properties_fetched` and `handle_inspector_properties_fetch_failed` to snap the active tab back to a visible tab (Properties is always visible).
- `Tab` / `Shift+Tab` cycle through the visible tabs only. Cycling with a single visible tab is a no-op.

**Style guidance:**
- Interface-level prose only; do not name internal helper functions like `parent_of` or `compute_details_context` unless the existing section already references comparable helpers (the Phase 2 follow-up task 05 noted some pre-existing function-name references — match that style if present).
- Reference DevTools' `DetailsTable` and `isFlexLayout` predicates by name (these are DevTools concepts, not internal code).
- Length budget: ~150–200 words. Do not duplicate Phase 1 / Phase 2 content; just add the visibility paragraph.

#### 2. ARCHITECTURE.md — Module Reference / Key Types updates

If ARCHITECTURE.md has a "Key Types" or "Domain Types" sub-table under `fdemon-core`, add `DetailsContext` to it as a new row (one line: name + one-sentence description).

If `InspectorState` is documented in a similar list under `fdemon-app`, update the row to mention the new `details_context` field (one line).

#### 3. KEYBINDINGS.md — Inspector Panel update

Locate the Inspector Panel section (around lines 445–457 per Phase 1's update). The existing entries for `Tab` and `Shift+Tab` likely read like:

```
Tab / Shift+Tab    Cycle Details tabs forward / backward
```

Update to:

```
Tab / Shift+Tab    Cycle visible Details tabs forward / backward
                   (hidden tabs are skipped; cycling with 1 visible tab is a no-op)
```

Similarly update the `Right` / `Left` entries that already share the cycle binding in details mode.

Add a new sub-bullet or note below the Inspector Panel section listing the three tabs and when each appears:

```
Details tab visibility:
- Widget properties: always.
- Render object: when the selected widget has a render object
  (e.g. Padding, Column, Stack — not Container).
- Flex explorer: when the selected widget or its parent is Row, Column, or Flex.
```

### Acceptance Criteria

1. `docs/ARCHITECTURE.md` DevTools Subsystem section includes a paragraph (or sub-section) explaining tab visibility rules, the `DetailsContext` cache, and the `visible_tabs` / `clamp_details_tab` pattern.
2. If ARCHITECTURE.md maintains a `fdemon-core` types table or `InspectorState` field list, `DetailsContext` and `details_context` are listed.
3. `docs/KEYBINDINGS.md` Inspector Panel section notes that tab cycling skips hidden tabs, and lists the per-widget-type tab visibility.
4. Both docs pass the `doc-standards` skill's content-boundary checks:
   - ARCHITECTURE.md: no implementation prose (function bodies, file-paths-in-prose past two clicks deep), no Rust syntax over ~3 lines, no task-level commentary.
   - KEYBINDINGS.md: keys + context + effect; no implementation references.
5. No content from CODE_STANDARDS.md or DEVELOPMENT.md is duplicated into either doc.
6. The git diff for these two docs is the minimal set of additions/changes needed to cover Phase 3. Do not refactor unrelated sections.

### Testing

Run the `doc-standards` skill against both files after editing:

```
/doc-standards docs/ARCHITECTURE.md
/doc-standards docs/KEYBINDINGS.md
```

If the skill is not invocable from a task, the doc_maintainer agent should perform an inline review against `~/.claude/skills/doc-standards/schemas.md`:

- ARCHITECTURE.md → schema for `ARCHITECTURE.md` (system design, layers, modules).
- KEYBINDINGS.md → unmanaged but should remain in its existing structure (key tables with context + effect).

### Notes

- The Phase 2 follow-up task 05 logged a non-blocking concern that ARCHITECTURE.md's "Inspector Properties Fetch" prose includes a Rust call expression. That concern is consistent with the surrounding section's style; do NOT attempt to refactor it as part of this task. Keep new prose in the interface-level style described above so it doesn't compound the concern.
- The `parent_type: Option<String>` field on `DetailsContext` is not currently consumed by visibility logic. Mention it only if discussing future use cases; otherwise omit to keep the section concise.
- Do not document the defensive renderer-fallback (task 04) as a public behavior — it is an internal safety net that the user should never observe.
- Do not document the deferred minor items from TASKS.md "Deferred / Out of Scope" — they are tracked in the phase-3 TASKS.md and post-Phase-3 cleanup will own them.
- The `--vm-service-debug` log surface is unchanged by Phase 3 (no new RPCs); do not update the debugging section.
- After both files are edited, run `cargo fmt --all -- --check && cargo check --workspace && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings` to confirm the implementation tasks still pass the full quality gate (this is the doc_maintainer's final verification step before marking the task done).
