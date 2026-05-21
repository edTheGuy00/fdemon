## Task: Add `details_context`, `visible_tabs()`, and `clamp_details_tab()` to `InspectorState`

**Objective**: Expose the visible-tab list on `InspectorState` so the handler and TUI renderer can consult a single source of truth for "which tabs should be drawn / cycled." Stores the precomputed `DetailsContext` (from task 01) on the state, derives `visible_tabs()` from it + `render_properties`, and provides `clamp_details_tab()` for post-state-change consistency.

**Depends on**: Task 01 (`DetailsContext`, `compute_details_context`, `parent_of` from `fdemon-core/src/widget_tree.rs`)

**Estimated Time**: 2–3 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/state.rs`

**Files Read (Dependencies):**
- `crates/fdemon-core/src/widget_tree.rs` — `DetailsContext` (from task 01)
- Existing `InspectorState` definition + `reset()` + `reset_details_and_groups()` at lines 202–483

### Details

#### Background

Phase 3 makes Inspector Details tab visibility data-driven. The handler (task 03) and the TUI renderer (task 04) both need to ask "which tabs are visible right now?" — sharing a single state-side method ensures they never diverge.

Visibility rules (parent PLAN §5.4):

- `Properties` — always.
- `RenderObject` — iff `!render_properties.is_empty()`.
- `FlexExplorer` — iff `details_context.is_flex_layout`.

`DetailsContext` is computed at `handle_open_details` time (task 03), not per render. This caches the tree walk and is invalidated only by reset / next open.

#### 1. Import the new core types

At the top of `crates/fdemon-app/src/state.rs` (or wherever `DiagnosticsNode` is already imported):

```rust
use fdemon_core::widget_tree::{DetailsContext, DiagnosticsNode, /* existing imports... */};
```

#### 2. Add `details_context` field to `InspectorState`

Add to the `InspectorState` struct (around lines 322–334, in the "Details view" block alongside `details_open`, `details_tab`, `details_node_id`):

```rust
    // ── Details view ──────────────────────────────────────────────────────────
    /// `true` when the user has opened the Details view (Enter pressed).
    pub details_open: bool,

    /// Which tab is currently active in the Details view.
    pub details_tab: DetailsTab,

    /// `value_id` of the widget whose details are currently displayed.
    pub details_node_id: Option<String>,

    /// Cached tree-derived predicates for the open details session.
    ///
    /// Populated by `handle_open_details` via
    /// [`fdemon_core::widget_tree::compute_details_context`]. Used by
    /// [`Self::visible_tabs`] to decide which tabs render. Cleared by
    /// [`Self::reset`] and [`Self::reset_details_and_groups`]; overwritten on
    /// every successful `handle_open_details`.
    ///
    /// Default value (`DetailsContext::default()`) is harmless because
    /// `visible_tabs` is only consumed while `details_open == true`, and
    /// `handle_open_details` always writes here before flipping `details_open`.
    pub details_context: DetailsContext,
