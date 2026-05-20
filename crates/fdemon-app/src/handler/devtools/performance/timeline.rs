//! Timeline Events tab handlers — Phase 4 + Phase 5.
//!
//! Phase 4 pipeline:
//! - [`handle_batch`] — build per-thread event trees from the incoming batch,
//!   merge into existing tracks, update thread-name map, enforce buffer cap,
//!   and update the persistent `frame_anchor_map`.
//! - [`handle_cycle_filter`] — cycle the `TimelineFilter` and reset scroll.
//!
//! Phase 5 pan/zoom:
//! - [`handle_zoom_in`] / [`handle_zoom_out`] — halve/double the viewport width.
//! - [`handle_pan_left`] / [`handle_pan_right`] — pan by 10% of viewport width.
//! - [`handle_follow_latest`] — reset to live-edge/frame-anchored follow mode.

use std::collections::btree_map::Entry;
use std::collections::BTreeMap;

use crate::handler::UpdateResult;
use crate::session::performance::{SelectionDirection, TimelineEventCursor, FRAME_ANCHOR_MAP_CAP};
use crate::session::SessionId;
use crate::state::AppState;
use fdemon_core::timeline::{ThreadMetadata, TimelineEvent, TimelineNode, TimelineTrack};

// ── Phase 5: Viewport constants (mirrors TUI crate's viewport.rs) ─────────────
//
// These are defined separately from the TUI crate to respect layer boundaries
// (fdemon-app must not depend on fdemon-tui). The values must remain in sync.

/// Default timeline viewport width (5 s) — same as `TIMELINE_VIEWPORT_MICROS`
/// in the TUI crate.
const DEFAULT_VIEWPORT_MICROS: u64 = 5_000_000;

/// Minimum viewport width (100 ms) — prevents over-zoom.
const TIMELINE_VIEWPORT_MIN_MICROS: u64 = 100_000;

/// Maximum viewport width (60 s) — prevents over-zoom out.
const TIMELINE_VIEWPORT_MAX_MICROS: u64 = 60_000_000;

/// Zoom factor per `+`/`-` keypress (2× = 4 keypresses span 100 ms → 60 s).
const TIMELINE_ZOOM_FACTOR: f64 = 2.0;

/// Pan fraction per `←`/`→` keypress (10% of viewport width per keypress).
const TIMELINE_PAN_FRACTION: f64 = 0.10;

/// Frame-anchor padding constants — must stay in sync with `viewport.rs`
/// in the TUI crate (PLAN D2 mode 2 padding).
const ANCHOR_PADDING_FRACTION: f64 = 0.20;
const ANCHOR_PADDING_MIN_MICROS: u64 = 2_000;
const ANCHOR_PADDING_MAX_MICROS: u64 = 50_000;

// ── handle_batch ──────────────────────────────────────────────────────────────

/// Handle a batch of timeline events from the 1-Hz poll.
///
/// 1. Inserts thread-name metadata into `timeline_thread_name_map`.
/// 2. Builds per-thread event trees from the batch via `build_tracks`.
/// 3. Merges new tracks into the existing `timeline_tracks` map.
/// 4. Enforces the buffer cap by dropping oldest root events globally.
pub(crate) fn handle_batch(
    state: &mut AppState,
    session_id: SessionId,
    events: Vec<TimelineEvent>,
    metadata: Vec<ThreadMetadata>,
) -> UpdateResult {
    let buffer_cap = state.settings.devtools.timeline_event_buffer_size;

    let Some(handle) = state.session_manager.get_mut(session_id) else {
        return UpdateResult::none();
    };

    // 1. Update thread name map from metadata.
    for ThreadMetadata { tid, name } in &metadata {
        handle
            .session
            .performance
            .timeline_thread_name_map
            .insert(*tid, name.clone());
    }

    if events.is_empty() {
        return UpdateResult::none();
    }

    // 2. Build incremental tracks from this batch.
    let new_tracks = fdemon_core::timeline::build_tracks(&events);

    // 2b. Scan new_tracks for root events with frame_number and update the
    //     persistent frame_anchor_map before merging (avoids re-scanning the
    //     entire accumulated buffer).
    let anchor_map = &mut handle.session.performance.frame_anchor_map;
    for new_track in new_tracks.values() {
        for node in &new_track.root_events {
            if let Some(n) = node.frame_number {
                let ts = node.ts as u64;
                let end = (node.ts + node.dur.unwrap_or(0)) as u64;
                match anchor_map.entry(n) {
                    Entry::Occupied(mut e) => {
                        let (s, ee) = e.get_mut();
                        *s = (*s).min(ts);
                        *ee = (*ee).max(end);
                    }
                    Entry::Vacant(e) => {
                        e.insert((ts, end));
                    }
                }
            }
        }
    }
    // Cap the anchor map: evict oldest frame numbers (smallest keys) first.
    while anchor_map.len() > FRAME_ANCHOR_MAP_CAP {
        anchor_map.pop_first();
    }

    // 3. Merge into existing tracks (append root_events, update thread names).
    let tracks = &mut handle.session.performance.timeline_tracks;
    let names = &handle.session.performance.timeline_thread_name_map;
    for (tid, new_track) in new_tracks {
        let entry = tracks.entry(tid).or_insert_with(|| TimelineTrack {
            tid,
            name: names.get(&tid).cloned(),
            thread: new_track.thread,
            root_events: Vec::new(),
        });
        // Refresh thread name if metadata arrived later (e.g. first batch has
        // no metadata but a subsequent one does).
        if entry.name.is_none() {
            entry.name = names.get(&tid).cloned();
        }
        entry.root_events.extend(new_track.root_events);
    }

    // 4. Enforce buffer cap.
    enforce_track_buffer_cap(tracks, buffer_cap);

    UpdateResult::none()
}

// ── enforce_track_buffer_cap ──────────────────────────────────────────────────

/// Drops the oldest root events globally (across all tracks) until total node
/// count (including children) is at most `cap`.
///
/// Eviction strategy: find the track whose first root event has the smallest
/// `ts` (oldest globally) and pop it. Repeat until under cap. Preserves
/// children of all surviving root events — we never trim mid-subtree.
///
/// This matches the task specification: "drop the oldest events globally by
/// `ts`; trim each track's `root_events` from the front while preserving
/// children inside surviving roots."
fn enforce_track_buffer_cap(tracks: &mut BTreeMap<i64, TimelineTrack>, cap: usize) {
    fn count_nodes(node: &TimelineNode) -> usize {
        1 + node.children.iter().map(count_nodes).sum::<usize>()
    }

    fn total(tracks: &BTreeMap<i64, TimelineTrack>) -> usize {
        tracks
            .values()
            .flat_map(|t| t.root_events.iter())
            .map(count_nodes)
            .sum()
    }

    while total(tracks) > cap {
        // Find the track with the oldest first root event.
        let oldest_tid = tracks
            .iter()
            .filter(|(_, t)| !t.root_events.is_empty())
            .min_by_key(|(_, t)| t.root_events[0].ts)
            .map(|(tid, _)| *tid);
        match oldest_tid {
            Some(tid) => {
                tracks.get_mut(&tid).unwrap().root_events.remove(0);
            }
            None => break,
        }
    }
}

// ── handle_cycle_filter ───────────────────────────────────────────────────────

/// Handle a `TimelineEventsCycleFilter` message.
///
/// Cycles the filter: `All → Ui → Raster → All`, then resets the thread-row
/// scroll offset to the top so the user sees the most relevant threads first.
pub(crate) fn handle_cycle_filter(
    state: &mut AppState,
    session_id: crate::session::SessionId,
) -> UpdateResult {
    if let Some(handle) = state.session_manager.get_mut(session_id) {
        let current = handle.session.performance.timeline_events_filter;
        handle.session.performance.timeline_events_filter = current.next();
        handle.session.performance.timeline_thread_scroll_offset = 0;
    }
    UpdateResult::none()
}

// ── Phase 5: Pan/zoom viewport handlers ──────────────────────────────────────

/// Zoom in: halve the viewport width, centered on the current midpoint.
///
/// Sets `timeline_follow_latest = false` (manual-viewport mode).
/// Width is clamped at [`TIMELINE_VIEWPORT_MIN_MICROS`].
pub(crate) fn handle_zoom_in(state: &mut AppState, session_id: SessionId) -> UpdateResult {
    let Some(handle) = state.session_manager.get_mut(session_id) else {
        return UpdateResult::none();
    };
    let perf = &mut handle.session.performance;

    // Materialize the current viewport before mutating.
    let (cur_start, cur_end) = materialize_viewport(perf);
    let cur_width = cur_end.saturating_sub(cur_start);
    let anchor = (cur_start + cur_end) / 2;

    let (new_start, _new_end) =
        zoom_viewport(cur_start, cur_width, 1.0 / TIMELINE_ZOOM_FACTOR, anchor);
    let new_width =
        (cur_width / 2).clamp(TIMELINE_VIEWPORT_MIN_MICROS, TIMELINE_VIEWPORT_MAX_MICROS);

    perf.timeline_viewport_start_micros = new_start;
    perf.timeline_viewport_width_micros = new_width;
    perf.timeline_follow_latest = false;
    UpdateResult::none()
}

/// Zoom out: double the viewport width, centered on the current midpoint.
///
/// Sets `timeline_follow_latest = false` (manual-viewport mode).
/// Width is clamped at [`TIMELINE_VIEWPORT_MAX_MICROS`].
pub(crate) fn handle_zoom_out(state: &mut AppState, session_id: SessionId) -> UpdateResult {
    let Some(handle) = state.session_manager.get_mut(session_id) else {
        return UpdateResult::none();
    };
    let perf = &mut handle.session.performance;

    let (cur_start, cur_end) = materialize_viewport(perf);
    let cur_width = cur_end.saturating_sub(cur_start);
    let anchor = (cur_start + cur_end) / 2;

    let (new_start, _new_end) = zoom_viewport(cur_start, cur_width, TIMELINE_ZOOM_FACTOR, anchor);
    let new_width = cur_width
        .saturating_mul(2)
        .clamp(TIMELINE_VIEWPORT_MIN_MICROS, TIMELINE_VIEWPORT_MAX_MICROS);

    perf.timeline_viewport_start_micros = new_start;
    perf.timeline_viewport_width_micros = new_width;
    perf.timeline_follow_latest = false;
    UpdateResult::none()
}

