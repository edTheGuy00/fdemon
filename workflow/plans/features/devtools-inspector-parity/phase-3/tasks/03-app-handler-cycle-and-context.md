## Task: Populate `details_context` on open, skip hidden tabs in cycle handler, clamp after properties fetch

**Objective**: Wire the new `DetailsContext` and `visible_tabs()` plumbing into the handler layer. `handle_open_details` computes and stores a fresh `DetailsContext`. `handle_cycle_tab` cycles through `visible_tabs()` only. `handle_inspector_properties_fetched` and `handle_inspector_properties_fetch_failed` call `clamp_details_tab()` so the active tab never points at a hidden tab after a fetch settles.

**Depends on**: Task 02 (`details_context` field, `visible_tabs()`, `clamp_details_tab()` on `InspectorState`)

**Estimated Time**: 3–4 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/handler/devtools/inspector.rs`

**Files Read (Dependencies):**
- `crates/fdemon-app/src/state.rs` — `InspectorState`, `visible_tabs`, `clamp_details_tab`, `details_context` (from task 02)
- `crates/fdemon-core/src/widget_tree.rs` — `compute_details_context` (from task 01)

### Details

#### Background

After task 02, `InspectorState` exposes the `visible_tabs()` view and a `clamp_details_tab()` mutator, but no handler actually USES them yet. Phase 1/2's `handle_cycle_tab` still calls `DetailsTab::next()` / `DetailsTab::prev()`, which traverse all three tab variants unconditionally. Phase 1/2's `handle_open_details` does not populate `details_context`. This task wires those handlers up.

#### 1. Locate the three target handlers

In `crates/fdemon-app/src/handler/devtools/inspector.rs`:

| Handler | Approximate lines |
|---------|-------------------|
| `handle_open_details` | ~669–725 |
| `handle_inspector_properties_fetched` | ~401–437 |
| `handle_inspector_properties_fetch_failed` | nearby (search for `properties_error` writes) |
| `handle_cycle_tab` | ~749–760 |

Run `grep -n` to confirm exact line numbers — the codebase research above is current as of the start of Phase 3 but may have drifted.

#### 2. Update `handle_open_details` to compute and store `details_context`

Current shape (verified by codebase research):

```rust
pub fn handle_open_details(state: &mut AppState) -> UpdateResult {
    let inspector = &mut state.devtools_view_state.inspector;
    // ... existing selected_value_id snapshot, details_open = true,
    //     details_tab = Properties, etc.
    inspector.details_open = true;
    inspector.details_tab = DetailsTab::Properties;
    inspector.details_node_id = Some(value_id.clone());
    // ... dispatch FetchLayoutData + FetchInspectorProperties ...
}
```

Phase 3 addition — after `details_node_id` is set and BEFORE `details_open = true` is flipped (or just before the action dispatch):

```rust
// Phase 3: precompute tree-derived visibility predicates for the open session.
//
// Walks the tree once; cached on `inspector.details_context` and consumed by
// `visible_tabs()` for the duration of the details session. Cleared by
// `reset_details_and_groups()` and overwritten by the next open.
if let Some(root) = inspector.root.as_ref() {
    inspector.details_context =
        fdemon_core::widget_tree::compute_details_context(root, &value_id);
} else {
    // Root absent (shouldn't happen if a node is selected, but defensive):
    // default context means only the Properties tab will render until a
    // future open lands.
    inspector.details_context = fdemon_core::widget_tree::DetailsContext::default();
}
```

Key points:
- `value_id` is the snapshot of `selected_value_id()` already computed earlier in the same handler.
- Use the same `inspector.root.as_ref()` access pattern already used elsewhere in the handler (consistency with existing code style).
- Place the assignment AFTER `details_node_id` is written so the assignment ordering reads as: "we've decided which node, now compute its context".
- `details_tab = Properties` (existing line 678) is unchanged — Properties is always visible, so no clamp needed at open.

#### 3. Update `handle_cycle_tab` to use `visible_tabs()`

Current implementation (lines 749–760):

```rust
pub fn handle_cycle_tab(state: &mut AppState, forward: bool) -> UpdateResult {
    let inspector = &mut state.devtools_view_state.inspector;
    if !inspector.details_open {
        return UpdateResult::none();
    }
    inspector.details_tab = if forward {
        inspector.details_tab.next()
    } else {
        inspector.details_tab.prev()
    };
    UpdateResult::none()
}
```

Replace with:

```rust
pub fn handle_cycle_tab(state: &mut AppState, forward: bool) -> UpdateResult {
    let inspector = &mut state.devtools_view_state.inspector;
    if !inspector.details_open {
        return UpdateResult::none();
    }

    let visible = inspector.visible_tabs();
    if visible.is_empty() {
        // Defensive: visible_tabs always returns at least [Properties].
        return UpdateResult::none();
    }

    // Find current tab in visible list. If somehow not present (e.g. clamp
    // was missed), fall back to first visible tab.
    let current_idx = visible.iter().position(|t| *t == inspector.details_tab);

    inspector.details_tab = match current_idx {
        Some(idx) => {
            let next_idx = if forward {
                (idx + 1) % visible.len()
            } else {
                (idx + visible.len() - 1) % visible.len()
            };
            visible[next_idx]
        }
        None => visible[0],
    };

    UpdateResult::none()
}
```

Notes:
- Cycling wraps within the visible-tab list, not the static enum.
- When only one tab is visible, both forward and backward are effective no-ops (current stays current). The handler still returns `UpdateResult::none()` cleanly.
- `DetailsTab::next()` / `DetailsTab::prev()` are NOT called here anymore. Leave the methods defined on the enum (other code or tests may rely on them); task 02 already noted this.

#### 4. Call `clamp_details_tab()` after properties-fetched / fetch-failed

In `handle_inspector_properties_fetched` (lines ~401–437) — find the location AFTER `inspector.properties` and `inspector.render_properties` are assigned. Add:

```rust
// Phase 3: the fetch may have changed which tabs are visible.
// If the active tab is now hidden, snap to Properties.
inspector.clamp_details_tab();
```

Place it after the field writes but before the function returns. The clamp call is idempotent and cheap.

In `handle_inspector_properties_fetch_failed` (search for the corresponding handler — sibling of fetched, sets `properties_error`):

```rust
// Phase 3: failure may leave render_properties empty; clamp if Render
// Object was the active tab.
inspector.clamp_details_tab();
```

Same placement rule: after the error field assignments, before return.

Note: The clamp does nothing if the previous data already kept the Render Object tab visible (e.g. cache hit followed by a failed re-fetch that didn't touch `render_properties`). It only kicks in when the visible set shrinks past the active tab.

### Acceptance Criteria

1. `handle_open_details` computes `details_context` via `compute_details_context(root, value_id)` and stores it on `InspectorState` before flipping `details_open = true`.
2. When `root` is `None` at open time, `details_context` is set to `DetailsContext::default()` (defensive, never panic).
3. `handle_cycle_tab` cycles within `visible_tabs()` only — never lands on a hidden tab.
4. `handle_cycle_tab` is a no-op when `visible_tabs().len() == 1` (forward/backward both end on the same single visible tab).
5. `handle_inspector_properties_fetched` and `handle_inspector_properties_fetch_failed` both call `clamp_details_tab()` after writing the properties / error fields.
6. New unit tests in `inspector.rs` cover:
   - Open details on a `Column` widget populates `details_context.is_flex_layout = true`.
   - Open details on a `Container` (root) populates `details_context.is_flex_layout = false`.
   - Cycling with 1 visible tab is a no-op (forward AND backward end on Properties).
   - Cycling with 2 visible tabs (Properties + RenderObject) wraps between the two.
   - Cycling with 3 visible tabs (Properties + RenderObject + FlexExplorer) cycles through all three.
   - Cycling skips FlexExplorer when only Properties + RenderObject are visible.
   - `clamp_details_tab` call from `handle_inspector_properties_fetched` snaps from RenderObject → Properties when the fetched response yields empty `render_properties`.
7. Existing tests `handle_cycle_tab_forward_advances_through_three_tabs_with_wrap` and `..backward..` are updated to populate `details_context` and `render_properties` so all three tabs are visible (preserving the original assertion intent).
8. `cargo fmt --all -- --check && cargo check --workspace --all-targets && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings` all pass.

### Testing

Update the two existing tab-cycling tests (around `handler/devtools/inspector.rs:2262–2326`):

```rust
#[test]
fn handle_cycle_tab_forward_advances_through_three_tabs_with_wrap() {
    let mut state = AppState::default();
    // Phase 3 update: populate state to make all three tabs visible.
    {
        let inspector = &mut state.devtools_view_state.inspector;
        inspector.details_open = true;
        inspector.details_tab = DetailsTab::Properties;
        inspector.render_properties = vec![DiagnosticsNode {
            description: "RenderFlex".into(),
            ..Default::default()
        }];
        inspector.details_context = DetailsContext {
            is_flex_layout: true,
            parent_type: None,
        };
    }
    handle_cycle_tab(&mut state, true);
    assert_eq!(state.devtools_view_state.inspector.details_tab, DetailsTab::RenderObject);
    handle_cycle_tab(&mut state, true);
    assert_eq!(state.devtools_view_state.inspector.details_tab, DetailsTab::FlexExplorer);
    handle_cycle_tab(&mut state, true);
    assert_eq!(state.devtools_view_state.inspector.details_tab, DetailsTab::Properties);
}

