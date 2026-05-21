## Task: Properties tab — populate the property list section

**Objective**: Replace the Phase 1 placeholder `"(properties will load here in Phase 2)"` in `crates/fdemon-tui/src/widgets/devtools/inspector/details/properties_tab.rs` with a real list rendered from `inspector.properties`. The existing layout/box-model preview above the list is kept unchanged.

**Depends on**: 06 (handlers populate `inspector.properties`)

**Estimated Time**: 2–3 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-tui/src/widgets/devtools/inspector/details/properties_tab.rs`

**Files Read (Dependencies):**
- `crates/fdemon-app/src/state.rs` — `InspectorState.properties`, `properties_loading`, `properties_error`
- `crates/fdemon-tui/src/widgets/devtools/inspector/layout_panel.rs` — existing layout preview helpers
- `crates/fdemon-tui/src/widgets/devtools/inspector/details/render_object_tab.rs` (task 07 output) — for matching the property-row visual style and the `filtered_and_sorted` helper

### Details

#### Current state

`properties_tab.rs` (227 lines) has two halves:

1. **Top half (lines 38–72)**: delegates to `render_layout_panel` for the box-model + dimensions + flex preview. **Kept as-is** — this is the layout preview lifted from Phase 1.
2. **Bottom half (lines 79–112, `render_property_list_placeholder`)**: a `Borders::TOP` block titled `" Properties "` containing a single muted line `"(properties will load here in Phase 2)"`. **Replaced** by this task.

The split logic (line 71-ish): if total height < `MIN_LAYOUT_PREVIEW_HEIGHT + PROPERTY_LIST_HEIGHT` (8 + 3 = 11), the entire area goes to `render_layout_panel`. Above that threshold, `Min(8)` for layout, `Length(3)` for placeholder.

**Update the split for Phase 2**:
- Rename `PROPERTY_LIST_HEIGHT` → `MIN_PROPERTY_LIST_HEIGHT: u16 = 3` (a minimum, not a fixed length).
- Use `Constraint::Min(MIN_LAYOUT_PREVIEW_HEIGHT)` for the top and `Constraint::Min(MIN_PROPERTY_LIST_HEIGHT)` for the bottom. ratatui distributes remaining space proportionally — good for long property lists.
- Below the threshold (height < 11), keep the existing layout-only fallback.

#### Property list section

The section is a bordered block (`Borders::TOP`, title `" Properties "`) containing the property list. The list is rendered from `inspector.properties` using the same sort/filter as the Render Object tab (factor `filtered_and_sorted` into a small helper that both tabs use — see Notes).

States:

1. `properties_loading && properties.is_empty()` — muted `"Loading properties..."` line.
2. `properties_error.is_some()` — error summary + hint, same style as the layout error path.
3. `properties.is_empty() && !properties_loading && properties_error.is_none()` — muted `"No properties for this widget."` (rare; even simple widgets have at least `key`, `widget`, etc., but `Text` is sometimes minimal).
4. `!properties.is_empty()` — render the property table.

Per-row layout: identical to the Render Object tab — `name` (left, ~20 cols) + `description` (right, ellipsis-truncated to fill). Default-level (`fine`) rows muted and sorted to end. Hidden-level rows filtered out.

#### Shared helper

The sort/filter logic (`filtered_and_sorted`) is identical to the Render Object tab. To avoid duplication, lift it to `details/mod.rs` as a `pub(super) fn filter_and_sort_by_level(...)`. Both tabs import and use it.

```rust
// In details/mod.rs:
pub(super) fn filter_and_sort_by_level<'a>(
    props: &'a [DiagnosticsNode],
) -> Vec<(&'a DiagnosticsNode, bool)> {
    let mut non_default = Vec::new();
    let mut default = Vec::new();
    for p in props {
        match p.level.as_deref() {
            Some("hidden") => continue,
            Some("fine") => default.push((p, true)),
            _ => non_default.push((p, false)),
        }
    }
    non_default.extend(default);
    non_default
}
```

Update task 07 mentally: the Render Object tab uses this helper too. Document this in 08's completion summary; if task 07 has already landed and inlined its own copy of the helper, this task should refactor 07's renderer to use the shared helper as part of its work. (Tasks 07 + 08 can both be implemented in parallel, so coordination at merge time may be needed — either implementor adds the helper; the second one rebases.)

### Acceptance Criteria

1. Pressing Enter on a widget and viewing the Properties tab shows a populated property list below the existing layout preview.
2. The property list section preserves its bordered-top frame from Phase 1 (title `" Properties "`).
3. Default-level (`fine`) properties render muted and sort to the end. Hidden-level properties do not render.
4. Loading / error / empty states render with consistent muted styling, matching the Render Object tab.
5. The Phase 1 placeholder `"(properties will load here in Phase 2)"` is removed.
6. The layout preview above the list is unchanged — no regressions to box-model / size / constraint rendering.
7. If both tabs (07 and 08) inline the same filter/sort logic, only one copy remains by the time both tasks merge. (Coordination at merge time.)

### Testing

Pattern follows existing `properties_tab.rs` tests. Add:

```rust
#[test]
fn properties_tab_shows_property_list_when_populated() {
    let mut state = InspectorState::default();
    state.details_open = true;
    state.details_tab = DetailsTab::Properties;
    state.details_node_id = Some("objects/42".into());
    state.properties = vec![
        sample_node("textDirection", "ltr", None),
        sample_node_with_level("locale", "null", "fine"),
    ];
    let buf = render_properties_tab(&state, (80, 30));
    let s = buffer_to_string(&buf);
    assert!(s.contains("textDirection"));
    assert!(s.contains("ltr"));
    assert!(s.contains("locale"));
    let pos_text = s.find("textDirection").unwrap();
    let pos_locale = s.find("locale").unwrap();
    assert!(pos_text < pos_locale, "default should sort to end");
}