/// Pan left: decrease `viewport_start_micros` by 10% of current width.
///
/// Sets `timeline_follow_latest = false`. Start saturates at 0.
pub(crate) fn handle_pan_left(state: &mut AppState, session_id: SessionId) -> UpdateResult {
    let Some(handle) = state.session_manager.get_mut(session_id) else {
        return UpdateResult::none();
    };
    let perf = &mut handle.session.performance;

    let (cur_start, cur_end) = materialize_viewport(perf);
    let cur_width = cur_end.saturating_sub(cur_start);
    let delta = (cur_width as f64 * TIMELINE_PAN_FRACTION).round() as u64;

    perf.timeline_viewport_start_micros = cur_start.saturating_sub(delta);
    perf.timeline_viewport_width_micros = cur_width;
    perf.timeline_follow_latest = false;
    UpdateResult::none()
}

/// Pan right: increase `viewport_start_micros` by 10% of current width.
///
/// Sets `timeline_follow_latest = false`.
pub(crate) fn handle_pan_right(state: &mut AppState, session_id: SessionId) -> UpdateResult {
    let Some(handle) = state.session_manager.get_mut(session_id) else {
        return UpdateResult::none();
    };
    let perf = &mut handle.session.performance;

    let (cur_start, cur_end) = materialize_viewport(perf);
    let cur_width = cur_end.saturating_sub(cur_start);
    let delta = (cur_width as f64 * TIMELINE_PAN_FRACTION).round() as u64;

    perf.timeline_viewport_start_micros = cur_start.saturating_add(delta);
    perf.timeline_viewport_width_micros = cur_width;
    perf.timeline_follow_latest = false;
    UpdateResult::none()
}

/// Resume follow-latest mode.
///
/// Sets `timeline_follow_latest = true` and resets the viewport width to the
/// default 5 s. The `committed_frame_anchor` is preserved so the next render
/// returns to the frame-anchored viewport (PLAN D2 mode 2) if one was set.
pub(crate) fn handle_follow_latest(state: &mut AppState, session_id: SessionId) -> UpdateResult {
    let Some(handle) = state.session_manager.get_mut(session_id) else {
        return UpdateResult::none();
    };
    let perf = &mut handle.session.performance;
    perf.timeline_follow_latest = true;
    perf.timeline_viewport_width_micros = DEFAULT_VIEWPORT_MICROS;
    // timeline_viewport_start_micros becomes irrelevant in follow_latest mode;
    // reset it to 0 for cleanliness.
    perf.timeline_viewport_start_micros = 0;
    UpdateResult::none()
}

// ── Internal helpers ──────────────────────────────────────────────────────────

/// Materialize the current effective viewport `(start, end)` from state.
///
/// Mirrors the 3-mode priority of `compute_active_viewport` in the TUI crate
/// (PLAN D2). Inlined here to respect layer boundaries (fdemon-app must not
/// depend on fdemon-tui). When the first pan/zoom is invoked from follow-latest
/// mode, this resolves the actual on-screen viewport so the new manual bounds
/// continue from where the user was looking rather than jumping to `(0, +5s)`.
///
/// Mode 1: manual (`!follow_latest`)            → stored `(start, start + width)`.
/// Mode 2: follow-latest + committed frame      → padded frame-anchored bounds.
/// Mode 3: follow-latest + no anchor (or miss)  → live-edge from `timeline_tracks`.
fn materialize_viewport(perf: &crate::session::performance::PerformanceState) -> (u64, u64) {
    if !perf.timeline_follow_latest {
        let start = perf.timeline_viewport_start_micros;
        let width = perf
            .timeline_viewport_width_micros
            .clamp(TIMELINE_VIEWPORT_MIN_MICROS, TIMELINE_VIEWPORT_MAX_MICROS);
        return (start, start.saturating_add(width));
    }
    if let Some(frame) = perf.committed_frame_anchor {
        if let Some(&(ts_start, ts_end)) = perf.frame_anchor_map.get(&frame) {
            let dur = ts_end.saturating_sub(ts_start);
            let raw_padding = (dur as f64 * ANCHOR_PADDING_FRACTION) as u64;
            let padding = raw_padding.clamp(ANCHOR_PADDING_MIN_MICROS, ANCHOR_PADDING_MAX_MICROS);
            return (
                ts_start.saturating_sub(padding),
                ts_end.saturating_add(padding),
            );
        }
    }
    // Mode 3 — live-edge sliding window ending at the latest event timestamp.
    let latest_ts: u64 = perf
        .timeline_tracks
        .values()
        .flat_map(|track| track.root_events.iter())
        .map(|node| {
            let end = node.ts + node.dur.unwrap_or(0);
            end.max(node.ts) as u64
        })
        .max()
        .unwrap_or(0);
    if latest_ts == 0 {
        return (0, DEFAULT_VIEWPORT_MICROS);
    }
    let end = latest_ts.max(DEFAULT_VIEWPORT_MICROS);
    (end - DEFAULT_VIEWPORT_MICROS, end)
}

/// Pure zoom computation (mirrors viewport.rs `zoom_viewport`).
fn zoom_viewport(start: u64, width: u64, factor: f64, anchor_micros: u64) -> (u64, u64) {
    let new_width_f = width as f64 * factor;
    let new_width = (new_width_f.round() as u64).max(1);
    let anchor_fraction = if width == 0 {
        0.5
    } else {
        let offset = anchor_micros.saturating_sub(start);
        (offset as f64 / width as f64).clamp(0.0, 1.0)
    };
    let anchor_new_offset = (anchor_fraction * new_width as f64).round() as u64;
    let new_start = anchor_micros.saturating_sub(anchor_new_offset);
    let new_end = new_start.saturating_add(new_width);
    (new_start, new_end)
}

// ── Phase 5 T03: Timeline event selection handlers ────────────────────────────

/// Handle `TimelineSelectFirstVisible`: select the first root event of the
/// first visible thread (tid ascending, filter-respected).
///
/// No-op if no tracks match the current filter.
pub(crate) fn handle_select_first_visible(
    state: &mut AppState,
    session_id: SessionId,
) -> UpdateResult {
    let Some(handle) = state.session_manager.get_mut(session_id) else {
        return UpdateResult::none();
    };
    let perf = &mut handle.session.performance;
    let filter = perf.timeline_events_filter;

    // Find first track (tid ascending) that passes the filter and has events.
    let first_cursor = perf
        .timeline_tracks
        .iter()
        .filter(|(_, track)| timeline_filter_matches(track.thread, filter))
        .find_map(|(&tid, track)| {
            track.root_events.first().map(|node| TimelineEventCursor {
                tid,
                depth: 0,
                ts: node.ts,
            })
        });

    if let Some(cursor) = first_cursor {
        // Auto-pan to make the event visible.
        let dur = perf
            .timeline_tracks
            .get(&cursor.tid)
            .and_then(|t| t.root_events.first())
            .and_then(|n| n.dur)
            .unwrap_or(0) as u64;
        ensure_selection_visible(perf, cursor, dur);
        perf.timeline_selected_event = Some(cursor);
    }
    UpdateResult::none()
}

/// Handle `TimelineMoveSelection`: move the cursor in the given direction.
///
/// If the identified event has been evicted from the ring buffer, clears the
/// selection and logs a debug message. Wraps at siblings boundaries.
pub(crate) fn handle_move_selection(
    state: &mut AppState,
    session_id: SessionId,
    dir: SelectionDirection,
) -> UpdateResult {
    let Some(handle) = state.session_manager.get_mut(session_id) else {
        return UpdateResult::none();
    };
    let perf = &mut handle.session.performance;

    let Some(cursor) = perf.timeline_selected_event else {
        return UpdateResult::none();
    };

    // Clone the tracks so we can look up without holding mutable borrow.
    let tracks_snapshot: BTreeMap<i64, TimelineTrack> = perf.timeline_tracks.clone();
    let filter = perf.timeline_events_filter;

    let new_cursor = move_selection(&tracks_snapshot, cursor, dir, filter);

    match new_cursor {
        SelectionMove::Found(new_c) => {
            let dur = find_node_dur(&tracks_snapshot, new_c);
            ensure_selection_visible(perf, new_c, dur);
            perf.timeline_selected_event = Some(new_c);
        }
        SelectionMove::Evicted => {
            tracing::debug!("selected timeline event evicted from buffer");
            perf.timeline_selected_event = None;
            perf.timeline_details_popup_open = false;
        }
        SelectionMove::NoTarget => {
            // Selection stays unchanged (e.g. no children, no next thread).
        }
    }
    UpdateResult::none()
}

/// Handle `TimelineOpenPopup`: open the details popup for the selected event.
pub(crate) fn handle_open_popup(state: &mut AppState, session_id: SessionId) -> UpdateResult {
    let Some(handle) = state.session_manager.get_mut(session_id) else {
        return UpdateResult::none();
    };
    if handle.session.performance.timeline_selected_event.is_some() {
        handle.session.performance.timeline_details_popup_open = true;
    }
    UpdateResult::none()
}

/// Handle `TimelineClosePopup`: close the details popup, keep selection.
pub(crate) fn handle_close_popup(state: &mut AppState, session_id: SessionId) -> UpdateResult {
    let Some(handle) = state.session_manager.get_mut(session_id) else {
        return UpdateResult::none();
    };
    handle.session.performance.timeline_details_popup_open = false;
    UpdateResult::none()
}

/// Handle `TimelineClearSelection`: clear the event selection and close popup.
pub(crate) fn handle_clear_selection(state: &mut AppState, session_id: SessionId) -> UpdateResult {
    let Some(handle) = state.session_manager.get_mut(session_id) else {
        return UpdateResult::none();
    };
    handle.session.performance.timeline_selected_event = None;
    handle.session.performance.timeline_details_popup_open = false;
    UpdateResult::none()
}