#[test]
fn handle_cycle_tab_backward_advances_through_three_tabs_with_wrap() {
    let mut state = AppState::default();
    {
        let inspector = &mut state.devtools_view_state.inspector;
        inspector.details_open = true;
        inspector.details_tab = DetailsTab::Properties;
        inspector.render_properties = vec![DiagnosticsNode {
            description: "RenderFlex".into(),
            ..Default::default()
        }];
        inspector.details_context = DetailsContext {
            is_flex_layout: true,
            parent_type: None,
        };
    }
    handle_cycle_tab(&mut state, false);
    assert_eq!(state.devtools_view_state.inspector.details_tab, DetailsTab::FlexExplorer);
    handle_cycle_tab(&mut state, false);
    assert_eq!(state.devtools_view_state.inspector.details_tab, DetailsTab::RenderObject);
    handle_cycle_tab(&mut state, false);
    assert_eq!(state.devtools_view_state.inspector.details_tab, DetailsTab::Properties);
}
```

Add new tests for the partial-visibility cases:

```rust
#[test]
fn handle_cycle_tab_is_noop_when_only_properties_visible() {
    let mut state = AppState::default();
    state.devtools_view_state.inspector.details_open = true;
    state.devtools_view_state.inspector.details_tab = DetailsTab::Properties;
    // Default: render_properties empty, details_context default → 1 visible tab.
    handle_cycle_tab(&mut state, true);
    assert_eq!(state.devtools_view_state.inspector.details_tab, DetailsTab::Properties);
    handle_cycle_tab(&mut state, false);
    assert_eq!(state.devtools_view_state.inspector.details_tab, DetailsTab::Properties);
}

