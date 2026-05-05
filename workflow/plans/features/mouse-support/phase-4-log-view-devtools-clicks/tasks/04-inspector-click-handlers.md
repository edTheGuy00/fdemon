## Task: Inspector Click Handlers (Select Row + Toggle Node)

**Objective**: Fill in the bodies of `handle_inspector_select_row` and `handle_inspector_toggle_node` in `handler/devtools/inspector.rs`. `SelectRow` mirrors the `InspectorNav::Up/Down` semantics (set `selected_index`, clear stale layout, dispatch a `FetchLayoutData` action under the existing debounce / cache-hit rules). `ToggleNode` selects the row first, then toggles the node's entry in `inspector.expanded` if the node has children.

**Depends on**: Task 01 (the `Message` variants and stub functions must already exist)

**Estimated Time**: 1 hour

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/handler/devtools/inspector.rs`: Replace the two stubs with real bodies. Extract a private helper `select_index_with_layout_fetch(state, index) -> UpdateResult` that the existing `handle_inspector_navigate` Up/Down branch can also delegate to (optional refactor — see Notes). Add ≥ 4 new unit tests.

**Files Read (Dependencies):**
- `crates/fdemon-app/src/state.rs::InspectorState::visible_nodes`, `InspectorState::is_expanded`, `InspectorState::is_layout_fetch_debounced`, `InspectorState::last_fetched_node_id`, `InspectorState::layout_loading`, `InspectorState::pending_node_id`, `InspectorState::layout_last_fetch_time`
- `crates/fdemon-app/src/handler/UpdateAction::FetchLayoutData`
- The existing `handle_inspector_navigate` body (lines 102–208) — the layout-fetch logic this task reuses verbatim.

### Details

#### `handle_inspector_select_row`

Mirror the Up/Down branch of `handle_inspector_navigate`:

```rust
pub fn handle_inspector_select_row(
    state: &mut AppState,
    index: usize,
) -> UpdateResult {
    // Phase 1: bounds-check and update selection.
    let (old_index, new_index, selection_changed) = {
        let inspector = &mut state.devtools_view_state.inspector;
        let visible = inspector.visible_nodes();
        let count = visible.len();

        if count == 0 || index >= count {
            // Click on a row that no longer exists (tree shrunk between
            // render and click). Silent no-op.
            return UpdateResult::none();
        }

        let old_index = inspector.selected_index;
        inspector.selected_index = index;
        let new_index = inspector.selected_index;
        let selection_changed = new_index != old_index;

        if selection_changed {
            // Clear stale layout immediately so the layout panel shows
            // a loading state — same as InspectorNav::Up/Down.
            inspector.layout = None;
            inspector.layout_error = None;
        }

        (old_index, new_index, selection_changed)
    };

    if !selection_changed {
        // Click on already-selected row → no fetch (cache hit / no-op).
        return UpdateResult::none();
    }

    // Phase 2: dispatch layout fetch (same logic as handle_inspector_navigate).
    let fetch_node_id: Option<String> = {
        let inspector = &mut state.devtools_view_state.inspector;

        if inspector.is_layout_fetch_debounced() {
            None
        } else if let Some(node_id) = get_selected_value_id(inspector) {
            if inspector.last_fetched_node_id.as_deref() == Some(node_id.as_str()) {
                None
            } else {
                inspector.layout_loading = true;
                inspector.pending_node_id = Some(node_id.clone());
                inspector.layout_last_fetch_time = Some(std::time::Instant::now());
                Some(node_id)
            }
        } else {
            None
        }
    };

    let _ = (old_index, new_index); // suppress unused warning if needed

    if let Some(node_id) = fetch_node_id {
        if let Some(session_id) = state.session_manager.selected().map(|h| h.session.id) {
            return UpdateResult::action(UpdateAction::FetchLayoutData {
                session_id,
                node_id,
                vm_handle: None,
            });
        }
    }

    UpdateResult::none()
}
```

#### `handle_inspector_toggle_node`

Selects, then toggles based on `is_expanded`:

```rust
pub fn handle_inspector_toggle_node(
    state: &mut AppState,
    index: usize,
) -> UpdateResult {
    // Step 1: select the row (mirrors InspectorNav::Up/Down semantics —
    // clears stale layout, dispatches fetch under debounce rules).
    let select_result = handle_inspector_select_row(state, index);

    // Step 2: toggle the node's expanded state.
    let inspector = &mut state.devtools_view_state.inspector;
    let visible = inspector.visible_nodes();
    let count = visible.len();
    if count == 0 || index >= count {
        return select_result;
    }

    let (value_id, has_children) = visible
        .get(index)
        .and_then(|(node, _depth)| {
            node.value_id
                .as_ref()
                .map(|id| (id.clone(), !node.children.is_empty()))
        })
        .unzip();

    if let (Some(value_id), Some(true)) = (value_id, has_children) {
        if inspector.is_expanded(&value_id) {
            inspector.expanded.remove(&value_id);
        } else {
            inspector.expanded.insert(value_id);
        }
    }

    select_result
}
```

#### Optional refactor: extract `select_index_with_layout_fetch`

Because `handle_inspector_navigate` Up/Down does almost exactly the same work, the body of `handle_inspector_select_row` can be lifted into a private helper that both call:

```rust
fn select_index_with_layout_fetch(state: &mut AppState, index: usize) -> UpdateResult { /* … */ }
```

Then `handle_inspector_navigate` Up branch becomes:

```rust
InspectorNav::Up => {
    let new_index = inspector.selected_index.saturating_sub(1);
    drop(inspector); // release borrow
    return select_index_with_layout_fetch(state, new_index);
}
```

This is a nice-to-have; if the refactor turns out to be invasive (the existing function has a complex two-phase borrow scope), keep `handle_inspector_select_row` as a copy-paste of the Phase-2 logic and revisit later. The reviewer should not block on this unless the duplication is truly painful.

### Acceptance Criteria

1. `handle_inspector_select_row(state, i)` sets `inspector.selected_index = i` when `i < visible_nodes().len()`, no-ops otherwise.
2. On a selection change, `inspector.layout` and `inspector.layout_error` are cleared and a `FetchLayoutData` action is dispatched, gated by:
   - Debounce (`is_layout_fetch_debounced` returns `true` → no dispatch)
   - Cache hit (`last_fetched_node_id == Some(value_id)` → no dispatch)
3. On a click on the already-selected row, no layout fetch is dispatched (selection_changed = false).
4. `handle_inspector_toggle_node(state, i)` first runs the same selection logic as `select_row`, then toggles `inspector.expanded` for the node's `value_id` *if* the node has children.
5. Toggle on a leaf node (no children) is a no-op for the `expanded` set; selection still changes if applicable.
6. Toggle on a node without a `value_id` is a no-op for the `expanded` set; selection still changes.
7. New tests cover at minimum:
   - SelectRow on out-of-range index → no-op
   - SelectRow on already-selected index → no fetch
   - SelectRow on different index → fetch dispatched
   - ToggleNode collapsed → expanded
   - ToggleNode expanded → collapsed
   - ToggleNode on leaf → expanded set unchanged
8. `cargo test --workspace`, `cargo fmt`, `cargo clippy -- -D warnings`, `cargo check` pass.

### Testing

Extend the existing `#[cfg(test)] mod tests` in `handler/devtools/inspector.rs`. Reuse `make_state_with_session()` and `make_tree_with_children()` helpers already in the file.

