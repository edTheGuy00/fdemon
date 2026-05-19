# Task 04 — Timeline Search Input and Match Highlighting

**Status:** Not Started
**Wave:** 2 (sequential after T03 — shared write files)
**Agent:** implementor
**Estimated Effort:** 3–5 hours
**Depends On:** T01 (viewport state), T03 (selection cursor — `n`/`N` selects-and-pans to next match)

## Problem

In a busy timeline with hundreds of events per second, finding a specific event by name is impractical with manual pan/zoom. Phase 5 adds a search input (`/`) that highlights matching event bars in the Gantt and provides `n`/`N` to jump the viewport to next/previous match.

Search is **highlighting + navigation**, not filtering — matching bars are visually emphasized; non-matching bars remain visible (dimmed at most). This matches DevTools' search-and-jump UX.

## Files (Write)

- `crates/fdemon-app/src/session/performance.rs` — new search-state fields
- `crates/fdemon-app/src/handler/keys.rs` — new arms: `/` opens input, `n`/`N` jump to match, char input while active, `Esc` clears
- `crates/fdemon-app/src/handler/devtools/performance/timeline.rs` — new handlers
- `crates/fdemon-app/src/message.rs` — new variants
- `crates/fdemon-tui/src/widgets/devtools/performance/details/timeline_events/search.rs` (NEW)
- `crates/fdemon-tui/src/widgets/devtools/performance/details/timeline_events/gantt.rs` — match highlight overlay
- `crates/fdemon-tui/src/widgets/devtools/performance/details/timeline_events/mod.rs` — declare `pub(super) mod search;`, insert search bar above filter strip when active

## Files (Read)

- T01 outputs (viewport state — `n`/`N` panning)
- T03 outputs (selection cursor — `n`/`N` selects-and-pans in one action)
- Phase 4 outputs

## Approach Hints

### State additions (in `PerformanceState`)

```rust
/// The active search query. None = no query. Empty string = input is open but
/// query is empty (still in input mode).
pub timeline_search_query: Option<String>,

/// True while the user is typing in the search input (`/` opened it, neither
/// Enter nor Esc has closed it yet). When false, the query is "committed" and
/// match navigation (`n`/`N`) is active.
pub timeline_search_input_active: bool,

/// Current match index when navigating with `n`/`N`. Wraps modulo match count.
pub timeline_search_match_cursor: usize,
```

### New Message variants

```rust
TimelineSearchOpen { session_id: SessionId },
TimelineSearchInputChar { session_id: SessionId, ch: char },
TimelineSearchInputBackspace { session_id: SessionId },
TimelineSearchInputCommit { session_id: SessionId },     // Enter: close input, keep query
TimelineSearchInputCancel { session_id: SessionId },     // Esc:   close input, clear query
TimelineSearchNextMatch { session_id: SessionId },        // n
TimelineSearchPrevMatch { session_id: SessionId },        // N
```

### Keybinding arms

```rust
// On TimelineEvents tab, when NOT in search input mode:
InputKey::Char('/') => Some(Message::TimelineSearchOpen { session_id }),
InputKey::Char('n') if perf.timeline_search_query.is_some() => Some(Message::TimelineSearchNextMatch { session_id }),
InputKey::Char('N') if perf.timeline_search_query.is_some() => Some(Message::TimelineSearchPrevMatch { session_id }),
// CRITICAL: `n` falls through to the top-level Network panel shortcut when
// no search query is active. Confirm against the existing `n` handler in keys.rs.

// When `timeline_search_input_active == true`:
InputKey::Char(c)        => Some(Message::TimelineSearchInputChar { session_id, ch: c }),
InputKey::Backspace      => Some(Message::TimelineSearchInputBackspace { session_id }),
InputKey::Enter          => Some(Message::TimelineSearchInputCommit { session_id }),
InputKey::Esc            => Some(Message::TimelineSearchInputCancel { session_id }),
```

The `timeline_search_input_active` check must happen **before** the general DevTools tab dispatch — when the search input is active, all character input goes to the query, not to tab navigation. Mirror the pattern from `keys.rs` lines 105–138 (existing search-input mode for logs view).

### Match collection

Iterate all events across all tracks (depth-first), filter by `name.to_lowercase().contains(&query.to_lowercase())`. Collect into `Vec<TimelineEventCursor>` sorted by `ts` ascending. Cache per-batch if perf becomes a concern; for MVP, recompute on `n`/`N` press.

### `n`/`N` navigation

```rust
pub fn handle_next_match(state: &mut AppState, session_id: SessionId) -> UpdateResult {
    let Some(handle) = state.session_manager.get_mut(session_id) else { return UpdateResult::none() };
    let perf = &mut handle.session.performance;
    let Some(query) = perf.timeline_search_query.as_ref() else { return UpdateResult::none() };
    let matches = collect_matches(&perf.timeline_tracks, query, perf.timeline_events_filter);
    if matches.is_empty() {
        return UpdateResult::none();
    }
    perf.timeline_search_match_cursor = (perf.timeline_search_match_cursor + 1) % matches.len();
    let cursor = matches[perf.timeline_search_match_cursor];
    // Update selection (depends on T03 having landed)
    perf.timeline_selected_event = Some(cursor);
    // Pan viewport to center on match
    let width = perf.timeline_viewport_width_micros;
    perf.timeline_viewport_start_micros = (cursor.ts as u64).saturating_sub(width / 2);
    perf.timeline_follow_latest = false;
    UpdateResult::none()
}
```

