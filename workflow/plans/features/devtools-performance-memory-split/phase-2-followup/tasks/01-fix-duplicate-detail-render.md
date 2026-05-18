## Task: Fix C1 — Suppress duplicate per-frame detail in dual-pane mode

**Objective:** When the Performance panel runs in dual-pane mode (terminal tall enough that the Frame Analysis tab is visible), the FrameChart must NOT render its own 3-row detail panel for the selected frame — that data now lives in the Frame Analysis tab. The chart-only fallback (small terminal, no tab visible) must continue rendering the chart's detail strip as before.

**Depends on:** — (Wave 1)

**Agent:** implementor

**Estimated Time:** 1.5–2 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-tui/src/widgets/devtools/performance/frame_chart/mod.rs` — add a `dual_pane: bool` field to `FrameChart` and a corresponding constructor parameter; thread it into the `Widget::render` and `render_with_regions` implementations so the chart can skip its internal detail-panel slot when `dual_pane == true`.
- `crates/fdemon-tui/src/widgets/devtools/performance/frame_chart/detail.rs` — gate `render_detail_panel` so the per-frame detail (`render_frame_detail`) is suppressed in dual-pane mode; the no-selection summary line (`render_summary_line`) continues to render in dual-pane mode in the chart's bottom strip so the chart still shows aggregate stats when no frame is selected.
- `crates/fdemon-tui/src/widgets/devtools/performance/mod.rs` — pass `dual_pane: true` from the dual-pane render branch and `dual_pane: false` from the chart-only render branch (and the compact-mode branch, if it also instantiates `FrameChart`).
- `crates/fdemon-tui/src/widgets/devtools/performance/tests.rs` — add the C1 regression tests described below.

**Files Read (Dependencies):**
- `crates/fdemon-tui/src/widgets/devtools/performance/details/frame_analysis_tab.rs` — to verify the Frame Analysis tab is the surviving renderer for the selected-frame detail in dual-pane mode.

### Background

The Phase 2 plan stated: *"the per-frame summary that lives there today moves into `frame_analysis_tab.rs`. The no-selection FPS / Avg / Jank / Shader summary line stays in `frame_chart/detail.rs` because the chart-only fallback (small terminal) still uses it."* The implementation kept both code paths active. At 200×30 with a frame selected, the chart still consumes 3 rows for `Frame #N  Total: … / UI: … / Raster: …`, while the Details pane below renders the same frame number, total/budget verdict, phase bar, and hints. The user sees the same data twice.

The doc comment at `frame_chart/detail.rs:21-24` already documents the intended contract:

```rust
/// **Used only in the chart-only fallback** (`area.height < MIN_DUAL_PANE_HEIGHT`).
/// The dual-pane Performance layout renders frame details inside the Details
/// pane via [`super::super::details::frame_analysis_tab`] — that path supersedes
/// this one when the terminal is tall enough.
```

The contract is not enforced by the caller. The fix is to enforce it.

### Design Choice

`FrameChart::new` currently takes `(frame_history, selected_frame, stats, icons, scroll_offset, frame_chart_visible_width)`. Add a 7th parameter `dual_pane: bool`. Set on construction by the parent `performance/mod.rs`:

- `render_chart_only` (the chart-only fallback, used when `usable.height < MIN_DUAL_PANE_HEIGHT`) → `dual_pane: false`
- `render_chart_panel_dual` / the dual-pane branch → `dual_pane: true`
- The compact-summary branch (`total_h < COMPACT_THRESHOLD`) does not instantiate `FrameChart`, so it is unaffected.

Inside `FrameChart`:

