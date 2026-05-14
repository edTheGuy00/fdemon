## Task: Allocation Table — Scroll, Row Selection, Mouse Regions

**Objective**: Replace the hard 10-row cap with a scrollable window. Render selected row with highlight. Register one click region per visible row.

**Depends on**: Phase 2

**Estimated Time**: 2-4 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-tui/src/widgets/devtools/performance/memory_chart/table.rs`:
  - Replace `MAX_TABLE_ROWS = 10` cap with a variable `visible_height` derived from the rendered `Rect`.
  - Accept `scroll_offset: usize`, `selected_row: Option<usize>`, and `focused: bool` parameters.
  - Render windowed slice of `profile.members` (sorted by current sort column). Don't use `top_by_size(10)` — sort `members` inline and slice `[scroll_offset .. scroll_offset + visible_height]`.
  - Write `alloc_table_visible_height` Cell each frame with EXCEPTION annotation.
  - Highlight `selected_row` (if visible) with a distinct row style.
  - Register one click region per visible row → `Message::PerfSelectAllocRow { index: Some(row_index_in_full_list) }`.
  - Register a section-level click region (empty area below rows) → `Message::PerfFocusSection(PerfSection::MemoryList)` so clicking outside any row still focuses the section.

**Files Read (Dependencies):**
- `crates/fdemon-app/src/session/performance.rs`: For state shape.
- `crates/fdemon-core/src/performance.rs` (or wherever `AllocationProfile` lives): For `profile.members` access and `ClassHeapStats` field shapes.
- `docs/CODE_STANDARDS.md`: Region Registry Pattern + Principle 3.

### Details

```rust
pub struct AllocationTable<'a> {
    profile: &'a AllocationProfile,
    sort_column: AllocationSortColumn,
    scroll_offset: usize,
    selected_row: Option<usize>,
    focused: bool,
    visible_height_cell: &'a Cell<usize>,
}

impl AllocationTable<'_> {
    pub fn render(&self, area: Rect, buf: &mut Buffer, mouse: Option<&mut MouseCtx>) {
        // Compute visible height from area (subtract header + borders)
        let visible_height = (area.height as usize).saturating_sub(2); // header + 1 padding
        // EXCEPTION (TEA): render-hint Cell — see docs/CODE_STANDARDS.md
        self.visible_height_cell.set(visible_height);

        // Sort and slice
        let mut sorted: Vec<&ClassHeapStats> = self.profile.members.iter().collect();
        sort_inline(&mut sorted, self.sort_column);
        let end = (self.scroll_offset + visible_height).min(sorted.len());
        let visible_slice = &sorted[self.scroll_offset..end];

        // Render rows
        for (row_idx, stat) in visible_slice.iter().enumerate() {
            let global_idx = self.scroll_offset + row_idx;
            let row_rect = compute_row_rect(area, row_idx);
            let style = if Some(global_idx) == self.selected_row {
                Style::default().bg(Color::Cyan).fg(Color::Black)
            } else {
                Style::default()
            };
            render_row(buf, row_rect, stat, style);

            if let Some(ctx) = mouse.as_deref_mut() {
                ctx.click(
                    row_rect,
                    MouseAction::emit(Message::PerfSelectAllocRow { index: Some(global_idx) }),
                );
            }
        }

        // Empty space below = section focus
        let used_rows = visible_slice.len() as u16;
        let remaining = Rect {
            x: area.x,
            y: area.y + 2 + used_rows,
            width: area.width,
            height: area.height.saturating_sub(2 + used_rows),
        };
        if remaining.height > 0 {
            if let Some(ctx) = mouse {
                ctx.click(
                    remaining,
                    MouseAction::emit(Message::PerfFocusSection(PerfSection::MemoryList)),
                );
            }
        }
    }
}
```

### Acceptance Criteria

1. Allocation table renders up to `visible_height` rows (no 10-row cap).
2. `alloc_table_scroll_offset` slides the visible window through the sorted profile.
3. `alloc_table_visible_height` Cell written every frame with EXCEPTION annotation.
4. Selected row (if within visible window) has distinct row style.
5. Clicking a row emits `PerfSelectAllocRow { index: Some(global_idx) }`.
6. Clicking empty space within the table area emits `PerfFocusSection(MemoryList)`.
7. Unit tests cover: render bounds at scroll offset 0, render at offset > 0, selected-row highlight, click region indexing.
8. `cargo test --workspace` and clippy pass.

### Testing

```rust
#[test]
fn alloc_table_renders_windowed_slice() {
    let profile = mock_profile_with_n_classes(50);
    let mut buf = Buffer::empty(Rect::new(0, 0, 80, 12));
    let cell = Cell::new(0);
    let table = AllocationTable {
        profile: &profile,
        sort_column: AllocationSortColumn::Size,
        scroll_offset: 20,
        selected_row: Some(25),
        focused: true,
        visible_height_cell: &cell,
    };
    table.render(buf.area, &mut buf, None);
    assert_eq!(cell.get(), 10);  // 12 - 2
    // Assert first visible row's class name matches sorted[20]
}