#[test]
fn handle_cycle_tab_skips_flex_explorer_when_hidden() {
    let mut state = AppState::default();
    {
        let inspector = &mut state.devtools_view_state.inspector;
        inspector.details_open = true;
        inspector.details_tab = DetailsTab::Properties;
        inspector.render_properties = vec![DiagnosticsNode {
            description: "RenderFlex".into(),
            ..Default::default()
        }];
        // details_context default → is_flex_layout = false → FlexExplorer hidden
    }
    handle_cycle_tab(&mut state, true);
    assert_eq!(state.devtools_view_state.inspector.details_tab, DetailsTab::RenderObject);
    handle_cycle_tab(&mut state, true);
    // Skip FlexExplorer, wrap to Properties.
    assert_eq!(state.devtools_view_state.inspector.details_tab, DetailsTab::Properties);
}

#[test]
fn handle_cycle_tab_skips_render_object_when_hidden() {
    let mut state = AppState::default();
    {
        let inspector = &mut state.devtools_view_state.inspector;
        inspector.details_open = true;
        inspector.details_tab = DetailsTab::Properties;
        // render_properties empty → RenderObject hidden
        inspector.details_context = DetailsContext {
            is_flex_layout: true,
            parent_type: None,
        };
    }
    handle_cycle_tab(&mut state, true);
    assert_eq!(state.devtools_view_state.inspector.details_tab, DetailsTab::FlexExplorer);
    handle_cycle_tab(&mut state, true);
    // Skip RenderObject, wrap to Properties.
    assert_eq!(state.devtools_view_state.inspector.details_tab, DetailsTab::Properties);
}

