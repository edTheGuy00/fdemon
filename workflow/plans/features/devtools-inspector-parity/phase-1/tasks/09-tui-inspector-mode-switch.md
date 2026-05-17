## Task: Switch `inspector/mod.rs` between tree-mode and details-mode

**Objective**: Make the `WidgetInspector::render` function branch on `inspector_state.details_open`: when `false`, render the existing tree + layout panel split (current behavior); when `true`, render tree + details panel (the new tabbed view from task 08).

**Depends on**: 08-tui-details-tabs

**Estimated Time**: ~2 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-tui/src/widgets/devtools/inspector/mod.rs`

**Files Read (Dependencies):**
- `crates/fdemon-tui/src/widgets/devtools/inspector/details/mod.rs` (from task 08).
- `crates/fdemon-app/src/state.rs` (to read `details_open`).
- `crates/fdemon-tui/src/widgets/devtools/inspector/layout_panel.rs` (unchanged caller).
- `crates/fdemon-tui/src/widgets/devtools/inspector/tree_panel.rs` (unchanged caller).

### Details

#### 1. Add the `details` module declaration

At the top of `inspector/mod.rs`, declare the new sub-module:

```rust
mod details;
```

#### 2. Branch on `details_open`

The current render path (around `mod.rs:201–223`, the "layout panel split logic") splits the area horizontally (50/50) or vertically depending on width, then calls `render_tree_panel` and `render_layout_panel`. Adapt this so:

```rust
fn render(/* params */) {
    // existing tree + right-pane split layout calculation
    let (tree_area, right_area) = compute_split(area);

    self.render_tree_panel(tree_area, buf, ...);

    if self.inspector_state.details_open {
        self.render_details_panel(right_area, buf);
    } else {
        self.render_layout_panel(right_area, buf, visible, selected);
    }
}
```

Verify the existing function names and signatures with a fresh read of mod.rs at editing time — they may have evolved since planning. The key change is the `if details_open` branch in the right-pane render call.

#### 3. Adjust the `visible` parameter

After task 07, `render_tree_panel_inner` no longer needs the `visible` parameter (it builds rows internally). The wrapping `render_tree_panel` may also need adjustment. Two options:

a. Drop the `visible` parameter throughout the call chain (cleaner).
b. Keep it for backwards-compatible argument list (simpler).

**Recommended: (a)** — drop the now-unused parameter, since this task is already restructuring the render entry point. The change cascades to `visible: &[(...)]` being removed from the function signature. Update the in-file tests at `tree_panel.rs` / `tests.rs` accordingly if any pass an empty Vec.

(If task 07 has already merged and left `_visible` as an `#[allow(unused_variables)]` placeholder, this task removes it.)

#### 4. Pass the `inspector_state` to `render_details_panel`

`render_details_panel` is defined in task 08 as a method on `WidgetInspector`. The struct already holds `inspector_state`, so no extra plumbing needed. Just call `self.render_details_panel(right_area, buf)`.

#### 5. Mouse region behavior

When details is open, the tree must still allow row clicks to MOVE selection (or — given the "frozen selection while details open" decision — those clicks should be SUPPRESSED). The current `render_tree_panel_inner` registers row + glyph click regions unconditionally.

Decision: pass `None` for `MouseCtx` to `render_tree_panel_inner` when `details_open == true` (suppresses click-region registration in the tree). This mirrors the renderer-level suppression pattern documented in CODE_STANDARDS.md "Modal Precedence and Sub-Modal Gates". The user can still see the tree (and the highlighted row) but cannot interact with it via mouse until they press Esc.

Update the call:

```rust
let tree_ctx = if self.inspector_state.details_open { None } else { ctx.as_deref_mut() };
self.render_tree_panel(tree_area, buf, /* …, */ tree_ctx);
```

#### 6. Tests

Add to `tests.rs`:

- `mod_switches_to_details_panel_when_details_open`.
- `mod_renders_layout_panel_when_details_closed`.
- `mod_suppresses_tree_mouse_regions_when_details_open`.
- `mod_passes_mouse_regions_to_tree_when_details_closed` (regression guard for existing behavior).

### Acceptance Criteria

1. Toggling `inspector_state.details_open` between renders switches the right pane between layout and details views.
2. The tree on the left remains visible and highlighted in both modes.
3. Tree mouse-region registration is suppressed when details is open.
4. Existing tree-mode tests in `tests.rs` continue to pass.
5. `cargo test -p fdemon-tui` passes with new tests.
6. `cargo clippy -p fdemon-tui --all-targets -- -D warnings` passes.

### Testing

```rust
#[test]
fn mod_switches_to_details_panel_when_details_open() {
    let mut state = make_inspector_state_with_tree();
    state.devtools_view_state.inspector.details_open = true;
    let widget = WidgetInspector::new(&state);
    let mut buf = Buffer::empty(Rect::new(0, 0, 120, 30));
    widget.render(buf.area, &mut buf, None);
    // Right half should contain the tab strip — look for one of the tab labels.
    let s = buf_to_string(&buf);
    assert!(s.contains("Widget properties"));
}
```

### Notes

- This task is the integration point. If something goes wrong with the details rendering (e.g., empty buffer, layout glitches), task 09 is where to debug.
- The split-logic constants (wide-mode threshold, narrow-mode threshold) at mod.rs:201–223 are unchanged.
- Do not modify `layout_panel.rs`. It must continue to work for tree mode.
- After this task lands, the full Phase 1 user journey works: navigate the tree, press Enter, see the tab strip + Properties tab content, press Tab to cycle, press Esc to close.

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
