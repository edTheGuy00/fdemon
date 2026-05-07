## Task: Tag Filter Overlay Regions

**Objective**: Fill in `widgets::tag_filter::render_tag_filter_with_regions` so each tag row becomes clickable (`Message::TagFilterClickRow { index }`), and the footer's `[a] All` / `[n] None` action labels become clickable (`Message::ShowAllNativeTags` / `Message::HideAllNativeTags`). All regions register at `z_index = 1`. The widget's existing visual output is unchanged.

**Depends on**: 01 (Phase-5 messages), 02 (sister `render_tag_filter_with_regions` stub)

**Estimated Time**: 1 hour

### Scope

**Files Modified (Write):**
- `crates/fdemon-tui/src/widgets/tag_filter.rs`: Replace the stub body of `render_tag_filter_with_regions` with the real implementation. The existing `render_tag_filter` free function is **unchanged**.

**Files Read (Dependencies):**
- `crates/fdemon-app/src/mouse_regions.rs` (`MouseRect`, `MouseAction`, `MouseRegionsBuilder::click_at_z`).
- `crates/fdemon-app/src/message.rs` (`TagFilterClickRow`, `ShowAllNativeTags`, `HideAllNativeTags`).

### Details

#### Where rows are rendered today

In `widgets/tag_filter.rs::render_tag_filter`, the tag rows are rendered as a `List` widget inside `chunks[0]` of `inner`. The list itself doesn't expose per-item rects, but we know:

- Each row is exactly 1 cell tall.
- Rows start at `chunks[0].y` and increment by 1.
- Visible rows are bounded by `chunks[0].height`.
- Scrolling: the list's `ListState::with_selected(...)` controls which item is at the top; the *visible* row at screen-y `chunks[0].y + i` corresponds to *tag index* `scroll_offset + i`, where `scroll_offset` is derived from `selected_index` and `chunks[0].height`.

For Phase 5 we record one region per *visible* row, with the *absolute* tag index (across the full sorted list) embedded in the message. The widget already re-computes `last_known_visible_height` each frame for keyboard-Page calculations — we can reuse that.

#### `render_tag_filter_with_regions` body