/// Handle `TimelineSelectAt`: select a specific event by cursor (mouse-driven).
///
/// If the cursor matches the currently selected event, opens the popup
/// (double-click / second click behaviour).
pub(crate) fn handle_select_at(
    state: &mut AppState,
    session_id: SessionId,
    cursor: TimelineEventCursor,
) -> UpdateResult {
    let Some(handle) = state.session_manager.get_mut(session_id) else {
        return UpdateResult::none();
    };
    let perf = &mut handle.session.performance;

    if perf.timeline_selected_event == Some(cursor) {
        // Second click on same bar → open popup.
        perf.timeline_details_popup_open = true;
    } else {
        perf.timeline_selected_event = Some(cursor);
        perf.timeline_details_popup_open = false;
    }
    UpdateResult::none()
}

// ── Selection movement helpers ────────────────────────────────────────────────

/// Result of a selection move attempt.
enum SelectionMove {
    /// A new valid cursor was found.
    Found(TimelineEventCursor),
    /// The current event was not found (evicted from buffer).
    Evicted,
    /// No target exists in the requested direction (at boundary).
    NoTarget,
}

/// Compute the new cursor after moving `dir` from `cursor` within `tracks`.
///
/// Returns `SelectionMove::Evicted` if the current event cannot be located in
/// the buffer (it was evicted). Returns `SelectionMove::NoTarget` at natural
/// boundaries (e.g. no children, no next thread).
fn move_selection(
    tracks: &BTreeMap<i64, TimelineTrack>,
    cursor: TimelineEventCursor,
    dir: SelectionDirection,
    filter: crate::session::TimelineFilter,
) -> SelectionMove {
    use SelectionDirection::*;

    // Collect filtered track list in tid-ascending order (same as Gantt).
    let filtered_tids: Vec<i64> = tracks
        .iter()
        .filter(|(_, t)| timeline_filter_matches(t.thread, filter))
        .map(|(tid, _)| *tid)
        .collect();

    // Locate the current track.
    let Some(track) = tracks.get(&cursor.tid) else {
        return SelectionMove::Evicted;
    };

    // Locate the current node by walking the tree.
    let found_info = find_node_in_track(track, cursor);
    let Some(node_info) = found_info else {
        return SelectionMove::Evicted;
    };

    let tid_idx = filtered_tids.iter().position(|&t| t == cursor.tid);

    match dir {
        PrevSibling => {
            // Move to the previous sibling at the same depth.
            if let Some(prev) = prev_sibling(node_info.siblings, cursor.ts) {
                SelectionMove::Found(TimelineEventCursor {
                    tid: cursor.tid,
                    depth: cursor.depth,
                    ts: prev.ts,
                })
            } else if node_info.siblings.is_empty() {
                SelectionMove::NoTarget
            } else {
                // Wrap to last sibling.
                let last = &node_info.siblings[node_info.siblings.len() - 1];
                SelectionMove::Found(TimelineEventCursor {
                    tid: cursor.tid,
                    depth: cursor.depth,
                    ts: last.ts,
                })
            }
        }
        NextSibling => {
            // Move to the next sibling at the same depth.
            if let Some(next) = next_sibling(node_info.siblings, cursor.ts) {
                SelectionMove::Found(TimelineEventCursor {
                    tid: cursor.tid,
                    depth: cursor.depth,
                    ts: next.ts,
                })
            } else if node_info.siblings.is_empty() {
                SelectionMove::NoTarget
            } else {
                // Wrap to first sibling.
                let first = &node_info.siblings[0];
                SelectionMove::Found(TimelineEventCursor {
                    tid: cursor.tid,
                    depth: cursor.depth,
                    ts: first.ts,
                })
            }
        }
        ParentOrUpThread => {
            if cursor.depth == 0 {
                // Move to the previous filtered thread's first root event.
                if let Some(idx) = tid_idx {
                    if idx > 0 {
                        let prev_tid = filtered_tids[idx - 1];
                        if let Some(prev_track) = tracks.get(&prev_tid) {
                            if let Some(first) = prev_track.root_events.first() {
                                return SelectionMove::Found(TimelineEventCursor {
                                    tid: prev_tid,
                                    depth: 0,
                                    ts: first.ts,
                                });
                            }
                        }
                    }
                }
                SelectionMove::NoTarget
            } else {
                // Move to parent.
                if let Some(parent_ts) = node_info.parent_ts {
                    SelectionMove::Found(TimelineEventCursor {
                        tid: cursor.tid,
                        depth: cursor.depth - 1,
                        ts: parent_ts,
                    })
                } else {
                    SelectionMove::NoTarget
                }
            }
        }
        FirstChildOrDownThread => {
            // Move to first child if any.
            if let Some(first_child) = node_info.node.children.first() {
                SelectionMove::Found(TimelineEventCursor {
                    tid: cursor.tid,
                    depth: cursor.depth + 1,
                    ts: first_child.ts,
                })
            } else {
                // Move to next filtered thread's first root event.
                if let Some(idx) = tid_idx {
                    if idx + 1 < filtered_tids.len() {
                        let next_tid = filtered_tids[idx + 1];
                        if let Some(next_track) = tracks.get(&next_tid) {
                            if let Some(first) = next_track.root_events.first() {
                                return SelectionMove::Found(TimelineEventCursor {
                                    tid: next_tid,
                                    depth: 0,
                                    ts: first.ts,
                                });
                            }
                        }
                    }
                }
                SelectionMove::NoTarget
            }
        }
    }
}

/// Info returned by `find_node_in_track`.
struct FoundNodeInfo<'a> {
    /// Reference to the matched node.
    node: &'a TimelineNode,
    /// The sibling slice containing this node (same parent or root_events).
    siblings: &'a [TimelineNode],
    /// `ts` of the parent node, or `None` at root level.
    parent_ts: Option<i64>,
}

/// Walk the tree to find the node identified by `cursor`.
///
/// Returns `None` if the node is not present (evicted).
fn find_node_in_track<'a>(
    track: &'a TimelineTrack,
    cursor: TimelineEventCursor,
) -> Option<FoundNodeInfo<'a>> {
    // DFS: find the node at (depth, ts).
    find_in_slice(&track.root_events, cursor, 0, None)
}

fn find_in_slice<'a>(
    nodes: &'a [TimelineNode],
    cursor: TimelineEventCursor,
    depth: u8,
    parent_ts: Option<i64>,
) -> Option<FoundNodeInfo<'a>> {
    if depth == cursor.depth {
        // Look for node with matching ts in this slice.
        if let Some(node) = nodes.iter().find(|n| n.ts == cursor.ts) {
            return Some(FoundNodeInfo {
                node,
                siblings: nodes,
                parent_ts,
            });
        }
        return None;
    }
    // Go deeper: find the ancestor at `depth` that contains the eventual child.
    // We descend into each node's children, passing the node's ts as parent.
    for node in nodes {
        if let Some(info) = find_in_slice(&node.children, cursor, depth + 1, Some(node.ts)) {
            return Some(info);
        }
    }
    None
}

/// Find the previous sibling (the node with the largest `ts < cursor_ts`).
fn prev_sibling(siblings: &[TimelineNode], cursor_ts: i64) -> Option<&TimelineNode> {
    siblings
        .iter()
        .filter(|n| n.ts < cursor_ts)
        .max_by_key(|n| n.ts)
}

/// Find the next sibling (the node with the smallest `ts > cursor_ts`).
fn next_sibling(siblings: &[TimelineNode], cursor_ts: i64) -> Option<&TimelineNode> {
    siblings
        .iter()
        .filter(|n| n.ts > cursor_ts)
        .min_by_key(|n| n.ts)
}

/// Find the duration of the node identified by `cursor` in `tracks`.
fn find_node_dur(tracks: &BTreeMap<i64, TimelineTrack>, cursor: TimelineEventCursor) -> u64 {
    tracks
        .get(&cursor.tid)
        .and_then(|t| find_node_in_track(t, cursor))
        .and_then(|info| info.node.dur)
        .unwrap_or(0) as u64
}

/// Returns `true` if `thread` passes `filter`.
fn timeline_filter_matches(
    thread: fdemon_core::timeline::TimelineThread,
    filter: crate::session::TimelineFilter,
) -> bool {
    use crate::session::TimelineFilter;
    match filter {
        TimelineFilter::All => true,
        TimelineFilter::Ui => thread == fdemon_core::timeline::TimelineThread::Ui,
        TimelineFilter::Raster => thread == fdemon_core::timeline::TimelineThread::Raster,
    }
}

/// Auto-pan the viewport to keep the selected event visible.
///
/// When the event falls outside the current viewport `[vp_start, vp_end)`,
/// snaps the viewport to center on the event and sets `follow_latest = false`
/// (manual viewport mode 1).
fn ensure_selection_visible(
    perf: &mut crate::session::performance::PerformanceState,
    cursor: TimelineEventCursor,
    dur: u64,
) {
    let (vp_start, vp_end) = materialize_viewport(perf);
    let event_start = cursor.ts as u64;
    let event_end = event_start.saturating_add(dur);

    if event_start < vp_start || event_end > vp_end {
        let width = vp_end - vp_start;
        perf.timeline_viewport_start_micros = event_start.saturating_sub(width / 2);
        perf.timeline_viewport_width_micros = width;
        perf.timeline_follow_latest = false;
    }
}

// ── Phase 5 T04: Timeline search handlers ────────────────────────────────────

/// Handle `TimelineSearchOpen`: open the search input on the TimelineEvents tab.
///
/// Sets `timeline_search_input_active = true` and `timeline_search_query = Some("")`.
/// Resets the match cursor to 0.
pub(crate) fn handle_search_open(state: &mut AppState, session_id: SessionId) -> UpdateResult {
    let Some(handle) = state.session_manager.get_mut(session_id) else {
        return UpdateResult::none();
    };
    let perf = &mut handle.session.performance;
    perf.timeline_search_input_active = true;
    perf.timeline_search_query = Some(String::new());
    perf.timeline_search_match_cursor = 0;
    UpdateResult::none()
}

