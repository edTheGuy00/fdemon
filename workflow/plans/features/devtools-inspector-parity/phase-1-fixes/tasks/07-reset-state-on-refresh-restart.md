## Task: Reset Details + Groups State on Tree Refresh and Hot Restart

**Objective**: When the widget tree refreshes (`r`) or a Flutter hot-restart completes, clear `details_open`, `details_node_id`, `details_tab`, `expanded_groups`, and the `properties_*` cache. Otherwise stale state survives across the boundary and the Details panel renders against freed Dart object ids.

**Depends on**: 06 (same file `handler/devtools/inspector.rs` — must run sequentially after 06 lands)

**Estimated Time**: 1 hour

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/state.rs` — add `InspectorState::reset_details_and_groups()` helper method.
- `crates/fdemon-app/src/handler/devtools/inspector.rs` — call the new helper from `handle_widget_tree_fetched`.
- `crates/fdemon-app/src/handler/update.rs` — call the new helper from `SessionRestartCompleted`.

**Files Read (Dependencies):**
- `crates/fdemon-app/src/state.rs` — current `InspectorState` field list to enumerate what to clear.

### Review Items Resolved

- **C2** — Stale `details_open` / `details_node_id` / `details_tab` / `expanded_groups` after `r` refresh and hot-restart

### Details

#### Add the helper on `InspectorState`

In `crates/fdemon-app/src/state.rs`, alongside the existing `reset()` method:

```rust
impl InspectorState {
    /// Clears state that does not survive a tree refresh or hot restart.
    ///
    /// Unlike [`reset()`], this preserves the user's tree-shape preferences
    /// (`hide_implementation_widgets`) and the sticky `has_ever_rendered_tree`
    /// flag. It clears state that points at specific widget identities that
    /// would be invalidated by a new tree (group leader ids, details snapshot)
    /// or a new Dart isolate (Dart object ids referenced by `details_node_id`).
    pub fn reset_details_and_groups(&mut self) {
        self.details_open = false;
        self.details_node_id = None;
        self.details_tab = DetailsTab::Properties;
        self.expanded_groups.clear();
        self.properties.clear();
        self.render_properties.clear();
        self.properties_loading = false;
        self.properties_error = None;
    }
}
```

Audit the actual `InspectorState` field list to ensure every details-related and groups-related field is covered. If task 01 added new fields (e.g. a per-frame cache invalidation counter), those may also need to participate — verify against the current state.rs.

#### Call from `handle_widget_tree_fetched`

In `crates/fdemon-app/src/handler/devtools/inspector.rs:21-78`, after the existing field clears, invoke the helper:

```rust
inspector.selected_index = 0;
inspector.expanded.clear();
// ... existing clears ...
inspector.reset_details_and_groups();
```

#### Call from `SessionRestartCompleted`

In `crates/fdemon-app/src/handler/update.rs:222-244`, immediately after the `has_ever_rendered_tree = false` line:

```rust
Message::SessionRestartCompleted { session_id } => {
    // ... existing reload-complete + log-emit logic ...
    state.devtools_view_state.inspector.has_ever_rendered_tree = false;
    state.devtools_view_state.inspector.reset_details_and_groups();
    UpdateResult::none()
}
```

### Acceptance Criteria

1. `InspectorState::reset_details_and_groups()` exists with the documented behaviour, doc-commented, and covers every relevant field.
2. `handle_widget_tree_fetched` invokes it on each successful tree fetch.
3. `SessionRestartCompleted` invokes it.
4. New tests:
   - `widget_tree_fetched_clears_details_state_when_details_was_open`: open Details, fetch a new tree, assert `details_open == false`, `details_node_id == None`, `expanded_groups.is_empty()`, `properties.is_empty()`.
   - `session_restart_completed_clears_details_state`: same setup, dispatch `SessionRestartCompleted`, assert the same fields are cleared and `has_ever_rendered_tree == false`.
   - `reset_details_and_groups_preserves_hide_implementation_widgets`: regression guard.
   - `reset_details_and_groups_preserves_has_ever_rendered_tree`: regression guard (the helper itself does not touch it; only `SessionRestartCompleted` does).
5. Existing tests on `handle_widget_tree_fetched` continue to pass.
6. `cargo test -p fdemon-app` passes.
7. `cargo clippy -p fdemon-app --all-targets -- -D warnings` passes.

### Testing

Use existing `make_app_state_with_inspector_*` test helpers as the fixture base. Where one doesn't exist, build inline.

```rust
#[test]
fn widget_tree_fetched_clears_details_state_when_details_was_open() {
    let mut state = make_state_with_inspector_details_open(); // details_open=true, expanded_groups has 1 entry
    let new_tree = make_test_widget_tree();
    let _ = handle_widget_tree_fetched(&mut state, new_tree);
    let inspector = &state.devtools_view_state.inspector;
    assert!(!inspector.details_open);
    assert!(inspector.details_node_id.is_none());
    assert_eq!(inspector.details_tab, DetailsTab::Properties);
    assert!(inspector.expanded_groups.is_empty());
    assert!(inspector.properties.is_empty());
}
```

### Notes

- The orchestrator's wave plan listed this task as writing only `inspector.rs` + `update.rs` — but adding the helper to `state.rs` is cleaner than duplicating the field-clear code in both call sites. The task explicitly extends its write list to include `state.rs`. This is fine because state.rs is not in conflict with any concurrent task (task 06 is the only other handler.rs writer, and 07 runs sequentially after 06 anyway).
- Wave: W3. Sequential with task 06.
- This task does not handle Phase 1.5's m11 (refs re-collect in `details/mod.rs`) — that's bundled in task 09.

---

## Completion Summary

**Status:** Not Started
**Branch:** —

### Files Modified

| File | Changes |
|------|---------|

### Notable Decisions/Tradeoffs

### Testing Performed

### Risks/Limitations