- `Widget::render` and `render_with_regions` keep their existing area-split logic. The only change is that `render_detail_panel` is called only when `!self.dual_pane`, OR `render_detail_panel` is split into two paths (`render_frame_detail` when a frame is selected and we're in chart-only mode; `render_summary_line` when no frame is selected — that line stays in both modes).
- When `dual_pane == true` and `selected_frame.is_some()`, the chart should still call `render_summary_line` in the bottom strip so the chart's strip remains useful (it shows FPS/Avg/Jank/Shader). When `dual_pane == false`, behaviour is unchanged.

This preserves the existing pixel layout — the chart still reserves `DETAIL_PANEL_HEIGHT` rows for its strip, whose content shifts from per-frame detail to the aggregate summary when in dual-pane.

> ALTERNATIVE: If reviewers prefer not to widen the bottom-strip semantics, an alternative is to also drop the `DETAIL_PANEL_HEIGHT` reservation in dual-pane mode, giving the chart more bar rows. This is a layout change with broader test churn; prefer the strip-swap approach above unless a reviewer pushes back.

### Details

1. **Add `dual_pane: bool` to `FrameChart`** (`frame_chart/mod.rs`):
   - Add the field to the struct.
   - Extend `FrameChart::new` to accept `dual_pane: bool` as the final parameter, with a doc-comment explaining the contract.
   - Thread `self.dual_pane` into both `Widget::render` and `render_with_regions`.

2. **Update `render_detail_panel`** (`frame_chart/detail.rs`):
   - When the panel is called, branch on `self.dual_pane`:
     - `dual_pane == true && self.selected_frame.is_some()` → render the no-selection summary line (`render_summary_line`) instead of the per-frame detail. This keeps the chart's bottom strip useful in dual-pane mode without duplicating data.
     - `dual_pane == true && self.selected_frame.is_none()` → render the summary line (unchanged from today).
     - `dual_pane == false` → render the existing branch (`render_frame_detail` if selected, else `render_summary_line`).
   - Update the doc comment to describe the new contract precisely.

3. **Update callsites** (`performance/mod.rs`):
   - In the dual-pane branch of `render_impl`, when constructing the `FrameChart` for the upper pane, pass `dual_pane: true`.
   - In `render_chart_only` (the chart-only fallback), pass `dual_pane: false`.

4. **Add regression tests** (`widgets/devtools/performance/tests.rs`):

   ```rust
   #[test]
   fn frame_detail_renders_once_at_200x30_with_selection() {
       // Set up a session with a selected frame, details_tab = FrameAnalysis.
       // Render at 200×30 (dual-pane).
       // Assert: substring "Frame #" appears in buffer count == 1 (only the
       // Frame Analysis tab's header line, not the chart's detail strip).
   }

   #[test]
   fn chart_only_fallback_still_renders_frame_detail() {
       // Set up a session with a selected frame.
       // Render at 200×16 (chart-only — usable.height < MIN_DUAL_PANE_HEIGHT).
       // Assert: substring "Frame #" appears in buffer (the chart's strip still
       // renders the per-frame detail when no Details pane is visible).
   }

   #[test]
   fn dual_pane_chart_strip_shows_summary_when_frame_selected() {
       // Set up a session with a selected frame.
       // Render at 200×30 (dual-pane).
       // Assert: the chart's bottom strip area contains "FPS:" or "Avg:" text
       // (the summary line), confirming the strip is not blank.
   }
   ```

   Use the existing helper pattern in `tests.rs` for buffer assertions. Reuse fixture builders if available.

### Acceptance Criteria

1. `FrameChart::new` takes a `dual_pane: bool` parameter; all call sites pass an explicit value (no defaults / unwraps).
2. At 200×30 with a frame selected, the rendered buffer contains exactly one occurrence of the substring `"Frame #"`. The Frame Analysis tab's header line carries it.
3. At 200×30 with a frame selected, the chart's bottom strip area contains the aggregate summary line (`FPS:` or `Avg:`).
4. At 200×16 with a frame selected (chart-only fallback), the chart's bottom strip still renders the per-frame detail (`Frame #N  Total:`).
5. All three new tests pass, and the existing Performance tests still pass.
6. `cargo fmt --all -- --check && cargo check --workspace --all-targets && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings` is green.

### Testing

- `cargo test -p fdemon-tui widgets::devtools::performance` — runs all Performance-panel tests including the new ones.
- `cargo test --workspace` — full quality gate.

### Risk

- **Widening `FrameChart::new`'s signature** ripples through all call sites — there are three (dual-pane, chart-only, plus possibly tests). Each must be updated.
- The bottom-strip swap (per-frame detail → aggregate summary) is a *visible UX change* in dual-pane mode. Reviewers may prefer to skip the bottom strip entirely in dual-pane and instead give those 3 rows back to the bar chart. If pushed, fall back to that approach: when `dual_pane`, set `chart_h = total_h` (no strip reservation) and skip the strip entirely. Either approach satisfies C1.

### Out of Scope

- Do NOT modify `frame_analysis_tab.rs`. The Frame Analysis tab already renders the per-frame detail correctly.
- Do NOT touch `MIN_DUAL_PANE_HEIGHT`, `MIN_DETAILS_HEIGHT`, or any layout-threshold constants. T04 may adjust comments on these but never the values.
- Do NOT touch any handler files. This is a render-only change.
