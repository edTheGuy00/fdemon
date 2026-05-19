# Task 01 — Timeline Viewport: Pan, Zoom, and Auto-Scroll Toggle

**Status:** Not Started
**Wave:** 1
**Agent:** implementor
**Estimated Effort:** 4–6 hours
**Depends On:** Phase 4 fully merged

## Problem

The Phase 4 Gantt has a fixed 5s viewport that auto-scrolls forward as new events arrive. Users can't:

1. **Zoom in** to study a 100ms window
2. **Zoom out** to see a 30s history
3. **Pan** to a specific time range
4. **Hold the viewport still** while observing a specific event window

Phase 5 introduces a manual-viewport mode toggled by user pan/zoom, with a one-key reset (`End`) back to live-follow.

## Files (Write)

- `crates/fdemon-app/src/session/performance.rs` — new viewport state fields
- `crates/fdemon-app/src/handler/keys.rs` — new arms for `+`/`-`/`←`/`→`/`End`/`g` on TimelineEvents tab
- `crates/fdemon-app/src/handler/devtools/performance/timeline.rs` — new handlers
- `crates/fdemon-app/src/message.rs` — new `Message` variants
- `crates/fdemon-tui/src/widgets/devtools/performance/details/timeline_events/viewport.rs` — extend with pan/zoom math
- `crates/fdemon-tui/src/widgets/devtools/performance/details/timeline_events/gantt.rs` — consume new state, render "PAUSED" indicator

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

```rust
/// Returns the (start, end) viewport bounds. Honors manual viewport when
/// `!follow_latest`; otherwise returns the live-follow window.
pub(super) fn compute_viewport(
    tracks: &BTreeMap<i64, TimelineTrack>,
    viewport_start_micros: u64,
    viewport_width_micros: u64,
    follow_latest: bool,
) -> (u64, u64) { ... }

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

### New Message variants

```rust
TimelineZoomIn { session_id: SessionId },
TimelineZoomOut { session_id: SessionId },
TimelinePanLeft { session_id: SessionId },
TimelinePanRight { session_id: SessionId },
TimelineFollowLatest { session_id: SessionId },
```

### Keybinding arms (in `handler/keys.rs`)

Add inside the existing `in_performance && active_details_tab == TimelineEvents` branch:

```rust
InputKey::Char('+') | InputKey::Char('=') => Some(Message::TimelineZoomIn { session_id }),
InputKey::Char('-') | InputKey::Char('_') => Some(Message::TimelineZoomOut { session_id }),
// Pan only when no selection is active (Phase 5 T03 will gate this further).
// For T01, assume no selection — T03 will refine.
InputKey::Left  => Some(Message::TimelinePanLeft { session_id }),
InputKey::Right => Some(Message::TimelinePanRight { session_id }),
InputKey::End | InputKey::Char('g') => Some(Message::TimelineFollowLatest { session_id }),
```

**Pre-emptive note for T03 implementor:** when selection is active, `←`/`→` will mean "navigate selection." T03 will replace the unconditional pan with a guard `if selected_event.is_none() { pan } else { move_selection }`.

### Handler logic

```rust
pub fn handle_zoom_in(state: &mut AppState, session_id: SessionId) -> UpdateResult {
    let Some(handle) = state.session_manager.get_mut(session_id) else { return UpdateResult::none() };
    let perf = &mut handle.session.performance;
    // Materialize current viewport before mutating
    let (cur_start, cur_end) = compute_viewport(
        &perf.timeline_tracks,
        perf.timeline_viewport_start_micros,
        perf.timeline_viewport_width_micros,
        perf.timeline_follow_latest,
    );
    let (new_start, new_end) = zoom_viewport(
        cur_start,
        cur_end - cur_start,
        1.0 / TIMELINE_ZOOM_FACTOR,
        (cur_start + cur_end) / 2,
    );
    let new_width = (new_end - new_start).clamp(TIMELINE_VIEWPORT_MIN_MICROS, TIMELINE_VIEWPORT_MAX_MICROS);
    perf.timeline_viewport_start_micros = new_start;
    perf.timeline_viewport_width_micros = new_width;
    perf.timeline_follow_latest = false;  // pin viewport
    UpdateResult::none()
}

