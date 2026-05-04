## Task: Performance Frame-Chart Bar Region Recording

**Objective**: Inside the Performance panel's bar-chart rendering loop (`widgets/devtools/performance/frame_chart/bars.rs::render_bar_chart`), register one `MouseAction::Emit(Message::SelectPerformanceFrame { index: Some(global_idx) })` rect per visible frame slot. Rects are `CHARS_PER_FRAME` (3 cols) wide and `chart_h` rows tall — clicking anywhere in the bar pair (UI + Raster) selects the frame.

**Depends on**: Task 02 (sister-function scaffold for `PerformancePanel`)

**Estimated Time**: 1 hour

### Scope

**Files Modified (Write):**
- `crates/fdemon-tui/src/widgets/devtools/performance/mod.rs`: Replace the no-op delegation in `render_with_regions` with a real body that splits into frame-chart + memory-chart sections (mirroring the existing `Widget::render`), passing `ctx` only into the frame chart.
- `crates/fdemon-tui/src/widgets/devtools/performance/frame_chart/bars.rs`: Promote `render_bar_chart` to accept `Option<&mut MouseCtx<'_>>`. Register one click region per visible frame slot inside the existing `for (slot, frame) in visible.iter().enumerate()` loop.

**Files Read (Dependencies):**
- `crates/fdemon-app/src/message.rs::Message::SelectPerformanceFrame`
- `crates/fdemon-app/src/mouse_regions.rs::MouseAction::emit`, `MouseRect`
- `crates/fdemon-tui/src/widgets/devtools/performance/frame_chart/mod.rs::CHARS_PER_FRAME` (= 3)

### Details

#### Recording strategy

In `render_bar_chart`, the existing loop already computes:

```rust
for (slot, frame) in visible.iter().enumerate() {
    let global_idx = start_idx + slot;
    let x = area.x + (slot as u16) * CHARS_PER_FRAME;

    if x + 1 >= area.right() {
        break;
    }
    // ... render bars ...
}
```

Augment to register a click region per slot:

```rust
for (slot, frame) in visible.iter().enumerate() {
    let global_idx = start_idx + slot;
    let x = area.x + (slot as u16) * CHARS_PER_FRAME;
    if x + 1 >= area.right() {
        break;
    }

    // ... existing bar rendering ...

    if let Some(c) = ctx.as_deref_mut() {
        // Width: 2 cells for UI + Raster bars, plus the 1-cell gap between
        // them and the next slot. Last slot may have less than the full
        // CHARS_PER_FRAME if the chart ends right after.
        let avail = area.right().saturating_sub(x);
        let rect_w = CHARS_PER_FRAME.min(avail);
        if rect_w == 0 || area.height == 0 {
            continue;
        }
        let rect = MouseRect::new(x, area.y, rect_w, area.height);
        c.click(
            rect,
            MouseAction::emit(Message::SelectPerformanceFrame {
                index: Some(global_idx),
            }),
        );
    }
}
```

The `area` here is `chart_area` — the bar-chart-only sub-rect of the Performance panel (height = `total_h - DETAIL_PANEL_HEIGHT`, width = full panel width). Clicks on the detail panel below do *not* match these regions.

#### Sister function for `render_bar_chart`

Two options:

**(A) Add a parameter to the existing function.** Update every caller to pass `None`. One file (frame_chart/mod.rs), one caller (`render` impl), low surface.

**(B) Add a new sibling function `render_bar_chart_with_regions`.** Existing callers don't change; the new path uses the new fn.

Pick (A) — fewer functions, the parameter is single-use.

```rust
pub(super) fn render_bar_chart(
    &self,
    area: Rect,
    buf: &mut Buffer,
    mut ctx: Option<&mut MouseCtx<'_>>,
) {
    // existing body, with `ctx.as_deref_mut()` calls inside the per-slot loop
}
```

Then in `frame_chart/mod.rs::Widget::render`:

```rust
self.render_bar_chart(chart_area, buf, None);
```

And add a `render_with_regions` free function or method:

```rust
pub fn render_with_regions(
    self,
    area: Rect,
    buf: &mut Buffer,
    ctx: Option<&mut MouseCtx<'_>>,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let total_h = area.height;
    if total_h < MIN_CHART_HEIGHT + DETAIL_PANEL_HEIGHT {
        self.render_summary_line(area, buf);
        return;
    }
    let chart_h = total_h - DETAIL_PANEL_HEIGHT;
    let chart_area = Rect { x: area.x, y: area.y, width: area.width, height: chart_h };
    let detail_area = Rect { x: area.x, y: area.y + chart_h, width: area.width, height: DETAIL_PANEL_HEIGHT };

    self.render_bar_chart(chart_area, buf, ctx);
    self.render_detail_panel(detail_area, buf);
}
```

#### `widgets/devtools/performance/mod.rs::render_with_regions`

```rust
pub fn render_with_regions(
    area: Rect,
    buf: &mut Buffer,
    widget: PerformancePanel<'_>,
    ctx: Option<&mut MouseCtx<'_>>,
) {
    // Mirror the existing Widget::render structure exactly.
    if !widget.vm_connected {
        widget.render_disconnected(area, buf);
        return;
    }
    // ... layout split into frame timing + memory chart sections ...
    // Forward ctx only into the frame-timing section.
    let frame_chart = FrameChart::new(
        widget.perf.frame_history(),
        widget.perf.selected_frame,
        widget.perf.stats(),
        widget.icons.use_unicode(),
    );
    frame_chart.render_with_regions(frame_section, buf, ctx);

    // Memory chart — no clicks in v1.
    let memory_chart = MemoryChart::new(/* ... */);
    memory_chart.render(memory_section, buf);
}
```

