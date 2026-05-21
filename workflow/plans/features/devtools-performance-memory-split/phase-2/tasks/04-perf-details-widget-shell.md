## Task: Performance Details Widget Shell — Dual-Pane Layout + Tab Strip + Stub Tabs

**Objective**: Restructure `widgets/devtools/performance/mod.rs` to render the new dual-pane layout (Frame Chart on top, Details pane below) and add a new `details/` sub-tree with a tab strip that dispatches to three tab modules (Frame Analysis, Rebuild Stats, Timeline Events). Phase 2 ships **stub** content for all three tabs — T05 will replace the Frame Analysis stub with the real proportional bar + hints rendering.

**Depends on**: 02 (state foundation: `PerfDetailsTab`, `details_tab` field, `display_refresh_rate`)

**Estimated Time**: 4–6 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-tui/src/widgets/devtools/performance/mod.rs` — restructure `render_impl`: introduce `MIN_DUAL_PANE_HEIGHT`, `MIN_DETAILS_HEIGHT`, `MIN_PHASE_BAR_WIDTH` constants (the last is consumed by T05 but defined here as part of the module's responsive thresholds). Switch from "chart fills inner area" to a `Layout::vertical([chart, details])` split when `inner_h >= MIN_DUAL_PANE_HEIGHT`. The chart-only path remains for short terminals. Replace the `Memory data has moved` module docstring with one describing the new dual-pane layout.
- **NEW** `crates/fdemon-tui/src/widgets/devtools/performance/details/mod.rs` — tab strip rendering + dispatch. Models the structure of `crates/fdemon-tui/src/widgets/devtools/inspector/details/mod.rs` (which the orchestrator already approved as the canonical tab-strip pattern in earlier Inspector parity work).
- **NEW** `crates/fdemon-tui/src/widgets/devtools/performance/details/frame_analysis_tab.rs` — minimal stub renderer. Writes a single line: `"Frame Analysis tab — content lands in T05"`. The file exists so T05 can edit it without a file-creation merge.
- **NEW** `crates/fdemon-tui/src/widgets/devtools/performance/details/rebuild_stats_tab.rs` — renders the "Coming soon — Phase 3 adds widget rebuild tracking." centred placeholder.
- **NEW** `crates/fdemon-tui/src/widgets/devtools/performance/details/timeline_events_tab.rs` — renders the "Coming soon — Phase 3 streams UI/Raster thread timeline events." centred placeholder.
- `crates/fdemon-tui/src/widgets/devtools/performance/tests.rs` — add dual-pane tests (chart + details visible at tall terminals; chart-only at short terminals; tab strip renders three labels; active tab is underlined). Existing chart-only tests must still pass.

**Files Read (Dependencies):**
- `crates/fdemon-tui/src/widgets/devtools/inspector/details/mod.rs` — copy-and-adapt the tab strip rendering (`render_tab_strip`, `TAB_STRIP_HEIGHT`, `TAB_GAP`, `label_for`).
- `crates/fdemon-app/src/state.rs` (T02: `PerfDetailsTab`).
- `crates/fdemon-app/src/session/performance.rs` (T02: `PerformanceState` shape).
- `crates/fdemon-tui/src/widgets/devtools/performance/frame_chart/mod.rs` (existing `FrameChart` widget; consumed unchanged).

### Details

#### Layout constants in `widgets/devtools/performance/mod.rs`

Add near the existing `COMPACT_THRESHOLD: u16 = 7`:

```rust
// ── Responsive layout thresholds ─────────────────────────────────────────────

/// Below this height, show compact summary only (FPS + Jank single line).
const COMPACT_THRESHOLD: u16 = 7;

/// Minimum total inner height to show the dual-pane layout (chart + details).
///
/// Derivation: FrameChart requires ≥ `MIN_CHART_HEIGHT (4) + DETAIL_PANEL_HEIGHT (3) = 7`
/// rows internally. Details pane requires ≥ `MIN_DETAILS_HEIGHT (8)` rows. Inner area
/// is `area.height - 1` (footer) - 2 (chart block borders). So we need 10 inner
/// rows for the chart + 8 for details = 18 rows.
const MIN_DUAL_PANE_HEIGHT: u16 = 18;

