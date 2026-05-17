## Task: Wire `expanded_groups` to Navigation + Mouse + Delete Duplicate Helper

**Objective**: Make the user-visible expand path work for chain leaders. Branch on `RowGroup` in `handle_inspector_navigate` (Right/Left) and `handle_inspector_toggle_node` (mouse glyph click) to mutate `expanded_groups` for leader rows; fall through to the existing `expanded` set for standalone rows. Delete the private `get_selected_value_id` duplicate; switch its three call sites to `InspectorState::selected_value_id()`. Fix the `handle_open_details` / `handle_close_details` policy contradiction around `details_tab`.

**Depends on**: 01 (consumes `InspectorState::selected_row()`), 04 (consumes `RowGroup` variants from `widget_tree.rs`)

**Estimated Time**: 2 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/handler/devtools/inspector.rs`

**Files Read (Dependencies):**
- `crates/fdemon-app/src/state.rs` — `InspectorState::selected_row()` (added by task 01); `selected_value_id()` for delegation.
- `crates/fdemon-core/src/widget_tree.rs` — `RowGroup::{None, LeaderCollapsed, LeaderExpanded, Member}` (Member arm removed by task 04).

### Review Items Resolved

- **C1** — `expanded_groups` never wired to user input (Right key, Enter, mouse glyph click)
- **M1** — `get_selected_value_id` private duplicate of `InspectorState::selected_value_id()`
- **M8** — Mouse click on `LeaderCollapsed` glyph silently mutates wrong set
- **m2** — `details_tab` reset on open contradicts `handle_close_details` doc comment

### Details

#### C1 + M8 — Wire `expanded_groups`

In `handle_inspector_navigate` (inspector.rs:110-202), the `InspectorNav::Expand` and `InspectorNav::Collapse` branches currently read `visible_nodes()` and mutate `inspector.expanded`. Switch to `inspector.selected_row()` (from task 01) and branch on the row's `group`:

```rust
let row = match inspector.selected_row() {
    Some(r) => r,
    None => return UpdateResult::none(),
};

match nav {
    InspectorNav::Expand => {
        match row.group {
            RowGroup::LeaderCollapsed { .. } => {
                if let Some(leader_id) = row.node.value_id.clone() {
                    inspector.expanded_groups.insert(leader_id);
                }
            }
            _ => {
                // existing path — insert into `expanded` for standard nodes
                if let Some(id) = row.node.value_id.clone() {
                    inspector.expanded.insert(id);
                }
            }
        }
    }
    InspectorNav::Collapse => {
        match row.group {
            RowGroup::LeaderExpanded => {
                if let Some(leader_id) = row.node.value_id.clone() {
                    inspector.expanded_groups.remove(&leader_id);
                }
            }
            _ => {
                if let Some(id) = row.node.value_id.clone() {
                    inspector.expanded.remove(&id);
                }
            }
        }
    }
    InspectorNav::Up | InspectorNav::Down => {
        // ... existing logic
    }
}
```

Apply the same `RowGroup` branching in `handle_inspector_toggle_node` (inspector.rs:433-468) — mouse glyph click on a leader should mutate `expanded_groups`, not `expanded`.

The frozen-selection guard at the top of `handle_inspector_navigate` (`if inspector.details_open { return UpdateResult::none(); }`) stays in place.

#### M1 — Delete `get_selected_value_id`

Remove the private function at inspector.rs:208-213. Replace each of its three call sites:

- Line 65 (`handle_widget_tree_fetched`): `let id = state.devtools_view_state.inspector.selected_value_id();`
- Line 225 (`maybe_fetch_layout`): same.
- Line 284 (`handle_layout_data_fetched`): same.

(The exact line numbers may shift after C1 wiring lands first within this task.)

#### m2 — `details_tab` reset policy

Two contradictory pieces of code:
- `handle_open_details` (inspector.rs:492): `inspector.details_tab = DetailsTab::Properties;`
- `handle_close_details` (inspector.rs:530-531): doc comment claims "`details_tab` is left at its last value so reopening defaults to where the user was."

The reset on open makes the "leave it" on close pointless. Pick one. **Recommendation:** keep the reset on open (more predictable UX — every Open lands the user on Properties) and **fix the close comment** to remove the misleading "preserved" claim.

### Acceptance Criteria

1. Pressing `Right` (or `l`) on a `RowGroup::LeaderCollapsed` row mutates `expanded_groups`, not `expanded`. Pressing `Left` (or `h`) on a `RowGroup::LeaderExpanded` row removes from `expanded_groups`. Non-leader rows continue to use `expanded`.
2. Mouse click on a leader's glyph cell branches identically — `LeaderCollapsed` → `expanded_groups.insert`, `LeaderExpanded` → `expanded_groups.remove`.
3. `get_selected_value_id` no longer exists; all callers use `InspectorState::selected_value_id()`.
4. The `handle_close_details` doc comment no longer claims `details_tab` is preserved.
5. New tests in inspector.rs's `mod tests`:
   - `expand_on_leader_collapsed_inserts_into_expanded_groups`
   - `expand_on_leader_collapsed_does_not_insert_into_expanded`
   - `collapse_on_leader_expanded_removes_from_expanded_groups`
   - `expand_on_standalone_row_inserts_into_expanded` (regression guard)
   - `mouse_toggle_on_leader_glyph_mutates_expanded_groups_not_expanded`
   - `mouse_toggle_on_standalone_glyph_mutates_expanded` (regression guard)
6. Existing tests on `handle_inspector_navigate`, `handle_inspector_toggle_node`, and `handle_open_details` continue to pass.
7. `cargo test -p fdemon-app` passes.
8. `cargo clippy -p fdemon-app --all-targets -- -D warnings` passes.

### Testing

Test setup pattern (existing tests in inspector.rs's tests module already build `AppState` fixtures; reuse):

```rust
#[test]
fn expand_on_leader_collapsed_inserts_into_expanded_groups() {
    let mut state = make_state_with_folded_chain(); // leader at selected_index=1
    let _ = handle_inspector_navigate(&mut state, InspectorNav::Expand);
    assert!(state.devtools_view_state.inspector.expanded_groups.contains("leader-id"));
    assert!(!state.devtools_view_state.inspector.expanded.contains("leader-id"));
}
```

If `make_state_with_folded_chain` doesn't exist, add it as a test helper.

### Notes

- After this task, the flagship "MultiBlocProvider chain" demo works end-to-end. Manual smoke test recommended: run fdemon against a real Flutter app with a chain; press Right on the leader, see the chain unfold; press Left, see it re-fold.
- Wave: W2. Parallel with task 05.
- Sequential dependency on tasks 01 (`selected_row()`) and 04 (`RowGroup` variants — `Member` is removed by 04 so the match arms here are exhaustive over `{None, LeaderCollapsed, LeaderExpanded}`).
- This task does **not** touch the per-frame `inspector_rows()` consolidation (task 09's job) or the lifecycle state reset (task 07's job).

---

## Completion Summary

**Status:** Done
**Branch:** worktree-agent-a9801e57e5f9b738e

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/handler/devtools/inspector.rs` | C1+M8: rewired Expand/Collapse to use `selected_row()` + `RowGroup` branching; M8: `handle_inspector_toggle_node` now branches on `RowGroup`; M1: deleted `get_selected_value_id`, all 3 callers replaced with `inspector.selected_value_id()`; m2: fixed `handle_close_details` doc comment; added 6 new tests + 2 test helpers |

