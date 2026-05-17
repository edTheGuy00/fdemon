## Task: Extend `InspectorState` with details + group + toggle fields, add `inspector_rows()`

**Objective**: Wire the new domain primitives from task 01 into the application state layer. Add the fields required for the Details view, group-collapse expansion tracking, and the hide-implementation toggle. Replace `visible_nodes()` with a thin backwards-compatible shim built on a new `inspector_rows()` method.

**Depends on**: 01-core-diagnostics-and-row-builder

**Estimated Time**: 3–4 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/state.rs`: Add new fields, `DetailsTab` enum, `inspector_rows()` method, shim `visible_nodes()`, update `reset()`, update or replace `selected_node_description` and `collect_visible`.

**Files Read (Dependencies):**
- `crates/fdemon-core/src/widget_tree.rs` (after task 01): import `InspectorRow`, `RowGroup`, `InspectorRowBuilderInputs`, `build_inspector_rows`.

### Details

#### 1. New `InspectorState` fields

Add to the struct at `crates/fdemon-app/src/state.rs:167–267`:

```rust
pub struct InspectorState {
    // …existing fields…

    /// Set of leader value_ids whose hideable chain is currently expanded.
    /// Independent of `expanded` (which tracks regular tree expand/collapse).
    pub expanded_groups: HashSet<String>,

    /// When true, contiguous chains of non-local-project wrapper widgets are
    /// folded into a leader row. Mirrors DevTools' "Hide implementation widgets"
    /// toggle. Defaults to `true`. Persisted via `[devtools]` in settings (the
    /// startup-time application happens in task 03; the field itself lives here).
    pub hide_implementation_widgets: bool,

    /// True when the user has opened the Details view (Enter pressed).
    pub details_open: bool,

    /// Which tab is currently active in the Details view.
    pub details_tab: DetailsTab,

    /// `value_id` of the widget whose details are currently displayed.
    /// Snapshotted from the selected row at Open time; not updated by
    /// navigation (selection is frozen while details are open).
    pub details_node_id: Option<String>,

    /// Widget property nodes returned by `getProperties` for the
    /// `details_node_id` widget. Populated in Phase 2; empty in Phase 1.
    pub properties: Vec<DiagnosticsNode>,

    /// Render-object diagnostics property nodes (those with
    /// `propertyType == "RenderObject"`) extracted from `properties`. Populated
    /// in Phase 2; empty in Phase 1.
    pub render_properties: Vec<DiagnosticsNode>,

    /// True when a properties fetch is in flight (Phase 2).
    pub properties_loading: bool,

    /// User-friendly error from the last properties fetch (Phase 2).
    pub properties_error: Option<DevToolsError>,
}
```

#### 2. New enum `DetailsTab`

In the same file (or in a small new private module if test count gets large — but inline is fine for now):

```rust
/// Which tab is active in the Details view of the Inspector.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DetailsTab {
    #[default]
    Properties,
    RenderObject,
    FlexExplorer,
}

