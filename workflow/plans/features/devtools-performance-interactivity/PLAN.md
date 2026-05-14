# Plan: DevTools Performance Tab Interactivity

## TL;DR

The DevTools Performance tab currently supports keyboard frame-bar selection (Left/Right) and per-bar clicks, but the rest of the panel is read-only. This feature adds **section focus** (frame chart / memory chart / memory consumption list), **scroll-history support** for both charts, and **scroll + row selection + click** for the allocation table. State is added per-session; new `Message` variants drive scrolling and focus changes; widgets accept scroll offsets and render-hint visible-height cells. The frame-history ring buffer is extended from 300 (~5 s) to 1800 frames (~30 s) so scroll-back is actually useful. Mouse support follows the established `MouseCtx` / `MouseRegions` pattern.

---

## Background

Per CLAUDE.md and `docs/ARCHITECTURE.md`, the DevTools Performance panel renders three sections: a frame timing bar chart, a memory time-series chart, and a memory allocation table. Today:

- **Frame chart**: Left/Right keys move `selected_frame`; per-bar clicks select a frame. No standalone "scroll the viewport back" mode — moving the selection drives both bar highlight and viewport anchor.
- **Memory chart**: Render-only. Always shows the full ring buffer window (~60 s of 500 ms samples). No scroll, no focus, no clicks.
- **Memory allocation table**: Capped at 10 rows (`MAX_TABLE_ROWS`). No scroll, no row selection, no clicks. The `s` key toggles sort column.

The user reported they cannot scroll the memory consumption list, cannot scroll back through chart history, and cannot click between sections to focus them. With the new mouse-region registry pattern (`docs/CODE_STANDARDS.md` Region Registry Pattern, established in commit `1cf2068`), full interactivity is now feasible.

---

## Affected Modules

- `crates/fdemon-app/src/session/performance.rs` — Add `PerfSection` enum + focus/scroll fields + `Cell<usize>` render-hints to `PerformanceState`. Extend frame history capacity.
- `crates/fdemon-app/src/message.rs` — Add 6 new `Message` variants.
- `crates/fdemon-app/src/handler/keys.rs` — Bind `Tab`/`Shift+Tab`, `j/k`/arrows, `PageUp`/`PageDown`, `Home`/`End` for Performance panel; route by `focused_section`.
- `crates/fdemon-app/src/handler/devtools/performance.rs` — Add handlers for focus + scroll + row selection.
- `crates/fdemon-app/src/handler/update.rs` — Route new message variants.
- `crates/fdemon-tui/src/widgets/devtools/performance/mod.rs` — Forward focus + scroll offsets; forward `MouseCtx` to memory section.
- `crates/fdemon-tui/src/widgets/devtools/performance/frame_chart/mod.rs` + `bars.rs` — Accept scroll offset; write visible-width render-hint; register section-level click region.
- `crates/fdemon-tui/src/widgets/devtools/performance/memory_chart/mod.rs` + `chart.rs` — Accept scroll offset; write visible-width render-hint; register section-level click region.
- `crates/fdemon-tui/src/widgets/devtools/performance/memory_chart/table.rs` — Accept scroll offset + selected row; render windowed slice; write visible-height render-hint; register per-row click regions.
- `docs/KEYBINDINGS.md` — Document new Performance key bindings.
- `docs/ARCHITECTURE.md` — Document the focused-section model + render-hint cells (`doc_maintainer`).

---

## Development Phases

### Phase 1: State + Messages

**Goal**: Introduce `PerfSection`, scroll offsets, render-hint cells, and the new `Message` variants. No behavior change yet (state exists but is unused by handlers/widgets).

**Duration**: 2-3 hours.

#### Steps

1. **Define `PerfSection` enum + scroll fields on `PerformanceState`**
   - Add `PerfSection` (`FrameChart`, `MemoryChart`, `MemoryList`) in `session/performance.rs`.
   - Add fields: `focused_section: PerfSection`, `frame_chart_scroll_offset: usize`, `memory_chart_scroll_offset: usize`, `alloc_table_scroll_offset: usize`, `alloc_table_selected_row: Option<usize>`.
   - Add render-hint cells: `frame_chart_visible_width: Cell<usize>`, `memory_chart_visible_width: Cell<usize>`, `alloc_table_visible_height: Cell<usize>` — each with the standard `// EXCEPTION (TEA): render-hint Cell — see docs/REVIEW_FOCUS.md` annotation.
   - Update `Default` impl and any `with_*` constructors.

2. **Extend frame-history capacity**
   - Bump `DEFAULT_FRAME_HISTORY_SIZE` from 300 to **1800** (~30 s at 60 FPS).
   - Confirm `RingBuffer` capacity-change does not need migration logic.

3. **Add `Message` variants**
   - `PerfFocusSection(PerfSection)`
   - `PerfScrollUp`, `PerfScrollDown`
   - `PerfPageUp`, `PerfPageDown`
   - `PerfJumpToStart`, `PerfJumpToEnd` (for `Home` / `End`)
   - `PerfSelectAllocRow { index: Option<usize> }`
   - Update `Message` Debug/PartialEq derives where required.

