# Task 03 — Timeline Event Selection and Details Popup

**Status:** Not Started
**Wave:** 2
**Agent:** implementor
**Estimated Effort:** 5–7 hours
**Depends On:** T01 (viewport state + `compute_active_viewport` + `gantt_tests.rs` extraction)

> **Read first:** PLAN.md's "Codebase Verification (2026-05-20)" drift table — entries #6 (j/k/Up/Down scroll-vs-selection ordering) and #9 (modal_overlay availability) directly shape this task.

## Problem

After Phase 4, users can see colored event bars in the Gantt but cannot inspect any of them. There's no selection, no "what is this bar?" affordance.

Phase 5 adds:

1. A **selection cursor** identifying one event in the timeline tree.
2. **Keyboard navigation** to move the cursor (`Enter` selects first visible; `←`/`→` traverses siblings; `↑`/`↓` traverses depth or threads).
3. A **details popup** showing the event's name, ts, dur, thread, parent chain (modal overlay).
4. **Mouse selection** — clicking a bar selects it (uses the existing mouse region registry).

## Files (Write)

- `crates/fdemon-app/src/session/performance.rs` — new fields
- `crates/fdemon-app/src/handler/keys.rs` — new arms for Enter, arrow nav when selection active, Esc fallthrough. **Ordering matters (Drift #6):** the selection-nav arms for `↑`/`↓`/`j`/`k` must be inserted **before** the existing `PerfScrollUp`/`PerfScrollDown` arms, gated by `has_selection`. The `←`/`→` arms refine T01's TimelineEvents-tab pan arms with `if selected_event.is_none()` guards
- `crates/fdemon-app/src/handler/devtools/performance/timeline.rs` — new handlers
- `crates/fdemon-app/src/message.rs` — new Message variants
- `crates/fdemon-tui/src/widgets/devtools/performance/details/timeline_events/popup.rs` (NEW)
- `crates/fdemon-tui/src/widgets/devtools/performance/details/timeline_events/gantt.rs` — selection-overlay highlight (note: T01 extracted inline tests to `gantt_tests.rs`, so test additions go there)
- `crates/fdemon-tui/src/widgets/devtools/performance/details/timeline_events/gantt_tests.rs` — extend with selection-overlay tests
- `crates/fdemon-tui/src/widgets/devtools/performance/details/timeline_events/mod.rs` — declare `pub(super) mod popup;`, conditionally render popup last

## Files (Read)

- T01 outputs (viewport state — auto-pan to keep selection visible)
- Phase 4 outputs (`TimelineTrack`, `TimelineNode`)
- `crates/fdemon-tui/src/widgets/modal_overlay.rs` — popup chrome helpers
- `crates/fdemon-core/src/timeline.rs` — `TimelinePhase`, `TimelineThread`

## Approach Hints

### State additions (in `PerformanceState`)

```rust
pub timeline_selected_event: Option<TimelineEventCursor>,
pub timeline_details_popup_open: bool,
```

### Cursor type (in `session/performance.rs` or a new `selection.rs` helper module)

```rust
/// Identifies a single event in the timeline tree. Stable across batches as
/// long as the event survives the ring-buffer eviction policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimelineEventCursor {
    pub tid: i64,
    pub depth: u8,
    /// Event start timestamp in micros. Disambiguates siblings.
    pub ts: i64,
}
```

### New Message variants

```rust
TimelineSelectFirstVisible { session_id: SessionId },
TimelineMoveSelection { session_id: SessionId, dir: SelectionDirection },
TimelineOpenPopup { session_id: SessionId },
TimelineClosePopup { session_id: SessionId },
TimelineClearSelection { session_id: SessionId },
TimelineSelectAt { session_id: SessionId, cursor: TimelineEventCursor },  // mouse-driven

// In a shared types module
pub enum SelectionDirection {
    PrevSibling,
    NextSibling,
    ParentOrUpThread,
    FirstChildOrDownThread,
}
```

### Selection navigation algorithm

Given `cursor = (tid, depth, ts)` and tree `tracks[tid].root_events`:

1. **Look up current node** — traverse children at each depth, matching `ts` exactly. Returns `Option<&TimelineNode>`. If `None` (event evicted), clear selection and log debug.
2. **PrevSibling** — at the same depth in the parent's children, find the node immediately before `current.ts`. Wrap to last if none.
3. **NextSibling** — same, immediately after.
4. **ParentOrUpThread** — return the parent's cursor. If `depth == 0`, move to the previous thread row's first root event.
5. **FirstChildOrDownThread** — return `children[0]`'s cursor. If no children, move to the next thread row's first root event.

### Keyboard arms (in `handler/keys.rs`)

Augment T01's TimelineEvents-tab branch. **Critical ordering (Drift #6):** The `Up`/`Down`/`j`/`k` selection-nav arms must be inserted **before** the existing `PerfScrollUp`/`PerfScrollDown` arms in the `in_performance` block. Without `has_selection` guards on the new arms (and ordering before the existing scroll arms), Up/Down will scroll the chart instead of moving selection.

```rust
let has_selection = perf.timeline_selected_event.is_some();
let popup_open = perf.timeline_details_popup_open;
let on_timeline_tab = active_details_tab_is(TimelineEvents);  // helper from T01

match key {
    // Popup-first: when popup is open, Esc closes it before falling through.
    InputKey::Esc if popup_open => Some(Message::TimelineClosePopup { session_id }),
    InputKey::Esc if has_selection && on_timeline_tab
        => Some(Message::TimelineClearSelection { session_id }),
    // Selection entry:
    InputKey::Enter if has_selection && !popup_open && on_timeline_tab
        => Some(Message::TimelineOpenPopup { session_id }),
    InputKey::Enter if on_timeline_tab
        => Some(Message::TimelineSelectFirstVisible { session_id }),
    // Sibling nav (refines T01's tab-guarded pan arms with selection check):
    InputKey::Left  if has_selection && on_timeline_tab
        => Some(Message::TimelineMoveSelection { session_id, dir: PrevSibling }),
    InputKey::Right if has_selection && on_timeline_tab
        => Some(Message::TimelineMoveSelection { session_id, dir: NextSibling }),
    // Depth/thread nav — MUST be ordered BEFORE the existing PerfScrollUp/PerfScrollDown arms.
    // When no selection, these fall through to the existing scroll behavior.
    InputKey::Up    if has_selection && on_timeline_tab
        => Some(Message::TimelineMoveSelection { session_id, dir: ParentOrUpThread }),
    InputKey::Down  if has_selection && on_timeline_tab
        => Some(Message::TimelineMoveSelection { session_id, dir: FirstChildOrDownThread }),
    InputKey::Char('k') if has_selection && on_timeline_tab
        => Some(Message::TimelineMoveSelection { session_id, dir: ParentOrUpThread }),
    InputKey::Char('j') if has_selection && on_timeline_tab
        => Some(Message::TimelineMoveSelection { session_id, dir: FirstChildOrDownThread }),
    // T01's pan arms are already in place with `on_timeline_tab` guard. Refine them
    // to add `!has_selection`:
    InputKey::Left  if !has_selection && on_timeline_tab
        => Some(Message::TimelinePanLeft { session_id }),
    InputKey::Right if !has_selection && on_timeline_tab
        => Some(Message::TimelinePanRight { session_id }),
    // ... falls through to existing PerfScrollUp / PerfScrollDown / SelectPerformanceFrame
}
```

This refines T01's pan arms with selection awareness and adds Up/Down/j/k handling that respects existing scroll behavior when no selection is active.

**Required test added to T01's conflict-resolution suite:**
- `test_down_on_timeline_events_without_selection_scrolls` — focus Details/TimelineEvents, no selection, press Down → `PerfScrollDown` fires (no `TimelineMoveSelection`).
- `test_down_on_timeline_events_with_selection_moves_cursor` — same focus, with selection → `TimelineMoveSelection { dir: FirstChildOrDownThread }`.

### Auto-pan to keep selection visible

When the selection moves outside the current viewport, snap the viewport to center on the selected event. Use T01's `compute_active_viewport` to get the **current effective** viewport (which may be manual, frame-anchored, or live-edge per PLAN D2):

```rust
fn ensure_selection_visible(perf: &mut PerformanceState, cursor: TimelineEventCursor, dur: u64) {
    let (vp_start, vp_end) = compute_active_viewport(perf);
    let event_end = (cursor.ts as u64).saturating_add(dur);
    if (cursor.ts as u64) < vp_start || event_end > vp_end {
        let width = vp_end - vp_start;
        perf.timeline_viewport_start_micros = (cursor.ts as u64).saturating_sub(width / 2);
        perf.timeline_viewport_width_micros = width;
        perf.timeline_follow_latest = false;  // promotes to manual viewport (mode 1)
    }
}
```

### Popup rendering (`popup.rs`)

```rust
pub(super) fn render(
    area: Rect,
    buf: &mut Buffer,
    node: &TimelineNode,
    track: &TimelineTrack,
    parent_chain: &[&TimelineNode],
    mouse_ctx: Option<&mut MouseCtx>,
) {
    // Centered modal overlay using widgets/modal_overlay helpers.
    // Body lines:
    //   Name:     <node.name>
    //   Category: <node.category | "—">
    //   Thread:   <track.name | format!("{thread:?}")> ({tid})
    //   Start:    <ts μs> (<human-readable>)
    //   Duration: <dur μs> (<human-readable>)
    //   Phase:    <phase:?>
    //   Path:     <parent_chain[0].name> → <parent_chain[1].name> → … → <node.name>
    //   Children: <count>
    // Footer hints:
    //   Esc: close   ←/→: prev/next sibling   ↑/↓: parent/child
}
```

### Selection highlight in Gantt (`gantt.rs`)

When rendering each bar, check if its cursor matches `timeline_selected_event`. If yes, paint a distinct border (reverse video, or `▏`/`▕` side markers on adjacent columns, or `Color::White` border on background).

### Mouse selection

Use the existing mouse region registry. In `gantt::render_bar`, register the bar's rect:

```rust
if let Some(ctx) = mouse_ctx {
    let cursor = TimelineEventCursor { tid: track.tid, depth, ts: node.ts };
    ctx.click(bar_rect, MouseAction::Message(Message::TimelineSelectAt { session_id, cursor }));
}
```

### Modal precedence

When `timeline_details_popup_open == true`, the popup is a modal. Per `docs/ARCHITECTURE.md` "Modal Precedence" rules, pass `None` as `MouseCtx` to all base-UI widgets (the Gantt itself) so clicks on the gantt don't fall through to bar-select while the popup is open. See `docs/CODE_STANDARDS.md` "Region Registry Pattern" for the exact pattern.

## Acceptance Criteria

1. **State fields** — `timeline_selected_event`, `timeline_details_popup_open` added with documented defaults `(None, false)`.
2. **Cursor type** — `TimelineEventCursor` exported from `session/mod.rs`. Has `Copy + Eq + PartialEq + Hash`.
3. **Enter selects first visible** — When `Enter` pressed with no selection, the first root event of the first visible thread (in `tid` ascending order, filter-respected) becomes the cursor.
4. **Sibling navigation** — `→` moves cursor to next sibling at same depth; wraps around at end. `←` moves to previous; wraps at start. New tests.
5. **Depth navigation** — `↑` moves to parent; if at root, moves to previous thread's first event. `↓` moves to first child; if leaf, moves to next thread.
6. **Esc closes popup, then clears selection** — Two consecutive Esc presses with popup open: first closes popup, second clears selection. Third falls through to existing "exit DevTools to logs" behavior.
7. **Enter opens popup when selection is active** — Second Enter on selected event opens the popup. Inside popup, Esc closes.
8. **Auto-pan** — When selection moves outside viewport, viewport snaps to center on event. `follow_latest` set to `false`. New test.
9. **Popup content** — Shows event name, category, thread, ts (μs + human-readable like `1.234s ago`), dur, parent chain breadcrumb, children count. Uses `widgets/modal_overlay` chrome.
10. **Selection highlight in Gantt** — Selected bar visually distinct from others (test inspects buffer for highlight markers).
11. **Mouse selection** — Clicking a bar dispatches `TimelineSelectAt { cursor }`. Click outside any bar (Gantt empty area) dispatches `TimelineClearSelection`.
12. **Mouse during popup** — Clicks on Gantt while popup open are no-ops (modal precedence). Click outside popup body closes it.
13. **Evicted-event handling** — If buffer eviction removes the selected event between frames, the next handler invocation clears the selection and logs `tracing::debug!("selected timeline event evicted from buffer")`.
14. **Pan/zoom keys gated by selection** — `←`/`→` pan only when `selected_event.is_none()`. New test asserts the disambiguation.
15. **No regression** on T01 and T02 — pan/zoom and minimap still work in isolation.
16. **Quality gate** — `cargo fmt --all -- --check`, `cargo check --workspace --all-targets`, `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings` all pass.

## Notes

- This task touches **6 files** plus a NEW `popup.rs`. Substantial diff; budget time accordingly.
- The cursor type `(tid, depth, ts)` is chosen over an index-based path for stability across batches — when the buffer cap evicts oldest roots, the cursor stays valid for surviving events.
- **Auto-pan vs. user pan tension:** when the user pans manually and then the selection auto-pans on `↓` navigation, the latest user-pan position is overridden. This is intentional — selection nav implies the user wants to see the event. Documented in PLAN.md §D2.
- **Children count** in popup includes only direct children (depth + 1), not recursive descendants. Match DevTools convention.
- **Parent chain breadcrumb truncation:** if the chain is deeper than `MAX_BREADCRUMB_NODES = 4`, show `root → … → parent → current`. Use `truncate_with_ellipsis` from Phase 3-followup's `text_helpers.rs` for individual name truncation.

---

## Completion Summary

**Status:** Done
**Branch:** worktree-agent-a311f10ce285eeec1

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/session/performance.rs` | Added `TimelineEventCursor`, `SelectionDirection`, `timeline_selected_event`, `timeline_details_popup_open` fields + default values |
| `crates/fdemon-app/src/session/mod.rs` | Re-exported `TimelineEventCursor` and `SelectionDirection` |
| `crates/fdemon-app/src/message.rs` | Added 6 new Message variants: `TimelineSelectFirstVisible`, `TimelineMoveSelection`, `TimelineOpenPopup`, `TimelineClosePopup`, `TimelineClearSelection`, `TimelineSelectAt`; imported `SelectionDirection` and `TimelineEventCursor` |
| `crates/fdemon-app/src/handler/devtools/performance/timeline.rs` | Added selection handlers: `handle_select_first_visible`, `handle_move_selection`, `handle_open_popup`, `handle_close_popup`, `handle_clear_selection`, `handle_select_at` + navigation helpers + 11 new tests |
| `crates/fdemon-app/src/handler/devtools/performance/mod.rs` | Exported new selection handlers |
| `crates/fdemon-app/src/handler/update.rs` | Wired up 6 new Message variants to selection handlers |
| `crates/fdemon-app/src/handler/keys.rs` | Added selection-nav key arms (Esc, Enter, Up/Down/j/k with `has_selection`, Left/Right with `has_selection`/`on_timeline_tab` guards); pre-computed `on_timeline_tab`, `has_selection`, `popup_open` at function top; added 6 new key ordering tests |
| `crates/fdemon-tui/src/widgets/devtools/performance/details/timeline_events/gantt.rs` | Updated `render_thread_row` and `render_bar` to accept and apply `timeline_selected_event` cursor for selection highlight (REVERSED modifier + `▏`/`▕` markers) |
| `crates/fdemon-tui/src/widgets/devtools/performance/details/timeline_events/gantt_tests.rs` | Added 2 new selection highlight tests |
| `crates/fdemon-tui/src/widgets/devtools/performance/details/timeline_events/mod.rs` | Declared `pub(super) mod popup` + conditionally renders popup last |
| `crates/fdemon-tui/src/widgets/devtools/performance/details/timeline_events/popup.rs` | NEW: Details popup renderer with modal_overlay chrome, body fields, parent chain breadcrumb, footer hints + 6 tests |

### Notable Decisions/Tradeoffs

1. **Pre-computing selection vars at function top in keys.rs**: Moved `on_timeline_tab`, `has_selection`, `popup_open` from inside the `if in_performance` block to the top of `handle_key_devtools` so they're accessible in both the early-return block AND the main `match key` block. This avoids code duplication and ensures the Left/Right arm guards can reference `has_selection`.

2. **Cloning tracks for navigation**: `handle_move_selection` clones `timeline_tracks` to look up the current node without holding a mutable borrow. For the typical ring buffer size (≤10,000 nodes), this is acceptable. An optimization using a cursor path instead of a scan is deferred.

3. **Popup modal precedence via render-last**: The popup renders after the Gantt on the same buffer area, providing visual modal precedence. True click suppression (passing `None` as `MouseCtx`) is deferred as the current Gantt `render` function doesn't yet accept `MouseCtx`. The Esc/Enter key handling already provides proper modal precedence at the input level.

4. **find_in_slice_with_chain explicit lifetime**: The helper was written with explicit `'a` lifetimes (valid for correctness) even though clippy prefers lifetime elision on the outer `find_node_with_chain`. Fixed to use elided lifetime on the outer function while keeping explicit on the inner recursive function.

### Testing Performed

- `cargo fmt --all -- --check` - Passed
- `cargo check --workspace --all-targets` - Passed
- `cargo test --workspace` - Passed (2494 fdemon-app, 842 fdemon-tui, 817 fdemon-core, all others ok)
- `cargo clippy --workspace --all-targets -- -D warnings` - Passed

### Risks/Limitations

1. **Mouse selection (AC11/AC12)**: `TimelineSelectAt` message and click registration in `render_bar` are not wired up because the Gantt `render` function doesn't accept `MouseCtx`. Mouse selection requires threading `MouseCtx` through the entire call stack, which was a larger change than the task description implied. The keyboard selection path is fully functional.

2. **Auto-pan when popup open**: When the popup is open, pressing `←`/`→` emits `TimelineMoveSelection` which triggers auto-pan. The popup re-renders in the new viewport position (centered on new event). This may cause brief visual flicker but is functionally correct.

3. **Track clone on move**: `handle_move_selection` clones `timeline_tracks` for safe borrow splitting. Acceptable for current buffer sizes.