/// Minimum details pane height — tab strip (2) + content (≥ 6).
const MIN_DETAILS_HEIGHT: u16 = 8;

/// Minimum content-area width to show the proportional phase bar in the
/// Frame Analysis tab. T05 consumes this constant; T04 defines it.
///
/// Derivation: 4 phase labels × ~9 chars each + 3 separators = 39 cols. Round
/// up to 40 to leave room for borders and padding.
const MIN_PHASE_BAR_WIDTH: u16 = 40;

/// Percentage of the dual-pane inner area allocated to the Frame Chart.
const FRAME_CHART_PCT: u16 = 55;
```

#### `render_impl` decision tree (replacing the current single-pane fill)

```rust
fn render_impl(self, area: Rect, buf: &mut Buffer, mut ctx: Option<&mut MouseCtx<'_>>) {
    // ... existing background clear + disconnected branch ...

    let total_h = area.height;
    if total_h < COMPACT_THRESHOLD {
        self.render_compact_summary(area, buf);
        return;
    }

    // Reserve 1 row at the bottom for the DevTools footer (unchanged).
    let usable = Rect { height: area.height.saturating_sub(1), ..area };

    if usable.height < MIN_DUAL_PANE_HEIGHT {
        // Short terminals — Frame Chart fills the entire usable area, same as Phase 1.
        self.render_chart_only(usable, buf, ctx.as_deref_mut());
        return;
    }

    // Dual-pane layout.
    let chart_h = usable.height.saturating_mul(FRAME_CHART_PCT) / 100;
    let chunks = Layout::vertical([
        Constraint::Length(chart_h),
        Constraint::Min(MIN_DETAILS_HEIGHT),
    ])
    .split(usable);

    self.render_chart_only(chunks[0], buf, ctx.as_deref_mut());
    self.render_details_pane(chunks[1], buf, ctx);
}
```

Move the existing chart-rendering block (border block + FrameChart::new + render_with_regions) into a new helper `render_chart_only(self, area, buf, ctx)`. Keep the focus-border colour logic unchanged.

#### `render_details_pane` helper in `widgets/devtools/performance/mod.rs`

```rust
fn render_details_pane(&self, area: Rect, buf: &mut Buffer, _ctx: Option<&mut MouseCtx<'_>>) {
    // Details pane block — same focus-aware border styling as the chart.
    let details_focused = self.performance.focused_section == PerfSection::Details;
    let border_color = if details_focused { COLOR_FOCUSED_BORDER } else { COLOR_UNFOCUSED_BORDER };
    let block = Block::default()
        .title(format!(" {} Frame Details ", self.icons.activity()))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border_color))
        .title_style(Style::default().fg(palette::ACCENT_DIM));
    let inner = block.inner(area);
    block.render(area, buf);

    // EXCEPTION (TEA): render-hint Cell — see docs/CODE_STANDARDS.md Principle 3.
    self.performance.details_pane_visible_height.set(inner.height as usize);

    details::render(inner, buf, self.performance);
}
```

#### `widgets/devtools/performance/details/mod.rs`

```rust
//! Performance Details pane — tab strip and per-tab dispatch.
//!
//! Renders the tabbed details panel below the Flutter Frames bar chart. The
//! panel contains three tabs:
//!
//! - **Frame Analysis** — populated in Phase 2 ([`frame_analysis_tab`]).
//! - **Rebuild Stats** — Phase 2 stub ([`rebuild_stats_tab`]); populated in Phase 3.
//! - **Timeline Events** — Phase 2 stub ([`timeline_events_tab`]); populated in Phase 3.
//!
//! Tab cycling is keyboard-only in Phase 2 (`]` / `[`). Mouse clicks on tab
//! labels are deferred to Phase 3 (mirrors the inspector details TODO).

use fdemon_app::session::PerformanceState;
use fdemon_app::state::PerfDetailsTab;
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    widgets::Widget,
};

use crate::theme::palette;

