# Task 01 — Timeline Viewport: Pan, Zoom, and Auto-Scroll Toggle

**Status:** Not Started
**Wave:** 1
**Agent:** implementor
**Estimated Effort:** 5–7 hours (includes pre-flight test extraction)
**Depends On:** Phase 4 fully merged

> **Read first:** PLAN.md's "Codebase Verification (2026-05-20)" drift table — five of its ten entries (#1, #2, #3, #4, #7) materially shape this task.

## Problem

The Phase 4 Gantt has a fixed 5s viewport that auto-scrolls forward as new events arrive. Users can't:

1. **Zoom in** to study a 100ms window
2. **Zoom out** to see a 30s history
3. **Pan** to a specific time range
4. **Hold the viewport still** while observing a specific event window

Phase 5 introduces a manual-viewport mode toggled by user pan/zoom, with a one-key reset (`g`, with `End` as a guarded alias on the TimelineEvents tab) back to live-follow.

## Files (Write)

- `crates/fdemon-app/src/session/performance.rs` — new viewport state fields **only** (the frame-anchor fields `committed_frame_anchor`, `frame_anchor_generation`, `frame_anchor_map` already exist — Drift #1, do not redeclare)
- `crates/fdemon-app/src/handler/keys.rs` — new arms for `+`/`-`/`←`/`→`/`g`/`End` on TimelineEvents tab, with the **conflict guards documented below** (Drift #3 + #4)
- `crates/fdemon-app/src/handler/devtools/performance/timeline.rs` — new handlers
- `crates/fdemon-app/src/message.rs` — new `Message` variants
- `crates/fdemon-tui/src/widgets/devtools/performance/details/timeline_events/viewport.rs` — add `compute_active_viewport` composer (3-mode priority), `pan_viewport`, `zoom_viewport`
- `crates/fdemon-tui/src/widgets/devtools/performance/details/timeline_events/gantt.rs` — call `compute_active_viewport`, render "PAUSED" indicator
- `crates/fdemon-tui/src/widgets/devtools/performance/details/timeline_events/gantt_tests.rs` — **NEW** (pre-flight test extraction, Drift #7) — move inline `#[cfg(test)] mod tests` block out of `gantt.rs`. Refactor-only step with no behavior change; do this **first** before adding viewport composition

## Files (Read)

- `crates/fdemon-app/src/state.rs` — verify Message dispatch is exhaustive
- Phase 4 outputs (entire `timeline_events/` subdirectory)

## Approach Hints

### State additions

```rust
// In PerformanceState
/// Manual viewport start. Honored when `timeline_follow_latest == false`.
pub timeline_viewport_start_micros: u64,
/// Viewport width in microseconds. Default = TIMELINE_VIEWPORT_MICROS (5s).
/// Bounded by [TIMELINE_VIEWPORT_MIN_MICROS, TIMELINE_VIEWPORT_MAX_MICROS].
pub timeline_viewport_width_micros: u64,
/// When true (default), `compute_viewport` returns the latest-N-micros window.
/// When false, returns `[viewport_start_micros, viewport_start_micros + viewport_width_micros]`.
pub timeline_follow_latest: bool,
```

### Constants

```rust
/// Minimum viewport width — prevents over-zoom into a single μs.
pub(super) const TIMELINE_VIEWPORT_MIN_MICROS: u64 = 100_000;       // 100 ms

/// Maximum viewport width — prevents over-zoom out beyond useful history.
pub(super) const TIMELINE_VIEWPORT_MAX_MICROS: u64 = 60_000_000;    // 60 s

/// Zoom factor per `+`/`-` keypress. 2x zoom per keypress is the DevTools
/// convention; finer granularity would require more keystrokes.
pub(super) const TIMELINE_ZOOM_FACTOR: f64 = 2.0;

/// Pan factor per `←`/`→` keypress. Pans by 10% of current viewport width.
pub(super) const TIMELINE_PAN_FRACTION: f64 = 0.10;
```

### Viewport math (`viewport.rs` extensions)

The existing `viewport.rs` has these functions (verified 2026-05-20):
- `compute_frame_anchored_viewport(frame_anchor_map: &BTreeMap<u64, (u64, u64)>, frame_number: u64) -> Option<(u64, u64)>` — **active** function, used by gantt.rs today
- `compute_viewport(tracks: &BTreeMap<i64, TimelineTrack>) -> (u64, u64)` — marked `#[allow(dead_code)]`, returns live-edge window of exactly `TIMELINE_VIEWPORT_MICROS`
- `micros_to_column`, `clip_bar` — keep as-is

**Add a new top-level composer that resolves PLAN D2's 3-mode priority order:**

```rust
/// Returns the (start, end) viewport bounds for the Gantt canvas.
/// Resolution priority (PLAN D2):
///   1. `!follow_latest` → manual viewport `(start, start + width)`
///   2. `follow_latest && committed_frame_anchor.is_some()`
///        → `compute_frame_anchored_viewport(frame_anchor_map, frame)`
///   3. `follow_latest && no frame anchor` → live-edge from `timeline_tracks`
pub(super) fn compute_active_viewport(perf: &PerformanceState) -> (u64, u64) {
    if !perf.timeline_follow_latest {
        let start = perf.timeline_viewport_start_micros;
        let width = perf.timeline_viewport_width_micros.max(TIMELINE_VIEWPORT_MIN_MICROS);
        return (start, start.saturating_add(width));
    }
    if let Some(frame) = perf.committed_frame_anchor {
        if let Some((s, e)) = compute_frame_anchored_viewport(&perf.frame_anchor_map, frame) {
            return (s, e);
        }
    }
    // Live-edge fallback. Reuse the existing dead-code function as the live-edge math.
    compute_viewport(&perf.timeline_tracks)
}

/// Pure: compute new viewport bounds after a zoom action.
/// `factor < 1.0` zooms in; `factor > 1.0` zooms out.
/// `anchor_micros` is the ts that should stay at the same column (defaults to viewport center).
pub(super) fn zoom_viewport(
    start: u64,
    width: u64,
    factor: f64,
    anchor_micros: u64,
) -> (u64, u64) { ... }

/// Pure: compute new viewport start after panning by `direction * fraction * width`.
pub(super) fn pan_viewport(
    start: u64,
    width: u64,
    direction: PanDirection,
    fraction: f64,
) -> u64 { ... }

pub(super) enum PanDirection { Left, Right }
```

**Remove `#[allow(dead_code)]` from `compute_viewport(tracks)`** once `compute_active_viewport` uses it as the live-edge branch.

### New Message variants

```rust
TimelineZoomIn { session_id: SessionId },
TimelineZoomOut { session_id: SessionId },
TimelinePanLeft { session_id: SessionId },
TimelinePanRight { session_id: SessionId },
TimelineFollowLatest { session_id: SessionId },
```

### Keybinding arms (in `handler/keys.rs`)

**Drift #3, #4 — conflict resolution required.** The Performance handler's `in_performance` early-return block (around lines 489–582) already binds:
- `Left` / `Right` (at the **outer** `match key` block, around lines 809–820) → `SelectPerformanceFrame` — fires for **any** focused section while `in_performance`
- `End` → `PerfJumpToEnd` (in the `in_performance` block)
- `Home` → `PerfJumpToStart` (in the `in_performance` block)

The new arms must be inserted with a tab guard **inside the `in_performance` block, ordered before the existing `End`/`Home` arms, and a `Left`/`Right` tab guard inserted into the outer block before `SelectPerformanceFrame`**. The fast path checks `focused_section == FocusedSection::Details && details_tab == DetailsTab::TimelineEvents`.

```rust
// Inside in_performance block, BEFORE the existing PerfJumpToEnd / PerfJumpToStart arms:
match key {
    InputKey::Char('+') | InputKey::Char('=')
        if active_details_tab_is(TimelineEvents) =>
            return Some(Message::TimelineZoomIn { session_id }),
    InputKey::Char('-') | InputKey::Char('_')
        if active_details_tab_is(TimelineEvents) =>
            return Some(Message::TimelineZoomOut { session_id }),
    InputKey::Char('g')
        if active_details_tab_is(TimelineEvents) =>
            return Some(Message::TimelineFollowLatest { session_id }),
    InputKey::End
        if active_details_tab_is(TimelineEvents) =>
            return Some(Message::TimelineFollowLatest { session_id }),
    _ => {}  // fall through to existing PerfJumpToEnd/etc.
}

// In the outer match block, BEFORE the existing `Left`/`Right` → SelectPerformanceFrame arm:
InputKey::Left
    if active_details_tab_is(TimelineEvents) =>
        Some(Message::TimelinePanLeft { session_id }),
InputKey::Right
    if active_details_tab_is(TimelineEvents) =>
        Some(Message::TimelinePanRight { session_id }),
```

`active_details_tab_is(...)` is a local helper closure that reads `state.session_manager.active_session().map(|h| h.session.performance.focused_section)` and `details_tab`. Choose whatever shape fits the existing pattern in `keys.rs`.

**Pre-emptive note for T03 implementor:** when selection is active, `←`/`→` will mean "navigate selection." T03 will refine the unconditional pan with a guard `if selected_event.is_none() { pan } else { move_selection }`. Land this transitional behavior in T01 and document it in the Completion Summary.

**Tests for the conflict resolution:**
- `test_left_on_frame_chart_still_selects_frame` — focus FrameChart, press Left → `SelectPerformanceFrame { index: prev }`. No `TimelinePanLeft` dispatched.
- `test_left_on_frame_analysis_tab_still_selects_frame` — focus Details/FrameAnalysis, press Left → `SelectPerformanceFrame`. Same.
- `test_left_on_timeline_events_tab_pans` — focus Details/TimelineEvents, press Left → `TimelinePanLeft`. No frame change.
- `test_end_on_frame_chart_jumps_to_end` — focus FrameChart, press End → `PerfJumpToEnd`. No `TimelineFollowLatest`.
- `test_end_on_timeline_events_follow_latest` — focus Details/TimelineEvents, press End → `TimelineFollowLatest`. No frame chart jump.

### Handler logic

```rust
pub fn handle_zoom_in(state: &mut AppState, session_id: SessionId) -> UpdateResult {
    let Some(handle) = state.session_manager.get_mut(session_id) else { return UpdateResult::none() };
    let perf = &mut handle.session.performance;
    // Materialize current viewport before mutating — composes 3 modes per PLAN D2.
    let (cur_start, cur_end) = compute_active_viewport(perf);
    let (new_start, new_end) = zoom_viewport(
        cur_start,
        cur_end - cur_start,
        1.0 / TIMELINE_ZOOM_FACTOR,
        (cur_start + cur_end) / 2,
    );
    let new_width = (new_end - new_start).clamp(TIMELINE_VIEWPORT_MIN_MICROS, TIMELINE_VIEWPORT_MAX_MICROS);
    perf.timeline_viewport_start_micros = new_start;
    perf.timeline_viewport_width_micros = new_width;
    perf.timeline_follow_latest = false;  // pin viewport (frame anchor preserved for `g`/End restore)
    UpdateResult::none()
}

pub fn handle_follow_latest(state: &mut AppState, session_id: SessionId) -> UpdateResult {
    let Some(handle) = state.session_manager.get_mut(session_id) else { return UpdateResult::none() };
    let perf = &mut handle.session.performance;
    perf.timeline_follow_latest = true;
    perf.timeline_viewport_width_micros = TIMELINE_VIEWPORT_MICROS;  // reset to default 5s
    // start_micros becomes irrelevant when follow_latest = true; frame_anchor still wins
    // if `committed_frame_anchor.is_some()` per PLAN D2.
    UpdateResult::none()
}
```

### Gantt rendering update

When `!follow_latest`, render a small "PAUSED" indicator (e.g., 1-cell `⏸` glyph + dim text "manual viewport — press End to resume") at the top-right corner of the Gantt area or in the time-axis row.

## Acceptance Criteria

1. **Test extraction landed (Drift #7)** — The `#[cfg(test)] mod tests` block inside `gantt.rs` has been moved to a new sibling file `gantt_tests.rs` declared from `mod.rs` (or via `#[path]` in `gantt.rs` per workspace convention). `gantt.rs` is under 800 lines after the move. All extracted tests still pass without modification. This is a **refactor-only** commit / first sub-step; review separately if convenient.
2. **State fields added (only the new ones, Drift #1)** — `PerformanceState::timeline_viewport_start_micros`, `timeline_viewport_width_micros`, `timeline_follow_latest` exist with documented defaults `(0, TIMELINE_VIEWPORT_MICROS, true)`. **Do NOT redeclare `committed_frame_anchor`, `frame_anchor_generation`, `frame_anchor_map`** — verify they remain untouched. Field-presence tests updated.
3. **Zoom in** — `TimelineZoomIn` halves `viewport_width_micros` (clamped at `MIN`) and sets `follow_latest = false`. New test `test_zoom_in_halves_viewport`.
4. **Zoom out** — `TimelineZoomOut` doubles `viewport_width_micros` (clamped at `MAX`) and sets `follow_latest = false`. New test `test_zoom_out_doubles_viewport_to_max`.
5. **Pan left/right** — `TimelinePanLeft` decreases `viewport_start_micros` by `width * TIMELINE_PAN_FRACTION` (clamped at 0); `TimelinePanRight` increases it. Both set `follow_latest = false`. New tests.
6. **Follow latest** — `TimelineFollowLatest` sets `follow_latest = true` and resets `width` to default. Frame anchor (if any) is preserved. New test `test_follow_latest_preserves_frame_anchor`.
7. **`compute_active_viewport` honors PLAN D2 priority order (Drift #2)** — three new tests:
   - `test_compute_active_viewport_manual_overrides_anchor` — `!follow_latest` returns manual `(start, start+width)` regardless of `committed_frame_anchor` value.
   - `test_compute_active_viewport_frame_anchor_when_follow_latest` — `follow_latest && Some(frame)` returns the frame-anchored window from `frame_anchor_map`.
   - `test_compute_active_viewport_live_edge_fallback` — `follow_latest && None` returns the live-edge window from `compute_viewport(tracks)`.
8. **All existing Phase 4 Gantt tests pass** — viewport contract is backward-compatible: default `follow_latest = true` + no frame anchor + Phase 4 fixture tracks → matches Phase 4 expected viewport.
9. **Keybinding conflict guards work (Drift #3, #4)** — five new tests as enumerated in the "Keybinding arms" section (`test_left_on_frame_chart_still_selects_frame`, etc.).
10. **Zoom anchor preserves center** — Zooming in from `(start=1000, width=4000)` with anchor 3000 produces `(start=2000, width=2000)`. Center column stays at the same ts.
11. **"PAUSED" indicator** — When `!follow_latest`, the renderer paints a visible indicator. New test inspects the buffer for the indicator glyph + text.
12. **Mouse interaction** — Scroll wheel on the Gantt canvas can also drive zoom (mouse wheel up = zoom in, down = zoom out). Stretch goal; if scope tight, skip and add a TODO.
13. **Quality gate** — `cargo fmt --all -- --check`, `cargo check --workspace --all-targets`, `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings` all pass.

## Notes

- This task is **foundational** — every other Phase 5 task reads `timeline_viewport_*` state and depends on `compute_active_viewport`. Gate Wave 2 on T01 fully landing.
- Pan/zoom does **not** modify `timeline_thread_scroll_offset` (vertical thread-row scroll from Phase 4). Pan is horizontal-only.
- The `InputKey::Left`/`Right` arms in T01 unconditionally pan **on the TimelineEvents tab** (with the tab guard from Drift #3). T03 will refine this to gate on `selected_event.is_none()`. Document this transitional behavior in the Completion Summary so T03's implementor doesn't get surprised.
- `End` key — confirmed bound to `PerfJumpToEnd` in the `in_performance` block (Drift #4). T01 uses `g` as primary follow-latest key; `End` as a tab-guarded alias inserted **before** `PerfJumpToEnd`. On terminals where `End` is unreliable, `g` continues to work.
- Zoom factor 2.0 means 4 keypresses cover the full 100ms → 60s range. If users want finer granularity, defer to a Phase 6 setting.
- **Frame anchor interaction:** When `committed_frame_anchor.is_some()` and the user starts panning/zooming, the frame anchor is **preserved** (not cleared). Pressing `g`/`End` returns to the frame-anchored viewport, not live-edge. This is the intent of PLAN D2 mode 2 having priority over mode 3. If the user wants live-edge regardless of frame anchor, they can clear the frame chart selection (existing Phase 4 behavior); T01 should not invent a new "clear anchor" key.

---

## Completion Summary

**Status:** Done
**Branch:** feat/devtools-inspector-parity

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/session/performance.rs` | Added `timeline_viewport_start_micros`, `timeline_viewport_width_micros`, `timeline_follow_latest` fields + defaults + tests |
| `crates/fdemon-app/src/message.rs` | Added 5 new Message variants: TimelineZoomIn/Out, TimelinePanLeft/Right, TimelineFollowLatest |
| `crates/fdemon-app/src/handler/devtools/performance/timeline.rs` | Added Phase 5 constants + handle_zoom_in/out, handle_pan_left/right, handle_follow_latest + tests |
| `crates/fdemon-app/src/handler/devtools/performance/mod.rs` | Re-exported the 5 new handler functions |
| `crates/fdemon-app/src/handler/update.rs` | Wired 5 new Message variants to their handlers |
| `crates/fdemon-app/src/handler/keys.rs` | Added +/-/g/End zoom+follow-latest bindings in in_performance block (before Home/End, Drift #4); added Left/Right pan guards before SelectPerformanceFrame (Drift #3); added 5 keybinding conflict tests |
| `crates/fdemon-tui/src/widgets/devtools/performance/details/timeline_events/viewport.rs` | Added compute_active_viewport (3-mode PLAN D2), zoom_viewport, pan_viewport, PanDirection, viewport constants; removed #[allow(dead_code)] from compute_viewport |
| `crates/fdemon-tui/src/widgets/devtools/performance/details/timeline_events/gantt.rs` | Updated to use compute_active_viewport; added anchor gate for follow_latest=true; added PAUSED indicator; extracted tests to gantt_tests.rs; added render_time_axis_pub shim |
| `crates/fdemon-tui/src/widgets/devtools/performance/details/timeline_events/gantt_tests.rs` | NEW — extracted Phase 4 gantt tests + added PAUSED indicator tests (gantt.rs down to 533 lines) |

### Notable Decisions/Tradeoffs

1. **Test extraction via `#[path]`**: Used `#[cfg(test)] #[path = "gantt_tests.rs"] mod tests;` (external module declaration) rather than an inline module. `gantt_tests.rs` imports `super::*` where `super` is the `gantt` module; `THREAD_LABEL_WIDTH` uses `super::super::` to reach the parent `timeline_events` module.

2. **Viewport constants duplicated in app crate**: `TIMELINE_VIEWPORT_MIN/MAX_MICROS`, `TIMELINE_ZOOM_FACTOR`, `TIMELINE_PAN_FRACTION` are defined in both `fdemon-tui/viewport.rs` and `fdemon-app/timeline.rs` to respect layer boundaries (app cannot depend on tui). The values must stay in sync; doc comments note this.

3. **Gantt anchor gate restructured**: In Phase 4, the gantt always showed a placeholder when `committed_frame_anchor == None`. Phase 5 preserves this behavior: the anchor gate now reads `if state.timeline_follow_latest { check anchor }` so that the manual-viewport mode (`!follow_latest`) bypasses the anchor check entirely and renders the Gantt with the manual window.

4. **Transitional `←`/`→` behavior (T03 note)**: On the TimelineEvents tab, `←`/`→` UNCONDITIONALLY pan the Gantt viewport. T03 will refine this to `if selected_event.is_none() { pan } else { move_selection }`. T03's implementor should add a guard check before the `TimelinePanLeft`/`TimelinePanRight` emission.

5. **materialize_viewport in app crate**: The handler functions call a local `materialize_viewport(perf)` that returns `(start, start+width)` even when `follow_latest=true`. This is an approximation for the handler context — the TUI's `compute_active_viewport` is the authoritative 3-mode resolver. The handler only needs the current effective window to compute zoom/pan deltas.

### Testing Performed

- `cargo fmt --all -- --check` - Passed
- `cargo check --workspace --all-targets` - Passed
- `cargo test --workspace` - Passed (2480 fdemon-app, 1261 fdemon-tui, 817 fdemon-daemon, etc.)
- `cargo clippy --workspace --all-targets -- -D warnings` - Passed

### Risks/Limitations

1. **Mouse scroll (AC12 stretch goal)**: Not implemented. Mouse wheel zoom requires a `Mouse(MouseInput)` handler that maps scroll-up/down on the Gantt canvas area to `TimelineZoomIn`/`TimelineZoomOut`. Deferred per task instructions ("if scope tight, skip and add a TODO"). Added TODO comment in `gantt.rs`.

2. **`←`/`→` always pan (transitional)**: On the TimelineEvents tab, Left/Right pan regardless of whether an event is selected. T03 will fix this; documented in Completion Summary for T03 implementor.