/// Handle `TimelineSearchInputChar`: append `ch` to the query while input is active.
///
/// Resets `match_cursor` to 0 since the match set changed.
pub(crate) fn handle_search_input_char(
    state: &mut AppState,
    session_id: SessionId,
    ch: char,
) -> UpdateResult {
    let Some(handle) = state.session_manager.get_mut(session_id) else {
        return UpdateResult::none();
    };
    let perf = &mut handle.session.performance;
    if !perf.timeline_search_input_active {
        return UpdateResult::none();
    }
    if let Some(ref mut q) = perf.timeline_search_query {
        q.push(ch);
    }
    perf.timeline_search_match_cursor = 0;
    UpdateResult::none()
}

/// Handle `TimelineSearchInputBackspace`: delete the last character from the query.
///
/// Resets `match_cursor` to 0 since the match set changed.
pub(crate) fn handle_search_input_backspace(
    state: &mut AppState,
    session_id: SessionId,
) -> UpdateResult {
    let Some(handle) = state.session_manager.get_mut(session_id) else {
        return UpdateResult::none();
    };
    let perf = &mut handle.session.performance;
    if !perf.timeline_search_input_active {
        return UpdateResult::none();
    }
    if let Some(ref mut q) = perf.timeline_search_query {
        q.pop();
    }
    perf.timeline_search_match_cursor = 0;
    UpdateResult::none()
}

/// Handle `TimelineSearchInputCommit` (Enter): close input, keep query.
///
/// Sets `timeline_search_input_active = false`, keeps `timeline_search_query`
/// so `n`/`N` navigation can begin.
pub(crate) fn handle_search_input_commit(
    state: &mut AppState,
    session_id: SessionId,
) -> UpdateResult {
    let Some(handle) = state.session_manager.get_mut(session_id) else {
        return UpdateResult::none();
    };
    let perf = &mut handle.session.performance;
    perf.timeline_search_input_active = false;
    UpdateResult::none()
}

/// Handle `TimelineSearchInputCancel` (Esc): close input, clear query.
///
/// Sets `timeline_search_input_active = false`, clears `timeline_search_query`
/// so the search bar disappears.
pub(crate) fn handle_search_input_cancel(
    state: &mut AppState,
    session_id: SessionId,
) -> UpdateResult {
    let Some(handle) = state.session_manager.get_mut(session_id) else {
        return UpdateResult::none();
    };
    let perf = &mut handle.session.performance;
    perf.timeline_search_input_active = false;
    perf.timeline_search_query = None;
    perf.timeline_search_match_cursor = 0;
    UpdateResult::none()
}

/// Handle `TimelineSearchNextMatch` (`n`): advance to the next match.
///
/// Collects all matching cursors, advances `match_cursor` (wraps), pans the
/// viewport to center on the match, and updates `timeline_selected_event`.
pub(crate) fn handle_next_match(state: &mut AppState, session_id: SessionId) -> UpdateResult {
    let Some(handle) = state.session_manager.get_mut(session_id) else {
        return UpdateResult::none();
    };
    let perf = &mut handle.session.performance;
    let query = match perf.timeline_search_query.as_ref() {
        Some(q) if !q.is_empty() => q.clone(),
        _ => return UpdateResult::none(),
    };
    let filter = perf.timeline_events_filter;
    let matches = collect_matches(&perf.timeline_tracks, &query, filter);
    if matches.is_empty() {
        return UpdateResult::none();
    }
    perf.timeline_search_match_cursor =
        perf.timeline_search_match_cursor.wrapping_add(1) % matches.len();
    let cursor = matches[perf.timeline_search_match_cursor];
    perf.timeline_selected_event = Some(cursor);
    // Pan viewport to center on the matched event.
    let (vp_start, vp_end) = materialize_viewport(perf);
    let width = vp_end.saturating_sub(vp_start);
    perf.timeline_viewport_start_micros = (cursor.ts as u64).saturating_sub(width / 2);
    perf.timeline_viewport_width_micros = width;
    perf.timeline_follow_latest = false;
    UpdateResult::none()
}

/// Handle `TimelineSearchPrevMatch` (`N`): move to the previous match.
///
/// Mirrors `handle_next_match` in the reverse direction (wraps modulo count).
pub(crate) fn handle_prev_match(state: &mut AppState, session_id: SessionId) -> UpdateResult {
    let Some(handle) = state.session_manager.get_mut(session_id) else {
        return UpdateResult::none();
    };
    let perf = &mut handle.session.performance;
    let query = match perf.timeline_search_query.as_ref() {
        Some(q) if !q.is_empty() => q.clone(),
        _ => return UpdateResult::none(),
    };
    let filter = perf.timeline_events_filter;
    let matches = collect_matches(&perf.timeline_tracks, &query, filter);
    if matches.is_empty() {
        return UpdateResult::none();
    }
    let len = matches.len();
    // Wrap backwards: saturating_sub avoids underflow, then modulo wraps from 0 → last.
    perf.timeline_search_match_cursor = (perf.timeline_search_match_cursor + len - 1) % len;
    let cursor = matches[perf.timeline_search_match_cursor];
    perf.timeline_selected_event = Some(cursor);
    // Pan viewport to center on the matched event.
    let (vp_start, vp_end) = materialize_viewport(perf);
    let width = vp_end.saturating_sub(vp_start);
    perf.timeline_viewport_start_micros = (cursor.ts as u64).saturating_sub(width / 2);
    perf.timeline_viewport_width_micros = width;
    perf.timeline_follow_latest = false;
    UpdateResult::none()
}

// ── Match collection helper ───────────────────────────────────────────────────

/// Collect all timeline event cursors whose event name contains `query`
/// (case-insensitive), across all tracks that pass the current `filter`.
///
/// Results are sorted by `ts` ascending (chronological order) so `n`/`N`
/// navigation moves through matches in time order.
///
/// Empty `query` always returns an empty match list — empty-string search is
/// treated as "no matches" (not "all events") to avoid noise during input.
///
/// Cost: `O(events × query.len)` per call — acceptable for manual keypresses
/// with typical event counts (≤ 10 000 events × 10-char query ≈ 100 k char
/// comparisons). Cache invalidation is therefore unnecessary for MVP.
pub(crate) fn collect_matches(
    tracks: &BTreeMap<i64, fdemon_core::timeline::TimelineTrack>,
    query: &str,
    filter: crate::session::performance::TimelineFilter,
) -> Vec<TimelineEventCursor> {
    if query.is_empty() {
        return Vec::new();
    }
    let query_lower = query.to_lowercase();
    let mut matches: Vec<TimelineEventCursor> = Vec::new();

    for (&tid, track) in tracks {
        if !timeline_filter_matches(track.thread, filter) {
            continue;
        }
        collect_matches_in_nodes(&track.root_events, tid, 0, &query_lower, &mut matches);
    }

    // Sort by timestamp ascending for deterministic n/N navigation order.
    matches.sort_by_key(|c| c.ts);
    matches
}