```rust
#[test]
fn test_select_row_out_of_range_is_noop() {
    let mut state = make_state_with_session();
    state.devtools_view_state.inspector.root = Some(make_tree_with_children());
    state
        .devtools_view_state
        .inspector
        .expanded
        .insert("root-id".to_string());
    state.devtools_view_state.inspector.selected_index = 0;

    let result = handle_inspector_select_row(&mut state, /*out of range=*/ 99);
    assert!(result.action.is_none());
    assert_eq!(state.devtools_view_state.inspector.selected_index, 0);
}

#[test]
fn test_select_row_same_index_skips_fetch() {
    let mut state = make_state_with_session();
    state.devtools_view_state.inspector.root = Some(make_tree_with_children());
    state.devtools_view_state.inspector.selected_index = 0;

    let result = handle_inspector_select_row(&mut state, 0);
    assert!(result.action.is_none(), "no fetch on same-index click");
}

#[test]
fn test_select_row_different_index_dispatches_fetch() {
    let mut state = make_state_with_session();
    state.devtools_view_state.inspector.root = Some(make_tree_with_children());
    state
        .devtools_view_state
        .inspector
        .expanded
        .insert("root-id".to_string());
    state.devtools_view_state.inspector.selected_index = 0;

    let result = handle_inspector_select_row(&mut state, 1);
    assert!(matches!(
        result.action,
        Some(UpdateAction::FetchLayoutData { .. })
    ));
    assert_eq!(state.devtools_view_state.inspector.selected_index, 1);
}

#[test]
fn test_toggle_node_collapsed_to_expanded() {
    let mut state = make_state_with_session();
    state.devtools_view_state.inspector.root = Some(make_tree_with_children());
    // Root is NOT in expanded set initially.
    assert!(!state
        .devtools_view_state
        .inspector
        .expanded
        .contains("root-id"));

    handle_inspector_toggle_node(&mut state, 0);

    assert!(state
        .devtools_view_state
        .inspector
        .expanded
        .contains("root-id"));
}

#[test]
fn test_toggle_node_expanded_to_collapsed() {
    let mut state = make_state_with_session();
    state.devtools_view_state.inspector.root = Some(make_tree_with_children());
    state
        .devtools_view_state
        .inspector
        .expanded
        .insert("root-id".to_string());

    handle_inspector_toggle_node(&mut state, 0);

    assert!(!state
        .devtools_view_state
        .inspector
        .expanded
        .contains("root-id"));
}

#[test]
fn test_toggle_node_on_leaf_does_not_modify_expanded_set() {
    let mut state = make_state_with_session();
    state.devtools_view_state.inspector.root = Some(make_tree_with_children());
    state
        .devtools_view_state
        .inspector
        .expanded
        .insert("root-id".to_string());

    let before = state.devtools_view_state.inspector.expanded.len();
    // Index 1 is "child-id" — a leaf in make_tree_with_children().
    handle_inspector_toggle_node(&mut state, 1);
    let after = state.devtools_view_state.inspector.expanded.len();

    assert_eq!(before, after, "leaf toggle should not change expanded set");
}
```

