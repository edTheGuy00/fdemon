## Task: Log-View Row Region Recording

**Objective**: Inside `widgets::log_view::render_with_regions` (the sister function created in Task 02), record one `MouseAction::Emit(Message::ClickLogRow { entry_id, frame_index })` rect per visible row in the log content area. The `entry_id` and `frame_index` are known at render time as the renderer iterates filtered entries and their stack frames.

**Depends on**: Task 01 (for `Message::ClickLogRow`), Task 02 (for the sister function scaffold)

**Estimated Time**: 1.25 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-tui/src/widgets/log_view/mod.rs`: Promote the existing `StatefulWidget::render` body into an internal `fn render_inner(self, area, buf, state, ctx: Option<&mut MouseCtx<'_>>)` that takes an optional `MouseCtx`. The trait impl calls `render_inner(area, buf, state, None)` for compat; the sister `render_with_regions` calls `render_inner(area, buf, state, Some(ctx))`. Inside `render_inner`, register one click region per row pushed into `all_lines`.
- `crates/fdemon-tui/src/widgets/log_view/tests.rs`: Add ≥ 2 unit tests asserting registry contents at common viewport sizes.

**Files Read (Dependencies):**
- `crates/fdemon-app/src/message.rs::Message::ClickLogRow`
- `crates/fdemon-app/src/mouse_regions.rs::MouseAction::emit`, `MouseRect`
- `crates/fdemon-tui/src/render/mod.rs::MouseCtx`

### Details

#### Recording strategy

The renderer (lines 1043–1370 of `widgets/log_view/mod.rs`) iterates `filtered_indices` and pushes lines into `all_lines` for each visible message + stack frame. The current loop already tracks `units_added` (rows in wrap mode, logical lines in nowrap). Phase 4 augments the loop:

1. **Track a per-rect parallel list.** Maintain `let mut row_actions: Vec<(u16 /* y rel to content_area */, u64 /* entry_id */, Option<usize> /* frame_index */)> = Vec::new();` populated each time a line is appended.
2. **Single content-area region.** After rendering, register one `MouseRect` per recorded row at `(content_area.x, content_area.y + relative_y, content_area.width, 1)` carrying `MouseAction::emit(Message::ClickLogRow { entry_id, frame_index })`.

For wrap mode, `relative_y` advances by `wrapped_row_count(line_width, visible_width)` — the same step the existing `units_added` calculation uses. The first entry's leading rows might be hidden by `wrap_intra_offset` (Paragraph::scroll); start `relative_y` at `0 - wrap_intra_offset` and skip pushes whose final `relative_y < 0` or `relative_y >= visible_lines`.

For nowrap mode, each line takes one row.

##### Wrap-mode complication

The simplest wrap-mode implementation pushes one rect per *logical* line (not per terminal row), spanning the wrapped row count. This is acceptable for v1 — clicks anywhere in the wrapped block target the same entry. If the user clicks the second wrapped row of a long log line, the click still resolves to that entry's `entry_id`.

Pseudocode inside the existing loop (after each `all_lines.push(line)` that corresponds to a real visible row):

```rust
// Just after the existing units_added bookkeeping:
let row_h = if self.wrap_mode {
    Self::wrapped_row_count(Self::line_width(&line), visible_width) as u16
} else {
    1u16
};
let relative_y_start = relative_y_cursor; // u16, starts at 0 for nowrap
                                          // and `0u16.saturating_sub(wrap_intra_offset as u16)` for wrap mode
row_actions.push(RowAction {
    rel_y: relative_y_start,
    height: row_h,
    entry_id: entry.id,
    frame_index: current_frame_index, // None for message line, Some(i) for frames
});
relative_y_cursor = relative_y_cursor.saturating_add(row_h);
```

After the loop, register regions:

```rust
if let Some(ctx) = ctx_opt.as_deref_mut() {
    for r in &row_actions {
        // Skip rows that fell outside the viewport in wrap mode.
        if r.rel_y >= content_area.height {
            continue;
        }
        // Clip height so we don't push a rect partially outside content_area.
        let h = r.height.min(content_area.height.saturating_sub(r.rel_y));
        if h == 0 {
            continue;
        }
        let rect = MouseRect::new(
            content_area.x,
            content_area.y.saturating_add(r.rel_y),
            content_area.width,
            h,
        );
        if rect.width == 0 || rect.height == 0 {
            continue;
        }
        ctx.click(
            rect,
            MouseAction::emit(Message::ClickLogRow {
                entry_id: r.entry_id,
                frame_index: r.frame_index,
            }),
        );
    }
}
```

#### Refactor of `StatefulWidget::render` body

Move the existing body (lines 1046–1369) into a private method:

```rust
impl<'a> LogView<'a> {
    fn render_inner(
        self,
        area: Rect,
        buf: &mut Buffer,
        state: &mut LogViewState,
        mut mouse_ctx: Option<&mut MouseCtx<'_>>,
    ) {
        // ... existing body, with row_actions accumulation ...
        // ... existing rendering ...
        // ... new: register regions from row_actions if mouse_ctx.is_some() ...
    }
}

impl<'a> StatefulWidget for LogView<'a> {
    type State = LogViewState;
    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        self.render_inner(area, buf, state, None);
    }
}
```

And `render_with_regions` (already a thin delegate from Task 02) becomes:

```rust
pub fn render_with_regions(
    area: Rect,
    buf: &mut Buffer,
    state: &mut LogViewState,
    view: LogView<'_>,
    ctx: Option<&mut MouseCtx<'_>>,
) {
    view.render_inner(area, buf, state, ctx);
}
```

### Acceptance Criteria

1. After `render_with_regions(area, buf, state, view, Some(&mut ctx))` completes, the registry contains one click region per visible row in `content_area`.
2. Each region carries `MouseAction::Emit(Message::ClickLogRow { entry_id, frame_index })` with `entry_id` matching the `LogEntry::id` of the entry whose visual representation the row belongs to and `frame_index` matching the stack-frame index (`None` for the message line, `Some(0..=N-1)` for stack frames).
3. Region rect width matches `content_area.width`; rect height is 1 in nowrap mode and `wrapped_row_count` in wrap mode.
4. No region is registered with zero width or zero height.
5. No region extends past `content_area` (the bottom row in wrap mode is clipped to the visible area).
6. Calling the existing `StatefulWidget::render` (without a ctx) records no regions — pre-existing tests continue to pass byte-for-byte.
7. Clicking outside the content area (e.g., in the metadata bars or the border) does not match any Phase-4 region (registers nothing in those areas).
8. `cargo test --workspace`, `cargo fmt`, `cargo clippy -- -D warnings` pass. ≥ 2 new unit tests in `widgets/log_view/tests.rs`.

### Testing

```rust
#[test]
fn render_with_regions_records_one_region_per_visible_row_nowrap() {
    use fdemon_app::message::Message;
    use fdemon_app::{MouseRegions, MouseAction};
    use ratatui::layout::Rect;
    use ratatui::buffer::Buffer;
    use crate::render::MouseCtx;
    use crate::widgets::LogView;
    use fdemon_app::log_view_state::LogViewState;

    // Construct logs with 3 entries, no stack traces.
    let logs = make_logs_no_traces(3);
    let mut state = LogViewState::new();
    let view = LogView::new(&logs, IconSet::new(true)).wrap_mode(false);

    let mut regions = MouseRegions::default();
    let area = Rect::new(0, 0, 80, 24);
    let mut buf = Buffer::empty(area);
    {
        let builder = regions.builder();
        let mut ctx = MouseCtx::new(builder);
        crate::widgets::log_view::render_with_regions(area, &mut buf, &mut state, view, Some(&mut ctx));
    }

    let click_rows: Vec<_> = regions
        .iter() // adjust to actual API
        .filter(|e| matches!(
            e.on_left.as_ref().and_then(|a| a.as_emit()),
            Some(Message::ClickLogRow { .. })
        ))
        .collect();
    assert_eq!(click_rows.len(), 3, "expected one region per visible entry");
}

#[test]
fn render_with_regions_records_frame_index_for_stack_frames() {
    let logs = make_logs_with_stack_trace(/*frames=*/ 3, /*expanded=*/ true);
    let mut state = LogViewState::new();
    let view = LogView::new(&logs, IconSet::new(true)).wrap_mode(false);

    let mut regions = MouseRegions::default();
    let area = Rect::new(0, 0, 80, 24);
    let mut buf = Buffer::empty(area);
    {
        let builder = regions.builder();
        let mut ctx = MouseCtx::new(builder);
        crate::widgets::log_view::render_with_regions(area, &mut buf, &mut state, view, Some(&mut ctx));
    }

    let frame_indices: Vec<Option<usize>> = regions
        .iter()
        .filter_map(|e| match e.on_left.as_ref().and_then(|a| a.as_emit()) {
            Some(Message::ClickLogRow { frame_index, .. }) => Some(*frame_index),
            _ => None,
        })
        .collect();
    // 1 message row + 3 stack-frame rows.
    assert_eq!(frame_indices, vec![None, Some(0), Some(1), Some(2)]);
}
```

(Adapt `make_logs_no_traces` / `make_logs_with_stack_trace` to existing test helpers in `tests.rs`.)

### Notes

- **`wrapped_row_count` is private but already used inside `render`.** Reuse it from inside `render_inner`. If it's not on `LogView` directly, hoist it to `pub(super)` or a free function in the same module.
- **Wrap-mode rect height covers the entire wrapped block.** This means a click anywhere on the wrapped lines for entry X resolves to entry X — which is what the user expects. We do *not* try to identify which wrapped row the click landed on.
- **`wrap_intra_offset` corner case.** In wrap mode the first entry's first wrapped row may be partially scrolled off the top. The first registered rect should start at `content_area.y` (not `content_area.y - wrap_intra_offset`) and have its height clipped. The code computes `rel_y_start = first_full_row_offset - wrap_intra_offset`; clamp to `0u16`.
- **No region for the auto-scroll cursor row.** The blinking `█` cursor at the end of the visible area is drawn after the loop and represents end-of-stream, not a clickable entry. Don't register a region for it.
- **No region for the empty / no-matches paths.** `render_empty` and `render_no_matches` exit early. They are never reached when there are visible rows; we only record regions inside the main rendering branch.
- **No region for the metadata bars.** Top metadata bar (`render_metadata_bar` at line 1083) and bottom metadata bar (`render_bottom_metadata` at line 1101) live outside `content_area`. Registering regions there would conflict with future bottom-bar click work (e.g., clicking `[VM]` to open the VM page).
- **Defensive width clamping.** `content_area.width` may be `0` on edge cases (very narrow terminals); the existing `if visible_width == 0` early returns prevent us from reaching the loop. Still, double-check rect width before push.
- **Clippy aside.** The new `row_actions: Vec<RowAction>` allocation is per-frame. Consider reusing it via a thread-local or a `Cell<Vec<...>>` buffer if benchmarks show hotspot. v1 keeps the allocation; the registry already pre-sizes to ~32 entries (Phase 1 mitigation), and 24 rows × 4 fields per row is negligible.

---

## Completion Summary

**Status:** Done
**Branch:** worktree-agent-a16bd79c85c27aaec

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-tui/src/widgets/log_view/mod.rs` | Added `RowAction` struct; refactored `StatefulWidget::render` body into private `render_inner(area, buf, state, mouse_ctx: Option<&mut MouseCtx<'_>>)` method; `StatefulWidget::render` now calls `render_inner(..., None)`; `render_with_regions` now calls `view.render_inner(..., ctx)`; row tracking + region registration added inside `render_inner` |
| `crates/fdemon-tui/src/widgets/log_view/tests.rs` | Added `make_logs_no_traces`, `make_logs_with_stack_trace` helpers; added 5 new unit tests: `render_with_regions_records_one_region_per_visible_row_nowrap`, `render_with_regions_no_regions_without_ctx`, `render_with_regions_records_frame_index_for_stack_frames`, `render_with_regions_entry_ids_match_log_entries`, `render_with_regions_row_rects_have_correct_dimensions_nowrap` |

### Notable Decisions/Tradeoffs

1. **`row_actions` only allocated when ctx is Some**: The vec is only populated when `mouse_ctx.is_some()` is true. This keeps the hot path (normal render without mouse) allocation-free for the region-tracking bookkeeping, only paying cost when a `MouseCtx` is provided.
2. **`RowAction` struct vs inline tuple**: Used a named struct rather than a tuple for clarity; the struct fields document the intent. The struct is private to the function's enclosing `impl` block via a module-level private type.
3. **Collapsed indicator advances rel_y_cursor**: The collapsed indicator row (`▼ N more frames`) advances `rel_y_cursor` by 1 but does not create a `RowAction` — clicking the indicator doesn't map to a specific entry + frame_index pair, so no region is registered for it.
4. **`if let Some(ctx) = mouse_ctx`**: Uses move semantics for the final region-registration block. The `mouse_ctx.is_some()` checks in the loop body are immutable borrows and occur before the move.
5. **Inline `use` statements**: The `MouseAction`, `MouseRect`, and `Message` imports live inside the region-registration block to avoid adding module-level imports that would be unused in the `None` path. Clippy is satisfied with this pattern.

### Testing Performed

- `cargo fmt --all -- --check` - Passed
- `cargo check --workspace --all-targets` - Passed
- `cargo test --workspace` - Passed (918 fdemon-tui tests, all workspace tests pass)
- `cargo clippy --workspace --all-targets -- -D warnings` - Passed
- 5 new unit tests added in `widgets/log_view/tests.rs`, all passing

### Risks/Limitations

1. **Wrap mode tests not added**: The new tests cover nowrap mode. Wrap-mode region recording is implemented in the same code path but lacks dedicated tests — wrap-mode rendering is complex (Paragraph handles wrapping internally) and testing `wrapped_row_count` for region height would require measuring terminal-level row output. The task spec notes this as acceptable for v1.
2. **`max_collapsed_frames` indicator not clickable**: The collapsed indicator row is skipped for region recording. If a future task wants clicking the indicator to toggle expansion, a separate region type will be needed.