```rust
pub fn render_tag_filter_with_regions(
    frame: &mut Frame,
    area: Rect,
    tag_state: &NativeTagState,
    ui_state: &TagFilterUiState,
    ctx: Option<&mut MouseCtx<'_>>,
) {
    // Compute overlay layout (mirrors render_tag_filter).
    let tag_count = tag_state.tag_count();
    let visible_tags = (tag_count as u16).min(TAG_FILTER_MAX_VISIBLE_TAGS);
    let overlay_height = (visible_tags + 4).min(area.height.saturating_sub(2)).max(6);
    let overlay_width = TAG_FILTER_MIN_WIDTH
        .max(area.width / 3)
        .min(area.width.saturating_sub(4));
    let x = area.x + (area.width.saturating_sub(overlay_width)) / 2;
    let y = area.y + (area.height.saturating_sub(overlay_height)) / 2;
    let overlay_area = Rect::new(x, y, overlay_width, overlay_height);

    // Render via the existing function (unchanged visual output).
    render_tag_filter(frame, area, tag_state, ui_state);

    // Without a context, there's nothing else to do.
    let Some(ctx) = ctx else { return };

    // Recompute the inner area + chunks the same way render_tag_filter does
    // so we know where the list rows + footer landed.
    let block = Block::default().borders(Borders::ALL);
    let inner = block.inner(overlay_area);

    if tag_count == 0 {
        // Empty state — no clickable rows.
        return;
    }

    let chunks = Layout::vertical([
        Constraint::Min(1),    // tag list
        Constraint::Length(1), // separator
        Constraint::Length(1), // footer
    ])
    .split(inner);

    let list_chunk = chunks[0];
    let footer_chunk = chunks[2];
    let visible_height = list_chunk.height as usize;

    // Compute the scroll offset that the list will use given selected_index
    // and visible_height. This must match ListState's internal calculation —
    // see how Ratatui's List/ListState picks the topmost rendered item.
    //
    // For our purposes: a simple "keep the selected row visible" calculation
    // matches Ratatui's default with `with_selected` (which scrolls the list
    // so the selection is visible).
    let scroll_offset = compute_scroll_offset(
        ui_state.selected_index,
        tag_count,
        visible_height,
        ui_state.last_known_visible_height.get(), // hint for stability across renders
    );

    // Register one region per visible row.
    for screen_row in 0..visible_height {
        let abs_index = scroll_offset + screen_row;
        if abs_index >= tag_count {
            break;
        }

        let rect = MouseRect::new(list_chunk.x, list_chunk.y + screen_row as u16, list_chunk.width, 1);
        if rect.is_empty() {
            continue;
        }

        ctx.click_at_z(
            rect,
            MouseAction::emit(Message::TagFilterClickRow { index: abs_index }),
            1,
        );
    }

    // ── Footer action labels ────────────────────────────────────────────────
    //
    // Footer text: "[a] All  [n] None  [Spc] Toggle  [Esc] Close"
    // Click targets:
    //   - "[a] All"  → ShowAllNativeTags
    //   - "[n] None" → HideAllNativeTags
    //   - [Spc] / [Esc] are not clickable (Spc requires a selected row,
    //     Esc has no mouse equivalent in v1).
    //
    // The footer is left-rendered (no centering) — see render_tag_filter:146.
    // We compute the byte offsets of "[a]" and "[n]" within the footer string
    // to derive their cell columns.

    let footer_text = "[a] All  [n] None  [Spc] Toggle  [Esc] Close";
    let a_offset = 0u16; // "[a]" starts at column 0
    let a_width = "[a] All".chars().count() as u16;
    let n_offset = footer_text.find("[n]").map(|i| i as u16).unwrap_or(0);
    let n_width = "[n] None".chars().count() as u16;

    if footer_chunk.width >= a_offset + a_width {
        ctx.click_at_z(
            MouseRect::new(footer_chunk.x + a_offset, footer_chunk.y, a_width, 1),
            MouseAction::emit(Message::ShowAllNativeTags),
            1,
        );
    }
    if footer_chunk.width >= n_offset + n_width {
        ctx.click_at_z(
            MouseRect::new(footer_chunk.x + n_offset, footer_chunk.y, n_width, 1),
            MouseAction::emit(Message::HideAllNativeTags),
            1,
        );
    }
}

/// Compute the topmost visible tag index given `selected_index`,
/// `tag_count`, and the visible window height. Matches Ratatui's
/// `ListState::with_selected` scrolling: the selected item is kept visible.
///
/// Note: `last_visible_height` is currently unused but provided for future
/// stability hints (e.g., when the visible height shrinks frame-to-frame).
fn compute_scroll_offset(
    selected: usize,
    tag_count: usize,
    visible: usize,
    _last_visible: usize,
) -> usize {
    if visible == 0 || tag_count <= visible {
        return 0;
    }
    // Selected item must be in the visible window.
    if selected < visible {
        0
    } else {
        selected.saturating_sub(visible - 1)
    }
}
```

(The `compute_scroll_offset` function may already exist somewhere in the codebase as a Ratatui-list-scroll-state helper. Reuse it if so. If not, this 7-line helper is fine inline.)

### Acceptance Criteria

1. `cargo check --workspace --all-targets` passes.
2. `cargo test --workspace` passes — existing `widgets/tag_filter.rs::tests` continue passing (visual output unchanged); new tests below are added.
3. `cargo fmt --all -- --check` and `cargo clippy --workspace --all-targets -- -D warnings` pass.
4. With `tag_count == 0`, `render_tag_filter_with_regions` registers zero regions (the empty-state message has no clickable surface in v1).
5. With N tags and `visible_height = M`, exactly `min(N, M) + 2` regions are registered (M tag rows + `[a] All` + `[n] None`).
6. Each tag-row region's `MouseAction` is `Emit(Message::TagFilterClickRow { index })`, where `index` is the absolute index into `tag_state.sorted_tags()` for the visible row.
7. The `[a] All` region's action is `Emit(Message::ShowAllNativeTags)`. The `[n] None` region's action is `Emit(Message::HideAllNativeTags)`.
8. All Phase-5 regions register at `z_index = 1`.
9. `render_tag_filter_with_regions` with `ctx = None` produces the same buffer output as the existing `render_tag_filter`.

### Testing

