# Task 04 — Timeline State: Thread-Grouped Tree (Breaking Change)

**Status:** Not Started
**Wave:** 2
**Agent:** implementor
**Estimated Effort:** 3–5 hours
**Depends On:** T02 (consumes `TimelineTrack`, `TimelineNode`, `pair_be_events`, `ThreadMetadata`, `build_tracks`)

## Problem

`PerformanceState::timeline_events: VecDeque<TimelineEvent>` is a flat ring buffer. The Gantt-style Timeline Events view (T05) needs per-thread trees with reconstructed durations. Replace the flat buffer with a thread-grouped tree structure, wire up the currently-dead `timeline_thread_name_map`, and update the handler that processes incoming batches.

This is a **breaking state-shape change**, but the only consumers are the polling-task → handler → widget pair (verified via codebase audit — see PLAN.md §D3). No MCP, headless, or service code touches these fields.

## Files (Write)

- `crates/fdemon-app/src/session/performance.rs` — replace `timeline_events` field, update `Default`, update unit tests
- `crates/fdemon-app/src/handler/devtools/performance/timeline.rs` — rewrite `handle_batch`, update tests
- `crates/fdemon-app/src/handler/devtools/mod.rs` — update `timeline_events.clear()` calls in `handle_exit_devtools_mode` and `handle_switch_panel`
- `crates/fdemon-app/src/message.rs` — extend `TimelineEventsBatchReceived` to carry `metadata`

## Files (Read)

- T02 outputs: `crates/fdemon-core/src/timeline.rs` — `TimelineTrack`, `TimelineNode`, `pair_be_events`, `ThreadMetadata`, `build_tracks`
- `crates/fdemon-app/src/actions/performance.rs` — verify the polling task is forwarding metadata (T03 may also need a small update to pass metadata through)

## Approach Hints

### State changes (`session/performance.rs`)

**Before:**

```rust
pub struct PerformanceState {
    // ...
    pub timeline_events: VecDeque<TimelineEvent>,
    pub timeline_events_scroll_offset: usize,
    pub timeline_thread_name_map: HashMap<i64, String>,
    pub timeline_events_filter: TimelineFilter,
    // ...
}
```

**After:**

```rust
pub struct PerformanceState {
    // ...
    /// Per-thread event trees, keyed by `tid`. Iteration order is `tid` ascending
    /// (BTreeMap) so the renderer produces stable thread-row ordering.
    pub timeline_tracks: BTreeMap<i64, TimelineTrack>,
    /// Render-hint write-back: actual visible thread-row count last frame.
    pub timeline_visible_row_count: Cell<usize>,
    /// Scroll offset measured in thread rows (not events).
    pub timeline_thread_scroll_offset: usize,
    /// Populated from `ph: "M"` thread-name metadata events. Used by the
    /// Gantt renderer to label thread rows.
    pub timeline_thread_name_map: HashMap<i64, String>,
    /// Existing thread-filter cycle preserved (UI / Raster / All).
    pub timeline_events_filter: TimelineFilter,
    // ...
}
```

Remove `timeline_events: VecDeque<TimelineEvent>` and `timeline_events_scroll_offset` entirely — no backward-compat shims.

### Buffer-cap policy

The current code caps `timeline_events.len()` at `settings.devtools.timeline_event_buffer_size` (default 1000). For the tree model, cap **total node count across all tracks** at the same limit. Eviction strategy:

- When total exceeds cap, drop the **oldest events globally** by `ts`. Trim each track's `root_events` from the front while preserving children inside surviving roots.
- Document the choice in the rendered Completion Summary.

### Handler rewrite (`handler/devtools/performance/timeline.rs`)

```rust
pub fn handle_batch(state: &mut AppState, msg: TimelineEventsBatchReceived) -> UpdateResult {
    let TimelineEventsBatchReceived { session_id, events, metadata } = msg;
    let Some(handle) = state.session_manager.get_mut(session_id) else {
        return UpdateResult::none();
    };
    let buffer_cap = state.settings.devtools.timeline_event_buffer_size;

    // 1. Update thread name map from metadata.
    for ThreadMetadata { tid, name } in metadata {
        handle.session.performance.timeline_thread_name_map.insert(tid, name);
    }

    // 2. Build incremental tracks from this batch.
    let new_tracks = fdemon_core::timeline::build_tracks(&events);

    // 3. Merge into existing tracks (append root_events, preserve thread names).
    let tracks = &mut handle.session.performance.timeline_tracks;
    let names = &handle.session.performance.timeline_thread_name_map;
    for (tid, new_track) in new_tracks {
        let entry = tracks.entry(tid).or_insert_with(|| TimelineTrack {
            tid,
            name: names.get(&tid).cloned(),
            thread: new_track.thread,
            root_events: Vec::new(),
        });
        // Refresh thread name if metadata arrived later.
        if entry.name.is_none() {
            entry.name = names.get(&tid).cloned();
        }
        entry.root_events.extend(new_track.root_events);
    }

    // 4. Enforce buffer cap.
    enforce_track_buffer_cap(tracks, buffer_cap);

    UpdateResult::none()
}

/// Drops the oldest root events globally (across all tracks) until total
/// node count <= cap. Counts nodes including children.
fn enforce_track_buffer_cap(tracks: &mut BTreeMap<i64, TimelineTrack>, cap: usize) {
    fn count_nodes(node: &TimelineNode) -> usize {
        1 + node.children.iter().map(count_nodes).sum::<usize>()
    }
    fn total(tracks: &BTreeMap<i64, TimelineTrack>) -> usize {
        tracks.values()
            .flat_map(|t| t.root_events.iter())
            .map(count_nodes)
            .sum()
    }
    while total(tracks) > cap {
        // Find the track with the oldest first root event and pop it.
        let oldest_tid = tracks.iter()
            .filter(|(_, t)| !t.root_events.is_empty())
            .min_by_key(|(_, t)| t.root_events[0].ts)
            .map(|(tid, _)| *tid);
        match oldest_tid {
            Some(tid) => { tracks.get_mut(&tid).unwrap().root_events.remove(0); }
            None => break,
        }
    }
}
```