### Search bar UI (`search.rs`)

Single-row widget rendered above the filter strip when `search_input_active` OR `search_query.is_some()`:

```
┌──────────────────────────────────────────────────────────────────────┐
│ / Raster▏                                            12 matches • n/N│
└──────────────────────────────────────────────────────────────────────┘
```

When input is committed (closed but query persists), render compactly:

```
│ / "Raster" • 12 matches • n/N for next/prev • Esc to clear           │
```

### Match highlight in Gantt (`gantt.rs`)

When rendering each bar, after the base color is applied:

```rust
if let Some(query) = &state.timeline_search_query {
    if node.name.to_lowercase().contains(&query.to_lowercase()) {
        // Apply highlight: brighter fg, or distinct border, or bold
        for dx in 0..width {
            if let Some(cell) = buf.cell_mut((x + dx, y)) {
                cell.add_modifier(Modifier::BOLD | Modifier::UNDERLINED);
            }
        }
    }
}
```

If the bar is **also** the current match cursor (`matches[match_cursor] == bar_cursor`), add an additional indicator (e.g., reverse video) to distinguish the "currently focused match" from "other matches."

## Acceptance Criteria

1. **State fields** — `timeline_search_query: Option<String>`, `timeline_search_input_active: bool`, `timeline_search_match_cursor: usize` added with documented defaults `(None, false, 0)`.
2. **`/` opens input** — Pressing `/` on TimelineEvents tab sets `search_input_active = true`, `search_query = Some("")`. The search bar renders. New test.
3. **Character input appends to query** — Typing `R`, `a`, `s`, `t`, `e`, `r` produces `query = "Raster"`. Backspace deletes last char. New test.
4. **Enter commits query** — `Enter` while input active sets `search_input_active = false`, keeps `query`. Now `n`/`N` are armed.
5. **Esc clears query** — `Esc` while input active sets `search_input_active = false`, `query = None`. The search bar disappears.
6. **`n` navigates to next match** — With `query = Some("Raster")` and 12 matches, pressing `n` once: `match_cursor = 1`, viewport pans to center on `matches[1].ts`, `follow_latest = false`, selection updated to `matches[1]`. New test.
7. **`N` navigates to previous match** — Mirror of `n`. Wraps modulo match count. New test.
8. **`n` falls through to Network panel when no query** — When `search_query.is_none()`, pressing `n` dispatches the existing Network-panel-enter message (no regression). New test asserts this.
9. **Match highlight in Gantt** — Bars whose name contains the query are visually highlighted (bold + underlined, or whatever style is chosen). Test inspects buffer for the modifier on a matching bar.
10. **Current-match emphasis** — The match at `match_cursor` is additionally emphasized (reverse video or distinct border) so users see which match `n`/`N` cycles around. Test.
11. **Search bar renders** — When input active OR query non-None, the search bar appears above the filter strip with the current query, match count, and hotkey hint. Test inspects buffer for the bar content.
12. **Match count updates with new events** — When a new batch arrives with events matching the query, the next render shows the increased match count. Recompute on render or on batch-receive; document choice.
13. **Filter interaction** — Pressing `T` (thread filter cycle) re-evaluates matches against the new filter (matches on hidden threads excluded). New test.
14. **Case-insensitive search** — `query = "raster"` matches event named `"GPURasterizer::Draw"`. New test.
15. **Empty query** — `query = Some("")` matches nothing (or all? document choice; recommend "no matches" — empty query is just input-mode noise).
16. **Selection sync** — When `n`/`N` advances, `timeline_selected_event` updates to the matched cursor. Verify it works alongside T03's selection model.
17. **Quality gate** — `cargo fmt --all -- --check`, `cargo check --workspace --all-targets`, `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings` all pass.

## Notes

- **`n` keybinding conflict resolution:** The fallthrough pattern (T08 of Phase 3-followup established this for `R`/HotRestart) is the right model. Within the TimelineEvents tab guarded branch, `n` returns `Some(Message::TimelineSearchNextMatch)` only when query is non-None; otherwise falls through to the main DevTools match where `n` → Network. Mirror that exact pattern.
- **Match collection cost:** `O(events × query.len)` per `n`/`N` press. With 1000 events and a 10-char query, that's ~10k char comparisons. Fast enough for a manual keypress. If profiling shows it as a hot path, cache match cursors in state and invalidate on batch-receive.
- **Match cursor on query change:** When the query changes (input chars added/deleted), reset `match_cursor = 0`. Document.
- **Stretch goal — incremental search:** highlight matches in real-time as the user types. Recommend enabling by default since match collection is cheap.
- **Mouse:** clicking the search bar (`/` glyph area) opens the input. Stretch goal — defer if scope tight.
- **Search persists across panel switches?** When the user leaves the Performance panel and returns, should the query survive? **Recommend yes** — preserves UX flow. Phase 3-followup's pause-and-clear logic (T01 there) clears `timeline_tracks` and `timeline_thread_name_map`; **add `timeline_search_*` fields to that clear list** so a stale query doesn't survive a buffer reset. Update `handler/devtools/mod.rs::handle_exit_devtools_mode` and `handle_switch_panel` accordingly.
