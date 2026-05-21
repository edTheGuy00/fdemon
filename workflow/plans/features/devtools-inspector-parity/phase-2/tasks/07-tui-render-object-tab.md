## Task: Render Object tab — populated property table

**Objective**: Replace the "Coming soon — Phase 2" stub in `crates/fdemon-tui/src/widgets/devtools/inspector/details/render_object_tab.rs` with a real renderer that displays `inspector.render_properties` as a key/value table. Implements default-value visual treatment (`level == "fine"` → muted, sorted to end), filters `level == "hidden"`, and surfaces fetch loading + error states.

**Depends on**: 06 (handlers populate `inspector.render_properties`)

**Estimated Time**: 3–4 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-tui/src/widgets/devtools/inspector/details/render_object_tab.rs`

**Files Read (Dependencies):**
- `crates/fdemon-app/src/state.rs` — `InspectorState.render_properties`, `properties_loading`, `properties_error`
- `crates/fdemon-tui/src/widgets/devtools/inspector/details/mod.rs` — module convention, palette access, parent `WidgetInspector` struct (or whatever owns `render`)
- `crates/fdemon-tui/src/widgets/devtools/inspector/layout_panel.rs` — existing `format_constraint_value` helper if useful for formatting numeric values
- `crates/fdemon-core/src/widget_tree.rs` — `DiagnosticsNode.name`, `description`, `level`
- `tmp/devtools/packages/devtools_app/lib/src/screens/inspector/widget_properties/properties_view.dart:313–333` — `_filterAndSortPropertiesByLevel` algorithm

### Details

#### Current state

`render_object_tab.rs` currently is 67 lines: a `render(area, buf)` function that calls a private `render_centered_text(area, buf, "Coming soon — Phase 2")`. The render_centered_text helper is duplicated in `flex_explorer_tab.rs` (n5 from Phase 1.5). This task removes both the stub call and the local `render_centered_text` helper.

The signature of the public `render` function may need to change. Currently it takes only `area` and `buf`. To consume state, it needs the inspector state. Look at how `properties_tab.rs::render_properties_tab` is declared — it's `impl WidgetInspector` with `&self` access to `self.inspector_state`. Either:
- Move this tab's `render` into the same `WidgetInspector` impl, OR
- Pass `&InspectorState` explicitly as a parameter.

Mirror whatever `properties_tab.rs` does (research confirms it's an `impl WidgetInspector` method). Then `details/mod.rs::render_details_panel` will call it through the same dispatch path the stub used.

#### Layout

The Render Object tab occupies the entire right pane (no nested split). Within it:

```
┌─ Render Object ──────────────────────────────────────────────────────┐
│ <empty / loading / error / table>                                   │
└──────────────────────────────────────────────────────────────────────┘
```

States to handle:

1. **No `details_node_id`** — shouldn't happen if `details_open == true`, but defensively render an empty pane.
2. **`properties_loading == true` AND `render_properties.is_empty()`** — show a loading spinner row (`"Loading render-object properties..."` muted text, centered or top-left).
3. **`properties_error.is_some()`** — show the error summary + hint (use the existing `DevToolsError` rendering convention — grep for how `layout_error` is rendered to match the style).
4. **`render_properties.is_empty() && !properties_loading && properties_error.is_none()`** — show a muted line: `"No render object for this widget."` (this is the cache-hit-with-no-render-object case; e.g., a `Container` widget has no `RenderObject` property).
5. **`!render_properties.is_empty()`** — render the property table.

#### Property table rendering

Each row in the table renders one property: `name`, `description`, with a "default" badge style for `level == "fine"` entries.

Order is determined by `_filterAndSortPropertiesByLevel`:
1. Filter out properties whose `level == Some("hidden")`.
2. Stable partition: keep `level != Some("fine")` first, append `level == Some("fine")` last.

Pseudocode in Rust:

```rust
fn filtered_and_sorted<'a>(
    props: &'a [DiagnosticsNode],
) -> impl Iterator<Item = (&'a DiagnosticsNode, /* is_default */ bool)> {
    let (non_default, default): (Vec<_>, Vec<_>) = props
        .iter()
        .filter(|p| p.level.as_deref() != Some("hidden"))
        .partition(|p| p.level.as_deref() != Some("fine"));
    non_default
        .into_iter()
        .map(|p| (p, false))
        .chain(default.into_iter().map(|p| (p, true)))
}
```

Each row visual:

```
needsCompositing         false
creator                  Padding ← Container ← Scaffold
parentData               <BoxParentData: offset=Offset(0.0, 0.0)>
constraints              BoxConstraints(0.0<=w<=414.0, 0.0<=h<=896.0)
size                     Size(414.0, 600.0)
                                           ↓ default-section divider ↓