### Notable Decisions/Tradeoffs

1. **Expand/Collapse refactor structure**: The original `handle_inspector_navigate` used a single scoped block that called `visible_nodes()` for all four nav variants. The Expand/Collapse arms now exit early using `selected_row()` + RowGroup branching before the scoped block for Up/Down is reached. This avoids a large refactor of the Up/Down path while keeping the RowGroup dispatch clean.

2. **`handle_inspector_toggle_node` uses `inspector_rows()` instead of `visible_nodes()`**: The toggle handler was already using `visible_nodes()` to check the index. Switched to `inspector_rows()` (the canonical method) to get the `RowGroup` metadata on the row. Both methods are equivalent for this purpose.

3. **`RowGroup::Member` treated as standard node in toggle**: When the user clicks the glyph of a Member row, the current code falls through to the standard `expanded` path. Member rows are internal to an expanded chain and typically have no children of their own, so the `has_children` guard means nothing happens — which is the correct UX (you can't collapse individual members).

4. **`use fdemon_core::RowGroup`** added at the top of the file to reduce verbosity in the match arms.

### Testing Performed

- `cargo fmt --all -- --check` - Passed
- `cargo check --workspace --all-targets` - Passed
- `cargo test --workspace` - Passed (2344 fdemon-app tests, all others passing)
- `cargo clippy --workspace --all-targets -- -D warnings` - Passed (0 warnings)

All 6 new required tests pass:
- `expand_on_leader_collapsed_inserts_into_expanded_groups`
- `expand_on_leader_collapsed_does_not_insert_into_expanded`
- `collapse_on_leader_expanded_removes_from_expanded_groups`
- `expand_on_standalone_row_inserts_into_expanded`
- `mouse_toggle_on_leader_glyph_mutates_expanded_groups_not_expanded`
- `mouse_toggle_on_standalone_glyph_mutates_expanded`

### Risks/Limitations

1. **`make_state_with_folded_chain` visibility**: The chain structure requires `hide_implementation_widgets = true` and exactly 3 non-local nodes to produce a `LeaderCollapsed` at index 1. If `build_inspector_rows` folding logic changes, these tests may need updating.