**Milestone**: All state fields and message variants compile. No user-visible behavior change yet.

---

### Phase 2: Handlers

**Goal**: Wire the new messages into `update()` and into per-handler logic in `handler/devtools/performance.rs`. Keyboard bindings are extended.

**Duration**: 3-4 hours.

#### Steps

1. **Key bindings (`handler/keys.rs`)**
   - Add `in_performance` guards (analogous to existing `in_inspector` / `in_network` guards).
   - Bind `Tab` / `Shift+Tab` → `PerfFocusSection(next/prev_section)`.
   - Bind `Up` / `Down` / `j` / `k` → `PerfScrollUp` / `PerfScrollDown`.
   - Bind `PageUp` / `PageDown` → `PerfPageUp` / `PerfPageDown`.
   - Bind `Home` / `End` → `PerfJumpToStart` / `PerfJumpToEnd`.
   - Existing Left/Right + click-on-bar behavior preserved.

2. **Section focus handler**
   - `handle_perf_focus_section(state, section)` → updates `perf_state.focused_section`.
   - Cycle order on Tab: FrameChart → MemoryChart → MemoryList → FrameChart.

3. **Scroll handlers**
   - `handle_perf_scroll_up/down`: branch on `focused_section`.
     - FrameChart: increment/decrement `frame_chart_scroll_offset`, clamp to `[0, frame_history.len() - frame_chart_visible_width.get()]`.
     - MemoryChart: same logic against `memory_samples`.
     - MemoryList: move `alloc_table_selected_row` (and adjust `alloc_table_scroll_offset` to keep selection visible using `alloc_table_visible_height` hint).
   - `handle_perf_page_up/down`: shift offsets by `visible_width/height` (with fallback if hint == 0).
   - `handle_perf_jump_to_start/end`: set offsets to max / 0 respectively; for MemoryList, jump selected row.

4. **Row selection handler**
   - `handle_perf_select_alloc_row(state, index)` → set `alloc_table_selected_row` and `focused_section = MemoryList`.

**Milestone**: Pressing `Tab` cycles focus (logged in state); scroll keys mutate offsets; nothing renders yet because widgets ignore the new state.

---

### Phase 3: Widget Wiring

**Goal**: Widgets honor focus + scroll + selection; mouse regions are registered for section focus and row selection. Render-hints are written every frame.

**Duration**: 4-5 hours.

#### Steps

1. **`performance/mod.rs` — top-level orchestration**
   - Compute `focus_block_style` based on `focused_section` (e.g., focused section gets a brighter border).
   - Forward `frame_chart_scroll_offset` to FrameChart widget.
   - Forward `memory_chart_scroll_offset` to MemoryChart widget.
   - Forward `MouseCtx` to memory section (currently passed as `None`).
   - Register section-level click regions (whole `Rect` of each section) → emit `PerfFocusSection(...)`.

2. **`performance/frame_chart/`**
   - Accept `scroll_offset: usize` in `FrameChart::new()`.
   - In `bars.rs`, modify `compute_visible_range` to anchor on `total_frames - scroll_offset` rather than `total_frames`. When `scroll_offset == 0`, behavior matches today (live edge).
   - Write `frame_chart_visible_width` Cell each frame with the rendered width.
   - When a frame is selected (`selected_frame != None`), keep current "anchor to selection" behavior — but only if `scroll_offset == 0`. If user has scrolled, do not auto-track.

3. **`performance/memory_chart/`**
   - In `chart.rs`, accept `scroll_offset` and slice the visible window from `memory_samples` accordingly.
   - Write `memory_chart_visible_width` Cell each frame.
   - In `mod.rs`, accept and forward `MouseCtx`; register section click region.

4. **`performance/memory_chart/table.rs`**
   - Replace `MAX_TABLE_ROWS = 10` cap with `visible_height` from render area.
   - Accept `scroll_offset` and `selected_row`; render windowed slice of profile entries.
   - Use `profile.members` directly (not `top_by_size(10)`) and apply sort inline.
   - Write `alloc_table_visible_height` Cell.
   - Register one click region per visible row → `PerfSelectAllocRow { index: row }`.
   - Highlight `selected_row`.

**Milestone**: All three sections respond to focus + scroll + click; chart history scrollable; allocation list scrollable and selectable.

---

### Phase 4: Polish + Documentation

**Goal**: Verify behavior, update docs, write integration tests for live-edge drift and conflict cases.

**Duration**: 2-3 hours.

#### Steps

1. **Live-edge drift tests**
   - Test: scroll back by 50 frames; advance the ring buffer by 10 frames (simulate new arrivals); confirm rendered window stays anchored where the user scrolled.
   - Test: scroll back, then press `End` → returns to live edge.
   - Test: select a frame via Left/Right while scrolled; verify behavior matches the design (Left/Right adjusts both `selected_frame` and `scroll_offset` to keep selection in view, or scroll-only mode clears selection — chosen design documented in task).

2. **Update `docs/KEYBINDINGS.md`** with the new Performance key bindings.