Add unit tests inside `widgets/tag_filter.rs::tests`:

```rust
#[test]
fn render_with_regions_records_row_per_visible_tag_plus_two_action_labels() {
    use fdemon_app::{
        message::Message, mouse_regions::MouseRegions, MouseCtx, NativeTagState,
        TagFilterUiState,
    };

    let mut tag_state = NativeTagState::default();
    for i in 0..5 {
        tag_state.observe_tag(&format!("Tag{:02}", i));
    }
    let ui_state = TagFilterUiState::default();

    let backend = ratatui::backend::TestBackend::new(80, 24);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    let mut regions = MouseRegions::default();

    terminal
        .draw(|frame| {
            let builder = regions.builder();
            let mut ctx = MouseCtx::new(builder);
            super::render_tag_filter_with_regions(
                frame,
                frame.area(),
                &tag_state,
                &ui_state,
                Some(&mut ctx),
            );
        })
        .unwrap();

    // 5 visible rows + [a] All + [n] None
    assert_eq!(regions.len(), 7);

    let click_count = regions
        .iter()
        .filter(|e| matches!(
            extract_action(e),
            Some(Message::TagFilterClickRow { .. })
        ))
        .count();
    assert_eq!(click_count, 5, "5 tag-row regions");

    assert!(regions
        .iter()
        .any(|e| matches!(extract_action(e), Some(Message::ShowAllNativeTags))));
    assert!(regions
        .iter()
        .any(|e| matches!(extract_action(e), Some(Message::HideAllNativeTags))));

    for entry in regions.iter() {
        assert_eq!(entry.z_index, 1, "all tag-filter regions register at z=1");
    }
}

#[test]
fn render_with_regions_empty_state_records_zero_regions() {
    use fdemon_app::{mouse_regions::MouseRegions, MouseCtx, NativeTagState, TagFilterUiState};

    let tag_state = NativeTagState::default(); // no tags discovered
    let ui_state = TagFilterUiState::default();

    let backend = ratatui::backend::TestBackend::new(60, 20);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    let mut regions = MouseRegions::default();

    terminal
        .draw(|frame| {
            let builder = regions.builder();
            let mut ctx = MouseCtx::new(builder);
            super::render_tag_filter_with_regions(
                frame,
                frame.area(),
                &tag_state,
                &ui_state,
                Some(&mut ctx),
            );
        })
        .unwrap();

    assert_eq!(regions.len(), 0, "empty state has no clickable rows");
}

#[test]
fn render_with_regions_no_ctx_matches_render_tag_filter_visually() {
    use fdemon_app::{NativeTagState, TagFilterUiState};
    let mut tag_state = NativeTagState::default();
    tag_state.observe_tag("alpha");
    tag_state.observe_tag("beta");
    let ui_state = TagFilterUiState::default();

    let backend_a = ratatui::backend::TestBackend::new(80, 24);
    let mut term_a = ratatui::Terminal::new(backend_a).unwrap();
    term_a
        .draw(|frame| render_tag_filter(frame, frame.area(), &tag_state, &ui_state))
        .unwrap();

    let backend_b = ratatui::backend::TestBackend::new(80, 24);
    let mut term_b = ratatui::Terminal::new(backend_b).unwrap();
    term_b
        .draw(|frame| {
            super::render_tag_filter_with_regions(
                frame,
                frame.area(),
                &tag_state,
                &ui_state,
                None,
            )
        })
        .unwrap();

    assert_eq!(term_a.backend().buffer(), term_b.backend().buffer());
}

#[test]
fn render_with_regions_scrolled_indices_are_absolute() {
    // Ensure that when the list is scrolled (selected_index past visible window),
    // recorded indices are absolute, not relative to the visible window.
    use fdemon_app::{
        message::Message, mouse_regions::MouseRegions, MouseCtx, NativeTagState, TagFilterUiState,
    };

    let mut tag_state = NativeTagState::default();
    for i in 0..30 {
        tag_state.observe_tag(&format!("Tag{:02}", i));
    }
    let ui_state = TagFilterUiState {
        selected_index: 25, // past the visible window of 15
        ..Default::default()
    };

    let backend = ratatui::backend::TestBackend::new(80, 24);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    let mut regions = MouseRegions::default();

    terminal
        .draw(|frame| {
            let builder = regions.builder();
            let mut ctx = MouseCtx::new(builder);
            super::render_tag_filter_with_regions(
                frame,
                frame.area(),
                &tag_state,
                &ui_state,
                Some(&mut ctx),
            );
        })
        .unwrap();

    // Find the largest row-click index recorded.
    let max_index = regions
        .iter()
        .filter_map(|e| match extract_action(e) {
            Some(Message::TagFilterClickRow { index }) => Some(index),
            _ => None,
        })
        .max()
        .expect("at least one tag-row region");
    assert!(
        max_index >= 25,
        "scrolled list must record absolute index >= 25, got {}",
        max_index
    );
}

// `extract_action` reads the inner Message from a region's `on_left` MouseAction.
// If a public helper for this doesn't yet exist, add a small private one in this
// test module — it can match `MouseAction::Emit(msg)` and clone the inner Message.
fn extract_action(entry: &fdemon_app::MouseRegionEntry) -> Option<Message> {
    use fdemon_app::mouse_regions::MouseAction;
    match entry.on_left.as_ref()? {
        MouseAction::Emit(msg) => Some((**msg).clone()),
        MouseAction::EmitWithCoord(_) => None,
    }
}
```

