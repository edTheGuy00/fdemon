## Task: Implement handlers for Open/Close/CycleTab/ToggleHideImpl + frozen-nav + tiered Esc

**Objective**: Add the TEA update-function handlers for the four new Phase 1 messages. Make `handle_inspector_navigate` no-op when Details is open. Route `Esc` through a new "close details first" check in the DevTools mode handler.

**Depends on**: 02-state-inspector-extensions, 04-message-variants

**Estimated Time**: 3–4 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/handler/devtools/inspector.rs`: New `handle_open_details`, `handle_close_details`, `handle_cycle_tab`, `handle_toggle_hide_implementation`. Update `handle_inspector_navigate`.
- `crates/fdemon-app/src/handler/devtools/mod.rs`: Route the new messages; adjust the Esc path for tiered close.

**Files Read (Dependencies):**
- `crates/fdemon-app/src/message.rs` (new variants from task 04).
- `crates/fdemon-app/src/state.rs` (new fields + `DetailsTab` enum + `selected_value_id` helper from task 02).
- `crates/fdemon-app/src/config/types.rs` (for the settings write-back; see task 03).

### Details

#### 1. `handle_open_details`

```rust
pub fn handle_open_details(state: &mut AppState) -> UpdateResult {
    let inspector = &mut state.devtools_view_state.inspector;
    if inspector.details_open { return UpdateResult::none(); }
    let Some(node_id) = inspector.selected_value_id() else {
        return UpdateResult::none(); // no selection, nothing to open
    };
    inspector.details_open = true;
    inspector.details_tab = DetailsTab::Properties; // always start on first tab
    inspector.details_node_id = Some(node_id.clone());

    // Layout data is fetched by the existing nav-driven path. If the user
    // opens details immediately on initial selection, the data is already
    // warm. If not, dispatch an extra FetchLayoutData here so the
    // Widget properties tab has content.
    let active_id = state.session_manager.selected().map(|h| h.session.id);
    if let Some(session_id) = active_id {
        if inspector.last_fetched_node_id.as_deref() != Some(&node_id)
            && !inspector.layout_loading
        {
            inspector.layout_loading = true;
            inspector.pending_node_id = Some(node_id.clone());
            return UpdateResult::action(UpdateAction::FetchLayoutData {
                session_id, node_id, vm_handle: None,
            });
        }
    }
    UpdateResult::none()
}
```

#### 2. `handle_close_details`

```rust
pub fn handle_close_details(state: &mut AppState) -> UpdateResult {
    let inspector = &mut state.devtools_view_state.inspector;
    if !inspector.details_open { return UpdateResult::none(); }
    inspector.details_open = false;
    inspector.details_node_id = None;
    // details_tab is left at its last value so reopening defaults to where
    // the user was. Reset to Properties only if you want fresh-open semantics.
    UpdateResult::none()
}
```

#### 3. `handle_cycle_tab`

```rust
pub fn handle_cycle_tab(state: &mut AppState, forward: bool) -> UpdateResult {
    let inspector = &mut state.devtools_view_state.inspector;
    if !inspector.details_open { return UpdateResult::none(); }
    inspector.details_tab = if forward {
        inspector.details_tab.next()
    } else {
        inspector.details_tab.prev()
    };
    UpdateResult::none()
}
```

#### 4. `handle_toggle_hide_implementation`

```rust
pub fn handle_toggle_hide_implementation(state: &mut AppState) -> UpdateResult {
    let inspector = &mut state.devtools_view_state.inspector;
    inspector.hide_implementation_widgets = !inspector.hide_implementation_widgets;
    // Clamp selected_index — row count may have shrunk if folding turned on.
    let row_count = inspector.inspector_rows().len();
    if row_count > 0 && inspector.selected_index >= row_count {
        inspector.selected_index = row_count - 1;
    }

    // Mirror back to Settings + persist to disk.
    state.settings.devtools.hide_implementation_widgets =
        state.devtools_view_state.inspector.hide_implementation_widgets;
    // Persist — use existing helper if available; otherwise document in
    // completion summary that disk persistence is deferred.
    // <implementor: verify with grep -rn "save_settings\|persist_settings\|Settings::write" crates/fdemon-app/src/>

    UpdateResult::none()
}
```

#### 5. Freeze navigation when details open

At the top of the existing `handle_inspector_navigate(state, nav)` in inspector.rs:109:

```rust
pub fn handle_inspector_navigate(state: &mut AppState, nav: InspectorNav) -> UpdateResult {
    // Phase 1: when Details is open, Up/Down/Left/Right are all no-ops in
    // the tree. The user must press Esc to return to tree mode first.
    if state.devtools_view_state.inspector.details_open {
        return UpdateResult::none();
    }
    // …existing body…
}
```

Decision note: `Left`/`Right` repurpose as cycle-tab while Details is open (this is the documented spec from PLAN.md §5.2). The cycle is triggered by the key handler (task 06), not by `handle_inspector_navigate`. Make sure the early-return here doesn't swallow the cycle — task 06 must route `Left`/`Right` to `DevToolsInspectorCycleTab` directly (NOT through `InspectorNav`) when `details_open == true`.

#### 6. Tiered Esc

In `crates/fdemon-app/src/handler/devtools/mod.rs`, find the existing Esc-to-Logs path (search: `grep -n "exit_devtools_mode\|ExitDevTools" crates/fdemon-app/src/handler/devtools/mod.rs`). Wrap it:

```rust
pub fn handle_devtools_escape(state: &mut AppState) -> UpdateResult {
    // Phase 1: tiered Esc. If the Inspector tab is showing the Details view,
    // first Esc closes Details; second Esc (with details already closed)
    // exits DevTools as today.
    if state.devtools_view_state.active_panel == DevToolsPanel::Inspector
        && state.devtools_view_state.inspector.details_open
    {
        return inspector::handle_close_details(state);
    }
    // …existing exit-DevTools logic…
}
```

The exact existing function name/location may differ; verify with grep before editing. If Esc dispatch happens inline in `keys.rs` rather than through a `handle_devtools_escape` helper, do the same check inline. Prefer extracting a helper if the call site is currently inline — keeps task 06 minimal.

#### 7. Dispatch in `update()` / `mod.rs`

Wire the four new messages into the DevTools handler dispatch (whichever `match` arm handles `Message::DevTools*` variants — check the existing pattern for `DevToolsInspectorNavigate`).

#### 8. Tests

In `crates/fdemon-app/src/handler/devtools/inspector.rs`'s test module (or `handler/tests.rs` if that's the convention — check existing tests):

- `handle_open_details_sets_details_open_and_snapshots_node_id`.
- `handle_open_details_is_no_op_when_no_selection`.
- `handle_open_details_dispatches_fetch_layout_when_data_stale`.
- `handle_close_details_clears_details_node_id`.
- `handle_cycle_tab_forward_advances_through_three_tabs_with_wrap`.
- `handle_cycle_tab_backward_advances_through_three_tabs_with_wrap`.
- `handle_cycle_tab_is_no_op_when_details_closed`.
- `handle_toggle_hide_implementation_flips_flag_and_clamps_selection`.
- `handle_toggle_hide_implementation_writes_back_to_settings`.
- `handle_inspector_navigate_is_no_op_when_details_open`.
- `tiered_esc_closes_details_first_then_exits_devtools` (two-step test).

### Acceptance Criteria

1. `cargo test -p fdemon-app` passes with the new tests.
2. Opening Details on a node where layout data is already cached does NOT trigger a redundant fetch.
3. Up/Down/Left/Right are no-ops in tree mode when `details_open == true` (Left/Right are repurposed by key handler in task 06; they don't reach `handle_inspector_navigate`).
4. Esc with `details_open == true` closes details only; Esc with `details_open == false` exits DevTools.
5. Toggling `hide_implementation_widgets` clamps `selected_index` to a valid row if the row count drops.
6. `cargo clippy -p fdemon-app --all-targets -- -D warnings` passes.

### Testing

```rust
#[test]
fn handle_open_details_sets_details_open_and_snapshots_node_id() {
    let mut state = make_state_with_tree();
    state.devtools_view_state.inspector.selected_index = 1;
    let _ = handle_open_details(&mut state);
    assert!(state.devtools_view_state.inspector.details_open);
    assert_eq!(state.devtools_view_state.inspector.details_node_id.as_deref(),
        Some("node-1-value-id"));
}
```

### Notes

- The settings write-back to `.fdemon/config.toml` is the only piece that may need a small infrastructure addition. If `Settings::write_to_disk()` doesn't exist, **implement a minimal version here** (use `toml::to_string_pretty(&self.settings)?` + `std::fs::write(path, …)?`). Document the choice in the Completion Summary.
- If the existing handler dispatch uses a match in `engine.rs` rather than `handler/devtools/mod.rs`, edit there instead. Verify the actual dispatch site before deciding which file to write to.
- The "selection frozen while details open" constraint applies to mouse clicks too. The existing `DevToolsInspectorSelectRow` handler at handler/devtools/inspector.rs (search the file for `handle_inspector_select_row`) must also early-return when `details_open == true`.

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
