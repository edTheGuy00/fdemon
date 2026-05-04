## Task: Inspector Tree Row + Glyph Region Recording

**Objective**: Replace the no-op delegation in `widgets::devtools::inspector::render_with_regions` (created in Task 02) with real rendering that records two click regions per visible tree row: a wide row region (`Emit(Message::DevToolsInspectorSelectRow { index })`) and a narrow leading-glyph region (`Emit(Message::DevToolsInspectorToggleNode { index })`). The glyph region is pushed *after* the row region so the registry's last-pushed-wins-at-same-z invariant (verified by Phase 3 unit test) makes the glyph region the hit on the glyph cell.

**Depends on**: Task 01 (for `Message::DevToolsInspectorSelectRow`, `DevToolsInspectorToggleNode`), Task 02 (for the sister-function scaffold)

**Estimated Time**: 1 hour

### Scope

**Files Modified (Write):**
- `crates/fdemon-tui/src/widgets/devtools/inspector/mod.rs`: Replace the body of `render_with_regions` so it threads `Option<&mut MouseCtx<'_>>` into a new `render_tree_panel_with_regions` helper. Reorganise the existing `render_tree` dispatch so both `Widget::render` and `render_with_regions` share the per-row loop.
- `crates/fdemon-tui/src/widgets/devtools/inspector/tree_panel.rs`: Rename `render_tree_panel` to `render_tree_panel_inner` (private), accept `Option<&mut MouseCtx>` parameter, register the two regions per row inside the existing `for (offset, ...) in visible[start..end].iter().enumerate()` loop. Keep behaviour identical when `ctx` is `None`.

**Files Read (Dependencies):**
- `crates/fdemon-app/src/message.rs::Message::DevToolsInspectorSelectRow`, `DevToolsInspectorToggleNode`
- `crates/fdemon-app/src/mouse_regions.rs::MouseAction::emit`, `MouseRect`
- `crates/fdemon-tui/src/render/mod.rs::MouseCtx`

### Details

#### Two-region pattern

For each visible tree row:

```rust
// Whole-row click region (left-click → select).
let row_rect = MouseRect::new(tree_inner.x, y, tree_inner.width, 1);
if row_rect.width > 0 && row_rect.height > 0 {
    if let Some(c) = ctx.as_deref_mut() {
        c.click(
            row_rect,
            MouseAction::emit(Message::DevToolsInspectorSelectRow { index: vis_index }),
        );
    }
}

// Glyph click region (left-click on ▶/▼/● → select + toggle).
//
// Pushed AFTER the row region so the registry's last-pushed-wins-at-same-z
// invariant (Phase 3 unit test `test_last_pushed_wins_at_same_z`) makes the
// glyph rect win on overlap.
let glyph_x = tree_inner.x.saturating_add((depth * 2) as u16);
let glyph_rect = MouseRect::new(glyph_x, y, 1, 1);
if let Some(c) = ctx.as_deref_mut() {
    c.click(
        glyph_rect,
        MouseAction::emit(Message::DevToolsInspectorToggleNode { index: vis_index }),
    );
}
```

The `vis_index = start + offset` mapping is already computed in the existing loop.

#### `glyph_x` computation

The current rendering code at `tree_panel.rs:60-63`:

```rust
let indent = "  ".repeat(*depth);
let expand_icon = self.expand_icon(node);
let name = node.display_name();
let line = format!("{indent}{expand_icon} {name}");
```

The glyph occupies cell `tree_inner.x + 2*depth`. (Two spaces per indent level, then the glyph.) Compute as:

```rust
let glyph_x = tree_inner.x.saturating_add((*depth as u16).saturating_mul(2));
```

The glyph is always 1 column wide (`▶` / `▼` / `●` are single-cell unicode glyphs in monospaced fonts). If a future change introduces multi-cell glyphs, this constant must move.

#### `render_with_regions` body

```rust
pub fn render_with_regions(
    area: Rect,
    buf: &mut Buffer,
    widget: WidgetInspector<'_>,
    ctx: Option<&mut MouseCtx<'_>>,
) {
    // Replicate the dispatch from Widget::render exactly, but pass ctx
    // into the only branch that has clickable rows.
    if !widget.is_connected() {
        widget.render_disconnected(area, buf);
        return;
    }
    if widget.is_loading() {
        widget.render_loading(area, buf);
        return;
    }
    if let Some(error) = widget.error() {
        widget.render_error_box(area, buf, error);
        return;
    }
    let visible = widget.visible_nodes_borrow();
    if visible.is_empty() {
        widget.render_empty(area, buf);
        return;
    }
    widget.render_tree_with_regions(area, buf, &visible, ctx);
}
```

