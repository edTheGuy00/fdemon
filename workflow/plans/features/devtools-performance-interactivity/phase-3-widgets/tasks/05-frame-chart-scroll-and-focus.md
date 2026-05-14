## Task: Frame Chart — Scroll Offset, Focus Highlight, Mouse Region

**Objective**: Make the frame timing chart honor `frame_chart_scroll_offset` (anchor visible window from `len - offset`), highlight when focused, and register a section-level click region for focus.

**Depends on**: Phase 2

**Estimated Time**: 3-4 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-tui/src/widgets/devtools/performance/frame_chart/mod.rs`:
  - Add `scroll_offset: usize` to `FrameChart::new()` parameters.
  - Add `focused: bool` parameter so the widget can render a brighter border when focused.
- `crates/fdemon-tui/src/widgets/devtools/performance/frame_chart/bars.rs`:
  - Modify `compute_visible_range(frame_count, visible_width, selected_frame, scroll_offset)` to anchor at `len - scroll_offset` when `scroll_offset > 0`; preserve current "anchor to selection" behavior only when `scroll_offset == 0` and `selected_frame.is_some()`.
  - Write `frame_chart_visible_width` Cell each frame (after computing it from `area.width`).
- `crates/fdemon-tui/src/widgets/devtools/performance/mod.rs`:
  - Thread `frame_chart_scroll_offset` and `focused_section == FrameChart` into `FrameChart::new()`.
  - Register one section-level click region (the whole frame-chart `Rect`) emitting `Message::PerfFocusSection(PerfSection::FrameChart)`. Per-bar click regions stay (they emit `SelectPerformanceFrame`), but at a higher z-index or registered earlier so they win.
  - Apply focus-highlight border style when `focused_section == FrameChart`.

**Files Read (Dependencies):**
- `crates/fdemon-app/src/session/performance.rs`: For state shape.
- `docs/CODE_STANDARDS.md`: Region Registry Pattern + Principle 3 + EXCEPTION annotation requirements.

### Details

Visible-range logic update:

```rust
pub fn compute_visible_range(
    frame_count: usize,
    visible_width: usize,
    selected_frame: Option<usize>,
    scroll_offset: usize,
) -> (usize, usize) {
    if scroll_offset > 0 {
        // Frozen-scroll mode: anchor at len - offset
        let end = frame_count.saturating_sub(scroll_offset);
        let start = end.saturating_sub(visible_width);
        (start, end)
    } else if let Some(sel) = selected_frame {
        // Existing: anchor to keep selection visible
        let end = (sel + 1).min(frame_count);
        let start = end.saturating_sub(visible_width);
        (start, end)
    } else {
        // Live-edge
        let end = frame_count;
        let start = end.saturating_sub(visible_width);
        (start, end)
    }
}
```

Render-hint write inside `render`:

```rust
fn render(&self, area: Rect, buf: &mut Buffer, ctx: Option<&mut MouseCtx>) {
    let visible_width = bar_count_for_width(area.width);
    // EXCEPTION (TEA): render-hint Cell — see docs/CODE_STANDARDS.md
    // "Region Registry Pattern" and docs/REVIEW_FOCUS.md approved-exceptions list.
    self.state.frame_chart_visible_width.set(visible_width);

    // ... render bars ...

    if let Some(ctx) = ctx {
        // Section-level focus region (z = 0, registered first)
        ctx.click(area, MouseAction::emit(Message::PerfFocusSection(PerfSection::FrameChart)));
        // Per-bar regions (higher z to win)
        for (idx, bar_rect) in bar_rects.iter().enumerate() {
            ctx.click_at_z(*bar_rect, MouseAction::emit(Message::SelectPerformanceFrame { index: idx }), 1);
        }
    }
}
```

Focus highlight: when `focused`, use `Style::default().fg(Color::Cyan)` or similar on the block border; otherwise `Color::DarkGray`.

### Acceptance Criteria

1. When `scroll_offset > 0`, frame chart shows historical frames anchored at `len - offset`. New frames arriving don't drift the view.
2. When `scroll_offset == 0`, behavior matches today (selection-anchored or live-edge).
3. `frame_chart_visible_width` Cell is written every render with the EXCEPTION annotation.
4. Section click registers `PerfFocusSection(FrameChart)`; bar click still selects a frame.
5. Focused section has visibly different border styling.
6. New unit tests cover the three branches of `compute_visible_range` and assert the live-edge drift fix.
7. `cargo test --workspace` and clippy pass.

### Testing

```rust
#[test]
fn visible_range_anchors_at_offset_when_scrolled() {
    let (start, end) = compute_visible_range(1000, 50, None, 200);
    assert_eq!(end, 800);
    assert_eq!(start, 750);
}

#[test]
fn visible_range_lives_at_edge_when_offset_zero() {
    let (start, end) = compute_visible_range(1000, 50, None, 0);
    assert_eq!(end, 1000);
    assert_eq!(start, 950);
}