### Notes

- **Why the empty-state has no clickable surface.** When `tag_count == 0`, the overlay shows the message "No native tags discovered yet." There is nothing meaningful to click; the user can press `Esc` to close.
- **Why `[Spc] Toggle` / `[Esc] Close` are not clickable.** `Toggle` requires a selected row — clicking the label without first clicking a tag row would be confusing. `Esc` has no mouse equivalent in any other surface; for consistency, we don't introduce one here. (Future Phase 7+ idea: a small `×` close button on the overlay.)
- **Why `[a] All` / `[n] None` *are* clickable.** They are unambiguous global actions. Mirrors how the keyboard handler treats them — they're top-level shortcuts, not selection-dependent.
- **Why `compute_scroll_offset` is reimplemented locally.** Ratatui's `ListState` doesn't expose its scroll math. The simple "keep selected visible" formula matches Ratatui's default behaviour for `with_selected` and is verified by the scrolled-indices test above.
- **Why the rect width spans the full `list_chunk.width`.** Clicks anywhere on a row should select that row — narrower rects (e.g., just over the checkbox or just over the tag name) would create dead zones and confuse users.
- **Why `z_index = 1`.** Tag-filter overlay is a primary modal (covers the log view, intercepts all input). Same z as ConfirmDialog, NewSessionDialog. Sub-modals (none in v1) would be z=2.

---

## Completion Summary

**Status:** Done
**Branch:** feat/mouse-support

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-tui/src/widgets/tag_filter.rs` | Added `fdemon_app::message::Message` and `mouse_regions::{MouseAction, MouseRect}` imports; replaced stub body of `render_tag_filter_with_regions` with real implementation; added `compute_scroll_offset` private helper; added 4 new unit tests |

### Notable Decisions/Tradeoffs

1. **Import path corrections**: The task file's test snippets used `fdemon_app::MouseCtx` and `fdemon_app::NativeTagState` which do not exist at crate root. Fixed to `crate::widgets::MouseCtx` (re-exported from `crate::render::MouseCtx`) and `fdemon_app::session::NativeTagState` respectively.
2. **`compute_scroll_offset` as private module function**: No existing scroll-offset helper was found in the codebase, so the 7-line helper from the task was added as a private `fn` in the same file.

### Testing Performed

- `cargo check --workspace --all-targets` - Passed
- `cargo test -p fdemon-tui -- tag_filter` - Passed (21 tests: 17 pre-existing + 4 new)
- `cargo test --workspace` - Passed (all crates, no failures)
- `cargo fmt --all -- --check` - Passed
- `cargo clippy --workspace --all-targets -- -D warnings` - Passed

### Risks/Limitations

1. **Scroll offset math**: The `compute_scroll_offset` function implements a simple "keep selected visible" heuristic that matches Ratatui's default `with_selected` behaviour. If Ratatui changes its scrolling algorithm in a future version, the scroll offset could diverge causing incorrect absolute indices to be registered. The `render_with_regions_scrolled_indices_are_absolute` test guards against regressions.
