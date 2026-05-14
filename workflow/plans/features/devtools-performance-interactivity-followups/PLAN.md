# Plan: DevTools Performance Interactivity — Follow-ups

## TL;DR

Two small follow-ups surfaced during phase-4 smoke verification of `devtools-performance-interactivity` (2026-05-14):

1. **Left/Right arrow does not clear `frame_chart_scroll_offset`** — already documented as `KNOWN DEFECT` in test 4 of task 08. Frame selection while scrolled-back leaves the viewport anchored at the old offset, so the newly-selected frame can land outside the visible window.

2. **Mouse-wheel scroll inside Performance sections is a no-op** — `crates/fdemon-tui/src/event.rs` lifts `crossterm` wheel events into `MouseInput::Scroll`, and `crates/fdemon-app/src/handler/mouse/devtools.rs:94` explicitly returns `None` for `DevToolsPanel::Performance`. Keyboard scroll already works; routing the wheel to `PerfScrollUp` / `PerfScrollDown` brings it to parity.

Neither item is a regression — both are pre-existing gaps the main feature didn't cover. They are independent and can ship in parallel or in any order.

## Scope

In scope:
- Task 01: clear `frame_chart_scroll_offset` in `handle_select_performance_frame`; update the existing `left_right_arrow_clears_scroll_offset` test to assert the new behaviour.
- Task 02: replace the `Performance => None` branch in `handler/mouse/devtools.rs:94` with a `handle_performance_scroll` that returns `PerfScrollUp` / `PerfScrollDown` (or `PerfPageUp` / `PerfPageDown` on Shift), mirroring `handle_inspector_scroll`. Wheel events route by `focused_section`, identical to keyboard scroll. No new `Message` variants.

Out of scope:
- Wheel-over-section focus changes (focus continues to follow Tab / click only).
- Horizontal wheel routing — `ScrollDir::Left` / `Right` stay `None`, mirroring the Inspector branch.
- Modifier-driven jump-to-edge — Ctrl+Wheel etc. stays `None` for parity with `keys.rs`.

## Task Dependency Graph

```
01-clear-scroll-offset-on-frame-select       (independent)
02-mouse-wheel-scroll-in-perf-panel          (independent)
```

## Success Criteria

- [ ] Selecting a frame via Left/Right arrow resets `frame_chart_scroll_offset` to 0; the existing KNOWN DEFECT test inverts to a passing forward assertion.
- [ ] Mouse-wheel up/down inside the Performance panel scrolls the focused section (frame chart, memory chart, or alloc table) consistently with keyboard `↑`/`↓`/`k`/`j`; Shift+wheel maps to PageUp/PageDown.
- [ ] All four CI quality gates pass.
- [ ] Manual smoke: scroll wheel inside each section, observe values change; select a frame while scrolled, observe viewport return to the selection.
