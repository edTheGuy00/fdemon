## Task: Wire Merged Inspector Layout, 50/50 Split, and Auto-Fetch

**Objective**: Connect the new layout panel into the inspector widget with a 50/50 split, replace the details panel, update the threshold for responsive layout, and add auto-fetch of layout data on tree navigation with 500ms debounce.

**Depends on**: Task 03 (remove-layout-panel-variant), Task 04 (extract-padding-from-vm-service), Task 05 (create-layout-panel-widget)

### Scope

- `crates/fdemon-tui/src/widgets/devtools/inspector/mod.rs`: Update split layout and wiring
- `crates/fdemon-tui/src/widgets/devtools/inspector/details_panel.rs`: DELETE
- `crates/fdemon-app/src/state.rs`: Add `layout_last_fetch_time` field to `InspectorState`
- `crates/fdemon-app/src/handler/devtools/inspector.rs`: Add auto-fetch on navigation
- `crates/fdemon-tui/src/widgets/devtools/mod.rs`: Update Inspector footer hints

### Details

#### 1. Update inspector/mod.rs — 50/50 split

**Change constants (around line 19-28):**

```rust
// Old
pub(super) const WIDE_TERMINAL_THRESHOLD: u16 = 80;
pub(super) const TREE_WIDTH_PCT: u16 = 60;
pub(super) const DETAILS_WIDTH_PCT: u16 = 40;

// New
pub(super) const WIDE_TERMINAL_THRESHOLD: u16 = 100;
pub(super) const TREE_WIDTH_PCT: u16 = 50;
pub(super) const LAYOUT_WIDTH_PCT: u16 = 50;
```

Rename `DETAILS_WIDTH_PCT` to `LAYOUT_WIDTH_PCT` for clarity.

**Update `render_tree` method (around lines 141-168):**

Change the call from `self.render_details(det_area, buf, visible, selected)` to `self.render_layout_panel(det_area, buf, visible, selected)`.

The layout split logic stays the same structure:
- Wide terminals (>= 100 cols): horizontal split — tree left | layout right
- Narrow terminals (< 100 cols): vertical split — tree top | layout bottom

Both use `Constraint::Percentage(50)` for even split.

**Update module declarations:**

```rust
// Replace
mod details_panel;
// With
mod layout_panel;
```

#### 2. Delete details_panel.rs

Delete `crates/fdemon-tui/src/widgets/devtools/inspector/details_panel.rs` (130 lines). Its functionality (widget name, properties list, source location) is superseded by `layout_panel.rs` which shows the same name/location plus richer layout data.

#### 3. Add `layout_last_fetch_time` to InspectorState (state.rs)

Add a new field for layout-specific debounce:

```rust
pub struct InspectorState {
    // ... existing fields ...
    pub layout_last_fetch_time: Option<Instant>,  // NEW: 500ms cooldown for auto-fetch
}
```

Add a helper method:

```rust
/// Returns true if a layout fetch should be skipped (debounced).
/// Layout fetch debounce is 500ms (shorter than the 2s tree fetch debounce).
pub fn is_layout_fetch_debounced(&self) -> bool {
    if self.layout_loading {
        return true;
    }
    match self.layout_last_fetch_time {
        Some(t) => t.elapsed() < Duration::from_millis(500),
        None => false,
    }
}
```

Update `reset()` to include `self.layout_last_fetch_time = None;`.

#### 4. Add auto-fetch on tree navigation (handler/devtools/inspector.rs)

Modify `handle_inspector_navigate` (currently lines 70-115) to trigger a layout fetch when the selected node changes on Up/Down navigation:

```rust
pub fn handle_inspector_navigate(state: &mut AppState, nav: InspectorNav) -> UpdateResult {
    let inspector = &mut state.devtools_view_state.inspector;
    let visible = inspector.visible_nodes();
    if visible.is_empty() {
        return UpdateResult::none();
    }

    let old_index = inspector.selected_index;

    match nav {
        InspectorNav::Up => {
            inspector.selected_index = inspector.selected_index.saturating_sub(1);
        }
        InspectorNav::Down => {
            inspector.selected_index = (inspector.selected_index + 1).min(visible.len() - 1);
        }
        InspectorNav::Expand => { /* existing expand logic */ }
        InspectorNav::Collapse => { /* existing collapse logic */ }
    }

    // Auto-fetch layout when selection changes (Up/Down only)
    let selection_changed = inspector.selected_index != old_index;
    if selection_changed {
        // Clear stale layout data — show loading state
        inspector.layout = None;
        inspector.layout_error = None;

        // Check debounce and fetch
        if !inspector.is_layout_fetch_debounced() {
            if let Some(node_id) = get_selected_value_id(inspector) {
                // Skip if same node already fetched
                if inspector.last_fetched_node_id.as_deref() != Some(&node_id) {
                    inspector.layout_loading = true;
                    inspector.pending_node_id = Some(node_id.clone());
                    inspector.layout_last_fetch_time = Some(Instant::now());

                    if let Some(session_id) = get_active_session_id(state) {
                        return UpdateResult::action(UpdateAction::FetchLayoutData {
                            session_id,
                            node_id,
                            vm_handle: None,
                        });
                    }
                }
            }
        }
    }

    UpdateResult::none()
}
```

