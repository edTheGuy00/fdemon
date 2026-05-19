# Task 03 — Timeline Event Selection and Details Popup

**Status:** Not Started
**Wave:** 2
**Agent:** implementor
**Estimated Effort:** 5–7 hours
**Depends On:** T01 (viewport state)

## Problem

After Phase 4, users can see colored event bars in the Gantt but cannot inspect any of them. There's no selection, no "what is this bar?" affordance.

Phase 5 adds:

1. A **selection cursor** identifying one event in the timeline tree.
2. **Keyboard navigation** to move the cursor (`Enter` selects first visible; `←`/`→` traverses siblings; `↑`/`↓` traverses depth or threads).
3. A **details popup** showing the event's name, ts, dur, thread, parent chain (modal overlay).
4. **Mouse selection** — clicking a bar selects it (uses the existing mouse region registry).

## Files (Write)

- `crates/fdemon-app/src/session/performance.rs` — new fields
- `crates/fdemon-app/src/handler/keys.rs` — new arms for Enter, arrow nav when selection active, Esc fallthrough
- `crates/fdemon-app/src/handler/devtools/performance/timeline.rs` — new handlers
- `crates/fdemon-app/src/message.rs` — new Message variants
- `crates/fdemon-tui/src/widgets/devtools/performance/details/timeline_events/popup.rs` (NEW)
- `crates/fdemon-tui/src/widgets/devtools/performance/details/timeline_events/gantt.rs` — selection-overlay highlight
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

Augment T01's TimelineEvents-tab branch:

```rust
let has_selection = perf.timeline_selected_event.is_some();
let popup_open = perf.timeline_details_popup_open;

match key {
    // Popup-first: when popup is open, Esc closes it before falling through.
    InputKey::Esc if popup_open => Some(Message::TimelineClosePopup { session_id }),
    InputKey::Esc if has_selection => Some(Message::TimelineClearSelection { session_id }),
    // Selection-first nav:
    InputKey::Enter if has_selection && !popup_open => Some(Message::TimelineOpenPopup { session_id }),
    InputKey::Enter => Some(Message::TimelineSelectFirstVisible { session_id }),
    InputKey::Left  if has_selection => Some(Message::TimelineMoveSelection { session_id, dir: PrevSibling }),
    InputKey::Right if has_selection => Some(Message::TimelineMoveSelection { session_id, dir: NextSibling }),
    InputKey::Up    if has_selection => Some(Message::TimelineMoveSelection { session_id, dir: ParentOrUpThread }),
    InputKey::Down  if has_selection => Some(Message::TimelineMoveSelection { session_id, dir: FirstChildOrDownThread }),
    // ... (T01's pan/zoom arms now guarded by `!has_selection`)
    InputKey::Left  if !has_selection => Some(Message::TimelinePanLeft { session_id }),
    InputKey::Right if !has_selection => Some(Message::TimelinePanRight { session_id }),
    // ... rest of T01 keys ...
}
```

This refines T01's unconditional pan arms.

### Auto-pan to keep selection visible

When the selection moves outside the current viewport, snap the viewport to center on the selected event:

```rust
fn ensure_selection_visible(perf: &mut PerformanceState, cursor: TimelineEventCursor) {
    let (vp_start, vp_end) = compute_viewport(/* args */);
    let event_end = cursor.ts as u64 + /* dur */;
    if (cursor.ts as u64) < vp_start || event_end > vp_end {
        let width = vp_end - vp_start;
        perf.timeline_viewport_start_micros = (cursor.ts as u64).saturating_sub(width / 2);
        perf.timeline_follow_latest = false;
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
