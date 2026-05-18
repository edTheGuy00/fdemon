//! Inspector-specific DevTools handlers.
//!
//! Handles widget tree fetch results and inspector navigation for the
//! Widget Inspector panel in DevTools mode.

use std::time::Instant;

use fdemon_core::RowGroup;

use crate::handler::{UpdateAction, UpdateResult};
use crate::message::InspectorNav;
use crate::session::SessionId;
use crate::state::{AppState, DetailsTab, DevToolsError, InspectorState};

use super::map_rpc_error;

/// Handle widget tree fetch completion.
///
/// Updates the inspector state with the fetched root node, auto-expands it,
/// and dispatches an initial layout fetch for the root node so the layout
/// panel shows data immediately without requiring a navigation event.
pub fn handle_widget_tree_fetched(
    state: &mut AppState,
    session_id: SessionId,
    root: Box<fdemon_core::DiagnosticsNode>,
) -> UpdateResult {
    // Only update if this is for the active session.
    let active_id = state.session_manager.selected().map(|h| h.session.id);

    if active_id == Some(session_id) {
        let root_node = *root;

        // Reset selection and expansion state so the user starts at the root.
        // Stale IDs from the previous tree are meaningless after a refresh.
        state.devtools_view_state.inspector.selected_index = 0;
        state.devtools_view_state.inspector.expanded.clear();

        // Auto-expand root node before storing.
        if let Some(ref value_id) = root_node.value_id {
            state
                .devtools_view_state
                .inspector
                .expanded
                .insert(value_id.clone());
        }

        state.devtools_view_state.inspector.root = Some(root_node);
        state.devtools_view_state.inspector.loading = false;
        state.devtools_view_state.inspector.error = None;
        state.devtools_view_state.inspector.has_object_group = true;
        // Sticky: once the tree has been rendered at least once, remember it
        // so that subsequent `r` presses can skip the readiness poll.
        state.devtools_view_state.inspector.has_ever_rendered_tree = true;

        // Clear stale layout data — value_ids from the previous tree are
        // meaningless after a refresh.
        state.devtools_view_state.inspector.layout = None;
        state.devtools_view_state.inspector.layout_loading = false;
        state.devtools_view_state.inspector.layout_error = None;
        state.devtools_view_state.inspector.last_fetched_node_id = None;
        state.devtools_view_state.inspector.pending_node_id = None;
        state.devtools_view_state.inspector.layout_last_fetch_time = None;

        // Clear Details and group expansion state — node ids from the previous
        // tree are invalid after a refresh; the Details panel must not render
        // against freed Dart object ids.
        state
            .devtools_view_state
            .inspector
            .reset_details_and_groups();

        // Auto-fetch layout for the initially selected node (root at index 0)
        // so the layout panel shows data immediately on Inspector entry.
        if let Some(node_id) = state.devtools_view_state.inspector.selected_value_id() {
            state.devtools_view_state.inspector.layout_loading = true;
            state.devtools_view_state.inspector.pending_node_id = Some(node_id.clone());
            state.devtools_view_state.inspector.layout_last_fetch_time = Some(Instant::now());
            return UpdateResult::action(UpdateAction::FetchLayoutData {
                session_id,
                node_id,
                vm_handle: None,
            });
        }
    }

    UpdateResult::none()
}

/// Handle widget tree fetch failure.
///
/// Maps the raw RPC error string to a user-friendly [`DevToolsError`] using
/// [`map_rpc_error`] so the TUI never displays a raw technical error.
///
/// Clears the fetch debounce timer so the user can press `r` again
/// immediately without waiting for the 2-second cooldown to expire.
pub fn handle_widget_tree_fetch_failed(
    state: &mut AppState,
    session_id: SessionId,
    error: String,
) -> UpdateResult {
    let active_id = state.session_manager.selected().map(|h| h.session.id);

    if active_id == Some(session_id) {
        state.devtools_view_state.inspector.loading = false;
        state.devtools_view_state.inspector.error = Some(map_rpc_error(&error));
        state.devtools_view_state.inspector.clear_fetch_debounce();
    }

    UpdateResult::none()
}

/// Handle inspector tree navigation (Up/Down/Expand/Collapse).
///
/// On Up/Down navigation: clears stale layout data immediately (so the UI
/// shows a loading state), then dispatches a `FetchLayoutData` action for the
/// newly selected node unless debounced or already fetched.
///
/// On Expand/Collapse: no layout fetch is triggered (selection does not change).
pub fn handle_inspector_navigate(state: &mut AppState, nav: InspectorNav) -> UpdateResult {
    // Phase 1: when Details is open, Up/Down/Left/Right are all no-ops in the
    // tree. The user must press Esc to return to tree mode first.
    // Left/Right are repurposed as cycle-tab by the key handler (task 06) and
    // must NOT reach this function when details_open == true — this guard is a
    // safety net for any path that bypasses the key handler.
    if state.devtools_view_state.inspector.details_open {
        return UpdateResult::none();
    }
    // Phase 2: read the visible node count and current selection, then handle navigation.
    // We scope the mutable borrow of `inspector` here so it ends before we access
    // `state.session_manager` below.
    // Expand / Collapse: use selected_row() to branch on RowGroup so that
    // chain leaders mutate `expanded_groups` and standalone nodes mutate
    // `expanded`.  These arms early-return (no layout fetch needed).
    if matches!(nav, InspectorNav::Expand | InspectorNav::Collapse) {
        let inspector = &mut state.devtools_view_state.inspector;
        let row = match inspector.selected_row() {
            Some(r) => r,
            None => return UpdateResult::none(),
        };
        // Capture the group and value_id before the immutable borrow on `row`
        // ends (row borrows from inspector).
        let group = row.group.clone();
        let value_id = row.node.value_id.clone();
        let has_children = !row.node.children.is_empty();

        match nav {
            InspectorNav::Expand => match group {
                RowGroup::LeaderCollapsed { .. } => {
                    if let Some(id) = value_id {
                        inspector.expanded_groups.insert(id);
                    }
                }
                _ => {
                    // Standard node: insert into `expanded`.
                    if has_children {
                        if let Some(id) = value_id {
                            if !inspector.is_expanded(&id) {
                                inspector.expanded.insert(id);
                            }
                        }
                    }
                }
            },
            InspectorNav::Collapse => match group {
                RowGroup::LeaderExpanded => {
                    if let Some(id) = value_id {
                        inspector.expanded_groups.remove(&id);
                    }
                }
                _ => {
                    if let Some(id) = value_id {
                        inspector.expanded.remove(&id);
                    }
                }
            },
            _ => unreachable!("arm limited to Expand | Collapse"),
        }
        return UpdateResult::none();
    }

    // Up / Down: use visible_nodes() for count and value_id extraction.
    let should_fetch_layout = {
        let inspector = &mut state.devtools_view_state.inspector;
        let visible = inspector.visible_nodes();
        let count = visible.len();

        if count == 0 {
            return UpdateResult::none();
        }

        let old_index = inspector.selected_index;

        match nav {
            InspectorNav::Up => {
                if inspector.selected_index > 0 {
                    inspector.selected_index -= 1;
                }
            }
            InspectorNav::Down => {
                if inspector.selected_index < count.saturating_sub(1) {
                    inspector.selected_index += 1;
                }
            }
            InspectorNav::Expand | InspectorNav::Collapse => {
                unreachable!("Expand/Collapse are handled above")
            }
        }

        let new_index = inspector.selected_index;
        let selection_changed = new_index != old_index;

        if selection_changed {
            // Clear stale layout data immediately — user sees loading state.
            inspector.layout = None;
            inspector.layout_error = None;
        }

        selection_changed
    };
    // `inspector` borrow has ended here — we can now access other fields.

    // Phase 2: auto-fetch layout for the newly selected node (Up/Down only).
    if should_fetch_layout {
        // Collect node_id while holding the borrow; borrow ends before session_manager access.
        let fetch_node_id = maybe_fetch_layout(&mut state.devtools_view_state.inspector);
        // `inspector` borrow has ended — we can now access session_manager.

        if let Some(node_id) = fetch_node_id {
            let session_id = state.session_manager.selected().map(|h| h.session.id);
            if let Some(session_id) = session_id {
                return UpdateResult::action(UpdateAction::FetchLayoutData {
                    session_id,
                    node_id,
                    vm_handle: None, // hydrated by process.rs
                });
            }
        }
    }

    UpdateResult::none()
}

/// If the currently-selected inspector node has a `value_id`, isn't debounced,
/// and isn't already cached as `last_fetched_node_id`, mark fetch state and
/// return the node id to fetch. Otherwise return `None`.
///
/// Mutates `inspector.layout_loading`, `pending_node_id`, and
/// `layout_last_fetch_time` only on the success path.
fn maybe_fetch_layout(inspector: &mut InspectorState) -> Option<String> {
    if inspector.is_layout_fetch_debounced() {
        return None;
    }
    let node_id = inspector.selected_value_id()?;
    if inspector.last_fetched_node_id.as_deref() == Some(node_id.as_str()) {
        return None;
    }
    inspector.layout_loading = true;
    inspector.pending_node_id = Some(node_id.clone());
    inspector.layout_last_fetch_time = Some(std::time::Instant::now());
    Some(node_id)
}

/// Handle widget tree fetch timeout.
///
/// Sets `inspector.loading = false` and stores an error message with a retry
/// hint, then marks `connection_status` as `TimedOut` so the tab bar can
/// indicate the degraded state.
///
/// Clears the fetch debounce timer so the user can press `r` again
/// immediately without waiting for the 2-second cooldown to expire.
pub fn handle_widget_tree_fetch_timeout(
    state: &mut AppState,
    session_id: SessionId,
) -> UpdateResult {
    use crate::state::VmConnectionStatus;

    let active_id = state.session_manager.selected().map(|h| h.session.id);

    if active_id == Some(session_id) {
        state.devtools_view_state.inspector.loading = false;
        state.devtools_view_state.inspector.error = Some(DevToolsError::new(
            "Request timed out",
            "Press [r] to retry",
        ));
        state.devtools_view_state.inspector.clear_fetch_debounce();
        state.devtools_view_state.connection_status = VmConnectionStatus::TimedOut;
    }

    UpdateResult::none()
}

// ─────────────────────────────────────────────────────────────────────────────
// Layout data handlers (merged from layout.rs)
// ─────────────────────────────────────────────────────────────────────────────

/// Handle layout data fetch completion.
///
/// Updates the inspector state's layout fields with the fetched layout info.
/// Discards stale responses using `details_node_id` as the authoritative
/// comparison key (Phase 2 follow-up M2). This is a unified stale guard:
/// an in-flight layout response is accepted only if the Details panel is
/// still open on the same node that was fetched. Tree navigation that moves
/// the selection without closing Details does not discard the response.
pub fn handle_layout_data_fetched(
    state: &mut AppState,
    session_id: SessionId,
    node_id: String,
    layout: fdemon_core::LayoutInfo,
) -> UpdateResult {
    let active_id = state.session_manager.selected().map(|h| h.session.id);

    if active_id == Some(session_id) {
        let inspector = &mut state.devtools_view_state.inspector;

        // Stale-guard: only apply if the response matches the currently-displayed
        // details panel node. Discards orphan responses from closed-then-reopened-
        // on-different-node races (Phase 2 follow-up M2).
        if inspector.details_node_id.as_deref() != Some(node_id.as_str()) {
            // Clear the pending flag if it still points to this stale node so
            // that a subsequent open on the correct node will dispatch a new fetch.
            if inspector.pending_node_id.as_deref() == Some(node_id.as_str()) {
                inspector.pending_node_id = None;
                inspector.layout_loading = false;
            }
            return UpdateResult::none();
        }

        inspector.layout = Some(layout);
        inspector.layout_loading = false;
        inspector.layout_error = None;
        inspector.has_layout_object_group = true;
        // Promote pending node ID to last_fetched so repeated panel switches
        // for the same node skip redundant fetches.
        inspector.last_fetched_node_id = inspector.pending_node_id.take();
    }

    UpdateResult::none()
}

/// Handle layout data fetch failure.
///
/// Maps the raw RPC error string to a user-friendly [`DevToolsError`] using
/// [`map_rpc_error`] so the TUI never displays a raw technical error.
pub fn handle_layout_data_fetch_failed(
    state: &mut AppState,
    session_id: SessionId,
    error: String,
) -> UpdateResult {
    let active_id = state.session_manager.selected().map(|h| h.session.id);

    if active_id == Some(session_id) {
        state.devtools_view_state.inspector.layout_loading = false;
        state.devtools_view_state.inspector.layout_error = Some(map_rpc_error(&error));
        // Clear pending node ID so a subsequent switch will retry the fetch.
        state.devtools_view_state.inspector.pending_node_id = None;
    }

    UpdateResult::none()
}

/// Handle layout data fetch timeout.
///
/// Sets `inspector.layout_loading = false` and stores an error message with a
/// retry hint, then marks `connection_status` as `TimedOut`.
pub fn handle_layout_data_fetch_timeout(
    state: &mut AppState,
    session_id: SessionId,
) -> UpdateResult {
    use crate::state::VmConnectionStatus;

    let active_id = state.session_manager.selected().map(|h| h.session.id);

    if active_id == Some(session_id) {
        state.devtools_view_state.inspector.layout_loading = false;
        state.devtools_view_state.inspector.layout_error = Some(DevToolsError::new(
            "Request timed out",
            "Press [r] to retry",
        ));
        state.devtools_view_state.connection_status = VmConnectionStatus::TimedOut;
        // Clear pending node ID so a subsequent switch will retry the fetch.
        state.devtools_view_state.inspector.pending_node_id = None;
    }

    UpdateResult::none()
}

// ── Inspector Properties Handlers (Phase 2, Task 06) ─────────────────────────