**Helper to extract `value_id` from selected node:**

```rust
fn get_selected_value_id(inspector: &InspectorState) -> Option<String> {
    let visible = inspector.visible_nodes();
    visible.get(inspector.selected_index)
        .and_then(|(node, _depth)| node.value_id.clone())
}
```

**Key behavior:**
- On Up/Down: clear existing layout data immediately (user sees loading state)
- Check 500ms debounce — skip fetch if within cooldown
- Check `last_fetched_node_id` — skip fetch if same node (cache hit)
- On Expand/Collapse: no layout fetch (selection index doesn't change)

#### 5. Update Inspector footer hints (devtools/mod.rs)

In `render_footer`, update the Inspector hints to mention the merged view:

```rust
DevToolsPanel::Inspector => {
    "[Esc] Logs  [↑↓] Navigate  [→] Expand  [←] Collapse  [r] Refresh  [b] Browser"
}
```

This is unchanged from current — the layout panel updates automatically on navigation, no extra keys needed.

#### 6. Initial layout fetch on entering Inspector panel

In `handle_enter_devtools_mode` and `handle_switch_panel(Inspector)` (handler/devtools/mod.rs), after the widget tree fetch is triggered, also consider triggering an initial layout fetch if the inspector already has a tree with a selected node. This ensures the layout panel shows data immediately when entering DevTools mode:

```rust
// After tree fetch is triggered or tree already loaded:
if inspector.root.is_some() && !inspector.layout_loading {
    if let Some(node_id) = get_selected_value_id(inspector) {
        if inspector.last_fetched_node_id.as_deref() != Some(&node_id) {
            // Trigger layout fetch for current selection
        }
    }
}
```

### Acceptance Criteria

1. Inspector widget shows 50/50 split: tree panel (50%) | layout panel (50%)
2. `WIDE_TERMINAL_THRESHOLD` is 100 (was 80)
3. Wide terminals (>= 100 cols): horizontal split
4. Narrow terminals (< 100 cols): vertical split
5. `details_panel.rs` is deleted — no "Properties" list rendering
6. Layout panel renders using `render_layout_panel` from Task 05
7. Navigating the tree (Up/Down) automatically triggers layout data fetch
8. Layout data fetch is debounced at 500ms
9. Stale layout data is cleared immediately on selection change (loading state shown)
10. `last_fetched_node_id` dedup prevents re-fetching the same node
11. Entering DevTools mode / switching to Inspector triggers initial layout fetch
12. `cargo check --workspace` passes
13. `cargo test --workspace` passes

### Testing

```bash
cargo test -p fdemon-tui -- inspector    # Widget rendering tests
cargo test -p fdemon-app -- devtools     # Handler tests
```

#### Handler tests to add (in handler/devtools/inspector.rs):

```rust
#[test]
fn test_navigate_down_triggers_layout_fetch() {
    // Set up state with loaded tree, navigate down
    // Assert UpdateAction::FetchLayoutData returned
}

#[test]
fn test_navigate_up_clears_stale_layout() {
    // Set up state with loaded layout, navigate up
    // Assert inspector.layout is None (cleared)
}

#[test]
fn test_navigate_debounced_skips_fetch() {
    // Set layout_last_fetch_time to recent
    // Navigate down
    // Assert UpdateResult::none() (no fetch action)
}

#[test]
fn test_navigate_same_node_skips_fetch() {
    // Set last_fetched_node_id to match the node we're navigating to
    // Navigate
    // Assert no fetch action (cache hit)
}

#[test]
fn test_expand_does_not_trigger_layout_fetch() {
    // Navigate with Expand
    // Assert no FetchLayoutData action
}
```

#### Widget tests to update (in inspector/tests.rs):

- Update tests that check the 60/40 split to expect 50/50
- Update tests that assert on details panel content (properties list) to expect layout panel content
- Add test for responsive threshold at 100 cols

### Notes

- The `render_layout_panel` signature matches `render_details` — same parameters `(area, buf, visible, selected)`. This makes the swap in `render_tree` a one-line change.
- The auto-fetch clears `inspector.layout = None` immediately on navigation. This means the user sees "Loading..." briefly while the fetch completes. This is intentional UX — it signals that data is updating.
- The 500ms debounce uses a simple time-based cooldown (not a trailing debounce). If the user navigates 3 nodes in 400ms, only the first navigation triggers a fetch. After 500ms, the next navigation will trigger another fetch. This prevents RPC spam during rapid scrolling.
- The `layout_loading` flag is set to `true` when a fetch is dispatched and `false` when the response arrives (via `handle_layout_data_fetched` / `handle_layout_data_fetch_failed` — both already exist from Task 02).
- Keep the existing `handle_layout_data_fetched` / `handle_layout_data_fetch_failed` / `handle_layout_data_fetch_timeout` handlers unchanged — they already update `inspector.layout_loading`, `inspector.layout`, `inspector.layout_error`, `inspector.last_fetched_node_id` correctly.

---

## Completion Summary

**Status:** Not started