(Adjust accessor names to whatever `WidgetInspector` already exposes; if methods are private, expose `pub(super)` accessors as needed.)

#### `render_tree_with_regions` and `render_tree_panel_inner`

`WidgetInspector::render_tree` already splits the area into a tree column + a layout column. The tree column uses `render_tree_panel`; the layout column uses `render_layout_panel`. Only the tree column is clickable in v1.

```rust
impl WidgetInspector<'_> {
    pub(super) fn render_tree_with_regions(
        &self,
        area: Rect,
        buf: &mut Buffer,
        visible: &[(&DiagnosticsNode, usize)],
        ctx: Option<&mut MouseCtx<'_>>,
    ) {
        // Same layout split as `render_tree`.
        let chunks = /* … existing split … */;
        self.render_tree_panel_inner(chunks[0], buf, visible, self.selected_index(), ctx);
        // Layout panel renders without ctx — Phase 5 may add clickable buttons.
        self.render_layout_panel(chunks[1], buf);
    }
}
```

In `tree_panel.rs`:

```rust
impl WidgetInspector<'_> {
    pub(super) fn render_tree_panel_inner(
        &self,
        area: Rect,
        buf: &mut Buffer,
        visible: &[(&DiagnosticsNode, usize)],
        selected: usize,
        mut ctx: Option<&mut MouseCtx<'_>>,
    ) {
        // (existing block setup unchanged)

        for (offset, (node, depth)) in visible[start..end].iter().enumerate() {
            let y = tree_inner.y + offset as u16;
            if y >= tree_inner.bottom() {
                break;
            }
            let vis_index = start + offset;

            // (existing rendering unchanged)

            // ── Phase 4: register click regions ─────────────────────────
            if let Some(c) = ctx.as_deref_mut() {
                let row_rect = MouseRect::new(tree_inner.x, y, tree_inner.width, 1);
                if row_rect.width > 0 && row_rect.height > 0 {
                    c.click(
                        row_rect,
                        MouseAction::emit(Message::DevToolsInspectorSelectRow {
                            index: vis_index,
                        }),
                    );
                }
                let glyph_x = tree_inner.x.saturating_add((*depth as u16).saturating_mul(2));
                if glyph_x < tree_inner.right() {
                    let glyph_rect = MouseRect::new(glyph_x, y, 1, 1);
                    c.click(
                        glyph_rect,
                        MouseAction::emit(Message::DevToolsInspectorToggleNode {
                            index: vis_index,
                        }),
                    );
                }
            }
        }

        // (existing scroll indicator block unchanged)
    }

    // Keep the original `render_tree_panel` as a no-ctx delegator so other
    // callers (if any) keep working. Or, if it has no other callers, delete
    // it and have `Widget::render` go through `render_tree_panel_inner` with
    // ctx = None.
    pub(super) fn render_tree_panel(
        &self,
        area: Rect,
        buf: &mut Buffer,
        visible: &[(&DiagnosticsNode, usize)],
        selected: usize,
    ) {
        self.render_tree_panel_inner(area, buf, visible, selected, None);
    }
}
```

### Acceptance Criteria

1. `render_with_regions` produces identical visible output to the existing `Widget::render` for the inspector panel — verified by an existing snapshot/text-buffer test that diffs the buffer before-and-after.
2. With `Some(ctx)` passed in, the registry contains exactly `2 * visible_rows_in_viewport` regions (one row + one glyph per visible row).
3. Row regions emit `Message::DevToolsInspectorSelectRow { index: vis_index }`; glyph regions emit `Message::DevToolsInspectorToggleNode { index: vis_index }`.
4. Rect for row region: `MouseRect::new(tree_inner.x, y, tree_inner.width, 1)`.
5. Rect for glyph region: `MouseRect::new(tree_inner.x + 2*depth, y, 1, 1)`.
6. With `None` passed in (legacy `Widget::render` path), no regions are registered — pre-existing tests continue to pass.
7. Glyph region is registered AFTER the row region in the builder — so on the glyph cell, hit-test's last-pushed-wins-at-same-z invariant returns the glyph region.
8. Empty rect guard: if `tree_inner.width == 0` or the glyph rect overflows `tree_inner.right()`, the region is skipped.
9. `cargo test --workspace`, `cargo fmt`, `cargo clippy -- -D warnings`, `cargo check` pass.
10. ≥ 2 new unit tests in `widgets/devtools/inspector/tests.rs` covering row count and glyph-priority resolution.