### Notes

- **Borrow scope discipline.** The existing `handle_inspector_navigate` carefully releases the `inspector` borrow before reaching `state.session_manager`. Mirror that pattern; do not try to be clever with simultaneous borrows. The block-scoped `let (..., ..., ...) = { let inspector = &mut state.devtools_view_state.inspector; ... };` shape is mandatory.
- **`get_selected_value_id` is private** to `handler/devtools/inspector.rs` and already exists. Reuse it.
- **No `fetch_node_id` for layout when the click selects an already-selected row.** Mirror the early-return check in `handle_inspector_select_row` — if `!selection_changed`, return `UpdateResult::none()` before computing `fetch_node_id`. This mirrors the keyboard handler's behaviour.
- **Toggle without children is silently dropped.** This matches `InspectorNav::Expand` which only acts when `(value_id, Some(true)) == (Some(_), has_children)`.
- **Refactor optionality.** If the planner notes' "extract `select_index_with_layout_fetch`" suggestion proves invasive (e.g., the borrow lifetimes become awkward when called from the existing `handle_inspector_navigate`), do not bundle the refactor into this task. Phase 5 may revisit. The two-phase borrow pattern in the current `handle_inspector_navigate` is fragile; preserving it is more important than removing duplication.
- **Out-of-range index handling.** A click on row 5 of a tree that just shrunk to 3 rows is possible if the user clicks at the same time as the tree updates. Silent no-op is correct — emitting a `Message` would be wrong because the action wouldn't make sense.
- **Toggle on leaf still calls `select_row`.** Per the Phase-3 last-pushed-wins contract, the glyph rect is always pushed *after* the row rect for the same row. So clicking on the glyph cell of a leaf will hit the glyph region, which emits `ToggleNode`. We don't want the click to be silently dropped — we still want the row to become selected. Hence: select first, then attempt toggle (which is a no-op for leaves).

---

## Completion Summary

**Status:** Done
**Branch:** worktree-agent-a8424283893cd1402

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/handler/devtools/inspector.rs` | Replaced two stubs (`handle_inspector_select_row`, `handle_inspector_toggle_node`) with real implementations; added 10 new unit tests |

### Notable Decisions/Tradeoffs

1. **No `select_index_with_layout_fetch` refactor**: The optional helper extraction was skipped per task guidance — the two-phase borrow pattern in `handle_inspector_navigate` is fragile. `handle_inspector_select_row` duplicates the Phase-2 layout-fetch logic from `handle_inspector_navigate` with clear block-scoped borrows. This is a deliberate copy to preserve borrow safety; the planner noted the refactor can be revisited later.
2. **10 tests added (task required ≥ 4)**: All 6 required scenarios from the acceptance criteria are covered, plus 4 additional edge cases (stale layout cleared on row change, debounced select skips fetch, out-of-range toggle is no-op, leaf toggle still selects row).

### Testing Performed

- `cargo fmt --all -- --check` - Passed
- `cargo check --workspace --all-targets` - Passed
- `cargo test -p fdemon-app` - Passed (2048 unit tests, 0 failed)
- `cargo clippy --workspace --all-targets -- -D warnings` - Passed (0 warnings)

### Risks/Limitations

1. **Borrow discipline**: The two-phase borrow pattern (scope inspector borrow, release, then access session_manager) is preserved verbatim from `handle_inspector_navigate`. Any future refactor must maintain this discipline or the Rust borrow checker will reject simultaneous field borrows.