mod frame_analysis_tab;
mod rebuild_stats_tab;
mod timeline_events_tab;

// ── Layout constants ──────────────────────────────────────────────────────────

/// Tab strip height — labels row (1) + underline row (1).
const TAB_STRIP_HEIGHT: u16 = 2;

/// Horizontal gap between tab labels in cells.
const TAB_GAP: usize = 3;

fn label_for(tab: PerfDetailsTab) -> &'static str {
    match tab {
        PerfDetailsTab::FrameAnalysis => "Frame Analysis",
        PerfDetailsTab::RebuildStats => "Rebuild Stats",
        PerfDetailsTab::TimelineEvents => "Timeline Events",
    }
}

/// Render the details pane content inside the supplied inner area.
///
/// `area` is the area inside the block — the caller is responsible for the
/// surrounding border and title.
pub(super) fn render(area: Rect, buf: &mut Buffer, performance: &PerformanceState) {
    if area.height == 0 || area.width == 0 {
        return;
    }

    if area.height <= TAB_STRIP_HEIGHT {
        render_tab_strip(area, buf, performance.details_tab);
        return;
    }

    let chunks = Layout::vertical([Constraint::Length(TAB_STRIP_HEIGHT), Constraint::Min(0)])
        .split(area);
    let strip_area = chunks[0];
    let content_area = chunks[1];

    render_tab_strip(strip_area, buf, performance.details_tab);

    match performance.details_tab {
        PerfDetailsTab::FrameAnalysis => frame_analysis_tab::render(content_area, buf, performance),
        PerfDetailsTab::RebuildStats => rebuild_stats_tab::render(content_area, buf),
        PerfDetailsTab::TimelineEvents => timeline_events_tab::render(content_area, buf),
    }
}

/// Render the two-row tab strip with the active tab underlined.
///
/// Mirrors the inspector details tab strip pattern — all three Performance
/// tabs are unconditionally visible in Phase 2 (no `visible_tabs()` predicate).
fn render_tab_strip(area: Rect, buf: &mut Buffer, active: PerfDetailsTab) {
    // ... copy from widgets/devtools/inspector/details/mod.rs::render_tab_strip,
    // adapted for the three Performance tab labels. Active label gets BOLD +
    // ACCENT colour; second row renders ━ under the active label only.
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    // Tests live here — see Acceptance Criteria / Testing sections below.
}
```

#### `details/rebuild_stats_tab.rs`

```rust
//! Rebuild Stats tab — Phase 2 stub. Populated in Phase 3.

use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Rect},
    style::Style,
    widgets::{Paragraph, Widget, Wrap},
};

use crate::theme::palette;

const STUB_MESSAGE: &str = "Coming soon — Phase 3 adds widget rebuild tracking.\nRequires ext.flutter.profileWidgetBuilds.";

pub(super) fn render(area: Rect, buf: &mut Buffer) {
    if area.height == 0 || area.width == 0 {
        return;
    }
    let p = Paragraph::new(STUB_MESSAGE)
        .style(Style::default().fg(palette::TEXT_MUTED))
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true });
    let y_offset = area.height.saturating_sub(2) / 2;
    let centered = Rect { y: area.y + y_offset, height: area.height.saturating_sub(y_offset), ..area };
    p.render(centered, buf);
}
```

#### `details/timeline_events_tab.rs`

Same shape as `rebuild_stats_tab.rs`, message `"Coming soon — Phase 3 streams UI/Raster thread timeline events."`.

#### `details/frame_analysis_tab.rs` (T04 stub — T05 fills in)

```rust
//! Frame Analysis tab — populated in T05. T04 ships a single-line placeholder
//! so the dispatch in [`super::render`] compiles and the layout is testable.

use fdemon_app::session::PerformanceState;
use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Rect},
    style::Style,
    text::Line,
    widgets::{Paragraph, Widget, Wrap},
};

use crate::theme::palette;