### Lifecycle integration (`handler/devtools/mod.rs`)

Replace existing `timeline_events.clear()` + `timeline_events_scroll_offset = 0` calls in two places:

- `handle_exit_devtools_mode`
- `handle_switch_panel` (Performance-leave branch)

With:

```rust
let perf = &mut handle.session.performance;
perf.timeline_tracks.clear();
perf.timeline_thread_name_map.clear();
perf.timeline_thread_scroll_offset = 0;
```

### Message extension (`message.rs`)

Add `metadata: Vec<ThreadMetadata>` to the `TimelineEventsBatchReceived` variant. If T03 already added a placeholder `metadata: vec![]`, just extend it to be a real field.

### Polling task touch-up (`actions/performance.rs`) — minor

The polling task currently strips `ph:"M"` metadata events during parsing. After T02, the parser exposes `parse_vm_timeline_with_metadata`. Update the polling task to call this new function and include the metadata in the dispatched message. **This is a minor change to a file T03 also touches — if T03 has already landed, this task picks up the work; if T03 hasn't landed yet, sequence accordingly (orchestrator will manage).**

### Filter handling

The `T`-key cycles `TimelineFilter::{All, Ui, Raster}`. In the Gantt view, this becomes "show only thread rows matching this filter." Update the filter cycle handler:

```rust
let current = handle.session.performance.timeline_events_filter;
handle.session.performance.timeline_events_filter = current.next();
handle.session.performance.timeline_thread_scroll_offset = 0;  // reset on filter change
```

## Acceptance Criteria

1. **State shape replaced** — `PerformanceState::timeline_events` and `timeline_events_scroll_offset` are removed. `timeline_tracks: BTreeMap<i64, TimelineTrack>`, `timeline_thread_scroll_offset: usize`, `timeline_visible_row_count: Cell<usize>` added. All field-presence tests in `session/performance.rs::tests` updated.
2. **Default state** — `PerformanceState::default()` initializes empty `timeline_tracks`, `0` scroll offset, empty thread name map.
3. **`handle_batch` rewritten** — Consumes `TimelineEventsBatchReceived { session_id, events, metadata }`, builds tracks via `build_tracks`, merges into existing tracks, updates `timeline_thread_name_map` from metadata, enforces buffer cap.
4. **Buffer cap enforcement** — With `buffer_cap = 5` and a batch of 10 events on `tid=1`, only the 5 most recent (by `ts`) survive. New test `enforce_track_buffer_cap_drops_oldest`.
5. **Thread name population** — A batch with `metadata: [ThreadMetadata { tid: 45067, name: "io.flutter.raster" }]` populates `timeline_thread_name_map[45067] = "io.flutter.raster"`. New test `metadata_populates_thread_name_map`.
6. **Lifecycle clears tracks** — Calling `handle_exit_devtools_mode` empties `timeline_tracks` and `timeline_thread_name_map`. Calling `handle_switch_panel` from Performance to Inspector empties them. Existing pause-related tests get updated to assert track clearing instead of event clearing.
7. **Filter cycle preserved** — `T` keypress still cycles `All → Ui → Raster → All`. `timeline_thread_scroll_offset` resets to 0 on filter change.
8. **All existing handler tests pass** with state-shape updates — e.g., the test that asserted `perf.timeline_events.len() == 2` becomes `perf.timeline_tracks.get(&tid).unwrap().root_events.len() == 2` (or equivalent depending on B/E pairing).
9. **No remaining references** to `timeline_events: VecDeque<...>` or `timeline_events_scroll_offset` anywhere in the workspace (`rg` clean).
10. **Quality gate** — `cargo fmt --all -- --check`, `cargo check --workspace --all-targets`, `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings` all pass. **Tests in `fdemon-tui` will fail until T05 lands** — this is expected; gate T04's completion on `cargo check --workspace` clean and `cargo test -p fdemon-app -p fdemon-core` green, and document that T05 will restore TUI tests.

## Notes

