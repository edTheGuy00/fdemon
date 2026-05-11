# Task 02: Inspector Handler Refactor (`maybe_fetch_layout` extraction)

## Goal

Extract the duplicated layout-fetch logic in `crates/fdemon-app/src/handler/devtools/inspector.rs` into a private helper, remove the `let _ = (old_index, new_index)` lint suppression anti-pattern, and hoist the `visible_nodes()` lookup in `handle_inspector_toggle_node` to avoid the double traversal.

## Background

Three review findings concentrate in the inspector handler:

1. **Major #2 (duplicated layout-fetch logic):** The "Phase 2: dispatch layout fetch" block — debounce check, cache-hit check, set `layout_loading` / `pending_node_id` / `layout_last_fetch_time`, return `FetchLayoutData` — is pasted verbatim across `handle_inspector_navigate` (lines 174–204) and `handle_inspector_select_row` (lines 389–417). The implementation comment even acknowledges it: `// same logic as handle_inspector_navigate`. Task 04's notes from Phase 4 explicitly stated this would be extracted; it never was.

2. **Major #4 (lint suppression anti-pattern):** Both functions contain `let _ = (old_index, new_index); // suppress unused warning`. Per project standards, suppressing legitimate compiler signals via underscore-binding is an anti-pattern. The values are intermediate steps; only `selection_changed` needs to surface.

3. **Minor #9 (`visible_nodes()` double-call):** `handle_inspector_toggle_node` calls `handle_inspector_select_row` (which internally calls `visible_nodes()`), then re-borrows `inspector` and calls `visible_nodes()` again. The result is correct in single-threaded TEA but the duplicate O(N) traversal is wasteful.

## Files

**Modify:**
- `crates/fdemon-app/src/handler/devtools/inspector.rs`

**Read (reference):**
- `crates/fdemon-app/src/state.rs` — `InspectorState::is_layout_fetch_debounced`, `InspectorState::last_fetched_node_id`, `InspectorState::visible_nodes()`

## Plan

1. **Extract `maybe_fetch_layout` helper.** Add a private function near the existing `get_selected_value_id` helper:

   ```rust
   /// If the currently-selected inspector node has a `value_id`, isn't debounced,
   /// and isn't already cached as `last_fetched_node_id`, mark fetch state and
   /// return the node id to fetch. Otherwise return `None`.
   ///
   /// Mutates `inspector.layout_loading`, `pending_node_id`, and
   /// `layout_last_fetch_time` only on the success path.
   fn maybe_fetch_layout(inspector: &mut InspectorState) -> Option<String> {
       if inspector.is_layout_fetch_debounced() {
           return None;
       }
       let node_id = get_selected_value_id(inspector)?;
       if inspector.last_fetched_node_id.as_deref() == Some(node_id.as_str()) {
           return None;
       }
       inspector.layout_loading = true;
       inspector.pending_node_id = Some(node_id.clone());
       inspector.layout_last_fetch_time = Some(std::time::Instant::now());
       Some(node_id)
   }
   ```

   Then replace both Phase-2 blocks in `handle_inspector_navigate` and `handle_inspector_select_row` with:
   ```rust
   let fetch_node_id = maybe_fetch_layout(&mut state.devtools_view_state.inspector);
   ```

   The borrow scope ends naturally before `state.session_manager` access.

2. **Remove `let _ = (old_index, new_index)` suppression.** In both functions, the inner scope was returning `(old_index, new_index, selection_changed)` and immediately discarding the first two. Since `old_index` and `new_index` are not needed outside the inner scope, refactor:

   ```rust
   let selection_changed = {
       let inspector = &mut state.devtools_view_state.inspector;
       // ... bounds check, set selected_index, clear stale layout/layout_error ...
       let old_index = inspector.selected_index;
       inspector.selected_index = index;
       let changed = old_index != Some(index);
       if changed {
           inspector.layout = None;
           inspector.layout_error = None;
       }
       changed
   };
   ```

   No `let _ = ...` line remains.