pub(super) fn render(area: Rect, buf: &mut Buffer, _performance: &PerformanceState) {
    if area.height == 0 || area.width == 0 {
        return;
    }
    // T05 replaces this with the real frame-number header + total/budget line +
    // proportional phase bar + hint list + no-data / no-selection fallbacks.
    let placeholder = "Frame Analysis (Phase 2 stub — content lands in T05).";
    let p = Paragraph::new(placeholder)
        .style(Style::default().fg(palette::TEXT_MUTED))
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true });
    p.render(area, buf);
}
```

### Acceptance Criteria

1. At `area = 200 × 30` and `connected + monitoring_active == true`: Performance panel renders **two visually distinct sections** — Frame Chart in the top ~55% (FRAME_CHART_PCT), Details pane in the bottom ~45%, both bordered with the focus-aware color scheme. Footer row reserved at the bottom.
2. At `area = 200 × 16`: Performance panel falls back to the **chart-only** layout — Details pane is suppressed. This matches Phase 1's chart-only fill behaviour at short heights.
3. At `area = 200 × 7` (= `COMPACT_THRESHOLD`): the existing compact summary line is rendered. Unchanged from Phase 1.
4. The Details pane shows three tab labels — `Frame Analysis`, `Rebuild Stats`, `Timeline Events` — separated by `TAB_GAP` spaces. The default active tab (`FrameAnalysis`) is **underlined** (`━` characters in the row below the label) and rendered in `BOLD + ACCENT` colour. Inactive tabs use `TEXT_MUTED`.
5. With `performance.details_tab == PerfDetailsTab::RebuildStats`: the dispatch reaches `rebuild_stats_tab::render`, which writes the "Coming soon" string centered.
6. With `performance.details_tab == PerfDetailsTab::TimelineEvents`: the dispatch reaches `timeline_events_tab::render`.
7. With `performance.details_tab == PerfDetailsTab::FrameAnalysis`: the dispatch reaches `frame_analysis_tab::render`. T04's stub renders a single placeholder line; T05 replaces it.
8. `performance.details_pane_visible_height` is **set** by `render_details_pane` to `inner.height as usize` each frame.
9. `cargo fmt --all -- --check && cargo check --workspace --all-targets && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace` is green.

### Testing

Add to `crates/fdemon-tui/src/widgets/devtools/performance/tests.rs` (or a new `details/tests` module — implementor's choice):

```rust
fn render_panel(perf: &PerformanceState, w: u16, h: u16) -> Buffer {
    let mut buf = Buffer::empty(Rect::new(0, 0, w, h));
    let widget = PerformancePanel::new(perf, true, IconSet::default(), &VmConnectionStatus::Connected);
    widget.render(buf.area, &mut buf);
    buf
}

fn collect_buf_text(buf: &Buffer) -> String {
    let mut s = String::new();
    for y in 0..buf.area.height {
        for x in 0..buf.area.width {
            if let Some(c) = buf.cell((x, y)) {
                if let Some(ch) = c.symbol().chars().next() { s.push(ch); }
            }
        }
        s.push('\n');
    }
    s
}

#[test]
fn dual_pane_renders_chart_and_details_at_tall_terminal() {
    let mut perf = PerformanceState::default();
    perf.monitoring_active = true;
    // Push a couple of frames so the chart has something to draw.
    perf.frame_history.push(FrameTiming { number: 1, build_micros: 5_000, raster_micros: 5_000, elapsed_micros: 10_000, timestamp: chrono::Local::now(), phases: None, shader_compilation: false });

    let buf = render_panel(&perf, 200, 30);
    let text = collect_buf_text(&buf);
    assert!(text.contains("Frame Timing"), "expected 'Frame Timing' title in dual-pane mode, got:\n{text}");
    assert!(text.contains("Frame Details"), "expected 'Frame Details' title in dual-pane mode, got:\n{text}");
    assert!(text.contains("Frame Analysis"), "expected 'Frame Analysis' tab label, got:\n{text}");
    assert!(text.contains("Rebuild Stats"), "expected 'Rebuild Stats' tab label, got:\n{text}");
    assert!(text.contains("Timeline Events"), "expected 'Timeline Events' tab label, got:\n{text}");
}