#[test]
fn properties_tab_hides_phase_1_placeholder_when_loaded() {
    let mut state = InspectorState::default();
    state.details_open = true;
    state.details_tab = DetailsTab::Properties;
    state.details_node_id = Some("objects/42".into());
    state.properties = vec![sample_node("foo", "bar", None)];
    let buf = render_properties_tab(&state, (80, 30));
    let s = buffer_to_string(&buf);
    assert!(!s.contains("properties will load here"));
}

#[test]
fn properties_tab_keeps_layout_preview() {
    let mut state = InspectorState::default();
    state.details_open = true;
    state.details_tab = DetailsTab::Properties;
    state.details_node_id = Some("objects/42".into());
    state.layout = Some(LayoutInfo {
        size: Some(WidgetSize { width: 414.0, height: 600.0 }),
        ..Default::default()
    });
    let buf = render_properties_tab(&state, (80, 30));
    let s = buffer_to_string(&buf);
    assert!(s.contains("414"), "layout preview's width label should appear");
    assert!(s.contains("600"));
}
```

### Notes

- Keep `MIN_LAYOUT_PREVIEW_HEIGHT = 8` as Phase 1 set it. The constant rename above (`PROPERTY_LIST_HEIGHT` → `MIN_PROPERTY_LIST_HEIGHT`) is cosmetic — it just clarifies that the value is a minimum, not a fixed length, now that the section grows with the available area.
- The shared `filter_and_sort_by_level` helper (in `details/mod.rs`) is the de-duplication step for what would otherwise be two copies in tasks 07 and 08. The helper is `pub(super)` so both tab files can import it.
- The file size before this task is 227 lines; the rewrite of the placeholder section is ~50–80 lines net (mostly the property-table renderer, which may also be shared with task 07 if both tabs end up using the same row rendering function — consider extracting `render_property_row(buf, area, node, is_default)` to `details/mod.rs` as `pub(super)`).
- Do NOT alter the upper layout-preview section's rendering — `render_layout_panel` is reused as-is.
- Phase 3 polish (per parent PLAN §5.4) introduces conditional tab visibility — at that point the Properties tab content may need a small adjustment for narrow terminals. Not in scope here.

---

## Completion Summary

**Status:** Done
**Branch:** feat/devtools-inspector-parity

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-tui/src/widgets/devtools/inspector/details/properties_tab.rs` | Replaced Phase 1 placeholder with full property list renderer (loading / error / empty / populated states); added 7 new Phase 2 tests; kept layout preview unchanged |
| `crates/fdemon-tui/src/widgets/devtools/inspector/details/mod.rs` | Added `filter_and_sort_by_level` shared helper (pub(super)); added `DiagnosticsNode` import |

### Notable Decisions/Tradeoffs

1. **Shared helper in `details/mod.rs`**: Added `filter_and_sort_by_level` as `pub(super)` to `details/mod.rs` as the task specified. Task 07 (`render_object_tab`) is still a stub — when it gets populated in Phase 2 it can import and use this helper instead of duplicating the logic.

2. **Layout split change**: Renamed `PROPERTY_LIST_HEIGHT` (fixed `Length(3)`) to `MIN_PROPERTY_LIST_HEIGHT: u16 = 3` (minimum) and changed the constraint from `Constraint::Length` to `Constraint::Min`. The property list now grows proportionally with available space — important for widgets with many properties.

3. **`render_muted_centered` fallback**: The empty-state message distinguishes between "has a node selected" (→ "No properties for this widget.") and "no node selected" (→ "Select a widget to see properties.") using `details_node_id.is_some()`.

4. **Struct literal syntax in tests**: All Phase 2 tests use `InspectorState { field: val, ..Default::default() }` to satisfy the `clippy::field_reassign_with_default` lint (-D warnings).

### Testing Performed

- `cargo test -p fdemon-tui --lib -- devtools::inspector::details::properties_tab` — PASS (11 tests)
- `cargo test --workspace --lib` — PASS (1097 tests)
- `cargo clippy --workspace --all-targets -- -D warnings` — PASS
- `cargo fmt --all -- --check` — PASS

### Risks/Limitations

1. **Scroll not implemented**: The property list does not scroll — rows beyond `area.height` are simply clipped. Phase 3 polish can add a `ListState`-driven scroll if needed.
2. **Task 07 coordination**: `render_object_tab.rs` is still a stub ("Coming soon — Phase 2"). When task 09 (flex explorer) or a future task populates the Render Object tab, the implementor should import `filter_and_sort_by_level` from `super` rather than inlining a duplicate.