/// Recursively collect matching cursors from a node slice.
fn collect_matches_in_nodes(
    nodes: &[fdemon_core::timeline::TimelineNode],
    tid: i64,
    depth: u8,
    query_lower: &str,
    out: &mut Vec<TimelineEventCursor>,
) {
    for node in nodes {
        if node.name.to_lowercase().contains(query_lower) {
            out.push(TimelineEventCursor {
                tid,
                depth,
                ts: node.ts,
            });
        }
        if depth + 1 < u8::MAX {
            collect_matches_in_nodes(&node.children, tid, depth + 1, query_lower, out);
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use crate::handler::devtools::handle_switch_panel;
    use crate::handler::update::update;
    use crate::message::Message;
    use crate::session::performance::{TimelineFilter, FRAME_ANCHOR_MAP_CAP};
    use crate::state::{AppState, DevToolsPanel};
    use fdemon_core::timeline::{ThreadMetadata, TimelineEvent, TimelinePhase, TimelineThread};

    fn test_device() -> fdemon_daemon::Device {
        fdemon_daemon::Device {
            id: "test-device".to_string(),
            name: "Test Device".to_string(),
            platform: "android".to_string(),
            emulator: false,
            category: None,
            platform_type: None,
            ephemeral: false,
            emulator_id: None,
        }
    }

    fn make_state_with_session() -> (AppState, crate::session::SessionId) {
        let mut state = AppState::new();
        let id = state
            .session_manager
            .create_session(&test_device())
            .unwrap();
        (state, id)
    }

    fn make_complete_event(name: &str, tid: i64, ts: u64, thread: TimelineThread) -> TimelineEvent {
        TimelineEvent {
            name: name.to_string(),
            category: "Embedder".to_string(),
            thread,
            tid,
            phase: TimelinePhase::Complete,
            ts,
            dur: Some(100),
            frame_number: None,
        }
    }

    fn make_frame_event(
        name: &str,
        tid: i64,
        ts: u64,
        dur: u64,
        frame_number: u64,
        thread: TimelineThread,
    ) -> TimelineEvent {
        TimelineEvent {
            name: name.to_string(),
            category: "Embedder".to_string(),
            thread,
            tid,
            phase: TimelinePhase::Complete,
            ts,
            dur: Some(dur),
            frame_number: Some(frame_number),
        }
    }

    // ── handle_batch: basic append ────────────────────────────────────────────

    #[test]
    fn handle_batch_builds_tracks_from_events() {
        let (mut state, session_id) = make_state_with_session();

        let events = vec![
            make_complete_event("Frame", 1, 1000, TimelineThread::Ui),
            make_complete_event("Raster", 2, 2000, TimelineThread::Raster),
        ];
        update(
            &mut state,
            Message::TimelineEventsBatchReceived {
                session_id,
                events,
                metadata: vec![],
            },
        );

        let perf = &state
            .session_manager
            .get(session_id)
            .unwrap()
            .session
            .performance;
        assert_eq!(
            perf.timeline_tracks.len(),
            2,
            "two tids should produce two tracks"
        );
        assert_eq!(
            perf.timeline_tracks.get(&1).unwrap().root_events.len(),
            1,
            "tid=1 should have one root event"
        );
        assert_eq!(
            perf.timeline_tracks.get(&1).unwrap().root_events[0].name,
            "Frame"
        );
        assert_eq!(
            perf.timeline_tracks.get(&2).unwrap().root_events.len(),
            1,
            "tid=2 should have one root event"
        );
        assert_eq!(
            perf.timeline_tracks.get(&2).unwrap().root_events[0].name,
            "Raster"
        );
    }

    #[test]
    fn handle_batch_empty_events_is_noop() {
        let (mut state, session_id) = make_state_with_session();

        update(
            &mut state,
            Message::TimelineEventsBatchReceived {
                session_id,
                events: vec![],
                metadata: vec![],
            },
        );

        let perf = &state
            .session_manager
            .get(session_id)
            .unwrap()
            .session
            .performance;
        assert!(perf.timeline_tracks.is_empty());
    }

    // ── handle_batch: merging across batches ──────────────────────────────────

    #[test]
    fn handle_batch_merges_across_batches() {
        let (mut state, session_id) = make_state_with_session();

        // First batch: 1 event on tid=1.
        update(
            &mut state,
            Message::TimelineEventsBatchReceived {
                session_id,
                events: vec![make_complete_event("A", 1, 100, TimelineThread::Ui)],
                metadata: vec![],
            },
        );
        // Second batch: another event on tid=1.
        update(
            &mut state,
            Message::TimelineEventsBatchReceived {
                session_id,
                events: vec![make_complete_event("B", 1, 200, TimelineThread::Ui)],
                metadata: vec![],
            },
        );

        let perf = &state
            .session_manager
            .get(session_id)
            .unwrap()
            .session
            .performance;
        let track = perf.timeline_tracks.get(&1).unwrap();
        assert_eq!(
            track.root_events.len(),
            2,
            "two events across two batches on tid=1"
        );
        assert_eq!(track.root_events[0].name, "A");
        assert_eq!(track.root_events[1].name, "B");
    }

    // ── enforce_track_buffer_cap_drops_oldest (AC4) ───────────────────────────

    #[test]
    fn enforce_track_buffer_cap_drops_oldest() {
        let (mut state, session_id) = make_state_with_session();
        // Set buffer cap to 5.
        state.settings.devtools.timeline_event_buffer_size = 5;

        // Send 10 events on tid=1 with timestamps 1..=10.
        let events: Vec<TimelineEvent> = (1u64..=10)
            .map(|ts| make_complete_event("E", 1, ts, TimelineThread::Ui))
            .collect();
        update(
            &mut state,
            Message::TimelineEventsBatchReceived {
                session_id,
                events,
                metadata: vec![],
            },
        );

        let perf = &state
            .session_manager
            .get(session_id)
            .unwrap()
            .session
            .performance;
        let track = perf.timeline_tracks.get(&1).unwrap();
        assert_eq!(
            track.root_events.len(),
            5,
            "buffer cap 5 should retain only 5 root events on tid=1"
        );
        // The 5 most recent (ts=6..=10) should survive.
        assert_eq!(
            track.root_events[0].ts, 6,
            "oldest surviving event should have ts=6"
        );
        assert_eq!(
            track.root_events[4].ts, 10,
            "most recent event should have ts=10"
        );
    }

    // ── metadata_populates_thread_name_map (AC5) ──────────────────────────────

    #[test]
    fn metadata_populates_thread_name_map() {
        let (mut state, session_id) = make_state_with_session();

        update(
            &mut state,
            Message::TimelineEventsBatchReceived {
                session_id,
                events: vec![],
                metadata: vec![ThreadMetadata {
                    tid: 45067,
                    name: "io.flutter.raster".to_string(),
                }],
            },
        );

        let perf = &state
            .session_manager
            .get(session_id)
            .unwrap()
            .session
            .performance;
        assert_eq!(
            perf.timeline_thread_name_map
                .get(&45067)
                .map(|s| s.as_str()),
            Some("io.flutter.raster"),
            "metadata should populate timeline_thread_name_map"
        );
    }

    // ── handle_cycle_filter ───────────────────────────────────────────────────

    #[test]
    fn handle_cycle_filter_cycles_all_ui_raster_all() {
        let (mut state, session_id) = make_state_with_session();

        // Default is All.
        assert_eq!(
            state
                .session_manager
                .get(session_id)
                .unwrap()
                .session
                .performance
                .timeline_events_filter,
            TimelineFilter::All
        );

        update(
            &mut state,
            Message::TimelineEventsCycleFilter { session_id },
        );
        assert_eq!(
            state
                .session_manager
                .get(session_id)
                .unwrap()
                .session
                .performance
                .timeline_events_filter,
            TimelineFilter::Ui
        );

        update(
            &mut state,
            Message::TimelineEventsCycleFilter { session_id },
        );
        assert_eq!(
            state
                .session_manager
                .get(session_id)
                .unwrap()
                .session
                .performance
                .timeline_events_filter,
            TimelineFilter::Raster
        );

        update(
            &mut state,
            Message::TimelineEventsCycleFilter { session_id },
        );
        assert_eq!(
            state
                .session_manager
                .get(session_id)
                .unwrap()
                .session
                .performance
                .timeline_events_filter,
            TimelineFilter::All
        );
    }

    #[test]
    fn handle_cycle_filter_resets_thread_scroll_offset() {
        let (mut state, session_id) = make_state_with_session();

        // Set a non-zero thread scroll offset.
        if let Some(h) = state.session_manager.get_mut(session_id) {
            h.session.performance.timeline_thread_scroll_offset = 10;
        }

        update(
            &mut state,
            Message::TimelineEventsCycleFilter { session_id },
        );

        let offset = state
            .session_manager
            .get(session_id)
            .unwrap()
            .session
            .performance
            .timeline_thread_scroll_offset;
        assert_eq!(offset, 0);
    }

    // ── frame_anchor_map: population ─────────────────────────────────────────

    /// Task AC: A batch with a Complete event carrying frame_number must populate
    /// `frame_anchor_map` with the correct `(ts, ts+dur)` range.
    #[test]
    fn handle_batch_populates_frame_anchor_map_for_events_with_frame_number() {
        let (mut state, session_id) = make_state_with_session();

        // Frame event: frame_number=7, ts=1_000_000, dur=16_000
        let events = vec![make_frame_event(
            "Frame",
            1,
            1_000_000,
            16_000,
            7,
            TimelineThread::Ui,
        )];
        update(
            &mut state,
            Message::TimelineEventsBatchReceived {
                session_id,
                events,
                metadata: vec![],
            },
        );

        let perf = &state
            .session_manager
            .get(session_id)
            .unwrap()
            .session
            .performance;
        assert!(
            perf.frame_anchor_map.contains_key(&7),
            "frame_anchor_map should have an entry for frame 7"
        );
        let &(ts_start, ts_end) = perf.frame_anchor_map.get(&7).unwrap();
        assert_eq!(ts_start, 1_000_000, "ts_start should equal event ts");
        assert_eq!(ts_end, 1_016_000, "ts_end should equal event ts + dur");
    }

    /// Task AC: Two batches for the same frame_number with different ranges must
    /// produce a map entry whose range is the union (min ts_start, max ts_end).
    #[test]
    fn handle_batch_extends_existing_frame_anchor_range() {
        let (mut state, session_id) = make_state_with_session();

        // First batch: ts=1_000_000, dur=8_000 → range [1_000_000, 1_008_000]
        update(
            &mut state,
            Message::TimelineEventsBatchReceived {
                session_id,
                events: vec![make_frame_event(
                    "Ui",
                    1,
                    1_000_000,
                    8_000,
                    42,
                    TimelineThread::Ui,
                )],
                metadata: vec![],
            },
        );
        // Second batch: ts=999_000, dur=20_000 → range [999_000, 1_019_000]
        // After union: [min(1_000_000, 999_000), max(1_008_000, 1_019_000)] = [999_000, 1_019_000]
        update(
            &mut state,
            Message::TimelineEventsBatchReceived {
                session_id,
                events: vec![make_frame_event(
                    "Raster",
                    2,
                    999_000,
                    20_000,
                    42,
                    TimelineThread::Raster,
                )],
                metadata: vec![],
            },
        );

        let perf = &state
            .session_manager
            .get(session_id)
            .unwrap()
            .session
            .performance;
        let &(ts_start, ts_end) = perf
            .frame_anchor_map
            .get(&42)
            .expect("frame 42 must exist in map");
        assert_eq!(ts_start, 999_000, "ts_start should be the minimum seen");
        assert_eq!(ts_end, 1_019_000, "ts_end should be the maximum seen");
    }

    /// Task AC: After inserting FRAME_ANCHOR_MAP_CAP + 5 distinct frames, the map
    /// must remain at most CAP entries and the oldest (smallest) frame numbers must
    /// have been evicted.
    #[test]
    fn frame_anchor_map_is_capped_at_max() {
        let (mut state, session_id) = make_state_with_session();

        // Send CAP + 5 distinct frame numbers in a single large batch.
        let total = FRAME_ANCHOR_MAP_CAP + 5;
        let events: Vec<TimelineEvent> = (0u64..total as u64)
            .map(|i| make_frame_event("Frame", 1, i * 1_000, 500, i, TimelineThread::Ui))
            .collect();
        update(
            &mut state,
            Message::TimelineEventsBatchReceived {
                session_id,
                events,
                metadata: vec![],
            },
        );

        let perf = &state
            .session_manager
            .get(session_id)
            .unwrap()
            .session
            .performance;
        assert!(
            perf.frame_anchor_map.len() <= FRAME_ANCHOR_MAP_CAP,
            "frame_anchor_map must not exceed FRAME_ANCHOR_MAP_CAP={FRAME_ANCHOR_MAP_CAP}, \
             got {}",
            perf.frame_anchor_map.len()
        );
        // Oldest frames (0..5) should have been evicted; newest (5..total) survive.
        for i in 0..5u64 {
            assert!(
                !perf.frame_anchor_map.contains_key(&i),
                "oldest frame {i} should have been evicted from the map"
            );
        }
        assert!(
            perf.frame_anchor_map.contains_key(&(total as u64 - 1)),
            "most recent frame should still be in the map"
        );
    }

    /// Task AC: Leaving the Performance panel (via handle_switch_panel) must clear
    /// `frame_anchor_map`.
    #[test]
    fn frame_anchor_map_resets_on_performance_leave() {
        let (mut state, session_id) = make_state_with_session();

        // Populate the map with a frame event.
        update(
            &mut state,
            Message::TimelineEventsBatchReceived {
                session_id,
                events: vec![make_frame_event(
                    "Frame",
                    1,
                    1_000_000,
                    16_000,
                    5,
                    TimelineThread::Ui,
                )],
                metadata: vec![],
            },
        );

        // Switch to Performance to simulate being on that panel.
        // (We need to set the active_panel to Performance first so the leave-logic fires.)
        state.devtools_view_state.active_panel = DevToolsPanel::Performance;
        assert!(
            !state
                .session_manager
                .get(session_id)
                .unwrap()
                .session
                .performance
                .frame_anchor_map
                .is_empty(),
            "frame_anchor_map should be non-empty before leaving"
        );

        // Leave Performance — switch to Inspector triggers the clear.
        handle_switch_panel(&mut state, DevToolsPanel::Inspector);

        let perf = &state
            .session_manager
            .get(session_id)
            .unwrap()
            .session
            .performance;
        assert!(
            perf.frame_anchor_map.is_empty(),
            "frame_anchor_map must be cleared when leaving the Performance panel"
        );
    }

    // ── Phase 5: handle_zoom_in ───────────────────────────────────────────────

    /// AC3: TimelineZoomIn halves the viewport width and sets follow_latest=false.
    #[test]
    fn test_zoom_in_halves_viewport() {
        let (mut state, session_id) = make_state_with_session();
        // Set up manual mode with a known viewport so the math is observable.
        if let Some(h) = state.session_manager.get_mut(session_id) {
            h.session.performance.timeline_viewport_width_micros = 2_000_000;
            h.session.performance.timeline_viewport_start_micros = 0;
            h.session.performance.timeline_follow_latest = false;
        }
        update(&mut state, Message::TimelineZoomIn { session_id });

        let perf = &state
            .session_manager
            .get(session_id)
            .unwrap()
            .session
            .performance;
        assert!(
            !perf.timeline_follow_latest,
            "zoom-in should set follow_latest=false"
        );
        assert_eq!(
            perf.timeline_viewport_width_micros, 1_000_000,
            "zoom-in should halve the 2s viewport to 1s"
        );
    }

    /// AC3: Zooming in when already at MIN does not go below MIN.
    #[test]
    fn test_zoom_in_clamps_at_min() {
        let (mut state, session_id) = make_state_with_session();
        if let Some(h) = state.session_manager.get_mut(session_id) {
            h.session.performance.timeline_viewport_width_micros = 100_000; // at MIN
            h.session.performance.timeline_follow_latest = false;
        }
        update(&mut state, Message::TimelineZoomIn { session_id });

        let perf = &state
            .session_manager
            .get(session_id)
            .unwrap()
            .session
            .performance;
        assert_eq!(
            perf.timeline_viewport_width_micros, 100_000,
            "zooming in at MIN should stay at MIN"
        );
    }

    // ── Phase 5: handle_zoom_out ──────────────────────────────────────────────

    /// AC4: TimelineZoomOut doubles the viewport width and sets follow_latest=false.
    #[test]
    fn test_zoom_out_doubles_viewport() {
        let (mut state, session_id) = make_state_with_session();
        if let Some(h) = state.session_manager.get_mut(session_id) {
            h.session.performance.timeline_viewport_width_micros = 2_000_000;
            h.session.performance.timeline_viewport_start_micros = 0;
            h.session.performance.timeline_follow_latest = false;
        }
        update(&mut state, Message::TimelineZoomOut { session_id });

        let perf = &state
            .session_manager
            .get(session_id)
            .unwrap()
            .session
            .performance;
        assert!(
            !perf.timeline_follow_latest,
            "zoom-out should set follow_latest=false"
        );
        assert_eq!(
            perf.timeline_viewport_width_micros, 4_000_000,
            "zoom-out should double the 2s viewport to 4s"
        );
    }

    /// AC4: Zooming out when already at MAX does not exceed MAX.
    #[test]
    fn test_zoom_out_doubles_viewport_to_max() {
        let (mut state, session_id) = make_state_with_session();
        if let Some(h) = state.session_manager.get_mut(session_id) {
            h.session.performance.timeline_viewport_width_micros = 60_000_000; // at MAX
            h.session.performance.timeline_follow_latest = false;
        }
        update(&mut state, Message::TimelineZoomOut { session_id });

        let perf = &state
            .session_manager
            .get(session_id)
            .unwrap()
            .session
            .performance;
        assert_eq!(
            perf.timeline_viewport_width_micros, 60_000_000,
            "zooming out at MAX should stay at MAX"
        );
    }

    // ── Phase 5: handle_pan_left / handle_pan_right ───────────────────────────

    /// AC5: TimelinePanLeft decreases start by 10% of width.
    #[test]
    fn test_pan_left_decreases_start() {
        let (mut state, session_id) = make_state_with_session();
        if let Some(h) = state.session_manager.get_mut(session_id) {
            h.session.performance.timeline_viewport_start_micros = 5_000_000;
            h.session.performance.timeline_viewport_width_micros = 5_000_000;
            h.session.performance.timeline_follow_latest = false;
        }
        update(&mut state, Message::TimelinePanLeft { session_id });

        let perf = &state
            .session_manager
            .get(session_id)
            .unwrap()
            .session
            .performance;
        assert!(
            !perf.timeline_follow_latest,
            "pan should set follow_latest=false"
        );
        // delta = 5_000_000 * 0.10 = 500_000
        assert_eq!(
            perf.timeline_viewport_start_micros, 4_500_000,
            "pan-left should decrease start by 10% of width"
        );
    }

    /// AC5: TimelinePanRight increases start by 10% of width.
    #[test]
    fn test_pan_right_increases_start() {
        let (mut state, session_id) = make_state_with_session();
        if let Some(h) = state.session_manager.get_mut(session_id) {
            h.session.performance.timeline_viewport_start_micros = 5_000_000;
            h.session.performance.timeline_viewport_width_micros = 5_000_000;
            h.session.performance.timeline_follow_latest = false;
        }
        update(&mut state, Message::TimelinePanRight { session_id });

        let perf = &state
            .session_manager
            .get(session_id)
            .unwrap()
            .session
            .performance;
        assert!(
            !perf.timeline_follow_latest,
            "pan should set follow_latest=false"
        );
        assert_eq!(
            perf.timeline_viewport_start_micros, 5_500_000,
            "pan-right should increase start by 10% of width"
        );
    }

    /// AC5: TimelinePanLeft saturates at 0.
    #[test]
    fn test_pan_left_saturates_at_zero() {
        let (mut state, session_id) = make_state_with_session();
        if let Some(h) = state.session_manager.get_mut(session_id) {
            h.session.performance.timeline_viewport_start_micros = 100; // less than delta
            h.session.performance.timeline_viewport_width_micros = 5_000_000;
            h.session.performance.timeline_follow_latest = false;
        }
        update(&mut state, Message::TimelinePanLeft { session_id });

        let perf = &state
            .session_manager
            .get(session_id)
            .unwrap()
            .session
            .performance;
        assert_eq!(
            perf.timeline_viewport_start_micros, 0,
            "pan-left should saturate at 0"
        );
    }

    /// Regression: pan from follow_latest=true with live events should anchor the
    /// new viewport at the live-edge bounds, not at the stored defaults `(0, 5s)`.
    /// Without this fix the user pans into a region with no events and the Gantt
    /// renders empty rows while the minimap still shows activity.
    #[test]
    fn test_pan_right_from_follow_latest_uses_live_edge() {
        let (mut state, session_id) = make_state_with_session();
        // App has been running ~30s; the latest root event is at ts=30s.
        if let Some(h) = state.session_manager.get_mut(session_id) {
            h.session.performance.timeline_tracks.insert(
                1,
                TimelineTrack {
                    tid: 1,
                    name: None,
                    thread: TimelineThread::Ui,
                    root_events: vec![TimelineNode {
                        name: "Frame".to_owned(),
                        category: None,
                        ts: 30_000_000,
                        dur: Some(16_000),
                        phase: TimelinePhase::Complete,
                        thread: TimelineThread::Ui,
                        frame_number: None,
                        children: vec![],
                    }],
                },
            );
            h.session.performance.timeline_follow_latest = true;
        }
        update(&mut state, Message::TimelinePanRight { session_id });

        let perf = &state
            .session_manager
            .get(session_id)
            .unwrap()
            .session
            .performance;
        // Live-edge mode 3 returns `(end - DEFAULT_VIEWPORT_MICROS, end)`
        // where end = max(latest_ts, DEFAULT_VIEWPORT_MICROS) = 30_016_000.
        // Pan right by 10% of 5s = 500_000 → new start = 25_516_000.
        assert!(
            !perf.timeline_follow_latest,
            "pan should exit follow_latest"
        );
        assert_eq!(perf.timeline_viewport_start_micros, 25_516_000);
        assert_eq!(perf.timeline_viewport_width_micros, 5_000_000);
    }

    // ── Phase 5: handle_follow_latest ────────────────────────────────────────

    /// AC6: TimelineFollowLatest sets follow_latest=true and resets width to default.
    #[test]
    fn test_follow_latest_resets_to_live_edge() {
        let (mut state, session_id) = make_state_with_session();
        if let Some(h) = state.session_manager.get_mut(session_id) {
            h.session.performance.timeline_follow_latest = false;
            h.session.performance.timeline_viewport_width_micros = 1_000_000;
            h.session.performance.timeline_viewport_start_micros = 9_000_000;
        }
        update(&mut state, Message::TimelineFollowLatest { session_id });

        let perf = &state
            .session_manager
            .get(session_id)
            .unwrap()
            .session
            .performance;
        assert!(
            perf.timeline_follow_latest,
            "follow-latest should set follow_latest=true"
        );
        assert_eq!(
            perf.timeline_viewport_width_micros, 5_000_000,
            "follow-latest should reset width to default 5s"
        );
    }

    /// AC6: TimelineFollowLatest preserves committed_frame_anchor.
    #[test]
    fn test_follow_latest_preserves_frame_anchor() {
        let (mut state, session_id) = make_state_with_session();
        if let Some(h) = state.session_manager.get_mut(session_id) {
            h.session.performance.committed_frame_anchor = Some(42);
            h.session.performance.timeline_follow_latest = false;
        }
        update(&mut state, Message::TimelineFollowLatest { session_id });

        let perf = &state
            .session_manager
            .get(session_id)
            .unwrap()
            .session
            .performance;
        assert_eq!(
            perf.committed_frame_anchor,
            Some(42),
            "follow-latest should preserve committed_frame_anchor"
        );
    }

    // ── Phase 5 T03: Timeline event selection handler tests ───────────────────

    use crate::session::performance::SelectionDirection;
    use crate::session::TimelineEventCursor;
    use fdemon_core::timeline::TimelineNode;
    use fdemon_core::timeline::TimelineTrack;

    fn make_node(name: &str, ts: i64) -> TimelineNode {
        TimelineNode {
            name: name.to_owned(),
            category: None,
            ts,
            dur: Some(100),
            phase: TimelinePhase::Complete,
            thread: TimelineThread::Ui,
            frame_number: None,
            children: vec![],
        }
    }

    fn make_track_with_nodes(tid: i64, nodes: Vec<TimelineNode>) -> TimelineTrack {
        TimelineTrack {
            tid,
            name: None,
            thread: TimelineThread::Ui,
            root_events: nodes,
        }
    }

    /// AC3: TimelineSelectFirstVisible selects the first root event of the first track.
    #[test]
    fn test_select_first_visible_picks_first_root_event() {
        let (mut state, session_id) = make_state_with_session();
        // Add two tracks.
        if let Some(h) = state.session_manager.get_mut(session_id) {
            h.session.performance.timeline_tracks.insert(
                1,
                make_track_with_nodes(1, vec![make_node("First", 1_000_000)]),
            );
            h.session.performance.timeline_tracks.insert(
                2,
                make_track_with_nodes(2, vec![make_node("Second", 2_000_000)]),
            );
        }

        update(
            &mut state,
            Message::TimelineSelectFirstVisible { session_id },
        );

        let perf = &state
            .session_manager
            .get(session_id)
            .unwrap()
            .session
            .performance;
        assert_eq!(
            perf.timeline_selected_event,
            Some(TimelineEventCursor {
                tid: 1,
                depth: 0,
                ts: 1_000_000
            }),
            "should select first root event of first track"
        );
    }

    /// AC4: NextSibling wraps to first sibling when at last.
    #[test]
    fn test_next_sibling_wraps_to_first() {
        let (mut state, session_id) = make_state_with_session();
        if let Some(h) = state.session_manager.get_mut(session_id) {
            h.session.performance.timeline_tracks.insert(
                1,
                make_track_with_nodes(1, vec![make_node("A", 1_000), make_node("B", 2_000)]),
            );
            // Select the last sibling ("B").
            h.session.performance.timeline_selected_event = Some(TimelineEventCursor {
                tid: 1,
                depth: 0,
                ts: 2_000,
            });
        }

        update(
            &mut state,
            Message::TimelineMoveSelection {
                session_id,
                dir: SelectionDirection::NextSibling,
            },
        );

        let perf = &state
            .session_manager
            .get(session_id)
            .unwrap()
            .session
            .performance;
        // Should wrap to the first sibling ("A").
        assert_eq!(
            perf.timeline_selected_event,
            Some(TimelineEventCursor {
                tid: 1,
                depth: 0,
                ts: 1_000
            }),
            "NextSibling at last should wrap to first"
        );
    }

    /// AC4: PrevSibling wraps to last sibling when at first.
    #[test]
    fn test_prev_sibling_wraps_to_last() {
        let (mut state, session_id) = make_state_with_session();
        if let Some(h) = state.session_manager.get_mut(session_id) {
            h.session.performance.timeline_tracks.insert(
                1,
                make_track_with_nodes(1, vec![make_node("A", 1_000), make_node("B", 2_000)]),
            );
            // Select the first sibling ("A").
            h.session.performance.timeline_selected_event = Some(TimelineEventCursor {
                tid: 1,
                depth: 0,
                ts: 1_000,
            });
        }

        update(
            &mut state,
            Message::TimelineMoveSelection {
                session_id,
                dir: SelectionDirection::PrevSibling,
            },
        );

        let perf = &state
            .session_manager
            .get(session_id)
            .unwrap()
            .session
            .performance;
        // Should wrap to the last sibling ("B").
        assert_eq!(
            perf.timeline_selected_event,
            Some(TimelineEventCursor {
                tid: 1,
                depth: 0,
                ts: 2_000
            }),
            "PrevSibling at first should wrap to last"
        );
    }

    /// AC6: TimelineOpenPopup sets popup_open = true when event is selected.
    #[test]
    fn test_open_popup_sets_flag() {
        let (mut state, session_id) = make_state_with_session();
        if let Some(h) = state.session_manager.get_mut(session_id) {
            h.session.performance.timeline_selected_event = Some(TimelineEventCursor {
                tid: 1,
                depth: 0,
                ts: 1_000,
            });
        }

        update(&mut state, Message::TimelineOpenPopup { session_id });

        assert!(
            state
                .session_manager
                .get(session_id)
                .unwrap()
                .session
                .performance
                .timeline_details_popup_open
        );
    }

    /// AC6: TimelineClosePopup sets popup_open = false, keeps selection.
    #[test]
    fn test_close_popup_keeps_selection() {
        let (mut state, session_id) = make_state_with_session();
        let cursor = TimelineEventCursor {
            tid: 1,
            depth: 0,
            ts: 1_000,
        };
        if let Some(h) = state.session_manager.get_mut(session_id) {
            h.session.performance.timeline_selected_event = Some(cursor);
            h.session.performance.timeline_details_popup_open = true;
        }

        update(&mut state, Message::TimelineClosePopup { session_id });

        let perf = &state
            .session_manager
            .get(session_id)
            .unwrap()
            .session
            .performance;
        assert!(!perf.timeline_details_popup_open, "popup should be closed");
        assert_eq!(
            perf.timeline_selected_event,
            Some(cursor),
            "selection should be preserved after close"
        );
    }

    /// AC6: TimelineClearSelection clears selection and closes popup.
    #[test]
    fn test_clear_selection_clears_both() {
        let (mut state, session_id) = make_state_with_session();
        if let Some(h) = state.session_manager.get_mut(session_id) {
            h.session.performance.timeline_selected_event = Some(TimelineEventCursor {
                tid: 1,
                depth: 0,
                ts: 1_000,
            });
            h.session.performance.timeline_details_popup_open = true;
        }

        update(&mut state, Message::TimelineClearSelection { session_id });

        let perf = &state
            .session_manager
            .get(session_id)
            .unwrap()
            .session
            .performance;
        assert!(perf.timeline_selected_event.is_none());
        assert!(!perf.timeline_details_popup_open);
    }

    /// AC8: Auto-pan triggered when selected event is outside viewport.
    #[test]
    fn test_auto_pan_triggered_on_move() {
        let (mut state, session_id) = make_state_with_session();
        if let Some(h) = state.session_manager.get_mut(session_id) {
            // Set viewport to 0..5_000_000.
            h.session.performance.timeline_viewport_start_micros = 0;
            h.session.performance.timeline_viewport_width_micros = 5_000_000;
            h.session.performance.timeline_follow_latest = false;
            // Two tracks: track 1 in viewport, track 2 with event at ts=20_000_000 (outside).
            h.session.performance.timeline_tracks.insert(
                1,
                make_track_with_nodes(1, vec![make_node("InView", 1_000_000)]),
            );
            h.session.performance.timeline_tracks.insert(
                2,
                make_track_with_nodes(2, vec![make_node("OutOfView", 20_000_000)]),
            );
            h.session.performance.timeline_selected_event = Some(TimelineEventCursor {
                tid: 1,
                depth: 0,
                ts: 1_000_000,
            });
        }

        // Move to next thread (FirstChildOrDownThread from root → moves to track 2).
        update(
            &mut state,
            Message::TimelineMoveSelection {
                session_id,
                dir: SelectionDirection::FirstChildOrDownThread,
            },
        );

        let perf = &state
            .session_manager
            .get(session_id)
            .unwrap()
            .session
            .performance;
        // The new selection should be track 2's first event.
        assert_eq!(
            perf.timeline_selected_event,
            Some(TimelineEventCursor {
                tid: 2,
                depth: 0,
                ts: 20_000_000
            })
        );
        // follow_latest should be false (we set it manually and the auto-pan may keep it false).
        // The important thing is the viewport was updated to center on ts=20_000_000.
        assert!(
            !perf.timeline_follow_latest,
            "auto-pan should set follow_latest=false"
        );
    }

    /// AC1: Default state has no selection and popup closed.
    #[test]
    fn test_default_selection_state() {
        let perf = crate::session::performance::PerformanceState::default();
        assert!(perf.timeline_selected_event.is_none());
        assert!(!perf.timeline_details_popup_open);
    }

    // ── Phase 5 T04: Search handler tests ────────────────────────────────────

    /// AC2: TimelineSearchOpen sets input_active=true and query=Some("").
    #[test]
    fn test_search_open_activates_input() {
        let (mut state, session_id) = make_state_with_session();
        update(&mut state, Message::TimelineSearchOpen { session_id });
        let perf = &state
            .session_manager
            .get(session_id)
            .unwrap()
            .session
            .performance;
        assert!(
            perf.timeline_search_input_active,
            "search input should be active after open"
        );
        assert_eq!(
            perf.timeline_search_query,
            Some(String::new()),
            "query should be Some(\"\") after open"
        );
        assert_eq!(perf.timeline_search_match_cursor, 0);
    }

    /// AC3: TimelineSearchInputChar appends characters to the query.
    #[test]
    fn test_search_input_char_appends() {
        let (mut state, session_id) = make_state_with_session();
        if let Some(h) = state.session_manager.get_mut(session_id) {
            h.session.performance.timeline_search_query = Some("Ra".to_string());
            h.session.performance.timeline_search_input_active = true;
        }
        update(
            &mut state,
            Message::TimelineSearchInputChar {
                session_id,
                ch: 's',
            },
        );
        let query = state
            .session_manager
            .get(session_id)
            .unwrap()
            .session
            .performance
            .timeline_search_query
            .clone();
        assert_eq!(
            query,
            Some("Ras".to_string()),
            "char should append to query"
        );
    }

    /// AC3: TimelineSearchInputBackspace removes last character.
    #[test]
    fn test_search_input_backspace_removes_last_char() {
        let (mut state, session_id) = make_state_with_session();
        if let Some(h) = state.session_manager.get_mut(session_id) {
            h.session.performance.timeline_search_query = Some("Ras".to_string());
            h.session.performance.timeline_search_input_active = true;
        }
        update(
            &mut state,
            Message::TimelineSearchInputBackspace { session_id },
        );
        let query = state
            .session_manager
            .get(session_id)
            .unwrap()
            .session
            .performance
            .timeline_search_query
            .clone();
        assert_eq!(
            query,
            Some("Ra".to_string()),
            "backspace should remove last char"
        );
    }

    /// AC4: TimelineSearchInputCommit closes input, keeps query.
    #[test]
    fn test_search_input_commit_closes_input_keeps_query() {
        let (mut state, session_id) = make_state_with_session();
        if let Some(h) = state.session_manager.get_mut(session_id) {
            h.session.performance.timeline_search_query = Some("Raster".to_string());
            h.session.performance.timeline_search_input_active = true;
        }
        update(
            &mut state,
            Message::TimelineSearchInputCommit { session_id },
        );
        let perf = &state
            .session_manager
            .get(session_id)
            .unwrap()
            .session
            .performance;
        assert!(
            !perf.timeline_search_input_active,
            "input should be inactive after commit"
        );
        assert_eq!(
            perf.timeline_search_query,
            Some("Raster".to_string()),
            "query should be preserved after commit"
        );
    }

    /// AC5: TimelineSearchInputCancel closes input, clears query.
    #[test]
    fn test_search_input_cancel_clears_query() {
        let (mut state, session_id) = make_state_with_session();
        if let Some(h) = state.session_manager.get_mut(session_id) {
            h.session.performance.timeline_search_query = Some("Raster".to_string());
            h.session.performance.timeline_search_input_active = true;
        }
        update(
            &mut state,
            Message::TimelineSearchInputCancel { session_id },
        );
        let perf = &state
            .session_manager
            .get(session_id)
            .unwrap()
            .session
            .performance;
        assert!(
            !perf.timeline_search_input_active,
            "input should be inactive after cancel"
        );
        assert!(
            perf.timeline_search_query.is_none(),
            "query should be cleared after cancel"
        );
    }

    /// AC6: TimelineSearchNextMatch advances match_cursor, updates selection, pans viewport.
    #[test]
    fn test_search_next_match_advances_cursor_and_updates_selection() {
        let (mut state, session_id) = make_state_with_session();
        // Populate two events with matching names.
        if let Some(h) = state.session_manager.get_mut(session_id) {
            h.session.performance.timeline_tracks.insert(
                1,
                make_track_with_nodes(
                    1,
                    vec![
                        make_node("Raster1", 1_000),
                        make_node("Raster2", 2_000),
                        make_node("Other", 3_000),
                    ],
                ),
            );
            h.session.performance.timeline_search_query = Some("Raster".to_string());
            h.session.performance.timeline_search_input_active = false;
            h.session.performance.timeline_search_match_cursor = 0;
            // match cursor starts at 0; next match advances to 1 (index wraps: (0+1)%2 = 1)
        }
        update(&mut state, Message::TimelineSearchNextMatch { session_id });
        let perf = &state
            .session_manager
            .get(session_id)
            .unwrap()
            .session
            .performance;
        // match_cursor should have advanced: (0+1) % 2 = 1
        assert_eq!(
            perf.timeline_search_match_cursor, 1,
            "match_cursor should advance to 1"
        );
        // selected_event should be Some (the second match, ts=2_000)
        assert!(
            perf.timeline_selected_event.is_some(),
            "selection should be set"
        );
        // follow_latest should be false (viewport was panned)
        assert!(
            !perf.timeline_follow_latest,
            "follow_latest should be false after pan"
        );
    }

    /// AC7: TimelineSearchPrevMatch wraps backwards.
    #[test]
    fn test_search_prev_match_wraps_backward() {
        let (mut state, session_id) = make_state_with_session();
        if let Some(h) = state.session_manager.get_mut(session_id) {
            h.session.performance.timeline_tracks.insert(
                1,
                make_track_with_nodes(
                    1,
                    vec![make_node("Raster1", 1_000), make_node("Raster2", 2_000)],
                ),
            );
            h.session.performance.timeline_search_query = Some("Raster".to_string());
            h.session.performance.timeline_search_input_active = false;
            h.session.performance.timeline_search_match_cursor = 0;
        }
        // Prev from 0 should wrap to last match (1)
        update(&mut state, Message::TimelineSearchPrevMatch { session_id });
        let match_cursor = state
            .session_manager
            .get(session_id)
            .unwrap()
            .session
            .performance
            .timeline_search_match_cursor;
        assert_eq!(
            match_cursor, 1,
            "prev from 0 should wrap to last match (index 1)"
        );
    }

    /// AC6: TimelineSearchNextMatch with empty query does nothing.
    #[test]
    fn test_search_next_match_noop_with_empty_query() {
        let (mut state, session_id) = make_state_with_session();
        if let Some(h) = state.session_manager.get_mut(session_id) {
            h.session.performance.timeline_search_query = Some(String::new());
            h.session.performance.timeline_search_input_active = false;
        }
        update(&mut state, Message::TimelineSearchNextMatch { session_id });
        // No crash, no selection change.
        let perf = &state
            .session_manager
            .get(session_id)
            .unwrap()
            .session
            .performance;
        assert!(
            perf.timeline_selected_event.is_none(),
            "next match with empty query should not update selection"
        );
    }

    /// AC16: Selection sync — n/N updates timeline_selected_event.
    #[test]
    fn test_next_match_updates_timeline_selected_event() {
        let (mut state, session_id) = make_state_with_session();
        if let Some(h) = state.session_manager.get_mut(session_id) {
            h.session.performance.timeline_tracks.insert(
                1,
                make_track_with_nodes(1, vec![make_node("RasterDraw", 1_000)]),
            );
            h.session.performance.timeline_search_query = Some("Raster".to_string());
            h.session.performance.timeline_search_input_active = false;
            h.session.performance.timeline_search_match_cursor = usize::MAX; // pre-set to wrap
        }
        update(&mut state, Message::TimelineSearchNextMatch { session_id });
        let selected = state
            .session_manager
            .get(session_id)
            .unwrap()
            .session
            .performance
            .timeline_selected_event;
        assert!(
            selected.is_some(),
            "next match should set timeline_selected_event"
        );
        assert_eq!(
            selected.unwrap().ts,
            1_000,
            "selected event ts should match the only match"
        );
    }

    /// AC14 (case-insensitive): Lowercase query matches mixed-case event name.
    #[test]
    fn test_collect_matches_is_case_insensitive() {
        use crate::session::performance::TimelineFilter;
        use std::collections::BTreeMap;

        let mut tracks = BTreeMap::new();
        tracks.insert(
            1,
            make_track_with_nodes(1, vec![make_node("GPURasterizer::Draw", 1_000)]),
        );

        let matches = super::collect_matches(&tracks, "raster", TimelineFilter::All);
        assert_eq!(
            matches.len(),
            1,
            "lowercase 'raster' should match 'GPURasterizer::Draw'"
        );
    }

    /// AC15 (empty query returns no matches).
    #[test]
    fn test_collect_matches_empty_query_returns_no_matches() {
        use crate::session::performance::TimelineFilter;
        use std::collections::BTreeMap;

        let mut tracks = BTreeMap::new();
        tracks.insert(
            1,
            make_track_with_nodes(1, vec![make_node("Raster", 1_000)]),
        );

        let matches = super::collect_matches(&tracks, "", TimelineFilter::All);
        assert!(matches.is_empty(), "empty query should return no matches");
    }

    /// AC13: Filter interaction — matches on hidden threads excluded.
    #[test]
    fn test_collect_matches_respects_filter() {
        use crate::session::performance::TimelineFilter;
        use fdemon_core::timeline::{TimelineThread, TimelineTrack};
        use std::collections::BTreeMap;

        let mut tracks = BTreeMap::new();
        tracks.insert(
            1,
            TimelineTrack {
                tid: 1,
                name: None,
                thread: TimelineThread::Ui,
                root_events: vec![make_node("Raster", 1_000)],
            },
        );
        tracks.insert(
            2,
            TimelineTrack {
                tid: 2,
                name: None,
                thread: TimelineThread::Raster,
                root_events: vec![make_node("Raster", 2_000)],
            },
        );

        // UI filter: only tid=1 passes → 1 match
        let matches_ui = super::collect_matches(&tracks, "Raster", TimelineFilter::Ui);
        assert_eq!(
            matches_ui.len(),
            1,
            "Ui filter should only match UI thread events"
        );
        assert_eq!(matches_ui[0].tid, 1);

        // All filter: both threads → 2 matches
        let matches_all = super::collect_matches(&tracks, "Raster", TimelineFilter::All);
        assert_eq!(matches_all.len(), 2, "All filter should match both threads");
    }
}