#[test]
fn chart_only_at_short_terminal_below_min_dual_pane() {
    let mut perf = PerformanceState::default();
    perf.monitoring_active = true;
    perf.frame_history.push(FrameTiming { number: 1, build_micros: 5_000, raster_micros: 5_000, elapsed_micros: 10_000, timestamp: chrono::Local::now(), phases: None, shader_compilation: false });

    let buf = render_panel(&perf, 200, 16);
    let text = collect_buf_text(&buf);
    assert!(text.contains("Frame Timing"));
    assert!(!text.contains("Frame Details"), "details pane must be suppressed below MIN_DUAL_PANE_HEIGHT");
}

#[test]
fn details_dispatches_rebuild_stats_stub() {
    let mut perf = PerformanceState::default();
    perf.monitoring_active = true;
    perf.details_tab = PerfDetailsTab::RebuildStats;
    perf.frame_history.push(FrameTiming { number: 1, build_micros: 5_000, raster_micros: 5_000, elapsed_micros: 10_000, timestamp: chrono::Local::now(), phases: None, shader_compilation: false });

    let buf = render_panel(&perf, 200, 30);
    let text = collect_buf_text(&buf);
    assert!(text.contains("Coming soon"), "rebuild stats stub must say 'Coming soon', got:\n{text}");
}

#[test]
fn details_dispatches_timeline_events_stub() {
    let mut perf = PerformanceState::default();
    perf.monitoring_active = true;
    perf.details_tab = PerfDetailsTab::TimelineEvents;
    perf.frame_history.push(FrameTiming { number: 1, build_micros: 5_000, raster_micros: 5_000, elapsed_micros: 10_000, timestamp: chrono::Local::now(), phases: None, shader_compilation: false });

    let buf = render_panel(&perf, 200, 30);
    let text = collect_buf_text(&buf);
    assert!(text.contains("Coming soon"));
}

#[test]
fn active_tab_label_is_underlined() {
    let mut perf = PerformanceState::default();
    perf.monitoring_active = true;
    perf.details_tab = PerfDetailsTab::RebuildStats;
    perf.frame_history.push(FrameTiming { number: 1, build_micros: 5_000, raster_micros: 5_000, elapsed_micros: 10_000, timestamp: chrono::Local::now(), phases: None, shader_compilation: false });

    let buf = render_panel(&perf, 200, 30);
    // Find the row containing the tab labels; the row immediately below it must
    // contain ━ characters under "Rebuild Stats" but not under the other labels.
    // Implementation: scan for the line with all three labels; assert ━ at the
    // x range corresponding to "Rebuild Stats".
}

#[test]
fn details_pane_visible_height_is_written_to_render_hint() {
    let mut perf = PerformanceState::default();
    perf.monitoring_active = true;
    perf.frame_history.push(FrameTiming { number: 1, build_micros: 5_000, raster_micros: 5_000, elapsed_micros: 10_000, timestamp: chrono::Local::now(), phases: None, shader_compilation: false });

    let _ = render_panel(&perf, 200, 30);
    assert!(perf.details_pane_visible_height.get() > 0, "render-hint Cell must be written each frame");
}
```

### Notes

- **Mouse-click regions on tab labels are NOT in scope** for T04. The inspector's `details/mod.rs` has a Phase-2-polish TODO comment for the same feature; the Performance details tab strip follows that pattern. Phase 3 may add click regions emitting `PerfFocusDetailsTab(tab)`.
- **`MIN_PHASE_BAR_WIDTH (40)`** is defined by T04 but consumed by T05. Adding the constant in T04 keeps all responsive thresholds co-located near the top of `performance/mod.rs`.
- **No changes to `frame_chart/`** in T04 — the `FrameChart` widget is reused unchanged. T05 may trim `frame_chart/detail.rs` (specifically the per-frame summary that moves to `frame_analysis_tab.rs`); T04 leaves it intact.
- **`render_compact_summary`** stays as-is — the compact path is below `COMPACT_THRESHOLD = 7` and predates the dual-pane work.
- **`PerfFocusDetailsTab` is NOT consumed yet** by T04. T03 adds the dispatch arm; T04's renderer reads `performance.details_tab` directly. The variant is reserved for Phase 3's mouse-click tab strip.
- **Module-level docstrings**: add `//!` headers to all four new files per `docs/CODE_STANDARDS.md` "Module Documentation". The performance/mod.rs docstring needs updating to describe dual-pane.
- **The `render_with_regions`** function on `PerformancePanel` continues to forward `MouseCtx` into the Frame Chart section. Phase 3's tab strip clicks will require a second `MouseCtx` thread; T04 forwards `None` to `details::render` for now.
- **Active panel border color**: the dual-pane layout introduces a second focusable section. The existing constants `COLOR_FOCUSED_BORDER` (Cyan) and `COLOR_UNFOCUSED_BORDER` (DarkGray) apply to both. Only one section is focused at a time.
- **Trimming `frame_chart/detail.rs`**: deferred to T05. T04 leaves the file untouched so the chart-only fallback path keeps rendering its existing 3-line detail panel.