#[test]
fn handle_open_details_populates_details_context_for_column_widget() {
    let mut state = AppState::default();
    let column = DiagnosticsNode {
        description: "Column".into(),
        value_id: Some("col-id".into()),
        ..Default::default()
    };
    {
        let inspector = &mut state.devtools_view_state.inspector;
        inspector.root = Some(column);
        inspector.selected_index = 0;
    }
    handle_open_details(&mut state);
    let ctx = &state.devtools_view_state.inspector.details_context;
    assert!(ctx.is_flex_layout, "Column should be is_flex_layout=true");
}

#[test]
fn handle_open_details_populates_details_context_for_non_flex_root() {
    let mut state = AppState::default();
    let container = DiagnosticsNode {
        description: "Container".into(),
        value_id: Some("c-id".into()),
        ..Default::default()
    };
    {
        let inspector = &mut state.devtools_view_state.inspector;
        inspector.root = Some(container);
        inspector.selected_index = 0;
    }
    handle_open_details(&mut state);
    let ctx = &state.devtools_view_state.inspector.details_context;
    assert!(!ctx.is_flex_layout, "Container with no parent should be is_flex_layout=false");
}

#[test]
fn handle_inspector_properties_fetched_clamps_active_tab_to_properties_when_render_object_disappears() {
    let mut state = AppState::default();
    {
        let inspector = &mut state.devtools_view_state.inspector;
        inspector.details_open = true;
        inspector.details_tab = DetailsTab::RenderObject;
        inspector.details_node_id = Some("node-id".into());
        inspector.pending_properties_node_id = Some("node-id".into());
        // Previously had render_properties → RenderObject was visible.
        inspector.render_properties = vec![DiagnosticsNode {
            description: "RenderOld".into(),
            ..Default::default()
        }];
    }
    // Simulate a successful fetch that returns no render-object properties
    // (e.g. user re-selected a widget with no RenderObject).
    handle_inspector_properties_fetched(
        &mut state,
        "node-id".to_string(),
        vec![], // widget_props
        vec![], // render_props — empty triggers clamp
    );
    assert_eq!(state.devtools_view_state.inspector.details_tab, DetailsTab::Properties);
}
```

The exact handler signature for `handle_inspector_properties_fetched` may differ — adapt the test to match the canonical signature found at `inspector.rs:401-437`.

### Notes

- The Phase 2-followup task 02 ("handler-stale-guard-unification") modified `handle_inspector_properties_fetched` to unify stale-guard on `state.details_node_id`. Verify the current shape before adding the clamp call — the response may now carry a `node_id` parameter (`Phase 2-followup task 02 scope expansion`). Whatever the post-followup shape, place `clamp_details_tab()` AFTER all the existing writes and stale-guard branches, so it runs only when the fetch result was actually applied to state.
- Do NOT add a clamp call inside `handle_open_details` itself. The handler always sets `details_tab = Properties` directly, and Properties is always visible — clamping is redundant and adds noise.
- Do NOT add a clamp call in `handle_close_details`. The renderer never sees state with `details_open == false`; clamping there is pointless.
- Do NOT modify `DetailsTab::next()` / `DetailsTab::prev()` in `state.rs`. They are unused by Phase 3 but may be referenced by older tests or future code; leaving them avoids unnecessary churn.
- The "Defensive: visible_tabs always returns at least [Properties]" branch in `handle_cycle_tab` should never fire in practice. If `clippy` warns about it as unreachable, leave it — defensive against future regressions in `visible_tabs`.
- After this task, `handle_open_details`'s line count grows by ~8 lines. `handle_cycle_tab` grows from ~12 lines to ~25 lines. `handle_inspector_properties_fetched` grows by 1 line. The file size stays well within the 500-line warn threshold (file is 2,921 lines total but distributed across many handlers).