3. **Hoist `visible_nodes()` lookup in `handle_inspector_toggle_node`.** Currently:
   ```rust
   let select_result = handle_inspector_select_row(state, index);
   let inspector = &mut state.devtools_view_state.inspector;
   let visible = inspector.visible_nodes();
   if index >= visible.len() { return select_result; }
   let node = visible[index];
   let value_id = node.value_id().map(|s| s.to_string());
   let has_children = !node.children().is_empty();
   // ...
   ```

   Refactor to capture `value_id` and `has_children` *before* delegating to `handle_inspector_select_row`:
   ```rust
   let (value_id, has_children) = {
       let inspector = &state.devtools_view_state.inspector;
       let visible = inspector.visible_nodes();
       if index >= visible.len() { return UpdateResult::none(); } // out-of-range guard
       let node = visible[index];
       (node.value_id().map(|s| s.to_string()), !node.children().is_empty())
   };
   let select_result = handle_inspector_select_row(state, index);
   // ... toggle expanded set using value_id, has_children ...
   ```

   The bounds check moves up; the now-redundant secondary `index >= visible.len()` check after delegate-return can be removed.

4. **Verify all 10 existing tests still pass** in `inspector.rs` test module: `test_select_row_out_of_range`, `test_select_row_no_change_no_fetch`, `test_select_row_changes_index_dispatches_fetch`, `test_select_row_clears_stale_layout`, `test_select_row_debounced_no_fetch`, `test_toggle_node_collapsed_to_expanded`, `test_toggle_node_expanded_to_collapsed`, `test_toggle_node_on_leaf_does_not_modify_expanded_set`, `test_toggle_node_out_of_range`, `test_toggle_node_leaf_still_selects_row`.

## Acceptance Criteria

- [ ] `maybe_fetch_layout(&mut InspectorState) -> Option<String>` helper exists.
- [ ] Both `handle_inspector_navigate` and `handle_inspector_select_row` call it (no duplicated debounce/cache-check/state-mutation block).
- [ ] No `let _ = (old_index, new_index)` lines remain.
- [ ] `handle_inspector_toggle_node` calls `visible_nodes()` only once.
- [ ] All 10 existing inspector tests pass; no test removed or skipped.
- [ ] `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo check --workspace --all-targets` pass.

## Notes

- The borrow ordering in the original code uses two phases (mutate inspector, then dispatch action) because `state.session_manager` and `state.devtools_view_state.inspector` are disjoint fields. The helper extraction preserves this — `maybe_fetch_layout` only borrows `inspector`; the caller subsequently accesses `session_manager`.
- If extracting `maybe_fetch_layout` causes any test to fail, prefer adapting the helper signature (e.g., returning `(Option<String>, bool)` for selection_changed) rather than reverting the extraction. The duplicate logic is a concrete debt; abandoning the extraction would re-incur it.
- **Do not touch** any other file in this task. The other Phase 4.5 tasks each have their own scope.

---

## Completion Summary

**Status:** Done
**Branch:** feat/mouse-support

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/handler/devtools/inspector.rs` | Extracted `maybe_fetch_layout` helper; replaced duplicated Phase-2 blocks in `handle_inspector_navigate` and `handle_inspector_select_row` with single call; removed both `let _ = (old_index, new_index)` suppressions; refactored `handle_inspector_toggle_node` to hoist `visible_nodes()` lookup before delegate call |

### Notable Decisions/Tradeoffs

1. **`maybe_fetch_layout` placement**: Added immediately after `get_selected_value_id` (its caller), keeping the two helpers co-located.
2. **`handle_inspector_navigate` inner tuple**: Replaced `(old_index, new_index, selection_changed)` tuple return with just `selection_changed` — `old_index`/`new_index` were only used for the `let _ = ...` suppression and are now removed entirely.
3. **`handle_inspector_toggle_node` bounds check**: Moved the out-of-range guard to the top (before the delegate call). The now-redundant secondary check after the delegate is removed. The early return uses `UpdateResult::none()` (same semantics as the old `return select_result` when `select_result` would also have been `UpdateResult::none()` due to the same bounds check in `handle_inspector_select_row`).

### Testing Performed

- `cargo test -p fdemon-app -- handler::devtools::inspector::tests` — 51 passed, 0 failed
- `cargo fmt --all -- --check` — Passed (no formatting issues)
- `cargo clippy --workspace --all-targets -- -D warnings` — Passed (no warnings)
- `cargo check --workspace --all-targets` — Passed