- This task gates T05 (TUI rewrite). T05 cannot start until this lands.
- The `Cell<usize>` for `timeline_visible_row_count` follows the CODE_STANDARDS Principle 3 render-hint pattern. Add the standard `// EXCEPTION:` annotation at the write site (in T05's renderer).
- Removing `timeline_events_scroll_offset` (event-line scroll) and adding `timeline_thread_scroll_offset` (thread-row scroll) is semantically correct: the Gantt scrolls by thread row, not by event line.
- If `cargo test -p fdemon-tui` shows compilation failures because `timeline_events_tab.rs` still references the old state shape, that's expected — T05 replaces that file. Confirm `cargo check --workspace` is green so T05 can compile from a clean baseline.

---

## Completion Summary

**Status:** Done
**Branch:** feat/devtools-inspector-parity

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/session/performance.rs` | Replaced `timeline_events: VecDeque<TimelineEvent>` and `timeline_events_scroll_offset` with `timeline_tracks: BTreeMap<i64, TimelineTrack>`, `timeline_visible_row_count: Cell<usize>`, `timeline_thread_scroll_offset: usize`. Updated `Default` impl and tests. |
| `crates/fdemon-app/src/handler/devtools/performance/timeline.rs` | Rewrote `handle_batch` to accept `(state, session_id, events, metadata)` — builds trees via `build_tracks`, merges into `timeline_tracks`, updates `timeline_thread_name_map`, enforces cap. Added `enforce_track_buffer_cap`. Updated `handle_cycle_filter` to reset `timeline_thread_scroll_offset`. Replaced all tests. |
| `crates/fdemon-app/src/handler/devtools/mod.rs` | Updated `handle_exit_devtools_mode` and `handle_switch_panel` Performance-leave branch to clear `timeline_tracks`, `timeline_thread_name_map`, reset `timeline_thread_scroll_offset`. Updated `test_leaving_performance_clears_timeline_buffer` → `test_leaving_performance_clears_timeline_tracks`. |
| `crates/fdemon-app/src/message.rs` | Added `metadata: Vec<ThreadMetadata>` to `TimelineEventsBatchReceived` variant. Updated doc comment. |
| `crates/fdemon-app/src/handler/update.rs` | Updated `TimelineEventsBatchReceived` dispatch to pass `metadata` to `handle_batch`. |
| `crates/fdemon-app/src/actions/performance.rs` | Switched `run_one_timeline_fetch_cycle` to use `fetch_timeline_chunk_with_metadata`; passes `metadata` in the dispatched message. |
| `crates/fdemon-daemon/src/vm_service/timeline.rs` | Added `fetch_timeline_chunk_with_metadata` function. Updated import to include `parse_vm_timeline_with_metadata` and `ThreadMetadata`. |
| `crates/fdemon-daemon/src/vm_service/mod.rs` | Re-exported `fetch_timeline_chunk_with_metadata`. |

### Notable Decisions/Tradeoffs

1. **Buffer cap eviction by global oldest root event**: When total node count exceeds cap, we find the track whose first root event has the smallest `ts` globally and remove it. This preserves tree integrity (no mid-subtree trimming) but may drop events from a single busy thread faster than evenly across threads. This matches the task spec verbatim and is correct for typical Flutter workloads where UI and Raster threads interleave.

2. **`handle_batch` signature uses positional args, not a struct wrapper**: The task's pseudocode showed a `TimelineEventsBatchReceived` struct passed to `handle_batch`. We kept the existing convention of destructuring the `Message` enum variant in `update.rs` and passing fields individually — consistent with all other handlers in the codebase.

3. **`fetch_timeline_chunk` left unchanged**: The original function is preserved for backward compatibility; `fetch_timeline_chunk_with_metadata` is a new additive function. No existing tests needed changing.

4. **TUI `timeline_events_tab.rs` compilation failures**: 4 errors in `fdemon-tui` are expected — the file references the removed `timeline_events` and `timeline_events_scroll_offset` fields. T05 replaces this file. `cargo check --workspace` does fail on `fdemon-tui`; `cargo check -p fdemon-app -p fdemon-core -p fdemon-daemon` is fully clean.

### Testing Performed

- `cargo fmt --all -- --check` — Passed
- `cargo check -p fdemon-app -p fdemon-core -p fdemon-daemon` — Passed (clean)
- `cargo test -p fdemon-app -p fdemon-core` — Passed (2458 + 357 tests)
- `cargo clippy -p fdemon-app -p fdemon-core -p fdemon-daemon -- -D warnings` — Passed
- `cargo test --workspace` — `fdemon-tui` fails to compile (expected; T05 fixes it); all other crates pass

### Risks/Limitations

1. **TUI compilation broken until T05**: `fdemon-tui/timeline_events_tab.rs` still references the old state shape. T05 must land before the full workspace compiles cleanly.
2. **`timeline_visible_row_count` Cell not yet written**: The `Cell<usize>` field is initialized to 0 but not written until T05's Gantt renderer. The `// EXCEPTION:` annotation should be added at the write site in T05.