### Testing

```rust
#[test]
fn inspector_records_row_and_glyph_regions_per_visible_row() {
    use fdemon_app::message::Message;
    use fdemon_app::{MouseRegions, MouseAction};
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;
    use crate::render::MouseCtx;

    // Tree with 5 nodes (use existing make_tree helper or inline).
    let inspector_state = make_state_with_5_node_tree();
    let widget = WidgetInspector::new(&inspector_state, true, &VmConnectionStatus::Connected);

    let mut regions = MouseRegions::default();
    let area = Rect::new(0, 0, 80, 24);
    let mut buf = Buffer::empty(area);
    {
        let builder = regions.builder();
        let mut ctx = MouseCtx::new(builder);
        super::render_with_regions(area, &mut buf, widget, Some(&mut ctx));
    }

    let select_count = regions
        .iter()
        .filter(|e| matches!(
            e.on_left.as_ref().and_then(|a| a.as_emit()),
            Some(Message::DevToolsInspectorSelectRow { .. })
        ))
        .count();
    let toggle_count = regions
        .iter()
        .filter(|e| matches!(
            e.on_left.as_ref().and_then(|a| a.as_emit()),
            Some(Message::DevToolsInspectorToggleNode { .. })
        ))
        .count();

    assert_eq!(select_count, 5);
    assert_eq!(toggle_count, 5);
}

#[test]
fn glyph_region_wins_over_row_region_at_glyph_cell() {
    // Render a single-row tree at depth 0 (glyph at x=0), then hit-test
    // at (0, tree_inner.y) and assert the result is ToggleNode, not SelectRow.
    let inspector_state = make_state_with_single_root();
    let widget = WidgetInspector::new(&inspector_state, true, &VmConnectionStatus::Connected);

    let mut regions = MouseRegions::default();
    let area = Rect::new(0, 0, 80, 24);
    let mut buf = Buffer::empty(area);
    {
        let builder = regions.builder();
        let mut ctx = MouseCtx::new(builder);
        super::render_with_regions(area, &mut buf, widget, Some(&mut ctx));
    }

    // Glyph cell at (tree_inner.x + 0, tree_inner.y) = first row.
    // Determining tree_inner.x exactly requires knowing the panel split — for
    // a smoke check, hit-test (1, 4) which is inside the tree column.
    let result = regions.hit_test(1, 4, fdemon_app::input_mouse::MouseButton::Left);
    let action = result
        .and_then(|e| e.on_left.as_ref())
        .map(|a| a.resolve(1, 4));
    assert!(matches!(
        action,
        Some(Message::DevToolsInspectorToggleNode { index: 0 })
    ));
}
```

### Notes

- **Why two separate regions instead of one EmitWithCoord.** The action shape `Emit(Message)` carries the index inline; the registry never needs to know x to resolve the message. Two flat regions are simpler than one wide region with a closure that switches on x.
- **Last-pushed-wins is the precedence rule, not z-index.** Phase 3 reserved `z_index = 1` for modal overlays. Within a single render of the same panel, all regions live at `z_index = 0`, and tie-breaking falls back to insertion order. We rely on this for glyph-vs-row precedence; future refactors must preserve the push order.
- **Why not a smaller (width+1) glyph rect that covers `▶ ` (glyph + space).** The trailing space is part of the row content (visually associated with the name). A user clicking the space probably wants to select, not toggle. Keep the glyph rect narrow (1 cell).
- **`render_tree_panel` callers.** A grep before refactoring confirms `render_tree_panel` is only called inside `WidgetInspector`. If that's still true, delete the old function and have `Widget::render` go through `render_tree_panel_inner` with `ctx: None`. If a test or external caller exists, keep the wrapper.
- **No region for the layout / properties side panel.** The layout panel shows the selected node's geometry; clicks there do nothing in v1. Phase 5 may add interactivity (e.g., copy-to-clipboard buttons).
- **Scroll indicator at the right edge.** The `█` thumb is render-only; not clickable in v1. Phase 5 may add drag-to-scroll.
- **Empty-tree, error, loading, disconnected branches** all return before the loop. The early-return path doesn't push regions, which is correct — there's nothing to click.