/// Apply a successful `getProperties` response to [`InspectorState`].
///
/// Stale-guarded: the response is accepted only if `details_node_id` still
/// matches `node_id` — i.e. the Details panel is still open on the same
/// widget that was fetched (Phase 2 follow-up C2 / M2).  This catches the
/// close-then-reopen-on-different-node race: even if `pending_properties_node_id`
/// still points to the old node, a response arriving after the user has
/// navigated to a different node is discarded.
///
/// When the response is discarded and `pending_properties_node_id` still
/// points to the stale node, both the pending id and the loading flag are
/// cleared so the user can fetch fresh data for the currently-displayed node.
pub fn handle_inspector_properties_fetched(
    state: &mut AppState,
    session_id: SessionId,
    node_id: String,
    widget_properties: Vec<fdemon_core::DiagnosticsNode>,
    render_properties: Vec<fdemon_core::DiagnosticsNode>,
) -> UpdateResult {
    let active_id = state.session_manager.selected().map(|h| h.session.id);
    if active_id != Some(session_id) {
        return UpdateResult::none();
    }

    let inspector = &mut state.devtools_view_state.inspector;

    // Stale-guard: only apply if the response matches the currently-displayed
    // details panel. Discards orphan responses from closed-then-reopened-on-
    // different-node races (Phase 2 follow-up C2).
    if inspector.details_node_id.as_deref() != Some(node_id.as_str()) {
        // Clear the pending flag if it still points to this stale node so
        // that a subsequent open on the correct node will dispatch a new fetch.
        if inspector.pending_properties_node_id.as_deref() == Some(node_id.as_str()) {
            inspector.pending_properties_node_id = None;
            inspector.properties_loading = false;
        }
        return UpdateResult::none();
    }

    inspector.properties = widget_properties;
    inspector.render_properties = render_properties;
    inspector.properties_loading = false;
    inspector.properties_error = None;
    // Promote pending to last-fetched so a subsequent open on the same node
    // skips the fetch (cache hit).
    inspector.last_fetched_properties_node_id = inspector.pending_properties_node_id.take();

    // Phase 3: the fetch may have changed which tabs are visible.
    // If the active tab is now hidden, snap to Properties.
    inspector.clamp_details_tab();

    UpdateResult::none()
}

/// Handle a `getProperties` RPC failure.
///
/// Maps the raw RPC error string to a user-friendly [`DevToolsError`] and
/// clears the loading state. The `last_fetched_properties_node_id` cache is
/// deliberately **not** updated on failure so that the next `Enter` on the same
/// node retries the fetch.
///
/// Stale-guarded: only applies the error if `details_node_id` still matches
/// `node_id` (Phase 2 follow-up C2 / M2). When discarding, the pending id
/// and loading flag are cleared if they still point to the stale node.
pub fn handle_inspector_properties_fetch_failed(
    state: &mut AppState,
    session_id: SessionId,
    node_id: String,
    error: String,
) -> UpdateResult {
    let active_id = state.session_manager.selected().map(|h| h.session.id);
    if active_id != Some(session_id) {
        return UpdateResult::none();
    }

    let inspector = &mut state.devtools_view_state.inspector;

    // Stale-guard: only apply if the response matches the currently-displayed
    // details panel (unified key — same as properties_fetched and layout_fetched).
    if inspector.details_node_id.as_deref() != Some(node_id.as_str()) {
        if inspector.pending_properties_node_id.as_deref() == Some(node_id.as_str()) {
            inspector.pending_properties_node_id = None;
            inspector.properties_loading = false;
        }
        return UpdateResult::none();
    }

    inspector.properties_loading = false;
    inspector.properties_error = Some(map_rpc_error(&error));
    inspector.pending_properties_node_id = None;
    // last_fetched_properties_node_id deliberately not updated; cache stays
    // empty so the next Enter retries.

    // Phase 3: failure may leave render_properties empty; clamp if Render
    // Object was the active tab.
    inspector.clamp_details_tab();

    UpdateResult::none()
}

/// Handle a `getProperties` RPC timeout.
///
/// Sets a timeout error on `properties_error` and clears the loading state.
/// Stale-guarded identically to [`handle_inspector_properties_fetch_failed`]:
/// only applies if `details_node_id` still matches `node_id`.
pub fn handle_inspector_properties_fetch_timeout(
    state: &mut AppState,
    session_id: SessionId,
    node_id: String,
) -> UpdateResult {
    let active_id = state.session_manager.selected().map(|h| h.session.id);
    if active_id != Some(session_id) {
        return UpdateResult::none();
    }

    let inspector = &mut state.devtools_view_state.inspector;

    // Stale-guard: only apply if the response matches the currently-displayed
    // details panel (unified key — same as properties_fetched and layout_fetched).
    if inspector.details_node_id.as_deref() != Some(node_id.as_str()) {
        if inspector.pending_properties_node_id.as_deref() == Some(node_id.as_str()) {
            inspector.pending_properties_node_id = None;
            inspector.properties_loading = false;
        }
        return UpdateResult::none();
    }

    inspector.properties_loading = false;
    inspector.properties_error = Some(DevToolsError::new(
        "Request timed out",
        "Press [r] to retry",
    ));
    inspector.pending_properties_node_id = None;

    UpdateResult::none()
}

// ── Mouse Click Handlers (Phase 4) ───────────────────────────────────────────

/// Select a visible inspector node by absolute row index.
///
/// Mirrors the `InspectorNav::Up`/`Down` semantics: sets `selected_index`,
/// clears stale layout data, and dispatches a `FetchLayoutData` action for the
/// newly selected node (gated by the same debounce / cache-hit rules as
/// keyboard navigation).
///
/// Out-of-range clicks (e.g. the tree shrank between render and click) are
/// silently ignored — no action is emitted.
pub fn handle_inspector_select_row(state: &mut AppState, index: usize) -> UpdateResult {
    // When Details is open, selection is frozen — clicks do not change the
    // selected row. The user must press Esc to close Details first.
    if state.devtools_view_state.inspector.details_open {
        return UpdateResult::none();
    }
    // Phase 1: bounds-check and update selection.
    // Scope the mutable borrow of `inspector` so it ends before we access
    // `state.session_manager` below.
    let selection_changed = {
        let inspector = &mut state.devtools_view_state.inspector;
        let visible = inspector.visible_nodes();
        let count = visible.len();

        if count == 0 || index >= count {
            // Click on a row that no longer exists (tree shrunk between
            // render and click). Silent no-op.
            return UpdateResult::none();
        }

        let old_index = inspector.selected_index;
        inspector.selected_index = index;
        let changed = old_index != index;

        if changed {
            // Clear stale layout immediately so the layout panel shows
            // a loading state — same as InspectorNav::Up/Down.
            inspector.layout = None;
            inspector.layout_error = None;
        }

        changed
    };
    // `inspector` borrow has ended here.

    if !selection_changed {
        // Click on already-selected row → no fetch (cache hit / no-op).
        return UpdateResult::none();
    }

    // Phase 2: dispatch layout fetch.
    // Collect node_id while holding the borrow; borrow ends before session_manager access.
    let fetch_node_id = maybe_fetch_layout(&mut state.devtools_view_state.inspector);
    // `inspector` borrow has ended — we can now access session_manager.

    if let Some(node_id) = fetch_node_id {
        if let Some(session_id) = state.session_manager.selected().map(|h| h.session.id) {
            return UpdateResult::action(UpdateAction::FetchLayoutData {
                session_id,
                node_id,
                vm_handle: None,
            });
        }
    }

    UpdateResult::none()
}

/// Toggle the expanded/collapsed state of an inspector node by row index.
///
/// First selects the row (same semantics as [`handle_inspector_select_row`],
/// including the layout fetch dispatch). Then, if the node has children and a
/// `value_id`, toggles its entry in `inspector.expanded`.
///
/// Clicking on a leaf node (no children) or a node without a `value_id` is a
/// no-op for the expanded set; selection still changes normally.
pub fn handle_inspector_toggle_node(state: &mut AppState, index: usize) -> UpdateResult {
    // Step 1: capture value_id, has_children, and group before delegating, so
    // that inspector_rows() is only traversed once (the delegate also calls it
    // internally).
    let (value_id, has_children, group) = {
        let inspector = &state.devtools_view_state.inspector;
        let rows = inspector.inspector_rows();
        if index >= rows.len() {
            // Out-of-range click — no selection change, no expanded-set mutation.
            return UpdateResult::none();
        }
        match rows.get(index) {
            Some(row) => (
                row.node.value_id.clone(),
                !row.node.children.is_empty(),
                row.group.clone(),
            ),
            None => return UpdateResult::none(),
        }
    };
    // `inspector` immutable borrow has ended.

    // Step 2: select the row (mirrors InspectorNav::Up/Down semantics —
    // clears stale layout, dispatches fetch under debounce rules).
    let select_result = handle_inspector_select_row(state, index);

    // Step 3: toggle the node's expanded state, branching on RowGroup so that
    // chain leader glyph clicks mutate `expanded_groups` and standalone nodes
    // mutate `expanded`.
    if let Some(id) = value_id {
        let inspector = &mut state.devtools_view_state.inspector;
        match group {
            RowGroup::LeaderCollapsed { .. } => {
                // Expand the chain.
                inspector.expanded_groups.insert(id);
            }
            RowGroup::LeaderExpanded => {
                // Collapse the chain.
                inspector.expanded_groups.remove(&id);
            }
            _ => {
                // Standard node: toggle `expanded` (guarded by has_children).
                if has_children {
                    if inspector.is_expanded(&id) {
                        inspector.expanded.remove(&id);
                    } else {
                        inspector.expanded.insert(id);
                    }
                }
            }
        }
    }

    select_result
}

// ── Details Panel Handlers (Phase 1, Task 05) ────────────────────────────────

/// Open the Details panel for the currently selected inspector node.
///
/// Sets `details_open = true`, snaps `details_tab` back to `Properties`, and
/// records `details_node_id` so the TUI can render the correct content.
///
/// Dispatches up to two fetch actions in one update cycle:
/// - **`FetchInspectorProperties`** — when the properties cache misses or
///   there is a prior error on the same node and no fetch is already in flight.
/// - **`FetchLayoutData`** — when layout cache misses and no fetch is in flight
///   (existing Phase 1 behaviour).
///
/// Multiple actions are returned via [`UpdateResult::actions_vec`].
///
/// No-op when:
/// - Details is already open.
/// - No tree row is currently selected (empty tree or out-of-range index).
pub fn handle_open_details(state: &mut AppState) -> UpdateResult {
    let inspector = &mut state.devtools_view_state.inspector;
    if inspector.details_open {
        return UpdateResult::none();
    }
    let Some(node_id) = inspector.selected_value_id() else {
        return UpdateResult::none(); // no selection, nothing to open
    };
    inspector.details_tab = DetailsTab::Properties; // always start on first tab
    inspector.details_node_id = Some(node_id.clone());

    // Phase 3: precompute tree-derived visibility predicates for the open session.
    //
    // Walks the tree once; cached on `inspector.details_context` and consumed by
    // `visible_tabs()` for the duration of the details session. Cleared by
    // `reset_details_and_groups()` and overwritten by the next open.
    if let Some(root) = inspector.root.as_ref() {
        inspector.details_context =
            fdemon_core::widget_tree::compute_details_context(root, &node_id);
    } else {
        // Root absent (shouldn't happen if a node is selected, but defensive):
        // default context means only the Properties tab will render until a
        // future open lands.
        inspector.details_context = fdemon_core::widget_tree::DetailsContext::default();
    }

    inspector.details_open = true;

    let session_id = match state.session_manager.selected().map(|h| h.session.id) {
        Some(id) => id,
        None => return UpdateResult::none(),
    };

    let mut actions: Vec<UpdateAction> = Vec::new();

    // (A) Properties fetch — skip when cached for this exact node and no prior
    //     error, or when a fetch is already in-flight.
    {
        let inspector = &mut state.devtools_view_state.inspector;
        let need_properties = inspector.last_fetched_properties_node_id.as_deref()
            != Some(node_id.as_str())
            || inspector.properties_error.is_some();
        if need_properties && !inspector.properties_loading {
            inspector.properties_loading = true;
            inspector.properties_error = None;
            inspector.pending_properties_node_id = Some(node_id.clone());
            actions.push(UpdateAction::FetchInspectorProperties {
                session_id,
                node_id: node_id.clone(),
                vm_handle: None,
            });
        }
    }

    // (B) Layout fetch — existing logic. Skip when already cached or in-flight.
    {
        let inspector = &mut state.devtools_view_state.inspector;
        let need_layout = inspector.last_fetched_node_id.as_deref() != Some(node_id.as_str())
            && !inspector.layout_loading;
        if need_layout {
            inspector.layout_loading = true;
            inspector.pending_node_id = Some(node_id.clone());
            inspector.layout_last_fetch_time = Some(Instant::now());
            actions.push(UpdateAction::FetchLayoutData {
                session_id,
                node_id,
                vm_handle: None,
            });
        }
    }

    UpdateResult::actions_vec(actions)
}

/// Close the Details panel.
///
/// Sets `details_open = false` and clears `details_node_id`. The `details_tab`
/// is left at its current value; [`handle_open_details`] always resets it to
/// `Properties` on the next open, so the value stored here is not observed by
/// the user.
///
/// No-op when Details is already closed.
pub fn handle_close_details(state: &mut AppState) -> UpdateResult {
    let inspector = &mut state.devtools_view_state.inspector;
    if !inspector.details_open {
        return UpdateResult::none();
    }
    inspector.details_open = false;
    inspector.details_node_id = None;
    UpdateResult::none()
}

/// Cycle the active Details tab forward or backward.
///
/// Wraps around at both ends within the set of currently-visible tabs
/// (as returned by [`InspectorState::visible_tabs`]). No-op when Details is
/// not open. When only one tab is visible, forward and backward both leave the
/// tab unchanged.
pub fn handle_cycle_tab(state: &mut AppState, forward: bool) -> UpdateResult {
    let inspector = &mut state.devtools_view_state.inspector;
    if !inspector.details_open {
        return UpdateResult::none();
    }

    let visible = inspector.visible_tabs();
    if visible.is_empty() {
        // Defensive: visible_tabs always returns at least [Properties].
        return UpdateResult::none();
    }

    // Find current tab in visible list. If somehow not present (e.g. clamp
    // was missed), fall back to first visible tab.
    let current_idx = visible.iter().position(|t| *t == inspector.details_tab);

    inspector.details_tab = match current_idx {
        Some(idx) => {
            let next_idx = if forward {
                (idx + 1) % visible.len()
            } else {
                (idx + visible.len() - 1) % visible.len()
            };
            visible[next_idx]
        }
        None => visible[0],
    };

    UpdateResult::none()
}