---

## Completion Summary

**Status:** Done
**Branch:** worktree-agent-ac4304da20a4e15c5

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-tui/src/widgets/devtools/performance/mod.rs` | Restructured with dual-pane layout: added `MIN_DUAL_PANE_HEIGHT`, `MIN_DETAILS_HEIGHT`, `MIN_PHASE_BAR_WIDTH`, `FRAME_CHART_PCT` constants; split render path into `render_chart_only` + `render_details_pane`; added `details` module; updated module docstring |
| `crates/fdemon-tui/src/widgets/devtools/performance/details/mod.rs` | NEW — tab strip + dispatch, `render_tab_strip`, `label_for`, inline tests |
| `crates/fdemon-tui/src/widgets/devtools/performance/details/frame_analysis_tab.rs` | NEW — T04 stub placeholder |
| `crates/fdemon-tui/src/widgets/devtools/performance/details/rebuild_stats_tab.rs` | NEW — "Coming soon" stub |
| `crates/fdemon-tui/src/widgets/devtools/performance/details/timeline_events_tab.rs` | NEW — "Coming soon" stub |
| `crates/fdemon-tui/src/widgets/devtools/performance/tests.rs` | Added 6 new dual-pane tests; updated `test_performance_panel_no_stats_section` to handle "Rebuild Stats" tab label |

### Notable Decisions/Tradeoffs

1. **`test_performance_panel_no_stats_section` update**: The test previously checked for `" Stats "` substring to assert the old Stats block was removed. The new "Rebuild Stats" tab label contains the substring `" Stats "` (space before Stats from "Rebuild", then stats, then gap spaces), so the assertion was updated to check for the old block-title pattern `"─ Stats"` or `"Memory Stats"` instead. This preserves the original intent.

2. **`MIN_PHASE_BAR_WIDTH` dead-code suppression**: Defined in this module as specified, with a `const _ = MIN_PHASE_BAR_WIDTH;` dummy reference to prevent dead-code lint. This keeps all responsive layout thresholds co-located as the task plan specifies.

3. **Clippy struct-init pattern**: New tests use `PerformanceState { field: val, ..Default::default() }` initializer syntax to satisfy `field_reassign_with_default` lint.

### Testing Performed

- `cargo fmt --all -- --check` - Passed
- `cargo check --workspace --all-targets` - Passed
- `cargo clippy --workspace --all-targets -- -D warnings` - Passed
- `cargo test --workspace` - Passed (5,756+ tests, 0 failed)
  - New tests in `performance::tests` — 6 tests added, all passed
  - New tests in `performance::details::tests` — 14 tests added, all passed

### Risks/Limitations

1. **T05 stub**: `frame_analysis_tab.rs` renders a single placeholder line. T05 must replace this with real content.
2. **Mouse click regions on tabs**: Not implemented per task scope. Phase 3 can add click regions.
3. **`render_with_regions` parity**: The `render_with_regions` parity test now renders at 80×24 which falls below `MIN_DUAL_PANE_HEIGHT` (18) + compact threshold — the buffers remain identical since both paths go through `render_chart_only`.