3. **Update `docs/ARCHITECTURE.md`** "DevTools Subsystem" section to mention `PerfSection`, scroll-offset model, and render-hint cells. (`doc_maintainer` task).

4. **Manual verification**
   - Run fdemon, enter DevTools → Performance.
   - Verify Tab cycles focus visibly (border style change).
   - Verify mouse clicks focus sections and select rows.
   - Verify j/k/PageUp/PageDown scroll each focused section.
   - Verify Home/End jump correctly.
   - Verify frame chart scroll-back stays anchored under live load.

**Milestone**: Full keyboard + mouse interactivity, polished UX, docs updated.

---

## Edge Cases & Risks

### Live-edge drift while scrolled
- **Risk:** New samples arrive while user is scrolled back; if renderer anchors to `total` instead of `total - offset`, the view drifts right.
- **Mitigation:** Anchor every render at `len - scroll_offset`; clamp scroll_offset on render if buffer grew. Unit-tested in Phase 4.

### `selected_frame` vs. `frame_chart_scroll_offset` conflict
- **Risk:** Two independent anchors can disagree about what's visible.
- **Mitigation:** Decision required at task-design time — design A: scroll keys clear `selected_frame`; Left/Right both move selection and reset scroll to 0 (live mode). Design B: keep them fully independent; renderer shows selection marker even off-screen via edge arrow. **Recommended: A** (simpler model, fewer renderer cases).

### `Cell<usize>` cloning
- **Risk:** `PerformanceState` derives `Clone`; cloned render-hints carry old values.
- **Mitigation:** Stale values are harmless (handlers fall back to defaults when hint == 0). Already standard practice in the codebase (see Principle 3 in CODE_STANDARDS.md).

### Profile member size
- **Risk:** Switching from `top_by_size(10)` to `profile.members` directly + inline sort may impact render cost if `members` is large.
- **Mitigation:** Cap the sorted slice at `visible_height + scroll_offset + buffer` rows to avoid sorting thousands of classes.

### Mouse region overlap with existing frame bars
- **Risk:** Section-level click region for FrameChart could intercept clicks meant for per-bar selection.
- **Mitigation:** Register per-bar click regions at higher z-index (or first — first-registered wins per `MouseRegions` semantics). Verify against current `MouseCtx` precedence rules.

### Memory ring-buffer size unchanged
- **Risk:** Memory chart shows only 60 s — may be insufficient for some workflows.
- **Mitigation:** Out of scope; future enhancement to make ring sizes configurable.

---

## Keyboard Shortcuts Summary

| Key | Action |
|-----|--------|
| `Tab` | Focus next section (frame → memory → list → frame) |
| `Shift+Tab` | Focus previous section |
| `↑` / `k` | Scroll focused section up (or move row selection up) |
| `↓` / `j` | Scroll focused section down (or move row selection down) |
| `PageUp` | Scroll one viewport-height up |
| `PageDown` | Scroll one viewport-height down |
| `Home` | Jump to oldest sample (max scroll back) |
| `End` | Jump to live edge (scroll = 0) |
| `←` / `→` | (existing) Select previous/next frame |
| `s` | (existing) Toggle allocation sort column |
| Click on section | Focus that section |
| Click on alloc row | Focus list + select row |
| Click on frame bar | Select that frame |

---

## Success Criteria

### Phase 1 Complete When:
- [ ] `PerfSection` enum exists with three variants.
- [ ] `PerformanceState` has 5 new behavioral fields + 3 render-hint cells.
- [ ] 7 new `Message` variants exist.
- [ ] `cargo check --workspace` + `cargo test --workspace` pass.

### Phase 2 Complete When:
- [ ] Key handlers route the 7 new messages by `focused_section`.
- [ ] Unit tests cover handler logic for each section + scroll bounds.

### Phase 3 Complete When:
- [ ] Widgets accept and honor scroll offsets and focus state.
- [ ] Render-hint cells written every frame.
- [ ] Mouse regions registered for section focus + row selection.
- [ ] Unit tests for visible-range computation under scroll offsets.

### Phase 4 Complete When:
- [ ] `docs/KEYBINDINGS.md` lists all new bindings.
- [ ] `docs/ARCHITECTURE.md` mentions `PerfSection` and the scroll model.
- [ ] Live-edge drift tests pass.
- [ ] Manual verification recorded.

---

## Future Enhancements

- Configurable ring-buffer sizes (`[devtools.performance] frame_history_size`, `memory_sample_count`).
- "Pin a frame" mode that keeps a specific frame visible regardless of new arrivals.
- Synchronized scroll between frame chart and memory chart (timestamp-correlated).
- Allocation table inline filtering (`/` to filter by class name).

---

## References

- `docs/CODE_STANDARDS.md` "Region Registry Pattern" — `MouseCtx` / `MouseRegions` usage.
- `docs/CODE_STANDARDS.md` Principle 3 — `Cell<usize>` render-hint pattern.
- `docs/ARCHITECTURE.md` "DevTools Subsystem" — current panel structure.
- Commit `1cf2068` — Terminal mouse support.