```

Add the field to the `impl Default for InspectorState` block (around lines 365–425) initialized as `details_context: DetailsContext::default(),`.

#### 3. Clear `details_context` in `reset()` and `reset_details_and_groups()`

`reset()` (around lines 425–456) — add `self.details_context = DetailsContext::default();` alongside the other Details-view clears.

`reset_details_and_groups()` (around lines 472–483) — same addition.

Both should clear in the existing Details-view block; preserve the existing ordering style so the diff stays small.

#### 4. Add `visible_tabs()` method

Add an `impl InspectorState` method (place it after the existing `reset_details_and_groups` if it shares an `impl` block, or in a new `impl` block near `selected_node_description`):

```rust
impl InspectorState {
    /// Return the ordered list of tabs that should be visible in the Details
    /// strip given current state.
    ///
    /// Visibility rules (DevTools parity, parent PLAN §5.4):
    /// - [`DetailsTab::Properties`] is always included.
    /// - [`DetailsTab::RenderObject`] is included iff
    ///   `!self.render_properties.is_empty()`.
    /// - [`DetailsTab::FlexExplorer`] is included iff
    ///   `self.details_context.is_flex_layout` (precomputed by
    ///   `handle_open_details` via `compute_details_context`).
    ///
    /// Returned in display order. Caller is free to assume the first element
    /// is always `Properties` and to use the order for cycling.
    ///
    /// Pure: does not walk the tree, does not allocate beyond the returned vec,
    /// and never mutates state. Safe to call from the TUI renderer.
    pub fn visible_tabs(&self) -> Vec<DetailsTab> {
        let mut tabs = Vec::with_capacity(3);
        tabs.push(DetailsTab::Properties);
        if !self.render_properties.is_empty() {
            tabs.push(DetailsTab::RenderObject);
        }
        if self.details_context.is_flex_layout {
            tabs.push(DetailsTab::FlexExplorer);
        }
        tabs
    }
}
```

Notes:
- Return type is `Vec<DetailsTab>` (not `SmallVec`) — keep dependencies stable and the allocation is trivial (≤3 entries, one allocation per render).
- The method is `pub` because the TUI renderer in another crate consumes it.

#### 5. Add `clamp_details_tab()` method

In the same `impl InspectorState` block:

```rust
    /// Ensure `self.details_tab` is in [`Self::visible_tabs`]; if not, set it
    /// to the first visible tab (always `Properties`).
    ///
    /// Call this after any state transition that may have removed the active
    /// tab from the visible set:
    /// - `handle_inspector_properties_fetched` (fetch may yield empty
    ///   `render_properties` → Render Object tab disappears).
    /// - `handle_inspector_properties_fetch_failed` (same, depending on
    ///   pre-failure cache state).
    ///
    /// `handle_open_details` already sets `details_tab = Properties` directly
    /// and does not need to call this method.
    ///
    /// `handle_close_details` does not need this — the renderer never sees
    /// state while `details_open == false`.
    pub fn clamp_details_tab(&mut self) {
        let visible = self.visible_tabs();
        if !visible.contains(&self.details_tab) {
            self.details_tab = visible
                .first()
                .copied()
                .unwrap_or(DetailsTab::Properties);
        }
    }
```

The `unwrap_or` is defensive; `visible_tabs` always contains at least `Properties`.

#### 6. Verify field ordering in `Default::default()`

After editing `Default`, run `cargo check -p fdemon-app` to ensure no field is missed. The new `details_context: DetailsContext::default()` should sit alongside `details_open: false`, `details_tab: DetailsTab::default()`, `details_node_id: None`.

### Acceptance Criteria

1. `InspectorState` has a public `details_context: DetailsContext` field, defaulting to `DetailsContext::default()`.
2. `reset()` and `reset_details_and_groups()` clear `details_context` back to default.
3. `visible_tabs()` returns:
   - `[Properties]` when `render_properties` is empty and `details_context.is_flex_layout == false`.
   - `[Properties, RenderObject]` when `render_properties` is non-empty and not flex.
   - `[Properties, FlexExplorer]` when empty `render_properties` and flex.
   - `[Properties, RenderObject, FlexExplorer]` when both conditions hold.
4. `clamp_details_tab()` snaps `details_tab` to `Properties` when the current tab is hidden; no-op when the current tab is visible.
5. New unit tests in `state.rs` cover all four `visible_tabs` permutations and `clamp_details_tab` for two hidden-tab cases.
6. `InspectorState` still derives `Debug, Clone` after adding the new field (since `DetailsContext` derives both — verified in task 01).
7. `cargo fmt --all -- --check && cargo check --workspace --all-targets && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings` all pass.

### Testing

Add to the existing `#[cfg(test)] mod tests` block in `crates/fdemon-app/src/state.rs`:

```rust
#[test]
fn visible_tabs_default_is_properties_only() {
    let state = InspectorState::default();
    assert_eq!(state.visible_tabs(), vec![DetailsTab::Properties]);
}

#[test]
fn visible_tabs_includes_render_object_when_render_properties_non_empty() {
    let state = InspectorState {
        render_properties: vec![DiagnosticsNode {
            description: "RenderFlex".into(),
            ..Default::default()
        }],
        ..Default::default()
    };
    assert_eq!(
        state.visible_tabs(),
        vec![DetailsTab::Properties, DetailsTab::RenderObject]
    );
}

#[test]
fn visible_tabs_includes_flex_explorer_when_context_is_flex_layout() {
    let state = InspectorState {
        details_context: DetailsContext {
            is_flex_layout: true,
            parent_type: None,
        },
        ..Default::default()
    };
    assert_eq!(
        state.visible_tabs(),
        vec![DetailsTab::Properties, DetailsTab::FlexExplorer]
    );
}

#[test]
fn visible_tabs_includes_all_three_when_both_conditions_hold() {
    let state = InspectorState {
        render_properties: vec![DiagnosticsNode {
            description: "RenderFlex".into(),
            ..Default::default()
        }],
        details_context: DetailsContext {
            is_flex_layout: true,
            parent_type: Some("Column".into()),
        },
        ..Default::default()
    };
    assert_eq!(
        state.visible_tabs(),
        vec![
            DetailsTab::Properties,
            DetailsTab::RenderObject,
            DetailsTab::FlexExplorer
        ]
    );
}

#[test]
fn clamp_details_tab_snaps_to_properties_when_render_object_hidden() {
    let mut state = InspectorState {
        details_tab: DetailsTab::RenderObject,
        // render_properties intentionally empty → RenderObject hidden
        ..Default::default()
    };
    state.clamp_details_tab();
    assert_eq!(state.details_tab, DetailsTab::Properties);
}

#[test]
fn clamp_details_tab_snaps_to_properties_when_flex_explorer_hidden() {
    let mut state = InspectorState {
        details_tab: DetailsTab::FlexExplorer,
        // details_context.is_flex_layout intentionally false → FlexExplorer hidden
        ..Default::default()
    };
    state.clamp_details_tab();
    assert_eq!(state.details_tab, DetailsTab::Properties);
}

#[test]
fn clamp_details_tab_noop_when_active_tab_visible() {
    let mut state = InspectorState {
        details_tab: DetailsTab::RenderObject,
        render_properties: vec![DiagnosticsNode {
            description: "RenderFlex".into(),
            ..Default::default()
        }],
        ..Default::default()
    };
    state.clamp_details_tab();
    assert_eq!(state.details_tab, DetailsTab::RenderObject);
}

#[test]
fn reset_clears_details_context() {
    let mut state = InspectorState {
        details_context: DetailsContext {
            is_flex_layout: true,
            parent_type: Some("Column".into()),
        },
        ..Default::default()
    };
    state.reset();
    assert_eq!(state.details_context, DetailsContext::default());
}

#[test]
fn reset_details_and_groups_clears_details_context() {
    let mut state = InspectorState {
        details_context: DetailsContext {
            is_flex_layout: true,
            parent_type: Some("Row".into()),
        },
        ..Default::default()
    };
    state.reset_details_and_groups();
    assert_eq!(state.details_context, DetailsContext::default());
}
```

If existing struct-literal `..Default::default()` patterns in nearby tests use different style, follow that. The Phase 2 post-merge clippy sweep mandates struct-literal init, not `let mut state = InspectorState::default(); state.x = ...;`.

### Notes

- `DetailsTab::next()` and `DetailsTab::prev()` (lines 178–193) are NOT modified by this task. They remain the unconditional 3-element cycle for backwards compatibility. Task 03's `handle_cycle_tab` will stop calling them and instead use `visible_tabs()` directly.
- The Phase 2 caching pattern (`last_fetched_properties_node_id`) is independent of `details_context` — properties caching does not invalidate `details_context`, and re-opening on the same node with a properties cache hit still re-runs `compute_details_context` in `handle_open_details` (cheap).
- If `cargo clippy` flags `Vec::with_capacity(3)` as unnecessary (it sometimes does for small vecs), feel free to simplify to plain `vec![DetailsTab::Properties]` + `push`. The performance is identical.
- Do NOT make `visible_tabs()` return `&[DetailsTab]` from a static cache — the contents are dynamic and the allocation is trivial.

---

## Completion Summary

**Status:** Done
**Branch:** feat/devtools-inspector-parity

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/state.rs` | Added `DetailsContext` import; added `details_context: DetailsContext` field to `InspectorState`; initialized in `Default`; cleared in `reset()` and `reset_details_and_groups()`; added `visible_tabs()` and `clamp_details_tab()` methods; added 8 unit tests |

### Notable Decisions/Tradeoffs

1. **Formatting fix**: The task spec showed `clamp_details_tab` with a multi-line chain (`.first() / .copied() / .unwrap_or(...)`), but `cargo fmt` collapsed it to one line. Applied rustfmt's format to pass the format check.
2. **`Vec::with_capacity(3)`**: Kept as-is; clippy did not flag it in this workspace.

### Testing Performed

- `cargo fmt --all -- --check` - Passed
- `cargo check --workspace --all-targets` - Passed
- `cargo test -p fdemon-app` - Passed (2,379 tests, 8 new)
- `cargo test --workspace` - Passed
- `cargo clippy --workspace --all-targets -- -D warnings` - Passed (0 warnings)

### Risks/Limitations

1. **None**: Pure additive change — new field, new methods, new tests. No existing behaviour changed.