(Adapt to the actual `PerformancePanel` field names.)

### Acceptance Criteria

1. After `render_with_regions(...)` with `Some(ctx)` and 8 frames in `frame_history`, the registry contains 8 click regions each carrying `MouseAction::Emit(Message::SelectPerformanceFrame { index: Some(i) })` where `i` is the global frame index.
2. Each region has `width = CHARS_PER_FRAME` (= 3) except the right-most slot which clamps to `area.right() - x` if less.
3. Each region has `height = chart_area.height` (the full bar-chart vertical span).
4. Click regions don't extend below `chart_area.bottom()` — the detail panel area has no bar regions.
5. With more frames than `area.width / CHARS_PER_FRAME`, only the visible window registers regions (matching the existing `compute_visible_range` behaviour).
6. `vm_connected = false` path → `render_disconnected` runs and no regions are registered.
7. Compact-mode path (`total_h < MIN_CHART_HEIGHT + DETAIL_PANEL_HEIGHT`) → `render_summary_line` runs and no regions are registered.
8. Calling the existing `Widget::render` (without ctx) registers no regions.
9. `cargo test --workspace`, `cargo fmt`, `cargo clippy -- -D warnings`, `cargo check` pass. ≥ 2 new unit tests.

### Testing

```rust
#[test]
fn frame_chart_records_one_region_per_visible_frame() {
    use fdemon_app::message::Message;
    use fdemon_app::{MouseRegions, MouseAction};
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;
    use crate::render::MouseCtx;
    use fdemon_core::performance::FrameTiming;

    let mut history = fdemon_core::performance::RingBuffer::new(120);
    for i in 1..=8 {
        history.push(FrameTiming { number: i, build_micros: 5_000, raster_micros: 5_000, elapsed_micros: 10_000, timestamp: chrono::Local::now(), phases: None, shader_compilation: false });
    }
    let stats = Default::default();
    let chart = FrameChart::new(&history, None, &stats, true);

    let mut regions = MouseRegions::default();
    let area = Rect::new(0, 0, 80, 24);
    let mut buf = Buffer::empty(area);
    {
        let builder = regions.builder();
        let mut ctx = MouseCtx::new(builder);
        chart.render_with_regions(area, &mut buf, Some(&mut ctx));
    }

    let frame_clicks: Vec<usize> = regions
        .iter()
        .filter_map(|e| match e.on_left.as_ref().and_then(|a| a.as_emit()) {
            Some(Message::SelectPerformanceFrame { index: Some(i) }) => Some(*i),
            _ => None,
        })
        .collect();
    assert_eq!(frame_clicks, vec![0, 1, 2, 3, 4, 5, 6, 7]);
}

#[test]
fn frame_chart_in_compact_mode_records_no_regions() {
    use fdemon_app::{MouseRegions};
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;
    use crate::render::MouseCtx;
    use fdemon_core::performance::FrameTiming;

    let mut history = fdemon_core::performance::RingBuffer::new(120);
    history.push(FrameTiming { number: 1, build_micros: 5_000, raster_micros: 5_000, elapsed_micros: 10_000, timestamp: chrono::Local::now(), phases: None, shader_compilation: false });
    let stats = Default::default();
    let chart = FrameChart::new(&history, None, &stats, true);

    // Compact: height < MIN_CHART_HEIGHT + DETAIL_PANEL_HEIGHT (= 4 + 3 = 7).
    let mut regions = MouseRegions::default();
    let area = Rect::new(0, 0, 80, 5);
    let mut buf = Buffer::empty(area);
    {
        let builder = regions.builder();
        let mut ctx = MouseCtx::new(builder);
        chart.render_with_regions(area, &mut buf, Some(&mut ctx));
    }

    assert_eq!(regions.iter().count(), 0, "compact mode → no regions");
}
```

### Notes

- **Why a frame-wide rect (`width = 3`).** The bar pair at slot N occupies columns `x` and `x+1`; column `x+2` is the gap to the next bar. Including the gap in the click target makes each frame's click target larger and easier to hit, especially with mouse-precision constraints in terminals.
- **Why click selects rather than deselects.** Clicking an already-selected frame currently re-emits `SelectPerformanceFrame { index: Some(i) }`. The handler at `handler/devtools/performance.rs::handle_select_performance_frame` simply re-assigns; no harm. Future refinement: clicking the selected bar could clear (`index: None`) — but that's a UX decision worth deferring.
- **Detail panel below the chart is not clickable in v1.** The frame number / timing labels in the detail panel could plausibly become click targets (e.g., copy timing to clipboard) but that's deferred. We pass the detail-panel area to `render_detail_panel` without ctx.
- **Memory chart not clickable in v1.** It's a time-series with no obvious per-pixel meaning. Phase 5 may add overlay-toggle clicks (e.g., toggle GC events visibility).
- **Empty-frame-history path.** When `total_frames == 0`, `compute_visible_range` returns an empty window and the loop runs zero times — no regions registered. Correct behaviour.
- **Selection highlight `▔`.** The selection indicator is drawn at the chart's top row only when the frame is already selected. Phase 4 click → set selected → next render draws the indicator. No interaction needed at the indicator level.
- **`MouseCtx::as_deref_mut()` pattern.** The same single-binding-shadowing pattern from Task 02 applies inside the per-slot loop. Don't move `ctx` inside the loop body without a re-borrow.
