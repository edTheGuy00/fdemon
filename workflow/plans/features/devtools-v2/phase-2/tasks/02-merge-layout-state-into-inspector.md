## Task: Merge LayoutExplorerState into InspectorState

**Objective**: Move all fields from `LayoutExplorerState` into `InspectorState`, eliminate the `LayoutExplorerState` struct, and update all handler references. This is a pure refactor — the Layout tab still functions after this task, it just reads layout data from `inspector.*` instead of `layout_explorer.*`.

**Depends on**: None

### Scope

- `crates/fdemon-app/src/state.rs`: Merge structs, update `DevToolsViewState`
- `crates/fdemon-app/src/handler/devtools/inspector.rs`: Absorb layout handler functions
- `crates/fdemon-app/src/handler/devtools/layout.rs`: DELETE (contents moved to inspector.rs)
- `crates/fdemon-app/src/handler/devtools/mod.rs`: Update re-exports and `handle_switch_panel` references

### Details

#### 1. Extend `InspectorState` (state.rs, lines 155-192)

Add the 6 fields from `LayoutExplorerState` with prefixed names to avoid conflicts with existing inspector fields:

```rust
#[derive(Debug, Clone, Default)]
pub struct InspectorState {
    // Existing inspector fields (unchanged)
    pub root: Option<DiagnosticsNode>,
    pub expanded: HashSet<String>,
    pub selected_index: usize,
    pub loading: bool,
    pub error: Option<DevToolsError>,
    pub has_object_group: bool,
    pub last_fetch_time: Option<Instant>,

    // Layout fields (moved from LayoutExplorerState)
    pub layout: Option<LayoutInfo>,
    pub layout_loading: bool,
    pub layout_error: Option<DevToolsError>,
    pub has_layout_object_group: bool,
    pub last_fetched_node_id: Option<String>,
    pub pending_node_id: Option<String>,
}
```

#### 2. Update `InspectorState::reset()` (state.rs, lines 212-220)

Add layout field resets:

```rust
pub fn reset(&mut self) {
    // Existing resets
    self.root = None;
    self.expanded.clear();
    self.selected_index = 0;
    self.loading = false;
    self.error = None;
    self.has_object_group = false;
    self.last_fetch_time = None;

    // Layout resets (moved from LayoutExplorerState::reset)
    self.layout = None;
    self.layout_loading = false;
    self.layout_error = None;
    self.has_layout_object_group = false;
    self.last_fetched_node_id = None;
    self.pending_node_id = None;
}
```

#### 3. Remove `LayoutExplorerState` (state.rs, lines 325-373)

Delete the entire `LayoutExplorerState` struct and its `impl` block.

#### 4. Update `DevToolsViewState` (state.rs, lines 376-458)

Remove the `layout_explorer` field:

```rust
pub struct DevToolsViewState {
    pub active_panel: DevToolsPanel,
    pub inspector: InspectorState,
    // REMOVED: pub layout_explorer: LayoutExplorerState,
    pub overlay_repaint_rainbow: bool,
    pub overlay_debug_paint: bool,
    pub overlay_performance: bool,
    pub vm_connection_error: Option<String>,
    pub connection_status: VmConnectionStatus,
    pub last_overlay_toggle: Option<Instant>,
}
```

Update `DevToolsViewState::reset()` — remove the `self.layout_explorer.reset()` call (already handled by `self.inspector.reset()`).

#### 5. Move layout handlers into inspector.rs

Move the 3 functions from `handler/devtools/layout.rs` into `handler/devtools/inspector.rs`, updating all `state.devtools_view_state.layout_explorer.*` references to `state.devtools_view_state.inspector.*` with renamed fields:

| Old reference | New reference |
|---------------|---------------|
| `layout_explorer.layout` | `inspector.layout` |
| `layout_explorer.loading` | `inspector.layout_loading` |
| `layout_explorer.error` | `inspector.layout_error` |
| `layout_explorer.has_object_group` | `inspector.has_layout_object_group` |
| `layout_explorer.last_fetched_node_id` | `inspector.last_fetched_node_id` |
| `layout_explorer.pending_node_id` | `inspector.pending_node_id` |

Functions to move:
- `handle_layout_data_fetched`
- `handle_layout_data_fetch_failed`
- `handle_layout_data_fetch_timeout`

#### 6. Update handler/devtools/mod.rs

- Update `handle_switch_panel` Layout arm (lines 162-199): Change all `state.devtools_view_state.layout_explorer.*` to `state.devtools_view_state.inspector.*` with renamed fields
- Update `pub use` re-exports: Move layout handler re-exports from the `layout` module to the `inspector` module
- Remove `mod layout;` declaration

#### 7. Delete handler/devtools/layout.rs

After moving all content to inspector.rs, delete the file entirely.

#### 8. Fix any remaining references

Search the entire workspace for `layout_explorer` to find any remaining references (likely in tests, engine.rs, or other handler files). Update them all.

### Acceptance Criteria

1. `LayoutExplorerState` struct no longer exists
2. `InspectorState` contains all 6 layout fields with `layout_` prefix where needed
3. `DevToolsViewState` has no `layout_explorer` field
4. `handler/devtools/layout.rs` is deleted
5. Layout handler functions (`handle_layout_data_fetched`, `handle_layout_data_fetch_failed`, `handle_layout_data_fetch_timeout`) live in `handler/devtools/inspector.rs`
6. All references to `layout_explorer` are updated to use `inspector.*` with renamed fields
7. **No behavior change**: The Layout tab still works as before (reads from inspector state now)
8. All existing tests pass (with updated field references)
9. `cargo check -p fdemon-app` passes
10. `cargo test -p fdemon-app` passes

### Testing

Run the full fdemon-app test suite:

```bash
cargo test -p fdemon-app
```

All existing layout and inspector handler tests must pass. Test assertions reference state fields, so tests must be updated to use the new field paths.

### Notes

- This is a **pure refactor** — no behavior change, no features added, no features removed
- The `DevToolsPanel::Layout` variant still exists after this task — it's removed in Task 03
- The `handle_switch_panel` Layout arm still works after this task — it just reads from `inspector.*` instead of `layout_explorer.*`
- The `layout.rs` handler file has a comment at line 5: "In Phase 2, these handlers will be merged into the inspector module when the Layout tab is absorbed." — this task fulfills that plan
- Keep the `last_fetched_node_id` / `pending_node_id` lifecycle intact — it's a well-designed dedup guard that will be reused by the auto-fetch logic in Task 06

---

## Completion Summary

**Status:** Not started