#[test]
fn alloc_table_selected_row_highlighted_when_visible() { /* ... */ }
#[test]
fn alloc_table_clicking_row_emits_correct_global_index() { /* ... */ }
```

### Notes

- Per CODE_STANDARDS.md, no magic numbers — define `TABLE_HEADER_ROWS: usize = 2` (or however many) and use the constant.
- `MAX_TABLE_ROWS` constant can be removed entirely; if it's used elsewhere (e.g., in `profile.top_by_size(MAX_TABLE_ROWS)` callers), audit and replace those call sites.
- Sort cost: with up to thousands of classes, an inline sort per render frame may be expensive. Profile after this lands; if needed, add a cached-sorted-handle to `AllocationProfile`. Out of scope here.

---

## Completion Summary

**Status:** Done
**Branch:** fix/devtools-improvements

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-tui/src/widgets/devtools/performance/memory_chart/table.rs` | Replaced free-function with `AllocationTable` struct; scrollable window via `scroll_offset` + `visible_height`; selected-row highlight with `palette::ACCENT` background; per-row click regions (`PerfSelectAllocRow`); empty-space focus region (`PerfFocusSection(MemoryList)`); `visible_height_cell` written every frame with EXCEPTION annotation; `TABLE_HEADER_ROWS` named constant; `MAX_TABLE_ROWS` removed; legacy `render_allocation_table` wrapper preserved for existing tests |
| `crates/fdemon-tui/src/widgets/devtools/performance/memory_chart/mod.rs` | Added `alloc_scroll_offset`, `alloc_selected_row`, `alloc_focused`, `alloc_visible_height_cell` fields to `MemoryChart`; added `with_alloc_state()` builder method; split `Widget::render` into `render_impl` + `render_with_regions`; ctx threading to `AllocationTable`; removed unused `TABLE_HEADER_HEIGHT` constant |
| `crates/fdemon-tui/src/widgets/devtools/performance/mod.rs` | Updated `render_impl` to pass `alloc_table_scroll_offset`, `alloc_table_selected_row`, `focused_section == MemoryList`, and `alloc_table_visible_height` to `MemoryChart`; use `ctx.as_deref_mut()` for FrameChart so MemoryChart retains ownership of `ctx` for its own click regions |
| `crates/fdemon-tui/src/widgets/devtools/performance/memory_chart/tests.rs` | Added 8 new acceptance-criteria tests: `alloc_table_visible_height_cell_written_each_frame`, `alloc_table_renders_windowed_slice_at_offset_zero`, `alloc_table_renders_windowed_slice_at_positive_offset`, `alloc_table_selected_row_highlighted_when_visible`, `alloc_table_selected_row_not_highlighted_when_scrolled_past`, `alloc_table_clicking_row_emits_correct_global_index`, `alloc_table_empty_space_emits_focus_section`, `alloc_table_no_focus_region_when_rows_fill_area` |

### Notable Decisions/Tradeoffs

1. **Legacy wrapper preserved**: `render_allocation_table` free-function left as a thin wrapper over `AllocationTable` (scroll=0, selected=None, no visible_height_cell). This keeps the ~20 existing tests green without refactoring them.
2. **`focused` field is dead code**: wired for future visual distinction (focused-border style) but not yet read. Annotated with `#[allow(dead_code)]` to pass clippy with `-D warnings`.
3. **Reborrow pattern for dual ctx use**: `ctx.as_deref_mut()` is passed to `FrameChart`, then `ctx` is passed by value to `MemoryChart::render_with_regions`. This allows both sections to register click regions in one `render_impl` call without cloning.
4. **Inline sort per frame**: `sort_by_key` with `Reverse` is used. Performance concern noted in task Notes — deferred.

### Testing Performed

- `cargo fmt --all -- --check` - Passed
- `cargo check --workspace --all-targets` - Passed
- `cargo test --workspace` - Passed (1032 fdemon-tui tests, 0 failures; all workspace tests green)
- `cargo clippy --workspace --all-targets -- -D warnings` - Passed

### Risks/Limitations

1. **Sort per frame**: Sorting all class members on every render call may be slow for profiles with thousands of classes. The task notes this as out of scope; a cached-sorted handle in `AllocationProfile` would be the mitigation path.
2. **Scroll clamping is visual-only**: If `alloc_table_scroll_offset` exceeds `sorted.len()-1`, the render clamps silently but the handler-layer state retains the out-of-range value. The handler should enforce clamping using the `alloc_table_visible_height` Cell feedback; this is the expected TEA pattern.