pub fn handle_follow_latest(state: &mut AppState, session_id: SessionId) -> UpdateResult {
    let Some(handle) = state.session_manager.get_mut(session_id) else { return UpdateResult::none() };
    let perf = &mut handle.session.performance;
    perf.timeline_follow_latest = true;
    perf.timeline_viewport_width_micros = TIMELINE_VIEWPORT_MICROS;  // reset to default 5s
    // start_micros becomes irrelevant when follow_latest = true
    UpdateResult::none()
}
```

### Gantt rendering update

When `!follow_latest`, render a small "PAUSED" indicator (e.g., 1-cell `⏸` glyph + dim text "manual viewport — press End to resume") at the top-right corner of the Gantt area or in the time-axis row.

## Acceptance Criteria

1. **State fields added** — `PerformanceState::timeline_viewport_start_micros`, `timeline_viewport_width_micros`, `timeline_follow_latest` exist with documented defaults. `Default::default()` returns `(0, TIMELINE_VIEWPORT_MICROS, true)`. Field-presence tests in `session/performance.rs::tests` updated.
2. **Zoom in** — `TimelineZoomIn` halves `viewport_width_micros` (clamped at `MIN`) and sets `follow_latest = false`. New test `test_zoom_in_halves_viewport`.
3. **Zoom out** — `TimelineZoomOut` doubles `viewport_width_micros` (clamped at `MAX`) and sets `follow_latest = false`. New test `test_zoom_out_doubles_viewport_to_max`.
4. **Pan left/right** — `TimelinePanLeft` decreases `viewport_start_micros` by `width * TIMELINE_PAN_FRACTION` (clamped at 0); `TimelinePanRight` increases it. Both set `follow_latest = false`. New tests.
5. **Follow latest** — `TimelineFollowLatest` sets `follow_latest = true` and resets `width` to default. New test.
6. **`compute_viewport` respects `follow_latest`** — When `true`, returns latest-window. When `false`, returns `(start, start+width)`. Existing Phase 4 tests still pass because Phase 4 default is `follow_latest = true`.
7. **Zoom anchor preserves center** — Zooming in from `(start=1000, width=4000)` with anchor 3000 produces `(start=2000, width=2000)`. Center column stays at the same ts.
8. **"PAUSED" indicator** — When `!follow_latest`, the renderer paints a visible indicator. New test inspects the buffer for the indicator glyph + text.
9. **Mouse interaction** — Scroll wheel on the Gantt canvas can also drive zoom (mouse wheel up = zoom in, down = zoom out). Stretch goal; if scope tight, skip and add a TODO.
10. **All existing Phase 4 Gantt tests pass** — viewport contract is backward-compatible when `follow_latest = true` (the default).
11. **Quality gate** — `cargo fmt --all -- --check`, `cargo check --workspace --all-targets`, `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings` all pass.

## Notes

- This task is **foundational** — every other Phase 5 task reads `timeline_viewport_*` state. Gate Wave 2 on T01 fully landing.
- Pan/zoom does **not** modify `timeline_thread_scroll_offset` (vertical thread-row scroll from Phase 4). Pan is horizontal-only.
- The `InputKey::Left`/`Right` arms in T01 unconditionally pan — T03 will refine this to gate on `selected_event.is_none()`. Document this transitional behavior in the Completion Summary so T03's implementor doesn't get surprised.
- `End` key — check `crossterm::event::KeyCode::End` is supported on macOS terminals; some terminals require modifier remapping. If `End` is unreliable, prefer `g` as the primary key with `End` as alternate.
- Zoom factor 2.0 means 4 keypresses cover the full 100ms → 60s range. If users want finer granularity, defer to a Phase 6 setting.