layer                    null              (muted style)
semantics node           null              (muted style)
```

Suggested column widths: name = `min(20, area.width / 3)`; description fills the remainder, truncated with ellipsis if longer than the column.

Use `palette::TEXT` for normal rows, `palette::TEXT_MUTED` for `is_default == true` rows. If there's at least one default row, draw a single muted horizontal divider line between the non-default and default sections (one row of `─` or similar).

Wrap-or-truncate policy: long descriptions (e.g. `creator` chains) get ellipsis-truncated to fit one line. Phase 3 may add a "page down" scrolling mode, but it's out of scope for Phase 2.

#### Scrolling

If the property list exceeds the visible area height, show only the top N rows that fit and append a muted `"... +N more (resize window or expand details to see)"` row at the bottom. Don't introduce scroll state in Phase 2 — Phase 3 polish.

### Acceptance Criteria

1. Selecting a `Column` widget, pressing Enter, and switching to the Render Object tab shows a populated key/value table from `inspector.render_properties` (which after task 06 includes the render-object's own properties such as `needsCompositing`, `creator`, `parentData`, `constraints`, `size`, plus flex-specific render-object properties).
2. Properties with `level == "fine"` render in `palette::TEXT_MUTED` and sort to the end of the list, after a single divider row.
3. Properties with `level == "hidden"` do not render.
4. While the fetch is in flight (`properties_loading == true && render_properties.is_empty()`), the tab shows `"Loading render-object properties..."` muted.
5. On fetch error (`properties_error.is_some()`), the tab shows the error summary and hint.
6. On a widget with no render-object properties (e.g. `Container`), the tab shows `"No render object for this widget."` muted.
7. The `render_centered_text` helper duplicated in this file (`render_object_tab.rs:17–30`) is deleted.
8. The new layout is bordered with `Borders::ALL`, title `" Render Object "`, in the existing details panel style. (Check `properties_tab.rs` / `layout_panel.rs` for the canonical border style and reuse it.)

### Testing

Add snapshot-style unit tests using ratatui's `TestBackend` (the existing inspector tests use this pattern — search for `TestBackend` in `crates/fdemon-tui/src/widgets/devtools/inspector/tests.rs`):

```rust
#[test]
fn render_object_tab_shows_loading_state() {
    let mut state = InspectorState::default();
    state.details_open = true;
    state.details_tab = DetailsTab::RenderObject;
    state.details_node_id = Some("objects/42".into());
    state.properties_loading = true;
    // render_properties empty
    let buf = render_render_object_tab(&state, (60, 10));
    assert!(buffer_to_string(&buf).contains("Loading"));
}

#[test]
fn render_object_tab_shows_error_state() {
    let mut state = InspectorState::default();
    state.details_open = true;
    state.details_tab = DetailsTab::RenderObject;
    state.details_node_id = Some("objects/42".into());
    state.properties_error = Some(DevToolsError::new("Fetch failed", "Press [r] to retry"));
    let buf = render_render_object_tab(&state, (60, 10));
    let s = buffer_to_string(&buf);
    assert!(s.contains("Fetch failed"));
    assert!(s.contains("retry"));
}

#[test]
fn render_object_tab_shows_no_render_object_message() {
    let mut state = InspectorState::default();
    state.details_open = true;
    state.details_tab = DetailsTab::RenderObject;
    state.details_node_id = Some("objects/42".into());
    // properties_loading == false, error == None, render_properties empty.
    let buf = render_render_object_tab(&state, (60, 10));
    assert!(buffer_to_string(&buf).contains("No render object"));
}

#[test]
fn render_object_tab_renders_property_rows() {
    let mut state = InspectorState::default();
    state.details_open = true;
    state.details_tab = DetailsTab::RenderObject;
    state.details_node_id = Some("objects/42".into());
    state.render_properties = vec![
        sample_node("needsCompositing", "false", None),
        sample_node("creator", "Padding ← Container", None),
        sample_node("size", "Size(414.0, 600.0)", None),
    ];
    let buf = render_render_object_tab(&state, (60, 10));
    let s = buffer_to_string(&buf);
    assert!(s.contains("needsCompositing"));
    assert!(s.contains("false"));
    assert!(s.contains("creator"));
    assert!(s.contains("size"));
}

