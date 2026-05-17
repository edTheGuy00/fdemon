## Task: New `inspector/details/` module — tab strip + Properties tab (populated) + Render-object & Flex-explorer tabs (stubs)

**Objective**: Add a self-contained `inspector/details/` directory in the TUI crate. The module owns the tab strip, dispatches to per-tab renderers, and provides:
- `properties_tab.rs` — adapted from the existing `layout_panel.rs` (mini box-model preview + size + constraints + flex line, plus a property-list scaffold).
- `render_object_tab.rs` — stub: centered "Coming soon — Phase 2" text.
- `flex_explorer_tab.rs` — stub: centered "Coming soon — Phase 2" text.

**Depends on**: 02-state-inspector-extensions

**Estimated Time**: 4–5 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-tui/src/widgets/devtools/inspector/details/mod.rs` **NEW**
- `crates/fdemon-tui/src/widgets/devtools/inspector/details/properties_tab.rs` **NEW**
- `crates/fdemon-tui/src/widgets/devtools/inspector/details/render_object_tab.rs` **NEW**
- `crates/fdemon-tui/src/widgets/devtools/inspector/details/flex_explorer_tab.rs` **NEW**

**Files Read (Dependencies):**
- `crates/fdemon-tui/src/widgets/devtools/inspector/layout_panel.rs` (source material for `properties_tab.rs`; existing helpers may stay there as `pub(super)`).
- `crates/fdemon-app/src/state.rs` (`DetailsTab`, `details_open`, `properties`, etc. — fields from task 02).
- `crates/fdemon-tui/src/widgets/devtools/inspector/mod.rs` (read the `WidgetInspector` struct shape).

### Details

#### 1. Module layout

```
crates/fdemon-tui/src/widgets/devtools/inspector/details/
├── mod.rs                  # Tab strip rendering + dispatch
├── properties_tab.rs       # Widget properties tab (populated)
├── render_object_tab.rs    # Stub
└── flex_explorer_tab.rs    # Stub
```

#### 2. `details/mod.rs` — tab strip + dispatch

Public surface:

```rust
//! Inspector details view — tab strip and per-tab dispatch.

use ratatui::{buffer::Buffer, layout::Rect, widgets::Block};
use fdemon_app::state::{DetailsTab, InspectorState};

use super::WidgetInspector;

mod properties_tab;
mod render_object_tab;
mod flex_explorer_tab;