impl DetailsTab {
    /// Cycle to the next tab in the strip (wraps).
    pub fn next(self) -> Self {
        match self {
            DetailsTab::Properties => DetailsTab::RenderObject,
            DetailsTab::RenderObject => DetailsTab::FlexExplorer,
            DetailsTab::FlexExplorer => DetailsTab::Properties,
        }
    }
    /// Cycle to the previous tab (wraps).
    pub fn prev(self) -> Self {
        match self {
            DetailsTab::Properties => DetailsTab::FlexExplorer,
            DetailsTab::RenderObject => DetailsTab::Properties,
            DetailsTab::FlexExplorer => DetailsTab::RenderObject,
        }
    }
}
```

(Phase 3 will add per-widget-type tab visibility; for Phase 1 all three tabs are always available.)

#### 3. `inspector_rows()` + shim `visible_nodes()`

```rust
impl InspectorState {
    /// Build the list of rendered rows with vertical-guideline + branch-tick
    /// metadata and chain-collapse applied.
    pub fn inspector_rows(&self) -> Vec<InspectorRow<'_>> {
        let Some(root) = &self.root else { return vec![]; };
        build_inspector_rows(InspectorRowBuilderInputs {
            root,
            expanded: &self.expanded,
            expanded_groups: &self.expanded_groups,
            hide_implementation: self.hide_implementation_widgets,
        })
    }

    /// Backwards-compatible shim for callers that only need `(node, depth)`
    /// tuples. Built on `inspector_rows()` so it respects chain folding.
    pub fn visible_nodes(&self) -> Vec<(&DiagnosticsNode, usize)> {
        self.inspector_rows()
            .into_iter()
            .map(|row| (row.node, row.depth))
            .collect()
    }
}
```

#### 4. Update `reset()`

The existing `InspectorState::reset` method clears most fields but must:
- **Preserve** `hide_implementation_widgets` (user preference).
- **Preserve** `has_ever_rendered_tree` (existing sticky flag).
- **Clear** `expanded_groups`, `details_open`, `details_tab`, `details_node_id`, `properties`, `render_properties`, `properties_loading`, `properties_error`.

Add unit tests around the new reset semantics.

#### 5. `selected_node_description` + `collect_visible`

These currently traverse the raw tree (state.rs:380–456). They must now traverse the row-folded view so:
- A node hidden inside a collapsed group is not counted by `selected_index`.
- The leader row's index counts as one row.

Options:
- Re-implement both helpers on top of `inspector_rows()` (simplest; small extra allocation).
- Or thread the same folding logic through the existing recursive helpers (more code, no extra Vec).

**Recommended: re-implement on top of `inspector_rows()`.** Selection state is only used in event handlers — the cost is negligible vs. correctness risk.

Also expose a helper used by handler/devtools/inspector.rs:

```rust
pub fn selected_value_id(&self) -> Option<String> {
    let rows = self.inspector_rows();
    rows.get(self.selected_index)
        .and_then(|r| r.node.value_id.clone())
}
```

(This consolidates the inline `get_selected_value_id` function currently in handler/devtools/inspector.rs:199, which task 05 will switch over to.)

#### 6. Tests

In the existing test module at the bottom of state.rs:

- `inspector_rows_returns_empty_when_no_root`.
- `inspector_rows_folds_chain_when_hide_implementation_true`.
- `inspector_rows_renders_full_chain_when_hide_implementation_false`.
- `visible_nodes_shim_matches_inspector_rows_node_depth_pairs`.
- `reset_preserves_hide_implementation_widgets_and_has_ever_rendered_tree`.
- `reset_clears_details_state`.
- `selected_value_id_returns_none_when_no_tree`.
- `selected_value_id_returns_node_id_for_current_selection`.
- `details_tab_next_wraps_through_three_variants`.
- `details_tab_prev_wraps_through_three_variants`.

### Acceptance Criteria

1. `cargo test -p fdemon-app` passes; new tests cover the cases above.
2. Every existing test in state.rs that uses `visible_nodes()` continues to pass unchanged (shim correctness).
3. `cargo clippy -p fdemon-app --all-targets -- -D warnings` passes.
4. `InspectorState::default()` returns `hide_implementation_widgets: true` (matches DevTools default).
5. `InspectorState::reset()` preserves `hide_implementation_widgets`.

### Testing

```rust
#[test]
fn test_inspector_rows_folds_chain_when_hide_implementation_true() {
    let mut state = InspectorState::default();
    // build a 5-deep wrapper chain (single child each, no createdByLocalProject)
    state.root = Some(make_chain(5));
    // expand all
    state.expanded = collect_value_ids(&state.root);
    let rows = state.inspector_rows();
    assert!(rows.iter().any(|r| matches!(r.group, RowGroup::LeaderCollapsed { .. })));
    assert!(rows.len() < 5, "chain should fold, got {} rows", rows.len());
}
```

### Notes

- Do NOT delete `collect_visible` if it's still used by `selected_node_description` — refactor incrementally. If both helpers end up being rewritten on top of `inspector_rows()`, delete the old recursive form and its tests in the same task.
- Resist the temptation to add Phase 2 fetch logic here — keep the new `properties` / `render_properties` fields as empty Vecs in Phase 1.
- Search for any code that increments `selected_index` past the row count: with chain folding the visible-row count can drop (e.g., when toggling Shift+H ON). Task 05 must clamp `selected_index` to `rows.len().saturating_sub(1)` after toggling.

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