#[test]
fn render_object_tab_sorts_default_level_to_end() {
    let mut state = InspectorState::default();
    state.details_open = true;
    state.details_tab = DetailsTab::RenderObject;
    state.details_node_id = Some("objects/42".into());
    state.render_properties = vec![
        sample_node_with_level("layer", "null", "fine"),
        sample_node("needsCompositing", "false", None),
        sample_node_with_level("semantics", "null", "fine"),
    ];
    let buf = render_render_object_tab(&state, (80, 10));
    let s = buffer_to_string(&buf);
    let pos_compositing = s.find("needsCompositing").unwrap();
    let pos_layer = s.find("layer").unwrap();
    let pos_semantics = s.find("semantics").unwrap();
    assert!(pos_compositing < pos_layer, "non-default should appear before default");
    assert!(pos_compositing < pos_semantics);
}

#[test]
fn render_object_tab_filters_hidden_level() {
    let mut state = InspectorState::default();
    state.details_open = true;
    state.details_tab = DetailsTab::RenderObject;
    state.details_node_id = Some("objects/42".into());
    state.render_properties = vec![
        sample_node("visible", "yes", None),
        sample_node_with_level("hiddenProp", "secret", "hidden"),
    ];
    let buf = render_render_object_tab(&state, (60, 10));
    let s = buffer_to_string(&buf);
    assert!(s.contains("visible"));
    assert!(!s.contains("hiddenProp"));
}
```

### Notes

- Use `DiagnosticsNode.name` (Option) for the left column when present; fall back to a derived label from `description` if `name` is `None`. Most render-object property nodes carry an explicit `name`.
- Use `DiagnosticsNode.description` for the right column.
- Phase 3 will add a per-property expand/contract for long values (e.g. expand the `creator` chain). For now, ellipsis-truncate.
- The "default" badge in DevTools is purely visual (muted text). Phase 2 mimics this with `palette::TEXT_MUTED`. A future enhancement could add a literal `[default]` text suffix; Phase 2 keeps the visual quiet.
- This task does NOT touch the tab strip / mode switch / footer hints. Those continue to work as Phase 1 left them.
- The local-file line budget should stay well under 500 with this rewrite (~150–200 lines including tests).

---

## Completion Summary

**Status:** Done
**Branch:** worktree-agent-a24489ac62fd04712

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-tui/src/widgets/devtools/inspector/details/render_object_tab.rs` | Full rewrite: replaced Phase 1 stub with `impl WidgetInspector` method `render_render_object_tab`. Implements all 5 render states (no node, loading, error, empty, table). Added `filtered_and_sorted`, `truncate_to`, `render_property_table`, `render_property_row`, `render_muted_text`, `render_error` helpers. Added 12 unit tests covering all acceptance criteria. |
| `crates/fdemon-tui/src/widgets/devtools/inspector/details/mod.rs` | Updated `render_details_panel` dispatch to call `self.render_render_object_tab(...)` instead of the old free function. Updated the now-stale "Coming soon" test to reflect Phase 2 behavior. |

### Notable Decisions/Tradeoffs

1. **Method vs free function**: Implemented as `impl WidgetInspector` method (mirroring `properties_tab.rs`) so the renderer has access to `self.inspector_state` without an extra parameter. The call site in `details/mod.rs` is clean: `self.render_render_object_tab(content_area, buf)`.

2. **`palette::TEXT` → `palette::TEXT_PRIMARY`**: The task description mentions `palette::TEXT` but the actual palette only defines `TEXT_PRIMARY`, `TEXT_SECONDARY`, `TEXT_MUTED`, and `TEXT_BRIGHT`. Used `TEXT_PRIMARY` for normal rows.

3. **`render_centered_text` helper removed**: The Phase 1 stub's `render_centered_text` helper (acceptance criterion #7) is gone from `render_object_tab.rs`. Replaced by `render_muted_text` (centred) for loading/empty states.

4. **Overflow indicator**: Rather than silently clipping, the last visible row shows `"... +N more (resize window or expand details to see)"` when properties exceed the available height (Phase 3 scroll is out of scope).

5. **Divider only when default section exists**: The muted `─` divider row is drawn only when there is at least one `level == "fine"` property, matching DevTools visual convention.

### Testing Performed

- `cargo check -p fdemon-tui` — Passed (no errors)
- `cargo clippy -p fdemon-tui` — Passed (no warnings)
- `cargo fmt --all` — Applied (minor formatting adjustments)
- `cargo test -p fdemon-tui --lib render_object` — Passed (12 tests)
- `cargo test --workspace --lib` — Passed (5,546 tests total, 0 failures)

### Risks/Limitations

1. **Phase 3 scrolling**: When property count exceeds visible height, only the first N rows are shown plus an overflow indicator. Phase 3 will add proper scroll state.
2. **Name column from `description` fallback**: When `node.name` is `None`, the name column falls back to `node.description`. This can produce awkward display for nodes where description is already the full value (rare for render-object property nodes).