/// Toggle the `hide_implementation_widgets` flag.
///
/// Flips the flag on the inspector state, clamps `selected_index` to a valid
/// row if the visible row count has shrunk (folding can hide rows), and
/// mirrors the new value back to `state.settings.devtools` so the setting
/// survives a DevTools-mode switch.
///
/// Returns [`UpdateAction::PersistSettings`] so the settings are written to
/// `.fdemon/config.toml` on a background tokio task, keeping the TEA event
/// loop unblocked.  Persistence failures are handled by the action dispatch
/// arm (logged at `warn!` level) and do not block the toggle.
pub fn handle_toggle_hide_implementation(state: &mut AppState) -> UpdateResult {
    state
        .devtools_view_state
        .inspector
        .hide_implementation_widgets = !state
        .devtools_view_state
        .inspector
        .hide_implementation_widgets;

    // Clamp selected_index — row count may have shrunk if folding turned on.
    let row_count = state.devtools_view_state.inspector.inspector_rows().len();
    if row_count > 0 && state.devtools_view_state.inspector.selected_index >= row_count {
        state.devtools_view_state.inspector.selected_index = row_count - 1;
    }

    // Mirror back to Settings so the value survives re-opening DevTools.
    state.settings.devtools.hide_implementation_widgets = state
        .devtools_view_state
        .inspector
        .hide_implementation_widgets;

    // Persist asynchronously — file I/O runs on a background tokio task so the
    // TUI event loop is never stalled by disk writes.
    UpdateResult::action(UpdateAction::PersistSettings {
        settings: state.settings.clone(),
        project_path: state.project_path.clone(),
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_state() -> AppState {
        AppState::new()
    }

    fn make_state_with_session() -> AppState {
        let mut state = AppState::new();
        let device = fdemon_daemon::Device {
            id: "test-device".to_string(),
            name: "Test Device".to_string(),
            platform: "android".to_string(),
            emulator: false,
            category: None,
            platform_type: None,
            ephemeral: false,
            emulator_id: None,
        };
        state.session_manager.create_session(&device).unwrap();
        state
    }

    fn make_node(description: &str) -> fdemon_core::DiagnosticsNode {
        serde_json::from_value(serde_json::json!({
            "description": description
        }))
        .expect("valid DiagnosticsNode")
    }

    // ── widget tree ───────────────────────────────────────────────────────────

    #[test]
    fn test_handle_widget_tree_fetched_with_no_active_session_is_noop() {
        let mut state = make_state();
        let node = make_node("MaterialApp");

        // session_id 999 does not match any active session.
        handle_widget_tree_fetched(&mut state, 999, Box::new(node));

        assert!(state.devtools_view_state.inspector.root.is_none());
    }

    #[test]
    fn test_handle_widget_tree_fetch_failed_no_active_session_is_noop() {
        let mut state = make_state();
        state.devtools_view_state.inspector.loading = true;

        // session_id 999 does not match any active session.
        handle_widget_tree_fetch_failed(&mut state, 999, "error".to_string());

        // Should not update state when session_id doesn't match.
        assert!(state.devtools_view_state.inspector.loading);
    }

    // ── inspector navigation ──────────────────────────────────────────────────

    #[test]
    fn test_handle_inspector_navigate_no_op_when_tree_empty() {
        let mut state = make_state();
        // No root → visible_nodes() returns empty.
        let result = handle_inspector_navigate(&mut state, InspectorNav::Down);
        assert!(result.action.is_none());
        assert_eq!(state.devtools_view_state.inspector.selected_index, 0);
    }

    // ── Performance Polish: Tree Refresh Cooldown (Phase 5, Task 04) ──────────

    #[test]
    fn test_tree_refresh_debounce_while_loading() {
        let mut state = make_state();
        state.devtools_view_state.inspector.loading = true;

        // is_fetch_debounced() returns true when loading
        assert!(
            state.devtools_view_state.inspector.is_fetch_debounced(),
            "Debounce should be active while loading"
        );
    }

    #[test]
    fn test_tree_refresh_debounce_cooldown() {
        let mut state = make_state();
        state.devtools_view_state.inspector.loading = false;
        state.devtools_view_state.inspector.last_fetch_time = Some(std::time::Instant::now());

        assert!(
            state.devtools_view_state.inspector.is_fetch_debounced(),
            "Debounce should be active within 2-second cooldown"
        );
    }

    #[test]
    fn test_tree_refresh_allowed_when_no_fetch_time() {
        let state = make_state();

        assert!(
            !state.devtools_view_state.inspector.is_fetch_debounced(),
            "Debounce should be inactive with no previous fetch"
        );
    }

    #[test]
    fn test_tree_refresh_allowed_after_cooldown() {
        let mut state = make_state();
        state.devtools_view_state.inspector.loading = false;
        // Set last_fetch_time to 3 seconds ago (past the 2-second cooldown).
        state.devtools_view_state.inspector.last_fetch_time = std::time::Instant::now()
            .checked_sub(std::time::Duration::from_secs(3))
            .or_else(|| Some(std::time::Instant::now()));

        assert!(
            !state.devtools_view_state.inspector.is_fetch_debounced(),
            "Debounce should be inactive after cooldown has elapsed"
        );
    }

    #[test]
    fn test_record_fetch_start_sets_loading_and_time() {
        let mut state = make_state();
        assert!(!state.devtools_view_state.inspector.loading);
        assert!(state
            .devtools_view_state
            .inspector
            .last_fetch_time
            .is_none());

        state.devtools_view_state.inspector.record_fetch_start();

        assert!(
            state.devtools_view_state.inspector.loading,
            "record_fetch_start should set loading = true"
        );
        assert!(
            state
                .devtools_view_state
                .inspector
                .last_fetch_time
                .is_some(),
            "record_fetch_start should set last_fetch_time"
        );
    }

    #[test]
    fn test_inspector_reset_clears_last_fetch_time() {
        let mut state = make_state();
        state.devtools_view_state.inspector.record_fetch_start();
        assert!(state
            .devtools_view_state
            .inspector
            .last_fetch_time
            .is_some());

        state.devtools_view_state.inspector.reset();

        assert!(
            state
                .devtools_view_state
                .inspector
                .last_fetch_time
                .is_none(),
            "reset() should clear last_fetch_time"
        );
        assert!(
            !state.devtools_view_state.inspector.is_fetch_debounced(),
            "Debounce should be inactive after reset"
        );
    }

    // ── Bug 2: Refresh resets selection ──────────────────────────────────────

    #[test]
    fn test_widget_tree_fetched_resets_selection_and_expanded() {
        let mut state = make_state_with_session();
        let session_id = state.session_manager.selected().unwrap().session.id;

        // Simulate stale state from a previous tree.
        state.devtools_view_state.inspector.selected_index = 15;
        state
            .devtools_view_state
            .inspector
            .expanded
            .insert("stale-id-1".to_string());
        state
            .devtools_view_state
            .inspector
            .expanded
            .insert("stale-id-2".to_string());

        let node: fdemon_core::DiagnosticsNode = serde_json::from_value(serde_json::json!({
            "description": "MyApp",
            "valueId": "new-root"
        }))
        .unwrap();

        handle_widget_tree_fetched(&mut state, session_id, Box::new(node));

        assert_eq!(
            state.devtools_view_state.inspector.selected_index, 0,
            "selected_index should be reset to 0 after refresh"
        );
        assert!(
            !state
                .devtools_view_state
                .inspector
                .expanded
                .contains("stale-id-1"),
            "Stale expanded IDs should be cleared"
        );
        assert!(
            state
                .devtools_view_state
                .inspector
                .expanded
                .contains("new-root"),
            "New root should be auto-expanded"
        );
        assert_eq!(
            state.devtools_view_state.inspector.expanded.len(),
            1,
            "Only the new root should be in expanded set"
        );
    }

    // ── Error integration ─────────────────────────────────────────────────────

    #[test]
    fn test_widget_tree_fetched_clears_error() {
        let mut state = make_state_with_session();
        let session_id = state.session_manager.selected().unwrap().session.id;

        // Pre-set an error.
        state.devtools_view_state.inspector.error = Some(DevToolsError::new("old error", "hint"));

        let node: fdemon_core::DiagnosticsNode = serde_json::from_value(serde_json::json!({
            "description": "MyApp"
        }))
        .unwrap();

        handle_widget_tree_fetched(&mut state, session_id, Box::new(node));

        assert!(
            state.devtools_view_state.inspector.error.is_none(),
            "error should be cleared after successful fetch"
        );
    }

    #[test]
    fn test_widget_tree_fetch_failed_stores_friendly_error() {
        let mut state = make_state_with_session();
        let session_id = state.session_manager.selected().unwrap().session.id;

        handle_widget_tree_fetch_failed(
            &mut state,
            session_id,
            "Method not found: ext.flutter.inspector.getRootWidgetTree".to_string(),
        );

        let error = state
            .devtools_view_state
            .inspector
            .error
            .as_ref()
            .expect("error should be set");
        assert_eq!(error.message, "Widget inspector not available in this mode");
        assert!(
            !state.devtools_view_state.inspector.loading,
            "loading should be false after failure"
        );
    }

    #[test]
    fn test_timeout_stores_friendly_error_inspector() {
        let mut state = make_state_with_session();
        let session_id = state.session_manager.selected().unwrap().session.id;

        handle_widget_tree_fetch_timeout(&mut state, session_id);

        let error = state
            .devtools_view_state
            .inspector
            .error
            .as_ref()
            .expect("error should be set after timeout");
        assert_eq!(error.message, "Request timed out");
        assert!(error.hint.contains("[r]"));
        assert!(!state.devtools_view_state.inspector.loading);
    }

    // ── Error classification (Phase 5, Task 03) ───────────────────────────────

    #[test]
    fn test_rpc_error_maps_extension_not_registered() {
        let error = map_rpc_error("Method not found: ext.flutter.inspector.getRootWidgetTree");
        assert_eq!(error.message, "Widget inspector not available in this mode");
        assert!(
            error.hint.contains("debug mode"),
            "Hint should mention debug mode, got: {:?}",
            error.hint
        );
    }

    #[test]
    fn test_rpc_error_maps_extension_not_registered_variant() {
        let error = map_rpc_error("extension not registered: ext.flutter.inspector");
        assert_eq!(error.message, "Widget inspector not available in this mode");
        assert!(error.hint.contains("debug mode"));
    }

    #[test]
    fn test_rpc_error_maps_isolate_not_found() {
        let error = map_rpc_error("Isolate not found: 123456");
        assert_eq!(error.message, "Flutter app isolate not found");
        assert!(error.hint.contains("[r]"), "Hint should include [r] key");
    }

    #[test]
    fn test_rpc_error_maps_timeout() {
        let error = map_rpc_error("Request timed out after 10 seconds");
        assert_eq!(error.message, "Request timed out");
        assert!(error.hint.contains("[r]"));
    }

    #[test]
    fn test_rpc_error_maps_connection_lost() {
        let error = map_rpc_error("WebSocket connection closed unexpectedly");
        assert_eq!(error.message, "VM Service connection lost");
        assert!(error.hint.contains("Reconnecting"));
    }

    #[test]
    fn test_rpc_error_maps_vm_handle_unavailable() {
        let error = map_rpc_error("VM Service handle unavailable");
        assert_eq!(error.message, "VM Service not available");
        assert!(error.hint.contains("debug mode"));
    }

    #[test]
    fn test_rpc_error_maps_object_group_expired() {
        let error = map_rpc_error("object group expired");
        assert_eq!(error.message, "Widget data expired");
        assert!(error.hint.contains("[r]"));
    }

    #[test]
    fn test_rpc_error_maps_parse_error() {
        let error = map_rpc_error("parse error: unexpected token at line 1");
        assert_eq!(error.message, "Unexpected response from Flutter");
        assert!(error.hint.contains("[r]"));
    }

    #[test]
    fn test_rpc_error_fallback_for_unknown_error() {
        let error = map_rpc_error("some completely unknown error xyz");
        assert_eq!(error.message, "DevTools request failed");
        assert!(error.hint.contains("[r]"));
    }

    // ── Layout data handlers ──────────────────────────────────────────────────

    #[test]
    fn test_layout_data_fetched_records_node_id() {
        let mut state = make_state_with_session();
        let session_id = state.session_manager.selected().unwrap().session.id;

        // Set up a tree so the selected node's value_id matches pending_node_id.
        let node: fdemon_core::DiagnosticsNode = serde_json::from_value(serde_json::json!({
            "description": "Widget",
            "valueId": "node-xyz"
        }))
        .expect("valid node");
        state.devtools_view_state.inspector.root = Some(node);
        state.devtools_view_state.inspector.selected_index = 0;

        // Simulate details open for "node-xyz" and a pending fetch for the same node.
        state.devtools_view_state.inspector.details_open = true;
        state.devtools_view_state.inspector.details_node_id = Some("node-xyz".to_string());
        state.devtools_view_state.inspector.pending_node_id = Some("node-xyz".to_string());
        state.devtools_view_state.inspector.layout_loading = true;

        let layout = fdemon_core::LayoutInfo::default();
        handle_layout_data_fetched(&mut state, session_id, "node-xyz".to_string(), layout);

        assert_eq!(
            state
                .devtools_view_state
                .inspector
                .last_fetched_node_id
                .as_deref(),
            Some("node-xyz"),
            "last_fetched_node_id should be set from pending_node_id on success"
        );
        assert!(
            state
                .devtools_view_state
                .inspector
                .pending_node_id
                .is_none(),
            "pending_node_id should be cleared after successful fetch"
        );
    }

    #[test]
    fn test_inspector_reset_clears_layout_node_ids() {
        let mut state = make_state_with_session();
        state.devtools_view_state.inspector.last_fetched_node_id = Some("node-1".to_string());
        state.devtools_view_state.inspector.pending_node_id = Some("node-2".to_string());

        state.devtools_view_state.inspector.reset();

        assert!(
            state
                .devtools_view_state
                .inspector
                .last_fetched_node_id
                .is_none(),
            "reset() should clear last_fetched_node_id"
        );
        assert!(
            state
                .devtools_view_state
                .inspector
                .pending_node_id
                .is_none(),
            "reset() should clear pending_node_id"
        );
    }

    // ── Error integration (layout) ─────────────────────────────────────────────

    #[test]
    fn test_layout_data_fetch_failed_stores_friendly_error() {
        let mut state = make_state_with_session();
        let session_id = state.session_manager.selected().unwrap().session.id;

        handle_layout_data_fetch_failed(&mut state, session_id, "Isolate not found".to_string());

        let error = state
            .devtools_view_state
            .inspector
            .layout_error
            .as_ref()
            .expect("layout_error should be set");
        assert_eq!(error.message, "Flutter app isolate not found");
        assert!(!state.devtools_view_state.inspector.layout_loading);
    }

    #[test]
    fn test_timeout_stores_friendly_error_layout() {
        let mut state = make_state_with_session();
        let session_id = state.session_manager.selected().unwrap().session.id;

        handle_layout_data_fetch_timeout(&mut state, session_id);

        let error = state
            .devtools_view_state
            .inspector
            .layout_error
            .as_ref()
            .expect("layout_error should be set after timeout");
        assert_eq!(error.message, "Request timed out");
        assert!(error.hint.contains("[r]"));
    }

    // ── Auto-fetch on navigation (Task 06) ────────────────────────────────────

    /// Build a minimal tree with a root node that has a value_id and one child.
    fn make_tree_with_children() -> fdemon_core::DiagnosticsNode {
        serde_json::from_value(serde_json::json!({
            "description": "Root",
            "valueId": "root-id",
            "children": [{
                "description": "Child",
                "valueId": "child-id",
                "children": []
            }]
        }))
        .expect("valid DiagnosticsNode")
    }

    #[test]
    fn test_navigate_down_triggers_layout_fetch() {
        let mut state = make_state_with_session();

        // Set up a tree with the root expanded so that Down changes selection.
        let tree = make_tree_with_children();
        state.devtools_view_state.inspector.root = Some(tree);
        state
            .devtools_view_state
            .inspector
            .expanded
            .insert("root-id".to_string());
        state.devtools_view_state.inspector.selected_index = 0;

        let result = handle_inspector_navigate(&mut state, InspectorNav::Down);

        assert!(
            matches!(result.action, Some(UpdateAction::FetchLayoutData { .. })),
            "Should return FetchLayoutData action when navigating Down"
        );
        assert!(
            state.devtools_view_state.inspector.layout_loading,
            "layout_loading should be true while fetch is in flight"
        );
    }

    #[test]
    fn test_navigate_up_clears_stale_layout() {
        let mut state = make_state_with_session();

        let tree = make_tree_with_children();
        state.devtools_view_state.inspector.root = Some(tree);
        state
            .devtools_view_state
            .inspector
            .expanded
            .insert("root-id".to_string());
        // Start at child (index 1) so Up changes selection.
        state.devtools_view_state.inspector.selected_index = 1;

        // Pre-set some stale layout data.
        state.devtools_view_state.inspector.layout = Some(fdemon_core::LayoutInfo::default());
        state.devtools_view_state.inspector.layout_error =
            Some(DevToolsError::new("old error", "hint"));

        handle_inspector_navigate(&mut state, InspectorNav::Up);

        assert!(
            state.devtools_view_state.inspector.layout.is_none(),
            "Stale layout should be cleared on selection change"
        );
        assert!(
            state.devtools_view_state.inspector.layout_error.is_none(),
            "Stale layout_error should be cleared on selection change"
        );
    }

    #[test]
    fn test_navigate_debounced_skips_fetch() {
        let mut state = make_state_with_session();

        let tree = make_tree_with_children();
        state.devtools_view_state.inspector.root = Some(tree);
        state
            .devtools_view_state
            .inspector
            .expanded
            .insert("root-id".to_string());
        state.devtools_view_state.inspector.selected_index = 0;

        // Simulate a very recent fetch — debounce should suppress a new one.
        state.devtools_view_state.inspector.layout_last_fetch_time =
            Some(std::time::Instant::now());

        let result = handle_inspector_navigate(&mut state, InspectorNav::Down);

        assert!(
            result.action.is_none(),
            "Should not dispatch FetchLayoutData when debounced, got: {:?}",
            result.action
        );
    }

    #[test]
    fn test_navigate_same_node_skips_fetch() {
        let mut state = make_state_with_session();

        let tree = make_tree_with_children();
        state.devtools_view_state.inspector.root = Some(tree);
        state
            .devtools_view_state
            .inspector
            .expanded
            .insert("root-id".to_string());
        state.devtools_view_state.inspector.selected_index = 0;

        // Pre-set last_fetched_node_id to match the node we'll navigate TO (child-id).
        state.devtools_view_state.inspector.last_fetched_node_id = Some("child-id".to_string());

        let result = handle_inspector_navigate(&mut state, InspectorNav::Down);

        assert!(
            result.action.is_none(),
            "Should not re-fetch layout for a node already fetched (cache hit)"
        );
    }

    #[test]
    fn test_expand_does_not_trigger_layout_fetch() {
        let mut state = make_state_with_session();

        let tree = make_tree_with_children();
        state.devtools_view_state.inspector.root = Some(tree);
        state.devtools_view_state.inspector.selected_index = 0;

        let result = handle_inspector_navigate(&mut state, InspectorNav::Expand);

        assert!(
            result.action.is_none(),
            "Expand should not trigger layout fetch"
        );
        assert!(
            !state.devtools_view_state.inspector.layout_loading,
            "layout_loading should remain false after Expand"
        );
    }

    #[test]
    fn test_collapse_does_not_trigger_layout_fetch() {
        let mut state = make_state_with_session();

        let tree = make_tree_with_children();
        state.devtools_view_state.inspector.root = Some(tree);
        state
            .devtools_view_state
            .inspector
            .expanded
            .insert("root-id".to_string());
        state.devtools_view_state.inspector.selected_index = 0;

        let result = handle_inspector_navigate(&mut state, InspectorNav::Collapse);

        assert!(
            result.action.is_none(),
            "Collapse should not trigger layout fetch"
        );
        assert!(
            !state.devtools_view_state.inspector.layout_loading,
            "layout_loading should remain false after Collapse"
        );
    }

    #[test]
    fn test_is_layout_fetch_debounced_while_loading() {
        let mut state = make_state();
        state.devtools_view_state.inspector.layout_loading = true;

        assert!(
            state
                .devtools_view_state
                .inspector
                .is_layout_fetch_debounced(),
            "Debounce should be active while layout_loading is true"
        );
    }

    #[test]
    fn test_is_layout_fetch_debounced_within_cooldown() {
        let mut state = make_state();
        state.devtools_view_state.inspector.layout_loading = false;
        state.devtools_view_state.inspector.layout_last_fetch_time =
            Some(std::time::Instant::now());

        assert!(
            state
                .devtools_view_state
                .inspector
                .is_layout_fetch_debounced(),
            "Debounce should be active within 500ms cooldown"
        );
    }

    #[test]
    fn test_is_layout_fetch_debounced_inactive_initially() {
        let state = make_state();
        assert!(
            !state
                .devtools_view_state
                .inspector
                .is_layout_fetch_debounced(),
            "Debounce should be inactive with no previous fetch"
        );
    }

    #[test]
    fn test_inspector_reset_clears_layout_last_fetch_time() {
        let mut state = make_state();
        state.devtools_view_state.inspector.layout_last_fetch_time =
            Some(std::time::Instant::now());

        state.devtools_view_state.inspector.reset();

        assert!(
            state
                .devtools_view_state
                .inspector
                .layout_last_fetch_time
                .is_none(),
            "reset() should clear layout_last_fetch_time"
        );
    }

    // ── Review fix: stale layout response guard ──────────────────────────────

    #[test]
    fn test_layout_data_fetched_discards_stale_response() {
        // Under unified stale-guard semantics (M2), the guard compares
        // response.node_id against details_node_id (not selected_value_id()).
        // When details is closed, all in-flight layout responses are discarded.
        let mut state = make_state_with_session();
        let session_id = state.session_manager.selected().unwrap().session.id;

        // Set up tree with root expanded, child visible.
        let tree = make_tree_with_children();
        state.devtools_view_state.inspector.root = Some(tree);
        state
            .devtools_view_state
            .inspector
            .expanded
            .insert("root-id".to_string());

        // Simulate: fetch was dispatched for "root-id" (index 0), but the
        // user closed Details before it completed (details_node_id is None).
        state.devtools_view_state.inspector.pending_node_id = Some("root-id".to_string());
        state.devtools_view_state.inspector.layout_loading = true;
        // details_node_id is None (default) — Details is closed.

        // Now the stale response arrives for "root-id".
        let layout = fdemon_core::LayoutInfo::default();
        handle_layout_data_fetched(&mut state, session_id, "root-id".to_string(), layout);

        // Response should be discarded — layout should remain None.
        assert!(
            state.devtools_view_state.inspector.layout.is_none(),
            "Stale layout response should be discarded when details is closed"
        );
        assert!(
            !state.devtools_view_state.inspector.layout_loading,
            "layout_loading should be cleared after discarding stale response"
        );
        assert!(
            state
                .devtools_view_state
                .inspector
                .pending_node_id
                .is_none(),
            "pending_node_id should be cleared after discarding stale response"
        );
    }

    #[test]
    fn test_layout_data_fetched_accepts_matching_response() {
        let mut state = make_state_with_session();
        let session_id = state.session_manager.selected().unwrap().session.id;

        // Set up tree, selected at root (index 0).
        let tree = make_tree_with_children();
        state.devtools_view_state.inspector.root = Some(tree);
        state
            .devtools_view_state
            .inspector
            .expanded
            .insert("root-id".to_string());
        state.devtools_view_state.inspector.selected_index = 0;

        // Details open for "root-id"; fetch was dispatched for the same node.
        state.devtools_view_state.inspector.details_open = true;
        state.devtools_view_state.inspector.details_node_id = Some("root-id".to_string());
        state.devtools_view_state.inspector.pending_node_id = Some("root-id".to_string());
        state.devtools_view_state.inspector.layout_loading = true;

        let layout = fdemon_core::LayoutInfo::default();
        handle_layout_data_fetched(&mut state, session_id, "root-id".to_string(), layout);

        assert!(
            state.devtools_view_state.inspector.layout.is_some(),
            "Matching layout response should be accepted"
        );
        assert_eq!(
            state
                .devtools_view_state
                .inspector
                .last_fetched_node_id
                .as_deref(),
            Some("root-id"),
            "last_fetched_node_id should be promoted from pending"
        );
    }

    // ── Review fix: tree refresh clears layout cache ─────────────────────────

    #[test]
    fn test_widget_tree_fetched_clears_layout_fields() {
        let mut state = make_state_with_session();
        let session_id = state.session_manager.selected().unwrap().session.id;

        // Pre-set stale layout state from a previous tree.
        state.devtools_view_state.inspector.layout = Some(fdemon_core::LayoutInfo::default());
        state.devtools_view_state.inspector.layout_loading = true;
        state.devtools_view_state.inspector.layout_error =
            Some(DevToolsError::new("old error", "hint"));
        state.devtools_view_state.inspector.last_fetched_node_id = Some("old-node".to_string());
        state.devtools_view_state.inspector.pending_node_id = Some("old-pending".to_string());
        state.devtools_view_state.inspector.layout_last_fetch_time = std::time::Instant::now()
            .checked_sub(std::time::Duration::from_secs(10))
            .or_else(|| Some(std::time::Instant::now()));

        let node: fdemon_core::DiagnosticsNode = serde_json::from_value(serde_json::json!({
            "description": "NewRoot",
            "valueId": "new-root-id"
        }))
        .unwrap();

        handle_widget_tree_fetched(&mut state, session_id, Box::new(node));

        // Stale layout data should be cleared.
        assert!(
            state.devtools_view_state.inspector.layout_error.is_none(),
            "layout_error should be cleared after tree refresh"
        );
        assert_eq!(
            state
                .devtools_view_state
                .inspector
                .last_fetched_node_id
                .as_deref(),
            None,
            "last_fetched_node_id should be cleared after tree refresh \
             (pending_node_id is set for the initial fetch, not last_fetched)"
        );
    }

    // ── Review fix: initial layout fetch on tree load ────────────────────────

    #[test]
    fn test_widget_tree_fetched_dispatches_initial_layout_fetch() {
        let mut state = make_state_with_session();
        let session_id = state.session_manager.selected().unwrap().session.id;

        let node: fdemon_core::DiagnosticsNode = serde_json::from_value(serde_json::json!({
            "description": "Root",
            "valueId": "root-id"
        }))
        .unwrap();

        let result = handle_widget_tree_fetched(&mut state, session_id, Box::new(node));

        assert!(
            matches!(result.action, Some(UpdateAction::FetchLayoutData { .. })),
            "Should dispatch FetchLayoutData for the root node on tree load"
        );
        assert!(
            state.devtools_view_state.inspector.layout_loading,
            "layout_loading should be true after initial fetch dispatch"
        );
        assert_eq!(
            state
                .devtools_view_state
                .inspector
                .pending_node_id
                .as_deref(),
            Some("root-id"),
            "pending_node_id should be set to root node for initial fetch"
        );
    }

    #[test]
    fn test_widget_tree_fetched_no_fetch_when_no_value_id() {
        let mut state = make_state_with_session();
        let session_id = state.session_manager.selected().unwrap().session.id;

        // Node without a value_id — cannot fetch layout.
        let node: fdemon_core::DiagnosticsNode = serde_json::from_value(serde_json::json!({
            "description": "Root"
        }))
        .unwrap();

        let result = handle_widget_tree_fetched(&mut state, session_id, Box::new(node));

        assert!(
            result.action.is_none(),
            "Should not dispatch FetchLayoutData when root has no value_id"
        );
        assert!(
            !state.devtools_view_state.inspector.layout_loading,
            "layout_loading should remain false when no fetch is dispatched"
        );
    }

    // ── Mouse click handlers: select_row ─────────────────────────────────────

    #[test]
    fn test_select_row_out_of_range_is_noop() {
        let mut state = make_state_with_session();
        state.devtools_view_state.inspector.root = Some(make_tree_with_children());
        state
            .devtools_view_state
            .inspector
            .expanded
            .insert("root-id".to_string());
        state.devtools_view_state.inspector.selected_index = 0;

        let result = handle_inspector_select_row(&mut state, 99);
        assert!(result.action.is_none());
        assert_eq!(state.devtools_view_state.inspector.selected_index, 0);
    }

    #[test]
    fn test_select_row_same_index_skips_fetch() {
        let mut state = make_state_with_session();
        state.devtools_view_state.inspector.root = Some(make_tree_with_children());
        state.devtools_view_state.inspector.selected_index = 0;

        let result = handle_inspector_select_row(&mut state, 0);
        assert!(result.action.is_none(), "no fetch on same-index click");
    }

    #[test]
    fn test_select_row_different_index_dispatches_fetch() {
        let mut state = make_state_with_session();
        state.devtools_view_state.inspector.root = Some(make_tree_with_children());
        state
            .devtools_view_state
            .inspector
            .expanded
            .insert("root-id".to_string());
        state.devtools_view_state.inspector.selected_index = 0;

        let result = handle_inspector_select_row(&mut state, 1);
        assert!(
            matches!(result.action, Some(UpdateAction::FetchLayoutData { .. })),
            "Should dispatch FetchLayoutData when clicking a different row"
        );
        assert_eq!(state.devtools_view_state.inspector.selected_index, 1);
    }

    #[test]
    fn test_select_row_clears_stale_layout_on_change() {
        let mut state = make_state_with_session();
        state.devtools_view_state.inspector.root = Some(make_tree_with_children());
        state
            .devtools_view_state
            .inspector
            .expanded
            .insert("root-id".to_string());
        state.devtools_view_state.inspector.selected_index = 0;
        state.devtools_view_state.inspector.layout = Some(fdemon_core::LayoutInfo::default());
        state.devtools_view_state.inspector.layout_error =
            Some(DevToolsError::new("old error", "hint"));

        handle_inspector_select_row(&mut state, 1);

        assert!(
            state.devtools_view_state.inspector.layout.is_none(),
            "Stale layout should be cleared on row change"
        );
        assert!(
            state.devtools_view_state.inspector.layout_error.is_none(),
            "Stale layout_error should be cleared on row change"
        );
    }

    #[test]
    fn test_select_row_debounced_skips_fetch() {
        let mut state = make_state_with_session();
        state.devtools_view_state.inspector.root = Some(make_tree_with_children());
        state
            .devtools_view_state
            .inspector
            .expanded
            .insert("root-id".to_string());
        state.devtools_view_state.inspector.selected_index = 0;
        // Simulate a very recent layout fetch — debounce should suppress a new one.
        state.devtools_view_state.inspector.layout_last_fetch_time =
            Some(std::time::Instant::now());

        let result = handle_inspector_select_row(&mut state, 1);

        assert!(
            result.action.is_none(),
            "Should not dispatch FetchLayoutData when debounced"
        );
    }

    // ── Mouse click handlers: toggle_node ────────────────────────────────────

    #[test]
    fn test_toggle_node_collapsed_to_expanded() {
        let mut state = make_state_with_session();
        state.devtools_view_state.inspector.root = Some(make_tree_with_children());
        // Root is NOT in expanded set initially.
        assert!(!state
            .devtools_view_state
            .inspector
            .expanded
            .contains("root-id"));

        handle_inspector_toggle_node(&mut state, 0);

        assert!(
            state
                .devtools_view_state
                .inspector
                .expanded
                .contains("root-id"),
            "Collapsed node should be expanded after toggle"
        );
    }

    #[test]
    fn test_toggle_node_expanded_to_collapsed() {
        let mut state = make_state_with_session();
        state.devtools_view_state.inspector.root = Some(make_tree_with_children());
        state
            .devtools_view_state
            .inspector
            .expanded
            .insert("root-id".to_string());

        handle_inspector_toggle_node(&mut state, 0);

        assert!(
            !state
                .devtools_view_state
                .inspector
                .expanded
                .contains("root-id"),
            "Expanded node should be collapsed after toggle"
        );
    }

    #[test]
    fn test_toggle_node_on_leaf_does_not_modify_expanded_set() {
        let mut state = make_state_with_session();
        state.devtools_view_state.inspector.root = Some(make_tree_with_children());
        state
            .devtools_view_state
            .inspector
            .expanded
            .insert("root-id".to_string());

        let before = state.devtools_view_state.inspector.expanded.len();
        // Index 1 is "child-id" — a leaf in make_tree_with_children().
        handle_inspector_toggle_node(&mut state, 1);
        let after = state.devtools_view_state.inspector.expanded.len();

        assert_eq!(before, after, "leaf toggle should not change expanded set");
    }

    #[test]
    fn test_toggle_node_out_of_range_is_noop() {
        let mut state = make_state_with_session();
        state.devtools_view_state.inspector.root = Some(make_tree_with_children());
        state
            .devtools_view_state
            .inspector
            .expanded
            .insert("root-id".to_string());
        state.devtools_view_state.inspector.selected_index = 0;

        let before_selected = state.devtools_view_state.inspector.selected_index;
        let result = handle_inspector_toggle_node(&mut state, 99);
        assert!(result.action.is_none());
        assert_eq!(
            state.devtools_view_state.inspector.selected_index, before_selected,
            "Out-of-range toggle should not change selection"
        );
    }

    #[test]
    fn test_toggle_node_still_selects_on_leaf() {
        let mut state = make_state_with_session();
        state.devtools_view_state.inspector.root = Some(make_tree_with_children());
        state
            .devtools_view_state
            .inspector
            .expanded
            .insert("root-id".to_string());
        // Start at root (index 0).
        state.devtools_view_state.inspector.selected_index = 0;

        // Toggle on the leaf child (index 1) — selection should change.
        handle_inspector_toggle_node(&mut state, 1);

        assert_eq!(
            state.devtools_view_state.inspector.selected_index, 1,
            "Toggling a leaf should still select that row"
        );
    }

    // ── Bug fix: fetch failed / timeout clears debounce (Task 02) ────────────

    #[test]
    fn fetch_failed_clears_debounce() {
        let mut state = make_state_with_session();
        let session_id = state.session_manager.selected().unwrap().session.id;

        // Simulate a fetch start — this stamps last_fetch_time and sets loading.
        state.devtools_view_state.inspector.record_fetch_start();
        assert!(
            state.devtools_view_state.inspector.is_fetch_debounced(),
            "Debounce should be active while loading"
        );

        handle_widget_tree_fetch_failed(&mut state, session_id, "some rpc error".to_string());

        assert!(
            !state.devtools_view_state.inspector.is_fetch_debounced(),
            "Debounce should be cleared immediately after fetch failure"
        );
        assert!(
            !state.devtools_view_state.inspector.loading,
            "loading should be false after failure"
        );
        assert!(
            state
                .devtools_view_state
                .inspector
                .last_fetch_time
                .is_none(),
            "last_fetch_time should be None after clear_fetch_debounce"
        );
    }

    #[test]
    fn fetch_timeout_clears_debounce() {
        let mut state = make_state_with_session();
        let session_id = state.session_manager.selected().unwrap().session.id;

        // Simulate a fetch start — this stamps last_fetch_time and sets loading.
        state.devtools_view_state.inspector.record_fetch_start();
        assert!(
            state.devtools_view_state.inspector.is_fetch_debounced(),
            "Debounce should be active while loading"
        );

        handle_widget_tree_fetch_timeout(&mut state, session_id);

        assert!(
            !state.devtools_view_state.inspector.is_fetch_debounced(),
            "Debounce should be cleared immediately after fetch timeout"
        );
        assert!(
            !state.devtools_view_state.inspector.loading,
            "loading should be false after timeout"
        );
        assert!(
            state
                .devtools_view_state
                .inspector
                .last_fetch_time
                .is_none(),
            "last_fetch_time should be None after clear_fetch_debounce"
        );
    }

    #[test]
    fn fetch_failed_no_session_does_not_clear_debounce() {
        let mut state = make_state_with_session();

        // Simulate a fetch start.
        state.devtools_view_state.inspector.record_fetch_start();

        // Use a session_id that does not match the active session.
        handle_widget_tree_fetch_failed(&mut state, 9999, "some rpc error".to_string());

        // State should be unchanged — debounce still active because loading == true.
        assert!(
            state.devtools_view_state.inspector.is_fetch_debounced(),
            "Debounce should remain active when session_id does not match"
        );
        assert!(
            state.devtools_view_state.inspector.loading,
            "loading should remain true when session_id does not match"
        );
    }

    #[test]
    fn fetch_success_leaves_debounce_intact() {
        let mut state = make_state_with_session();
        let session_id = state.session_manager.selected().unwrap().session.id;

        // Simulate a fetch start.
        state.devtools_view_state.inspector.record_fetch_start();

        // Successful fetch — root has no value_id so no layout fetch is dispatched.
        let node: fdemon_core::DiagnosticsNode = serde_json::from_value(serde_json::json!({
            "description": "Root"
        }))
        .unwrap();
        handle_widget_tree_fetched(&mut state, session_id, Box::new(node));

        // loading is now false but last_fetch_time was set by record_fetch_start,
        // so is_fetch_debounced() returns true (cooldown still active).
        assert!(
            state.devtools_view_state.inspector.is_fetch_debounced(),
            "Success path should leave debounce active (last_fetch_time not cleared)"
        );
        assert!(
            !state.devtools_view_state.inspector.loading,
            "loading should be false after successful fetch"
        );
        assert!(
            state
                .devtools_view_state
                .inspector
                .last_fetch_time
                .is_some(),
            "last_fetch_time should remain Some after successful fetch"
        );
    }

    // ── Task 07: integration-style lifecycle tests ────────────────────────────

    /// Scenario 1: initial inspector open → success.
    ///
    /// After `WidgetTreeFetched` the full state contract holds:
    /// `loading == false`, `root == Some(_)`, `error == None`.
    #[test]
    fn test_inspector_open_success_loading_false_root_some_error_none() {
        let mut state = make_state_with_session();
        let session_id = state.session_manager.selected().unwrap().session.id;

        // Prime error / loading state that a real fetch start would set.
        state.devtools_view_state.inspector.loading = true;
        state.devtools_view_state.inspector.error =
            Some(DevToolsError::new("stale error", "retry"));

        let node: fdemon_core::DiagnosticsNode = serde_json::from_value(serde_json::json!({
            "description": "MaterialApp",
            "valueId": "root-value-id"
        }))
        .unwrap();

        handle_widget_tree_fetched(&mut state, session_id, Box::new(node));

        assert!(
            !state.devtools_view_state.inspector.loading,
            "loading must be false after successful fetch (scenario 1)"
        );
        assert!(
            state.devtools_view_state.inspector.root.is_some(),
            "root must be Some(_) after successful fetch (scenario 1)"
        );
        assert!(
            state.devtools_view_state.inspector.error.is_none(),
            "error must be None after successful fetch (scenario 1)"
        );
    }

    /// Scenario 4: after failure → `r` press → new RPC fires immediately (not debounced).
    ///
    /// Verifies the integration between `handle_widget_tree_fetch_failed`
    /// clearing the debounce and a subsequent `RequestWidgetTree` message being
    /// processed without suppression.  The key assertion is that
    /// `is_fetch_debounced()` is `false` immediately after the failure handler
    /// returns, so the caller of the message loop can immediately re-issue the
    /// fetch.
    #[test]
    fn test_inspector_open_then_fail_clears_debounce() {
        let mut state = make_state_with_session();
        let session_id = state.session_manager.selected().unwrap().session.id;

        // Simulate a fetch in progress.
        state.devtools_view_state.inspector.record_fetch_start();
        assert!(
            state.devtools_view_state.inspector.is_fetch_debounced(),
            "Debounce should be active while fetch is in flight"
        );

        // Fetch fails.
        handle_widget_tree_fetch_failed(
            &mut state,
            session_id,
            "ext.flutter.inspector.getRootWidgetTree: null check failure".to_string(),
        );

        // The debounce must be cleared so that a subsequent `r` press is NOT
        // suppressed and a new RPC fires immediately.
        assert!(
            !state.devtools_view_state.inspector.is_fetch_debounced(),
            "Debounce must be cleared after fetch failure so `r` fires immediately (scenario 4)"
        );
        assert!(
            !state.devtools_view_state.inspector.loading,
            "loading must be false after fetch failure (scenario 4)"
        );
        assert!(
            state
                .devtools_view_state
                .inspector
                .last_fetch_time
                .is_none(),
            "last_fetch_time must be None after debounce clear (scenario 4)"
        );
        assert!(
            state.devtools_view_state.inspector.error.is_some(),
            "error must be set after fetch failure (scenario 4)"
        );
    }

    // ── Details panel handlers (Phase 1, Task 05) ─────────────────────────────

    /// Build a state with a session and a tree whose root has value_id "node-0-value-id"
    /// and a child at index 1 with value_id "node-1-value-id". The root is expanded
    /// so both rows are visible.
    fn make_state_with_tree() -> AppState {
        let mut state = make_state_with_session();
        let tree: fdemon_core::DiagnosticsNode = serde_json::from_value(serde_json::json!({
            "description": "Root",
            "valueId": "node-0-value-id",
            "children": [{
                "description": "Child",
                "valueId": "node-1-value-id",
                "children": []
            }]
        }))
        .expect("valid DiagnosticsNode for make_state_with_tree");
        state.devtools_view_state.inspector.root = Some(tree);
        state
            .devtools_view_state
            .inspector
            .expanded
            .insert("node-0-value-id".to_string());
        state
    }

    #[test]
    fn handle_open_details_sets_details_open_and_snapshots_node_id() {
        let mut state = make_state_with_tree();
        // Select the child (index 1) whose value_id is "node-1-value-id".
        state.devtools_view_state.inspector.selected_index = 1;

        let _ = handle_open_details(&mut state);

        assert!(state.devtools_view_state.inspector.details_open);
        assert_eq!(
            state
                .devtools_view_state
                .inspector
                .details_node_id
                .as_deref(),
            Some("node-1-value-id"),
            "details_node_id should be set to the selected node's value_id"
        );
        assert_eq!(
            state.devtools_view_state.inspector.details_tab,
            crate::state::DetailsTab::Properties,
            "details_tab should be reset to Properties on open"
        );
    }

    #[test]
    fn handle_open_details_is_no_op_when_no_selection() {
        let mut state = make_state_with_session();
        // No tree loaded — selected_value_id() returns None.
        let result = handle_open_details(&mut state);
        assert!(!state.devtools_view_state.inspector.details_open);
        assert!(result.action.is_none());
    }

    #[test]
    fn handle_open_details_is_no_op_when_already_open() {
        let mut state = make_state_with_tree();
        state.devtools_view_state.inspector.details_open = true;
        state.devtools_view_state.inspector.details_node_id = Some("existing-id".to_string());

        let result = handle_open_details(&mut state);
        // Should return immediately — no state change.
        assert_eq!(
            state
                .devtools_view_state
                .inspector
                .details_node_id
                .as_deref(),
            Some("existing-id"),
            "details_node_id should not be updated when details already open"
        );
        assert!(result.action.is_none());
    }

    #[test]
    fn handle_open_details_dispatches_fetch_layout_when_data_stale() {
        let mut state = make_state_with_tree();
        state.devtools_view_state.inspector.selected_index = 0;

        // Ensure layout data is stale (last_fetched_node_id differs or is None).
        state.devtools_view_state.inspector.last_fetched_node_id = None;
        state.devtools_view_state.inspector.layout_loading = false;

        let result = handle_open_details(&mut state);

        // Phase 2: handle_open_details now returns multiple actions.
        // Check via result.actions() which combines primary + extra actions.
        assert!(
            result
                .actions()
                .iter()
                .any(|a| matches!(a, UpdateAction::FetchLayoutData { .. })),
            "Should dispatch FetchLayoutData when layout data is stale"
        );
        assert!(state.devtools_view_state.inspector.layout_loading);
    }

    #[test]
    fn handle_open_details_skips_fetch_when_data_already_cached() {
        let mut state = make_state_with_tree();
        state.devtools_view_state.inspector.selected_index = 0;

        // Pre-cache both layout and properties data for "node-0-value-id".
        state.devtools_view_state.inspector.last_fetched_node_id =
            Some("node-0-value-id".to_string());
        state
            .devtools_view_state
            .inspector
            .last_fetched_properties_node_id = Some("node-0-value-id".to_string());
        state.devtools_view_state.inspector.layout_loading = false;

        let result = handle_open_details(&mut state);

        // Phase 2: both caches are warm — no action should be dispatched.
        assert!(
            result.actions().is_empty(),
            "Should NOT dispatch any fetch when both caches are warm, got {:?}",
            result.actions()
        );
        assert!(
            !state.devtools_view_state.inspector.layout_loading,
            "layout_loading should remain false when data is cached"
        );
    }

    #[test]
    fn handle_close_details_clears_details_node_id() {
        let mut state = make_state_with_tree();
        state.devtools_view_state.inspector.details_open = true;
        state.devtools_view_state.inspector.details_node_id = Some("some-node-id".to_string());
        state.devtools_view_state.inspector.details_tab = crate::state::DetailsTab::RenderObject;

        handle_close_details(&mut state);

        assert!(!state.devtools_view_state.inspector.details_open);
        assert!(state
            .devtools_view_state
            .inspector
            .details_node_id
            .is_none());
        // details_tab is intentionally preserved.
        assert_eq!(
            state.devtools_view_state.inspector.details_tab,
            crate::state::DetailsTab::RenderObject,
            "details_tab should be preserved across close/reopen"
        );
    }

    #[test]
    fn handle_close_details_is_no_op_when_already_closed() {
        let mut state = make_state_with_tree();
        state.devtools_view_state.inspector.details_open = false;

        let result = handle_close_details(&mut state);
        assert!(!state.devtools_view_state.inspector.details_open);
        assert!(result.action.is_none());
    }

    #[test]
    fn handle_cycle_tab_forward_advances_through_three_tabs_with_wrap() {
        let mut state = make_state_with_tree();
        // Phase 3 update: populate state to make all three tabs visible.
        {
            let inspector = &mut state.devtools_view_state.inspector;
            inspector.details_open = true;
            inspector.details_tab = crate::state::DetailsTab::Properties;
            inspector.render_properties = vec![fdemon_core::DiagnosticsNode {
                description: "RenderFlex".into(),
                ..Default::default()
            }];
            inspector.details_context = fdemon_core::widget_tree::DetailsContext {
                is_flex_layout: true,
                parent_type: None,
            };
        }

        handle_cycle_tab(&mut state, true);
        assert_eq!(
            state.devtools_view_state.inspector.details_tab,
            crate::state::DetailsTab::RenderObject
        );

        handle_cycle_tab(&mut state, true);
        assert_eq!(
            state.devtools_view_state.inspector.details_tab,
            crate::state::DetailsTab::FlexExplorer
        );

        // Wrap around.
        handle_cycle_tab(&mut state, true);
        assert_eq!(
            state.devtools_view_state.inspector.details_tab,
            crate::state::DetailsTab::Properties
        );
    }

    #[test]
    fn handle_cycle_tab_backward_advances_through_three_tabs_with_wrap() {
        let mut state = make_state_with_tree();
        // Phase 3 update: populate state to make all three tabs visible.
        {
            let inspector = &mut state.devtools_view_state.inspector;
            inspector.details_open = true;
            inspector.details_tab = crate::state::DetailsTab::Properties;
            inspector.render_properties = vec![fdemon_core::DiagnosticsNode {
                description: "RenderFlex".into(),
                ..Default::default()
            }];
            inspector.details_context = fdemon_core::widget_tree::DetailsContext {
                is_flex_layout: true,
                parent_type: None,
            };
        }

        handle_cycle_tab(&mut state, false);
        assert_eq!(
            state.devtools_view_state.inspector.details_tab,
            crate::state::DetailsTab::FlexExplorer
        );

        handle_cycle_tab(&mut state, false);
        assert_eq!(
            state.devtools_view_state.inspector.details_tab,
            crate::state::DetailsTab::RenderObject
        );

        handle_cycle_tab(&mut state, false);
        assert_eq!(
            state.devtools_view_state.inspector.details_tab,
            crate::state::DetailsTab::Properties
        );
    }

    #[test]
    fn handle_cycle_tab_is_no_op_when_details_closed() {
        let mut state = make_state_with_tree();
        state.devtools_view_state.inspector.details_open = false;
        state.devtools_view_state.inspector.details_tab = crate::state::DetailsTab::Properties;

        handle_cycle_tab(&mut state, true);

        assert_eq!(
            state.devtools_view_state.inspector.details_tab,
            crate::state::DetailsTab::Properties,
            "Tab should not change when details is closed"
        );
    }

    // ── Phase 3 cycle-tab / visible-tabs / details-context tests ─────────────

    #[test]
    fn handle_cycle_tab_is_noop_when_only_properties_visible() {
        let mut state = AppState::new();
        state.devtools_view_state.inspector.details_open = true;
        state.devtools_view_state.inspector.details_tab = crate::state::DetailsTab::Properties;
        // Default: render_properties empty, details_context default → 1 visible tab.
        handle_cycle_tab(&mut state, true);
        assert_eq!(
            state.devtools_view_state.inspector.details_tab,
            crate::state::DetailsTab::Properties,
            "forward cycle with 1 visible tab should be a no-op"
        );
        handle_cycle_tab(&mut state, false);
        assert_eq!(
            state.devtools_view_state.inspector.details_tab,
            crate::state::DetailsTab::Properties,
            "backward cycle with 1 visible tab should be a no-op"
        );
    }

    #[test]
    fn handle_cycle_tab_skips_flex_explorer_when_hidden() {
        let mut state = AppState::new();
        {
            let inspector = &mut state.devtools_view_state.inspector;
            inspector.details_open = true;
            inspector.details_tab = crate::state::DetailsTab::Properties;
            inspector.render_properties = vec![fdemon_core::DiagnosticsNode {
                description: "RenderFlex".into(),
                ..Default::default()
            }];
            // details_context default → is_flex_layout = false → FlexExplorer hidden
        }
        handle_cycle_tab(&mut state, true);
        assert_eq!(
            state.devtools_view_state.inspector.details_tab,
            crate::state::DetailsTab::RenderObject,
            "forward from Properties should land on RenderObject"
        );
        handle_cycle_tab(&mut state, true);
        // Skip FlexExplorer, wrap to Properties.
        assert_eq!(
            state.devtools_view_state.inspector.details_tab,
            crate::state::DetailsTab::Properties,
            "forward from RenderObject should skip FlexExplorer and wrap to Properties"
        );
    }

    #[test]
    fn handle_cycle_tab_skips_render_object_when_hidden() {
        let mut state = AppState::new();
        {
            let inspector = &mut state.devtools_view_state.inspector;
            inspector.details_open = true;
            inspector.details_tab = crate::state::DetailsTab::Properties;
            // render_properties empty → RenderObject hidden
            inspector.details_context = fdemon_core::widget_tree::DetailsContext {
                is_flex_layout: true,
                parent_type: None,
            };
        }
        handle_cycle_tab(&mut state, true);
        assert_eq!(
            state.devtools_view_state.inspector.details_tab,
            crate::state::DetailsTab::FlexExplorer,
            "forward from Properties should skip RenderObject and land on FlexExplorer"
        );
        handle_cycle_tab(&mut state, true);
        // Skip RenderObject, wrap to Properties.
        assert_eq!(
            state.devtools_view_state.inspector.details_tab,
            crate::state::DetailsTab::Properties,
            "forward from FlexExplorer should skip RenderObject and wrap to Properties"
        );
    }

    #[test]
    fn handle_open_details_populates_details_context_for_column_widget() {
        let mut state = make_state_with_session();
        let column = serde_json::from_value::<fdemon_core::DiagnosticsNode>(serde_json::json!({
            "description": "Column",
            "valueId": "col-id"
        }))
        .expect("valid DiagnosticsNode");
        {
            let inspector = &mut state.devtools_view_state.inspector;
            inspector.root = Some(column);
            inspector.selected_index = 0;
        }
        handle_open_details(&mut state);
        let ctx = &state.devtools_view_state.inspector.details_context;
        assert!(ctx.is_flex_layout, "Column should be is_flex_layout=true");
    }

    #[test]
    fn handle_open_details_populates_details_context_for_non_flex_root() {
        let mut state = make_state_with_session();
        let container = serde_json::from_value::<fdemon_core::DiagnosticsNode>(serde_json::json!({
            "description": "Container",
            "valueId": "c-id"
        }))
        .expect("valid DiagnosticsNode");
        {
            let inspector = &mut state.devtools_view_state.inspector;
            inspector.root = Some(container);
            inspector.selected_index = 0;
        }
        handle_open_details(&mut state);
        let ctx = &state.devtools_view_state.inspector.details_context;
        assert!(
            !ctx.is_flex_layout,
            "Container with no parent should be is_flex_layout=false"
        );
    }

    #[test]
    fn handle_inspector_properties_fetched_clamps_active_tab_to_properties_when_render_object_disappears(
    ) {
        let mut state = make_state_with_session();
        {
            let inspector = &mut state.devtools_view_state.inspector;
            inspector.details_open = true;
            inspector.details_tab = crate::state::DetailsTab::RenderObject;
            inspector.details_node_id = Some("node-id".into());
            inspector.pending_properties_node_id = Some("node-id".into());
            // Previously had render_properties → RenderObject was visible.
            inspector.render_properties = vec![fdemon_core::DiagnosticsNode {
                description: "RenderOld".into(),
                ..Default::default()
            }];
        }
        let session_id = state.session_manager.selected().unwrap().session.id;
        // Simulate a successful fetch that returns no render-object properties
        // (e.g. user re-selected a widget with no RenderObject).
        handle_inspector_properties_fetched(
            &mut state,
            session_id,
            "node-id".to_string(),
            vec![], // widget_props
            vec![], // render_props — empty triggers clamp
        );
        assert_eq!(
            state.devtools_view_state.inspector.details_tab,
            crate::state::DetailsTab::Properties,
            "clamp_details_tab should snap RenderObject → Properties when render_properties becomes empty"
        );
    }

    #[test]
    fn handle_toggle_hide_implementation_flips_flag_and_clamps_selection() {
        let mut state = make_state_with_tree();
        // Default is hide_implementation_widgets = true.
        // With our two-node tree (root + child), tree shows both rows when hide=false
        // and may collapse chains when hide=true.
        let was = state
            .devtools_view_state
            .inspector
            .hide_implementation_widgets;

        handle_toggle_hide_implementation(&mut state);

        assert_ne!(
            state
                .devtools_view_state
                .inspector
                .hide_implementation_widgets,
            was,
            "hide_implementation_widgets should be flipped"
        );

        // selected_index must be valid (< row_count).
        let row_count = state.devtools_view_state.inspector.inspector_rows().len();
        assert!(
            state.devtools_view_state.inspector.selected_index < row_count.max(1),
            "selected_index should be clamped to a valid row after toggle"
        );
    }

    #[test]
    fn handle_toggle_hide_implementation_writes_back_to_settings() {
        let mut state = make_state_with_tree();
        let original = state
            .devtools_view_state
            .inspector
            .hide_implementation_widgets;

        handle_toggle_hide_implementation(&mut state);

        assert_eq!(
            state.settings.devtools.hide_implementation_widgets, !original,
            "Settings must reflect the toggled value"
        );
        assert_eq!(
            state.settings.devtools.hide_implementation_widgets,
            state
                .devtools_view_state
                .inspector
                .hide_implementation_widgets,
            "Settings and inspector state must be in sync after toggle"
        );
    }

    #[test]
    fn handle_toggle_hide_implementation_returns_persist_settings_action() {
        let mut state = make_state_with_tree();
        let initial = state
            .devtools_view_state
            .inspector
            .hide_implementation_widgets;
        let expected_project_path = state.project_path.clone();

        let result = handle_toggle_hide_implementation(&mut state);

        // Must return a PersistSettings action — never UpdateResult::none().
        let action = match result.action {
            Some(a) => a,
            None => panic!("expected PersistSettings action, got none"),
        };

        match action {
            UpdateAction::PersistSettings {
                settings,
                project_path,
            } => {
                assert_eq!(
                    settings.devtools.hide_implementation_widgets, !initial,
                    "PersistSettings payload must carry the toggled value"
                );
                assert_eq!(
                    project_path, expected_project_path,
                    "PersistSettings must use the state's project_path"
                );
            }
            other => panic!("expected PersistSettings, got {:?}", other),
        }
    }

    #[test]
    fn handle_inspector_navigate_is_no_op_when_details_open() {
        let mut state = make_state_with_tree();
        state.devtools_view_state.inspector.details_open = true;
        state.devtools_view_state.inspector.selected_index = 0;

        let result = handle_inspector_navigate(&mut state, InspectorNav::Down);

        assert!(
            result.action.is_none(),
            "navigate should return no action when details is open"
        );
        assert_eq!(
            state.devtools_view_state.inspector.selected_index, 0,
            "selected_index should not change when details is open"
        );
    }

    #[test]
    fn handle_inspector_select_row_is_no_op_when_details_open() {
        let mut state = make_state_with_tree();
        state.devtools_view_state.inspector.details_open = true;
        state.devtools_view_state.inspector.selected_index = 0;

        let result = handle_inspector_select_row(&mut state, 1);

        assert!(result.action.is_none());
        assert_eq!(
            state.devtools_view_state.inspector.selected_index, 0,
            "row selection should be frozen when details is open"
        );
    }

    // ── Task 06: expanded_groups wiring ──────────────────────────────────────

    /// Build a state with a session and a folded chain:
    ///   - root wrapper (local project, expanded) at index 0
    ///   - chain leader (non-local, `RowGroup::LeaderCollapsed`) at index 1
    ///     with `value_id` = "leader-id"
    ///
    /// `hide_implementation_widgets` is set to `true` so the builder folds the
    /// chain.
    fn make_state_with_folded_chain() -> AppState {
        let mut state = make_state_with_session();
        // Chain: 3 non-local-project nodes (chain-0 → chain-1 → chain-2).
        let chain = fdemon_core::DiagnosticsNode {
            description: "chain-0".to_string(),
            value_id: Some("leader-id".to_string()),
            created_by_local_project: false,
            children: vec![fdemon_core::DiagnosticsNode {
                description: "chain-1".to_string(),
                value_id: Some("member-1-id".to_string()),
                created_by_local_project: false,
                children: vec![fdemon_core::DiagnosticsNode {
                    description: "chain-2".to_string(),
                    value_id: Some("member-2-id".to_string()),
                    created_by_local_project: false,
                    children: vec![],
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        };
        let root = fdemon_core::DiagnosticsNode {
            description: "RootWrapper".to_string(),
            value_id: Some("wrapper-id".to_string()),
            created_by_local_project: true,
            children: vec![chain],
            ..Default::default()
        };
        state.devtools_view_state.inspector.root = Some(root);
        // Expand the wrapper so the chain leader is visible at index 1.
        state
            .devtools_view_state
            .inspector
            .expanded
            .insert("wrapper-id".to_string());
        state
            .devtools_view_state
            .inspector
            .hide_implementation_widgets = true;
        state.devtools_view_state.inspector.selected_index = 1; // chain leader
        state
    }

    /// Build a state with a folded chain whose leader is already expanded in
    /// `expanded_groups` (i.e. `RowGroup::LeaderExpanded`).
    fn make_state_with_expanded_chain() -> AppState {
        let mut state = make_state_with_folded_chain();
        state
            .devtools_view_state
            .inspector
            .expanded_groups
            .insert("leader-id".to_string());
        state
    }

    #[test]
    fn expand_on_leader_collapsed_inserts_into_expanded_groups() {
        let mut state = make_state_with_folded_chain();
        // Verify precondition: the selected row is a LeaderCollapsed.
        {
            let row = state
                .devtools_view_state
                .inspector
                .selected_row()
                .expect("row should exist at index 1");
            assert!(
                matches!(row.group, fdemon_core::RowGroup::LeaderCollapsed { .. }),
                "Expected LeaderCollapsed but got: {:?}",
                row.group
            );
        }

        let _ = handle_inspector_navigate(&mut state, InspectorNav::Expand);

        assert!(
            state
                .devtools_view_state
                .inspector
                .expanded_groups
                .contains("leader-id"),
            "expanded_groups should contain leader-id after Expand on LeaderCollapsed"
        );
    }

    #[test]
    fn expand_on_leader_collapsed_does_not_insert_into_expanded() {
        let mut state = make_state_with_folded_chain();
        let before_len = state.devtools_view_state.inspector.expanded.len();

        let _ = handle_inspector_navigate(&mut state, InspectorNav::Expand);

        assert_eq!(
            state.devtools_view_state.inspector.expanded.len(),
            before_len,
            "expanded set must not be mutated when expanding a chain leader"
        );
    }

    #[test]
    fn collapse_on_leader_expanded_removes_from_expanded_groups() {
        let mut state = make_state_with_expanded_chain();
        // Verify precondition: the selected row is a LeaderExpanded.
        {
            let row = state
                .devtools_view_state
                .inspector
                .selected_row()
                .expect("row should exist at index 1");
            assert_eq!(
                row.group,
                fdemon_core::RowGroup::LeaderExpanded,
                "Expected LeaderExpanded but got: {:?}",
                row.group
            );
        }

        let _ = handle_inspector_navigate(&mut state, InspectorNav::Collapse);

        assert!(
            !state
                .devtools_view_state
                .inspector
                .expanded_groups
                .contains("leader-id"),
            "expanded_groups should not contain leader-id after Collapse on LeaderExpanded"
        );
    }

    #[test]
    fn expand_on_standalone_row_inserts_into_expanded() {
        // Regression guard: standalone nodes must still use `expanded`.
        let mut state = make_state_with_session();
        // Build a simple parent/child tree, both local-project (so RowGroup::None).
        let tree: fdemon_core::DiagnosticsNode = serde_json::from_value(serde_json::json!({
            "description": "Parent",
            "valueId": "parent-id",
            "createdByLocalProject": true,
            "children": [{
                "description": "Child",
                "valueId": "child-id",
                "createdByLocalProject": true,
                "children": []
            }]
        }))
        .expect("valid tree");
        state.devtools_view_state.inspector.root = Some(tree);
        state
            .devtools_view_state
            .inspector
            .hide_implementation_widgets = true;
        state.devtools_view_state.inspector.selected_index = 0;

        let _ = handle_inspector_navigate(&mut state, InspectorNav::Expand);

        assert!(
            state
                .devtools_view_state
                .inspector
                .expanded
                .contains("parent-id"),
            "expanded should contain parent-id after Expand on a standalone row"
        );
        assert!(
            state
                .devtools_view_state
                .inspector
                .expanded_groups
                .is_empty(),
            "expanded_groups must remain empty when expanding a standalone row"
        );
    }

    #[test]
    fn mouse_toggle_on_leader_glyph_mutates_expanded_groups_not_expanded() {
        let mut state = make_state_with_folded_chain();
        let before_expanded_len = state.devtools_view_state.inspector.expanded.len();

        // Index 1 = chain leader (LeaderCollapsed).
        handle_inspector_toggle_node(&mut state, 1);

        assert!(
            state
                .devtools_view_state
                .inspector
                .expanded_groups
                .contains("leader-id"),
            "expanded_groups should contain leader-id after toggle on LeaderCollapsed"
        );
        assert_eq!(
            state.devtools_view_state.inspector.expanded.len(),
            before_expanded_len,
            "expanded set must not be mutated when toggling a chain leader"
        );
    }

    #[test]
    fn mouse_toggle_on_standalone_glyph_mutates_expanded() {
        // Regression guard: standalone nodes must still use `expanded`.
        let mut state = make_state_with_session();
        let tree = make_tree_with_children();
        state.devtools_view_state.inspector.root = Some(tree);
        // hide_implementation_widgets = true but root + child are not part of a
        // chain (root has a child, so root is not a single-child non-local node
        // in the strict chain definition — verify RowGroup::None at index 0).
        state
            .devtools_view_state
            .inspector
            .hide_implementation_widgets = true;
        state.devtools_view_state.inspector.selected_index = 0;

        handle_inspector_toggle_node(&mut state, 0);

        assert!(
            state
                .devtools_view_state
                .inspector
                .expanded
                .contains("root-id"),
            "expanded should contain root-id after toggle on a standalone row"
        );
        assert!(
            state
                .devtools_view_state
                .inspector
                .expanded_groups
                .is_empty(),
            "expanded_groups must remain empty when toggling a standalone row"
        );
    }

    // ── Task 07: reset_details_and_groups ────────────────────────────────────

    /// Build a state with details open, a node id snapshotted, a non-default
    /// tab, a non-empty expanded_groups set, and populated properties vectors.
    /// This is the "dirty" precondition for tests below.
    fn make_state_with_details_open() -> (AppState, crate::session::SessionId) {
        let mut state = make_state_with_session();
        let session_id = state.session_manager.selected().unwrap().session.id;
        let tree: fdemon_core::DiagnosticsNode = serde_json::from_value(serde_json::json!({
            "description": "Root",
            "valueId": "root-id",
            "children": [{
                "description": "Child",
                "valueId": "child-id",
                "children": []
            }]
        }))
        .expect("valid DiagnosticsNode for make_state_with_details_open");
        state.devtools_view_state.inspector.root = Some(tree);
        state
            .devtools_view_state
            .inspector
            .expanded
            .insert("root-id".to_string());
        // Open details with a snapshotted node id.
        state.devtools_view_state.inspector.details_open = true;
        state.devtools_view_state.inspector.details_node_id = Some("root-id".to_string());
        state.devtools_view_state.inspector.details_tab = crate::state::DetailsTab::RenderObject;
        // Populate expanded_groups with a fake leader id.
        state
            .devtools_view_state
            .inspector
            .expanded_groups
            .insert("leader-id".to_string());
        // Simulate populated properties cache.
        state.devtools_view_state.inspector.properties =
            vec![serde_json::from_value(serde_json::json!({
                "description": "prop-a",
                "propertyType": "color"
            }))
            .unwrap()];
        state.devtools_view_state.inspector.render_properties =
            vec![serde_json::from_value(serde_json::json!({
                "description": "render-prop-b",
                "propertyType": "RenderObject"
            }))
            .unwrap()];
        state.devtools_view_state.inspector.properties_loading = true;
        state.devtools_view_state.inspector.properties_error =
            Some(DevToolsError::new("old error", "hint"));
        (state, session_id)
    }

    /// After fetching a new tree, the Details panel must be closed and all
    /// details-related state must be cleared (C2 fix).
    #[test]
    fn widget_tree_fetched_clears_details_state_when_details_was_open() {
        let (mut state, session_id) = make_state_with_details_open();

        let new_tree: fdemon_core::DiagnosticsNode = serde_json::from_value(serde_json::json!({
            "description": "NewRoot",
            "valueId": "new-root-id"
        }))
        .unwrap();
        let _ = handle_widget_tree_fetched(&mut state, session_id, Box::new(new_tree));

        let inspector = &state.devtools_view_state.inspector;
        assert!(
            !inspector.details_open,
            "details_open must be false after tree refresh"
        );
        assert!(
            inspector.details_node_id.is_none(),
            "details_node_id must be None after tree refresh"
        );
        assert_eq!(
            inspector.details_tab,
            crate::state::DetailsTab::Properties,
            "details_tab must be reset to Properties after tree refresh"
        );
        assert!(
            inspector.expanded_groups.is_empty(),
            "expanded_groups must be empty after tree refresh"
        );
        assert!(
            inspector.properties.is_empty(),
            "properties must be empty after tree refresh"
        );
        assert!(
            inspector.render_properties.is_empty(),
            "render_properties must be empty after tree refresh"
        );
        assert!(
            !inspector.properties_loading,
            "properties_loading must be false after tree refresh"
        );
        assert!(
            inspector.properties_error.is_none(),
            "properties_error must be None after tree refresh"
        );
    }

    /// Regression guard: `hide_implementation_widgets` must be preserved by
    /// `reset_details_and_groups` because it is a user preference.
    #[test]
    fn reset_details_and_groups_preserves_hide_implementation_widgets() {
        let mut state = make_state_with_session();
        // Set the flag to a non-default value (default is true).
        state
            .devtools_view_state
            .inspector
            .hide_implementation_widgets = false;

        state
            .devtools_view_state
            .inspector
            .reset_details_and_groups();

        assert!(
            !state
                .devtools_view_state
                .inspector
                .hide_implementation_widgets,
            "hide_implementation_widgets must not be touched by reset_details_and_groups"
        );
    }

    /// Regression guard: `reset_details_and_groups` itself does NOT clear
    /// `has_ever_rendered_tree` — only `SessionRestartCompleted` does that.
    #[test]
    fn reset_details_and_groups_preserves_has_ever_rendered_tree() {
        let mut state = make_state_with_session();
        state.devtools_view_state.inspector.has_ever_rendered_tree = true;

        state
            .devtools_view_state
            .inspector
            .reset_details_and_groups();

        assert!(
            state.devtools_view_state.inspector.has_ever_rendered_tree,
            "has_ever_rendered_tree must not be cleared by reset_details_and_groups; \
             only SessionRestartCompleted should clear it"
        );
    }

    /// After hot restart, Details and groups state must be cleared alongside
    /// `has_ever_rendered_tree`.
    #[test]
    fn session_restart_completed_clears_details_state() {
        let (mut state, session_id) = make_state_with_details_open();
        // Also prime has_ever_rendered_tree so we can check it is cleared.
        state.devtools_view_state.inspector.has_ever_rendered_tree = true;

        use crate::handler::update::update;
        use crate::message::Message;
        let _ = update(&mut state, Message::SessionRestartCompleted { session_id });

        let inspector = &state.devtools_view_state.inspector;
        assert!(
            !inspector.details_open,
            "details_open must be false after hot restart"
        );
        assert!(
            inspector.details_node_id.is_none(),
            "details_node_id must be None after hot restart"
        );
        assert_eq!(
            inspector.details_tab,
            crate::state::DetailsTab::Properties,
            "details_tab must be reset to Properties after hot restart"
        );
        assert!(
            inspector.expanded_groups.is_empty(),
            "expanded_groups must be empty after hot restart"
        );
        assert!(
            inspector.properties.is_empty(),
            "properties must be empty after hot restart"
        );
        assert!(
            !inspector.has_ever_rendered_tree,
            "has_ever_rendered_tree must be false after hot restart"
        );
    }

    // ── Properties handlers (Phase 2, Task 06) ───────────────────────────────

    /// Build a minimal `DiagnosticsNode` suitable for use as a test property.
    fn sample_diagnostic(
        name: &str,
        description: &str,
        property_type: Option<&str>,
    ) -> fdemon_core::DiagnosticsNode {
        let mut node: fdemon_core::DiagnosticsNode = serde_json::from_value(serde_json::json!({
            "description": description
        }))
        .expect("valid DiagnosticsNode");
        node.name = Some(name.to_string());
        if let Some(pt) = property_type {
            node.property_type = Some(pt.to_string());
        }
        node
    }

    /// Build a state with a session and the given node selected (value_id =
    /// `node_id`). The tree contains exactly that one node as the root; the
    /// root is auto-selected at index 0.
    fn make_state_with_selected_widget(node_id: &str) -> AppState {
        let mut state = make_state_with_session();
        let tree: fdemon_core::DiagnosticsNode = serde_json::from_value(serde_json::json!({
            "description": "TestWidget",
            "valueId": node_id
        }))
        .expect("valid DiagnosticsNode");
        state.devtools_view_state.inspector.root = Some(tree);
        state.devtools_view_state.inspector.selected_index = 0;
        state
    }

    /// Build a state with a session, the given node selected, **and** the
    /// properties fetch already in-flight (`pending_properties_node_id` set,
    /// `properties_loading = true`).
    fn make_state_with_inspector_open(node_id: &str) -> AppState {
        let mut state = make_state_with_selected_widget(node_id);
        state.devtools_view_state.inspector.details_open = true;
        state.devtools_view_state.inspector.details_node_id = Some(node_id.to_string());
        state
    }

    #[test]
    fn properties_fetched_stores_into_state() {
        let mut state = make_state_with_inspector_open("objects/42");
        let session_id = state.session_manager.selected().unwrap().session.id;
        state
            .devtools_view_state
            .inspector
            .pending_properties_node_id = Some("objects/42".into());
        state.devtools_view_state.inspector.properties_loading = true;

        let widget_props = vec![sample_diagnostic("name", "value", None)];
        let render_props = vec![sample_diagnostic(
            "renderObject",
            "RenderFlex",
            Some("RenderObject"),
        )];

        handle_inspector_properties_fetched(
            &mut state,
            session_id,
            "objects/42".into(),
            widget_props.clone(),
            render_props.clone(),
        );

        let i = &state.devtools_view_state.inspector;
        assert_eq!(i.properties.len(), widget_props.len());
        assert_eq!(i.render_properties.len(), render_props.len());
        assert!(!i.properties_loading, "properties_loading must be cleared");
        assert!(
            i.properties_error.is_none(),
            "properties_error must be None"
        );
        assert_eq!(
            i.last_fetched_properties_node_id.as_deref(),
            Some("objects/42"),
            "last_fetched_properties_node_id should be set"
        );
        assert!(
            i.pending_properties_node_id.is_none(),
            "pending_properties_node_id must be cleared"
        );
    }

    #[test]
    fn properties_fetched_discards_stale_response() {
        let mut state = make_state_with_inspector_open("objects/B");
        let session_id = state.session_manager.selected().unwrap().session.id;
        // B is in-flight.
        state
            .devtools_view_state
            .inspector
            .pending_properties_node_id = Some("objects/B".into());
        state.devtools_view_state.inspector.properties_loading = true;

        // A's response arrives late, while B is in-flight.
        handle_inspector_properties_fetched(
            &mut state,
            session_id,
            "objects/A".into(),
            vec![sample_diagnostic("stale", "stale", None)],
            vec![],
        );

        let i = &state.devtools_view_state.inspector;
        assert!(
            i.properties.is_empty(),
            "stale response must not mutate properties"
        );
        assert!(
            i.properties_loading,
            "loading flag should still be set for in-flight B"
        );
        assert!(
            i.last_fetched_properties_node_id.is_none(),
            "last_fetched must not be set from a stale response"
        );
    }

    #[test]
    fn properties_fetched_cross_session_guard() {
        let mut state = make_state_with_inspector_open("objects/42");
        state
            .devtools_view_state
            .inspector
            .pending_properties_node_id = Some("objects/42".into());
        state.devtools_view_state.inspector.properties_loading = true;

        // Use a session_id that does not match any active session.
        handle_inspector_properties_fetched(
            &mut state,
            9999,
            "objects/42".into(),
            vec![sample_diagnostic("name", "value", None)],
            vec![],
        );

        let i = &state.devtools_view_state.inspector;
        assert!(
            i.properties.is_empty(),
            "cross-session response must not mutate properties"
        );
        assert!(
            i.properties_loading,
            "loading flag must not change on cross-session response"
        );
    }

    #[test]
    fn properties_fetch_failed_sets_error() {
        let mut state = make_state_with_inspector_open("objects/42");
        let session_id = state.session_manager.selected().unwrap().session.id;
        state
            .devtools_view_state
            .inspector
            .pending_properties_node_id = Some("objects/42".into());
        state.devtools_view_state.inspector.properties_loading = true;

        handle_inspector_properties_fetch_failed(
            &mut state,
            session_id,
            "objects/42".into(),
            "Isolate not found: 12345".to_string(),
        );

        let i = &state.devtools_view_state.inspector;
        assert!(!i.properties_loading, "loading must be false after failure");
        assert!(
            i.properties_error.is_some(),
            "error must be set after failure"
        );
        assert!(
            i.pending_properties_node_id.is_none(),
            "pending must be cleared after failure"
        );
        // Cache must NOT be updated on failure — next Enter will retry.
        assert!(
            i.last_fetched_properties_node_id.is_none(),
            "last_fetched must not be updated on failure"
        );
    }

    #[test]
    fn properties_fetch_timeout_sets_error() {
        let mut state = make_state_with_inspector_open("objects/42");
        let session_id = state.session_manager.selected().unwrap().session.id;
        state
            .devtools_view_state
            .inspector
            .pending_properties_node_id = Some("objects/42".into());
        state.devtools_view_state.inspector.properties_loading = true;

        handle_inspector_properties_fetch_timeout(&mut state, session_id, "objects/42".into());

        let i = &state.devtools_view_state.inspector;
        assert!(!i.properties_loading, "loading must be false after timeout");
        let err = i
            .properties_error
            .as_ref()
            .expect("error must be set after timeout");
        assert!(
            err.message.contains("timed out"),
            "error message should mention timeout, got: {:?}",
            err.message
        );
        assert!(
            i.pending_properties_node_id.is_none(),
            "pending must be cleared after timeout"
        );
    }

    #[test]
    fn open_details_dispatches_properties_fetch_on_cache_miss() {
        let mut state = make_state_with_selected_widget("objects/42");
        // No cache: last_fetched_properties_node_id is None.
        let result = handle_open_details(&mut state);

        let actions = result.actions();
        assert!(
            actions.iter().any(|a| matches!(
                a,
                UpdateAction::FetchInspectorProperties { node_id, .. } if node_id == "objects/42"
            )),
            "FetchInspectorProperties must be dispatched on cache miss, got {:?}",
            actions
        );

        let i = &state.devtools_view_state.inspector;
        assert!(i.properties_loading, "properties_loading should be true");
        assert_eq!(i.pending_properties_node_id.as_deref(), Some("objects/42"));
    }

    #[test]
    fn open_details_cache_hit_skips_properties_dispatch() {
        let mut state = make_state_with_selected_widget("objects/42");
        // Both caches warm — no fetch should be dispatched.
        state
            .devtools_view_state
            .inspector
            .last_fetched_properties_node_id = Some("objects/42".into());
        state.devtools_view_state.inspector.last_fetched_node_id = Some("objects/42".into());

        let result = handle_open_details(&mut state);
        let actions = result.actions();

        assert!(
            actions.is_empty(),
            "no fetch should be dispatched on full cache hit, got {:?}",
            actions
        );
    }

    #[test]
    fn open_details_cache_hit_on_properties_but_layout_miss_dispatches_only_layout() {
        let mut state = make_state_with_selected_widget("objects/42");
        // Properties cache warm, layout cache cold.
        state
            .devtools_view_state
            .inspector
            .last_fetched_properties_node_id = Some("objects/42".into());
        state.devtools_view_state.inspector.last_fetched_node_id = None;

        let result = handle_open_details(&mut state);
        let actions = result.actions();

        assert!(
            !actions
                .iter()
                .any(|a| matches!(a, UpdateAction::FetchInspectorProperties { .. })),
            "FetchInspectorProperties must not be dispatched on properties cache hit"
        );
        assert!(
            actions
                .iter()
                .any(|a| matches!(a, UpdateAction::FetchLayoutData { .. })),
            "FetchLayoutData must be dispatched on layout cache miss"
        );
    }

    #[test]
    fn open_details_retries_properties_when_prior_error() {
        let mut state = make_state_with_selected_widget("objects/42");
        // Cache appears hot but there was a prior error — must retry.
        state
            .devtools_view_state
            .inspector
            .last_fetched_properties_node_id = Some("objects/42".into());
        state.devtools_view_state.inspector.properties_error =
            Some(DevToolsError::new("prev error", "retry"));

        let result = handle_open_details(&mut state);
        let actions = result.actions();

        assert!(
            actions
                .iter()
                .any(|a| matches!(a, UpdateAction::FetchInspectorProperties { .. })),
            "FetchInspectorProperties must be dispatched when prior error exists"
        );
    }

    #[test]
    fn open_details_does_not_double_dispatch_when_properties_already_loading() {
        let mut state = make_state_with_selected_widget("objects/42");
        // A fetch is already in-flight — must not dispatch another.
        state.devtools_view_state.inspector.properties_loading = true;
        state
            .devtools_view_state
            .inspector
            .pending_properties_node_id = Some("objects/42".into());

        let result = handle_open_details(&mut state);
        let actions = result.actions();

        assert!(
            !actions
                .iter()
                .any(|a| matches!(a, UpdateAction::FetchInspectorProperties { .. })),
            "FetchInspectorProperties must not be double-dispatched when already loading"
        );
    }

    // ── Regression tests: stale-guard unification (Phase 2 follow-up C2 / M2) ──

    /// Regression test for C2: open details on A → close → open on B → A's
    /// fetch completes → B's details must NOT be mutated.
    ///
    /// This test reproduces the exact race described in the C2 review finding:
    /// `pending_properties_node_id` still points to A, but the Details panel is
    /// now open on B. The stale guard must use `details_node_id` (not the pending
    /// id) to decide whether to apply the response.
    #[test]
    fn properties_response_discarded_when_user_reopened_details_on_different_node() {
        let mut state = make_state_with_session();

        // Step 1: open details on A. Schedules a fetch, sets pending=A.
        state.devtools_view_state.inspector.details_open = true;
        state.devtools_view_state.inspector.details_node_id = Some("A".into());
        state
            .devtools_view_state
            .inspector
            .pending_properties_node_id = Some("A".into());
        state.devtools_view_state.inspector.properties_loading = true;

        // Step 2: user closes details (simulates handle_close_details behavior):
        // details_open and details_node_id are cleared, but pending and loading
        // are deliberately left as-is — this is the original close-details
        // behaviour that opens the race.
        state.devtools_view_state.inspector.details_open = false;
        state.devtools_view_state.inspector.details_node_id = None;

        // Step 3: user reopens details on B. Loading is still true so no new
        // fetch is dispatched; pending stays at A.
        state.devtools_view_state.inspector.details_open = true;
        state.devtools_view_state.inspector.details_node_id = Some("B".into());

        // Step 4: A's response arrives.
        let session_id = state.session_manager.selected().unwrap().session.id;
        let widget_props = vec![sample_diagnostic("colorA", "Color(0xff0000ff)", None)];
        let render_props = vec![];
        let result = handle_inspector_properties_fetched(
            &mut state,
            session_id,
            "A".into(),
            widget_props,
            render_props,
        );

        // Step 5: verify B's details were NOT mutated.
        let inspector = &state.devtools_view_state.inspector;
        assert!(
            inspector.properties.is_empty(),
            "properties for B must remain empty; A's response should be discarded"
        );
        assert!(
            inspector.render_properties.is_empty(),
            "render_properties for B must remain empty"
        );
        assert_eq!(
            inspector.details_node_id.as_deref(),
            Some("B"),
            "details_node_id should still point to B"
        );

        // Step 6: pending should be cleared since A's fetch is now resolved,
        // and loading cleared so the user can refetch for B.
        assert!(
            inspector.pending_properties_node_id.is_none(),
            "pending should be cleared once A's stale response arrives"
        );
        assert!(
            !inspector.properties_loading,
            "properties_loading should be cleared so user can refetch for B"
        );

        assert!(result.action.is_none(), "no action should be returned");
    }

    /// Companion test for the unified-key layout handler (M2).
    ///
    /// Verifies that a layout response for node A is accepted when the Details
    /// panel is still open on A, even if the tree selection has moved to a
    /// different node. Under unified semantics the guard checks `details_node_id`,
    /// not `selected_value_id()`.
    #[test]
    fn layout_response_applied_when_details_node_matches_even_if_selection_moved() {
        let mut state = make_state_with_session();

        // Build a two-node tree (root A + child B) with root expanded.
        let tree: fdemon_core::DiagnosticsNode = serde_json::from_value(serde_json::json!({
            "description": "Root",
            "valueId": "A",
            "children": [{
                "description": "Child",
                "valueId": "B",
                "children": []
            }]
        }))
        .expect("valid DiagnosticsNode");
        state.devtools_view_state.inspector.root = Some(tree);
        state
            .devtools_view_state
            .inspector
            .expanded
            .insert("A".to_string());

        // Details open for A; in-flight layout fetch is also for A.
        state.devtools_view_state.inspector.details_open = true;
        state.devtools_view_state.inspector.details_node_id = Some("A".into());
        state.devtools_view_state.inspector.pending_node_id = Some("A".into());
        state.devtools_view_state.inspector.layout_loading = true;

        // User navigates tree to B (selection moves to index 1) while keeping
        // details open on A.
        state.devtools_view_state.inspector.selected_index = 1;

        // A's layout response arrives.
        let layout = fdemon_core::LayoutInfo::default();
        let session_id = state.session_manager.selected().unwrap().session.id;
        let result = handle_layout_data_fetched(&mut state, session_id, "A".into(), layout.clone());

        // Layout for A should be applied because details_node_id is A.
        let inspector = &state.devtools_view_state.inspector;
        assert!(
            inspector.layout.is_some(),
            "Layout for A must be applied when details_node_id == A"
        );
        assert!(!inspector.layout_loading, "layout_loading must be cleared");
        assert_eq!(
            inspector.last_fetched_node_id.as_deref(),
            Some("A"),
            "last_fetched_node_id should be promoted from pending"
        );
        assert!(result.action.is_none(), "no action should be returned");
    }
}