impl WidgetInspector<'_> {
    /// Render the tabbed details view in `area`.
    /// Called from `inspector/mod.rs` when `inspector_state.details_open == true`.
    pub(super) fn render_details_panel(&self, area: Rect, buf: &mut Buffer) {
        // 1. Outer block + title
        // 2. Split into tab strip (height 2) + content area
        // 3. Render the three tab labels with the active tab underlined
        // 4. Dispatch on `inspector_state.details_tab` to the per-tab renderer
    }
}
```

#### 3. Tab strip rendering

Constants:

```rust
/// Height of the tab strip above the tab content.
/// 1 row for tab labels + 1 row for the underline / separator.
const TAB_STRIP_HEIGHT: u16 = 2;
```

Layout: a row of three tab labels (`Widget properties`, `Render object`, `Flex explorer`) separated by spaces; the active tab is highlighted (bg color + bold) and underlined with `━` characters in the row below. Inactive tabs get `palette::TEXT_MUTED`.

Mouse-region: each tab label can register a click region that emits `Message::DevToolsInspectorCycleTab { forward: true }` repeated as necessary, OR a more direct `Message::DevToolsInspectorSelectTab(DetailsTab)` (NEW variant — would need to coordinate with task 04). **For Phase 1, keep mouse clicks on tabs unbound** — keyboard cycling is sufficient. Add a TODO comment noting that tab-mouse-clicks are a Phase 2 polish item.

#### 4. `properties_tab.rs` — port from layout_panel.rs

The existing `layout_panel.rs` already renders:
- Selected widget name + source location.
- Box model visualization (`render_box_model`).
- Size box (`render_size_box`).
- Dimensions row (`render_dimensions_row`).
- Constraints + flex properties (`render_flex_properties`).

**Two approaches:**

a. **Move** these helpers from `layout_panel.rs` to `properties_tab.rs` and call them from BOTH places. Existing `layout_panel.rs` continues to work for tree mode (right pane when details closed) by `use super::details::properties_tab::render_box_model;`.

b. **Re-export** the existing helpers and call them from `properties_tab.rs`. Mark `render_box_model`, `render_size_box`, etc. as `pub(super)` in their parent (`inspector` module), making them visible to `details/*`.

**Recommended: (b)** — fewer line moves, less merge surface. Just bump visibility of the existing helpers and call them from the new tab.

`properties_tab.rs` then adds:
1. The mini layout preview (existing box-model rendering).
2. A property list table BELOW the layout preview, fed from `inspector_state.properties`. In Phase 1 `properties` is empty, so the table just renders an "(properties will load here in Phase 2)" placeholder. This keeps the tab structurally complete and visually equivalent to the existing layout explorer.

#### 5. `render_object_tab.rs` — stub

```rust
//! Render object tab — stub for Phase 1. Phase 2 will populate this from
//! `inspector_state.render_properties` (fetched via `getProperties` RPC).

use ratatui::{buffer::Buffer, layout::Rect, style::Style};
use crate::theme::palette;

pub(super) fn render(area: Rect, buf: &mut Buffer) {
    render_centered_text_local(area, buf, "Coming soon — Phase 2");
}

fn render_centered_text_local(area: Rect, buf: &mut Buffer, text: &str) {
    // ~5 lines: compute center coord, set_string with palette::TEXT_MUTED.
    // Inline copy of the helper from layout_panel.rs — kept here to avoid
    // bumping layout_panel.rs visibility just for stub tabs.
}
```

This avoids touching `layout_panel.rs` (which would force a write-file overlap with task 09's mod.rs branch wiring).

#### 6. `flex_explorer_tab.rs` — stub

Same pattern as `render_object_tab.rs` but with text `"Coming soon — Phase 2"`.

#### 7. Tests

Each new file gets a minimal test:

- `details/mod.rs`:
  - `tab_strip_underlines_active_tab`.
  - `tab_strip_renders_three_labels_in_order`.
- `details/properties_tab.rs`:
  - `properties_tab_renders_box_model_for_selected_widget` (verify box-model is present at the top).
  - `properties_tab_renders_property_placeholder_when_properties_empty`.
- `details/render_object_tab.rs`:
  - `render_object_stub_renders_coming_soon`.
- `details/flex_explorer_tab.rs`:
  - `flex_explorer_stub_renders_coming_soon`.

### Acceptance Criteria

1. The four new files compile and pass tests.
2. The tab strip visually matches DevTools' style (active tab underlined, inactive tabs dim).
3. The Properties tab renders the existing box-model + size + constraints, identical to today's `layout_panel.rs` content, with an extra placeholder area for the future property list.
4. The two stub tabs render a centered "Coming soon — Phase 2" message.
5. `cargo test -p fdemon-tui` passes with new tests.
6. `cargo clippy -p fdemon-tui --all-targets -- -D warnings` passes.
7. The module integration with the rest of the inspector (i.e., the call to `render_details_panel` from `mod.rs`) is left to task 09. This task can verify by adding a temporary `#[cfg(test)]` exercise that calls `render_details_panel` directly into a `Buffer`.

### Testing

```rust
#[test]
fn tab_strip_underlines_active_tab() {
    let mut state = make_state_with_details_open(DetailsTab::RenderObject);
    let widget = WidgetInspector::new(&state);
    let mut buf = Buffer::empty(Rect::new(0, 0, 80, 20));
    widget.render_details_panel(buf.area, &mut buf);
    // Find the "Render object" label; check the underline row below is `━━━` over its span.
}
```

### Notes

- **Do not** delete `layout_panel.rs` or change its public-ish surface. Tree mode keeps using it as today. Task 09 only adds a branch around the call site.
- The "Coming soon" stubs are intentional and visible — they communicate to early users that the feature is in progress. Once Phase 2 lands these files are rewritten in full.
- The tab labels are intentionally lowercased in DevTools too — match for visual parity if possible (`Widget properties`, `Render object`, `Flex explorer`).
- Keep the new `details/` directory under `inspector/` (the existing widget) rather than promoting it to a peer of `inspector/`. This keeps related code colocated and avoids touching the `widgets/devtools/mod.rs` module tree.

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
