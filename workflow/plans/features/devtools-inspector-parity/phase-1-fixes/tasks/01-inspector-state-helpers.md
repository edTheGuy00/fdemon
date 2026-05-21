## Task: Add `InspectorState::selected_row()` Helper

**Objective**: Add a single ergonomic helper on `InspectorState` that returns the currently-selected `InspectorRow`, including its `RowGroup`. This is a prerequisite for task 06 (wiring `expanded_groups` to navigation/mouse handlers) which needs to branch on `RowGroup` variants.

**Depends on**: —

**Estimated Time**: 0.5–1 hour

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/state.rs` — add `selected_row()` method on `InspectorState`; refactor existing `selected_value_id()` to use it (DRY).

**Files Read (Dependencies):**
- `crates/fdemon-core/src/widget_tree.rs` — `InspectorRow<'_>` and `RowGroup` type definitions (lines 235–278).

### Review Items Resolved

- Foundation for **C1** (expanded_groups wiring in task 06)
- Foundation for **M1** (delete `get_selected_value_id` in task 06)

### Details

Currently `InspectorState::selected_value_id()` (state.rs:565–569) does:

```rust
pub fn selected_value_id(&self) -> Option<String> {
    let rows = self.inspector_rows();
    rows.get(self.selected_index)
        .and_then(|r| r.node.value_id.clone())
}
```

Add a sibling `selected_row()` that returns the whole row, and rewrite `selected_value_id()` in terms of it.

```rust
/// Returns the currently-selected row from the active row list, or `None`
/// if the selection is out of bounds.
///
/// The returned row carries its [`RowGroup`] which callers can match on
/// to decide whether the row is a chain leader, member, or standalone.
pub fn selected_row(&self) -> Option<InspectorRow<'_>> {
    let rows = self.inspector_rows();
    rows.into_iter().nth(self.selected_index)
}

pub fn selected_value_id(&self) -> Option<String> {
    self.selected_row().and_then(|r| r.node.value_id.clone())
}
```

Note: `inspector_rows()` returns `Vec<InspectorRow<'_>>` — the helper consumes the vector and pulls out the nth element, which is fine because the row itself only borrows from `self.root`. The implementor may instead use `rows.into_iter().nth(idx)` or `let mut rows = ...; rows.swap_remove(idx)` for performance; either is acceptable since the per-event call path is not hot.

### Acceptance Criteria

1. `InspectorState::selected_row()` exists with a `///` doc comment and the signature above.
2. `InspectorState::selected_value_id()` delegates to `selected_row()` (no longer rebuilds the row list independently).
3. Existing tests on `selected_value_id()` continue to pass unchanged.
4. New test in state.rs's `mod tests`: `selected_row_returns_row_with_group_for_chain_leader` — builds an `InspectorState` with a folded chain, sets `selected_index` to the leader, asserts `selected_row().unwrap().group` is `RowGroup::LeaderCollapsed { .. }`.
5. New test: `selected_row_returns_none_when_index_out_of_bounds`.
6. New test: `selected_row_returns_row_for_standalone_widget` — asserts `group == RowGroup::None` for a non-chain row.
7. `cargo test -p fdemon-app` passes.
8. `cargo clippy -p fdemon-app --all-targets -- -D warnings` passes.

### Testing

Use existing `make_tree_for_inspector_rows_*` helpers (or similar) already present in state.rs's test module. If no fixture exists for a chain-leader scenario, build one inline.

```rust
#[test]
fn selected_row_returns_row_with_group_for_chain_leader() {
    let mut inspector = InspectorState::default();
    // build a tree with a foldable chain (non-local-project, single-child)
    let root = make_root_with_chain(); // 3-node chain inside a wrapper
    inspector.root = Some(root);
    inspector.hide_implementation_widgets = true;
    inspector.selected_index = 1; // the leader row
    let row = inspector.selected_row().expect("row should exist");
    assert!(matches!(row.group, RowGroup::LeaderCollapsed { .. }));
}
```

### Notes

- This helper is consumed by task 06 (handler wiring). The orchestrator's wave plan dispatches 06 in a later wave so the dependency is satisfied at merge time.
- Do **not** introduce a `Cell<>`-based render-hint cache here — per `docs/REVIEW_FOCUS.md`, the `Cell<T>` exception is for layout-feedback hints only. The per-frame `inspector_rows()` consolidation is task 09's responsibility.
- Do **not** modify `inspector_rows()` itself, or `visible_nodes()`, or any caller in this task. Scope is intentionally narrow.

---

## Completion Summary

**Status:** Done
**Branch:** feat/devtools-inspector-parity

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/state.rs` | Added `selected_row()` method on `InspectorState`; refactored `selected_value_id()` to delegate to it; added 3 new tests plus a `make_root_with_chain()` fixture helper |

### Notable Decisions/Tradeoffs

1. **`rows.into_iter().nth(idx)` over `rows.get(idx).cloned()`**: The task spec calls for `into_iter().nth()` and it cleanly owns the vector and extracts the element without a clone, consistent with the existing `selected_value_id` pattern. The vector is short-lived so there is no performance concern.
2. **`make_root_with_chain()` fixture reuses `make_chain()`**: Rather than duplicating inline node construction, the new fixture composes the existing `make_chain(3)` helper into a local-project wrapper, keeping test setup DRY.

### Testing Performed

- `cargo test -p fdemon-app selected_row` — 3 new tests pass
- `cargo test -p fdemon-app` — 2336 tests pass, 0 failures
- `cargo clippy -p fdemon-app --all-targets -- -D warnings` — no warnings
- `cargo fmt --all -- --check` — passes
- `cargo check --workspace --all-targets` — passes

### Risks/Limitations

1. **None identified**: The change is purely additive — a new method and delegation. No existing behaviour was altered.