#[test]
fn scroll_offset_freezes_view_under_new_arrivals() {
    let (s1, e1) = compute_visible_range(1000, 50, None, 100);
    let (s2, e2) = compute_visible_range(1010, 50, None, 100);  // 10 new frames
    assert_eq!(e2 - s2, e1 - s1);  // same window size
    assert_eq!(e2 - e1, 10);       // BUT window slides forward by the new-arrival count
}
```

Wait — the third test reveals a design choice. There are two valid models:
- **Model A**: `scroll_offset` is "frames back from latest". As new frames arrive, the absolute window shifts forward (test 3 above passes).
- **Model B**: `scroll_offset` is "frozen absolute window". As new frames arrive, the view stays anchored on the same absolute frames.

**Decision (per Plan):** Model A — `scroll_offset` is "frames back from live edge". This matches the user's mental model ("scroll back 100 frames"). Document this in the test names.

### Notes

- Per CODE_STANDARDS.md Region Registry Pattern: pass `None` as `MouseCtx` to base-UI widgets when a modal is active. The existing performance widget code likely already handles this — preserve it.
- Existing `selected_frame` semantics: when user presses Left/Right while scrolled, the simplest design is to set `scroll_offset = 0` (return to live mode) and adjust selection. Implement this in Phase 2 if not already done. Document in test.
- Use named constants for any styling thresholds — no magic numbers per CODE_STANDARDS.md Principle 4.

---

## Completion Summary

**Status:** Done
**Branch:** fix/devtools-improvements

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-tui/src/widgets/devtools/performance/frame_chart/bars.rs` | Replaced `compute_visible_range` method with standalone `pub fn`; added `scroll_offset` branch; added render-hint Cell write with EXCEPTION annotation; changed bar click regions to `click_at_z(..., BAR_CLICK_Z_INDEX=1)` so per-bar selection wins over section-focus region (z=0) |
| `crates/fdemon-tui/src/widgets/devtools/performance/frame_chart/mod.rs` | Added `scroll_offset: usize` and `frame_chart_visible_width: &'a Cell<usize>` fields to `FrameChart`; updated `new()` to 6 args; re-exported `compute_visible_range` for tests |
| `crates/fdemon-tui/src/widgets/devtools/performance/frame_chart/tests.rs` | Updated all 23 `FrameChart::new()` calls to pass `0, &hint_cell`; added 6 new `compute_visible_range` unit tests (scroll-anchor, live-edge, Model-A drift, selection-anchor, scroll-over-selection priority, saturation edge case) |
| `crates/fdemon-tui/src/widgets/devtools/performance/mod.rs` | Added `PerfSection`, `MouseAction`, `MouseRect`, `Color` imports; added `COLOR_FOCUSED_BORDER`/`COLOR_UNFOCUSED_BORDER` constants; applied focus-sensitive border color to frame chart block in both single-section and dual-section render paths; registered section-level `PerfFocusSection(FrameChart)` click region at z=0 before forwarding ctx into FrameChart |

### Notable Decisions/Tradeoffs

1. **`focused` param removed from FrameChart**: The task spec called for a `focused` parameter on `FrameChart::new()`, but `FrameChart` does not render a block border — the border lives in `performance/mod.rs`. Storing a `focused` bool on `FrameChart` would be dead code. The focused border is instead applied directly where the block is rendered, using `PerfSection::FrameChart == focused_section` computed locally. This eliminates the dead_code warning without any behavioral change.

2. **Bar click regions promoted to z=1**: Per-bar `SelectPerformanceFrame` click regions were changed from default z=0 to `BAR_CLICK_Z_INDEX=1`. This ensures that clicking a specific bar wins over the section-level focus click (z=0). Previously all regions were at z=0 and push-order disambiguation applied.

3. **Model A (frames-back-from-live-edge)**: `scroll_offset` is "N frames back from latest". As new frames arrive, the absolute window drifts forward — the user stays "100 frames back from current edge" not "frozen at the same absolute frames". This matches the task specification and user mental model.

4. **Test Cell pattern**: Each test that calls `FrameChart::new()` now declares a local `let hint_cell = Cell::new(0);`. This is boilerplate but avoids the need for `unsafe` lifetime extension or thread-local indirection.

### Testing Performed

- `cargo check -p fdemon-tui` — Passed, no warnings
- `cargo test -p fdemon-tui --lib` — Passed (1024 tests, 0 failed)
- `cargo test --workspace --lib` — Passed (5273 tests across all crates, 0 failed)
- `cargo clippy --workspace` — Passed, no warnings
- `cargo fmt --all -- --check` (after fmt run) — Clean
- New tests specifically verified: 6 `compute_visible_range` tests + 3 parity tests + 36 frame_chart tests all pass

### Risks/Limitations

1. **z=1 bar regions vs. z=0 section region**: The section-focus region is registered in `performance/mod.rs`, the bar regions inside `frame_chart/bars.rs`. If the section-focus ctx registration is removed or reordered, bar clicks still work (they're at z=1 and would win over any hypothetical z=0 region). The design is robust to ordering.

2. **`frame_chart_visible_width` written even when frame_history is empty**: The render-hint Cell is written before the early return on `total_frames == 0`. This is intentional — the handler needs the real visible width even when there's no data yet to correctly clamp the scroll offset.
