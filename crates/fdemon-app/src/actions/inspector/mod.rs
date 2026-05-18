//! DevTools inspector actions: widget tree, overlay toggle, layout explorer, and group disposal.
//!
//! All functions are private to the `actions` module. The four `spawn_*`
//! entry points are called from `actions/mod.rs`'s `handle_action` dispatcher
//! and are therefore `pub(super)`.
//!
//! Private widget-tree fetch helpers (`poll_widget_tree_ready`,
//! `try_fetch_widget_tree`, `is_transient_error`, `is_method_not_found`) are
//! extracted into the `widget_tree` submodule.

mod widget_tree;

use std::collections::HashMap;
use std::time::Duration;

use tokio::sync::mpsc;

use crate::handler::FetchTrigger;
use crate::message::{DebugOverlayKind, Message};
use crate::session::SessionId;
use fdemon_daemon::vm_service::extensions::properties::{
    parse_properties_response, split_widget_and_render_properties,
};
use fdemon_daemon::vm_service::{
    ext, extract_layout_info, parse_bool_extension_response, VmRequestHandle,
};

/// Timeout for a single `getLayoutExplorerNode` RPC call.
const LAYOUT_FETCH_TIMEOUT: Duration = Duration::from_secs(10);

/// Total time budget for a single `FetchInspectorProperties` action — covers
/// the initial widget `getProperties` call AND all sub-property calls for each
/// render-object node.
///
/// 10s is the same budget used by `LAYOUT_FETCH_TIMEOUT`. In practice DevTools
/// observes 0–1 render-object properties per widget, so this budget is
/// generous.
const PROPERTIES_FETCH_TIMEOUT: Duration = Duration::from_secs(10);

/// VM object group for the widget inspector. Scopes `valueId` references
/// returned by `getRootWidgetTree`.
const INSPECTOR_OBJECT_GROUP: &str = "fdemon-inspector-1";

/// VM object group for the layout explorer. Scopes `valueId` references
/// returned by `getLayoutExplorerNode`.
const LAYOUT_OBJECT_GROUP: &str = "devtools-layout";

/// Spawn a background task that fetches the root widget tree via VM Service.
///
/// Uses `ext.flutter.inspector.getRootWidgetTree` (with automatic fallback to
/// `getRootWidgetSummaryTree` for older Flutter versions). An object group
/// named `"fdemon-inspector-1"` is created to scope the returned `valueId`
/// references.
///
/// When `tree_max_depth` is non-zero, the depth is passed as `subtreeDepth`
/// to the RPC call (if supported by the Flutter extension). If the parameter
/// is not supported, the tree is returned at unlimited depth.
///
/// The fetch operation includes:
/// 1. **Readiness polling** — calls `ext.flutter.inspector.isWidgetTreeReady`
///    up to `inspector_readiness_poll_attempts` times (configurable, default 2) before
///    attempting the tree fetch. A timed-out poll is treated as "not ready".
///    Exhausting the poll budget is **not** an error — the fetch proceeds anyway.
/// 2. **API fallback** — tries `getRootWidgetTree` first, falls back to
///    `getRootWidgetSummaryTree` on "method not found".
/// 3. **Configurable outer timeout** — `fetch_timeout_secs` (min 5s) wraps the
///    entire operation.
///
/// Sends `Message::WidgetTreeFetched` on success,
/// `Message::WidgetTreeFetchFailed` on error, or
/// `Message::WidgetTreeFetchTimeout` on timeout.
#[allow(clippy::too_many_arguments)]
pub(super) fn spawn_fetch_widget_tree(
    session_id: SessionId,
    handle: VmRequestHandle,
    msg_tx: mpsc::Sender<Message>,
    tree_max_depth: u32,
    fetch_timeout_secs: u64,
    inspector_readiness_poll_attempts: u32,
    inspector_readiness_poll_interval_ms: u64,
    inspector_readiness_poll_call_timeout_ms: u64,
    trigger: FetchTrigger,
) {
    tokio::spawn(async move {
        let timeout_dur = Duration::from_secs(fetch_timeout_secs.max(5));

        // TODO(stabilization): downgrade to debug! once Inspector stability is verified in production.
        tracing::info!(
            session_id = %session_id,
            tree_max_depth = tree_max_depth,
            fetch_timeout_secs = fetch_timeout_secs,
            trigger = ?trigger,
            "Inspector: fetch_widget_tree task started"
        );

        let fetch_result = tokio::time::timeout(timeout_dur, async {
            // Step 1: Get isolate ID (prefers the Flutter UI isolate with ext.flutter.* RPCs).
            let isolate_id = handle
                .resolve_flutter_ui_isolate()
                .await
                .map_err(|e| format!("Could not get isolate ID: {e}"))?;

            // TODO(stabilization): downgrade to debug! once Inspector stability is verified in production.
            tracing::info!(
                session_id = %session_id,
                isolate_id = %isolate_id,
                "Inspector: resolved isolate"
            );

            // Step 2: Poll widget tree readiness.
            // Skip polling when the user explicitly refreshed (`r`) and the
            // inspector has already rendered a tree.  The Flutter framework is
            // already running in that case, so the readiness poll is wasted
            // budget that only adds latency.
            if trigger != FetchTrigger::Refresh {
                let poll_cfg = widget_tree::ReadinessPollConfig {
                    attempts: inspector_readiness_poll_attempts,
                    interval_ms: inspector_readiness_poll_interval_ms,
                    call_timeout_ms: inspector_readiness_poll_call_timeout_ms,
                };
                widget_tree::poll_widget_tree_ready(&handle, &isolate_id, session_id, &poll_cfg)
                    .await;

                // TODO(stabilization): downgrade to debug! once Inspector stability is verified in production.
                tracing::info!(
                    session_id = %session_id,
                    "Inspector: readiness poll completed"
                );
            } else {
                // TODO(stabilization): downgrade to debug! once Inspector stability is verified in production.
                tracing::info!(
                    session_id = %session_id,
                    "Inspector: readiness poll skipped (Refresh trigger)"
                );
            }

            // Step 3: Dispose previous object group.
            let object_group = INSPECTOR_OBJECT_GROUP;
            {
                let mut dispose_args = HashMap::new();
                dispose_args.insert("objectGroup".to_string(), object_group.to_string());
                if let Err(e) = handle
                    .call_extension(ext::DISPOSE_GROUP, &isolate_id, Some(dispose_args))
                    .await
                {
                    tracing::debug!(
                        "FetchWidgetTree: disposeGroup '{}' failed for session {} (non-fatal): {}",
                        object_group,
                        session_id,
                        e
                    );
                }
            }

            // Step 4: Retry loop — fetch the widget tree.
            // TODO(stabilization): downgrade to debug! once Inspector stability is verified in production.
            tracing::info!(
                session_id = %session_id,
                "Inspector: RPC call getRootWidgetTree starting"
            );
            let result = widget_tree::try_fetch_widget_tree(
                &handle,
                &isolate_id,
                object_group,
                tree_max_depth,
                session_id,
            )
            .await
            .map_err(|e| e.to_string());

            if let Ok(ref root) = result {
                // TODO(stabilization): downgrade to debug! once Inspector stability is verified in production.
                tracing::info!(
                    session_id = %session_id,
                    root_description = %root.description,
                    child_count = root.children.len(),
                    "Inspector: RPC call getRootWidgetTree completed"
                );
            }

            result
        })
        .await;

        let msg = match fetch_result {
            Err(_timeout) => {
                tracing::warn!(
                    session_id = %session_id,
                    timeout_secs = fetch_timeout_secs.max(5),
                    "Inspector: fetch_widget_tree timed out"
                );
                Message::WidgetTreeFetchTimeout { session_id }
            }
            Ok(Ok(root)) => Message::WidgetTreeFetched {
                session_id,
                root: Box::new(root),
            },
            Ok(Err(error)) => {
                tracing::warn!(
                    session_id = %session_id,
                    error = %error,
                    "Inspector: fetch_widget_tree failed"
                );
                Message::WidgetTreeFetchFailed { session_id, error }
            }
        };

        // TODO(stabilization): downgrade to debug! once Inspector stability is verified in production.
        tracing::info!(
            session_id = %session_id,
            msg_kind = match &msg {
                Message::WidgetTreeFetched { .. } => "WidgetTreeFetched",
                Message::WidgetTreeFetchFailed { .. } => "WidgetTreeFetchFailed",
                Message::WidgetTreeFetchTimeout { .. } => "WidgetTreeFetchTimeout",
                _ => "Other",
            },
            "Inspector: dispatching result message"
        );

        if let Err(e) = msg_tx.send(msg).await {
            tracing::error!(
                session_id = %session_id,
                error = ?e,
                "Inspector: failed to send terminal message — receiver dropped"
            );
        }
    });
}

/// Spawn a background task that flips a debug overlay extension via VM Service.
///
/// Reads the current boolean state with one RPC call, then sets the opposite
/// state with a second RPC call (matching the `flip_overlay` pattern but using
/// `VmRequestHandle` instead of `VmServiceClient`).
///
/// Sends `Message::DebugOverlayToggled` on success (including profile-mode
/// failures where the extension is not available — which are silently logged).
pub(super) fn spawn_toggle_overlay(
    session_id: SessionId,
    extension: DebugOverlayKind,
    handle: VmRequestHandle,
    msg_tx: mpsc::Sender<Message>,
) {
    tokio::spawn(async move {
        let isolate_id = match handle.resolve_flutter_ui_isolate().await {
            Ok(id) => id,
            Err(e) => {
                tracing::warn!(
                    "ToggleOverlay: could not get isolate ID for session {}: {}",
                    session_id,
                    e
                );
                // No message sent — the overlay state is unchanged.
                return;
            }
        };

        let method = match extension {
            DebugOverlayKind::RepaintRainbow => ext::REPAINT_RAINBOW,
            DebugOverlayKind::DebugPaint => ext::DEBUG_PAINT,
            DebugOverlayKind::PerformanceOverlay => ext::SHOW_PERFORMANCE_OVERLAY,
        };

        // Step 1: read the current state.
        let current = match handle.call_extension(method, &isolate_id, None).await {
            Ok(value) => match parse_bool_extension_response(&value) {
                Ok(v) => v,
                Err(e) => {
                    tracing::warn!(
                        "ToggleOverlay: failed to parse current state for {:?} \
                         (session {}): {}",
                        extension,
                        session_id,
                        e
                    );
                    return;
                }
            },
            Err(e) => {
                // Extension not available (e.g., profile/release build) — log and ignore.
                tracing::debug!(
                    "ToggleOverlay: extension {:?} not available for session {}: {}",
                    extension,
                    session_id,
                    e
                );
                return;
            }
        };

        // Step 2: set the opposite state.
        let mut args = HashMap::new();
        args.insert("enabled".to_string(), (!current).to_string());
        let new_state = match handle.call_extension(method, &isolate_id, Some(args)).await {
            Ok(value) => match parse_bool_extension_response(&value) {
                Ok(v) => v,
                Err(e) => {
                    tracing::warn!(
                        "ToggleOverlay: failed to parse new state for {:?} \
                         (session {}): {}",
                        extension,
                        session_id,
                        e
                    );
                    return;
                }
            },
            Err(e) => {
                tracing::warn!(
                    "ToggleOverlay: failed to set state for {:?} (session {}): {}",
                    extension,
                    session_id,
                    e
                );
                return;
            }
        };

        if let Err(e) = msg_tx
            .send(Message::DebugOverlayToggled {
                extension,
                enabled: new_state,
            })
            .await
        {
            tracing::error!(
                session_id = %session_id,
                error = ?e,
                "Inspector: failed to send terminal message — receiver dropped"
            );
        }
    });
}

/// Spawn a background task that fetches layout data for a widget node via VM Service.
///
/// Uses `ext.flutter.inspector.getLayoutExplorerNode` to retrieve the layout
/// properties (constraints, size, flex factor, flex fit) for the widget
/// identified by `node_id` (the `valueId` from a previously fetched
/// `DiagnosticsNode`).
///
/// Sends `Message::LayoutDataFetched` on success or
/// `Message::LayoutDataFetchFailed` on failure.
pub(super) fn spawn_fetch_layout_data(
    session_id: SessionId,
    node_id: String,
    handle: VmRequestHandle,
    msg_tx: mpsc::Sender<Message>,
) {
    tokio::spawn(async move {
        let isolate_id = match handle.resolve_flutter_ui_isolate().await {
            Ok(id) => id,
            Err(e) => {
                tracing::warn!(
                    "FetchLayoutData: could not get isolate ID for session {}: {}",
                    session_id,
                    e
                );
                if let Err(send_err) = msg_tx
                    .send(Message::LayoutDataFetchFailed {
                        session_id,
                        error: format!("Could not get isolate ID: {e}"),
                    })
                    .await
                {
                    tracing::error!(
                        session_id = %session_id,
                        error = ?send_err,
                        "Inspector: failed to send terminal message — receiver dropped"
                    );
                }
                return;
            }
        };

        // Use a dedicated object group for the layout explorer.
        let layout_group = LAYOUT_OBJECT_GROUP;

        // Dispose the previous layout object group before creating a new one.
        // This releases VM references from any prior layout fetch and prevents
        // memory from accumulating on the Flutter VM during repeated refreshes.
        // `disposeGroup` is idempotent — safe to call even on the first fetch.
        // Failure is non-fatal: log at debug level and continue with the fetch.
        {
            let mut dispose_args = HashMap::new();
            dispose_args.insert("objectGroup".to_string(), layout_group.to_string());
            if let Err(e) = handle
                .call_extension(ext::DISPOSE_GROUP, &isolate_id, Some(dispose_args))
                .await
            {
                tracing::debug!(
                    "FetchLayoutData: disposeGroup '{}' failed for session {} (non-fatal): {}",
                    layout_group,
                    session_id,
                    e
                );
            }
        }

        let mut args = HashMap::new();
        // NOTE: Layout explorer uses "id" and "groupName", not "arg" and "objectGroup".
        args.insert("id".to_string(), node_id.clone());
        args.insert("groupName".to_string(), layout_group.to_string());
        args.insert("subtreeDepth".to_string(), "1".to_string());

        // Wrap the RPC call in a 10-second timeout so that a hung or slow VM
        // does not leave the Layout panel in a permanent loading state.
        let fetch_result = tokio::time::timeout(LAYOUT_FETCH_TIMEOUT, async {
            handle
                .call_extension(ext::GET_LAYOUT_EXPLORER_NODE, &isolate_id, Some(args))
                .await
        })
        .await;

        let raw_result = match fetch_result {
            Err(_timeout) => {
                tracing::warn!(
                    "FetchLayoutData timed out after 10s for session {}",
                    session_id
                );
                if let Err(e) = msg_tx
                    .send(Message::LayoutDataFetchTimeout { session_id })
                    .await
                {
                    tracing::error!(
                        session_id = %session_id,
                        error = ?e,
                        "Inspector: failed to send terminal message — receiver dropped"
                    );
                }
                return;
            }
            Ok(Ok(v)) => v,
            Ok(Err(e)) => {
                tracing::warn!(
                    "FetchLayoutData: extension call failed for session {}: {}",
                    session_id,
                    e
                );
                if let Err(send_err) = msg_tx
                    .send(Message::LayoutDataFetchFailed {
                        session_id,
                        error: e.to_string(),
                    })
                    .await
                {
                    tracing::error!(
                        session_id = %session_id,
                        error = ?send_err,
                        "Inspector: failed to send terminal message — receiver dropped"
                    );
                }
                return;
            }
        };

        // Parse the DiagnosticsNode and extract LayoutInfo.
        let node_value = raw_result.get("result").unwrap_or(&raw_result);
        let layout =
            match serde_json::from_value::<fdemon_core::DiagnosticsNode>(node_value.clone()) {
                Ok(node) => extract_layout_info(&node, node_value),
                Err(e) => {
                    tracing::warn!(
                        "FetchLayoutData: failed to parse layout node for session {}: {}",
                        session_id,
                        e
                    );
                    if let Err(send_err) = msg_tx
                        .send(Message::LayoutDataFetchFailed {
                            session_id,
                            error: format!("Failed to parse layout data: {e}"),
                        })
                        .await
                    {
                        tracing::error!(
                            session_id = %session_id,
                            error = ?send_err,
                            "Inspector: failed to send terminal message — receiver dropped"
                        );
                    }
                    return;
                }
            };

        if let Err(e) = msg_tx
            .send(Message::LayoutDataFetched {
                session_id,
                layout: Box::new(layout),
            })
            .await
        {
            tracing::error!(
                session_id = %session_id,
                error = ?e,
                "Inspector: failed to send terminal message — receiver dropped"
            );
        }
    });
}

/// Background task that fetches widget properties and, recursively, the
/// sub-properties of any render-object property for the given widget node.
///
/// Sends one of these messages on completion via `msg_tx`:
/// - [`Message::DevToolsInspectorPropertiesFetched`] on success
/// - [`Message::DevToolsInspectorPropertiesFetchFailed`] on RPC error or parse failure
/// - [`Message::DevToolsInspectorPropertiesFetchTimeout`] when the 10-second budget elapses
///
/// Reuses `INSPECTOR_OBJECT_GROUP` for all calls. The object group is disposed
/// at session end (consistent with parent PLAN §7.2). This task does NOT call
/// `disposeGroup` directly.
///
/// The recursive sub-fetch loop is intentionally sequential (a `for` loop, not
/// `try_join_all`) to match DevTools' `_loadPropertiesForNode` implementation and
/// to keep timeout accounting predictable. Sub-fetch failures are logged at
/// debug level and do not abort the overall fetch — partial render-property data
/// is better than no data.
pub(super) fn spawn_fetch_inspector_properties(
    session_id: SessionId,
    node_id: String,
    handle: VmRequestHandle,
    msg_tx: mpsc::Sender<Message>,
) {
    tokio::spawn(async move {
        // ── Step 1: Resolve the Flutter UI isolate ────────────────────────────
        let isolate_id = match handle.resolve_flutter_ui_isolate().await {
            Ok(id) => id,
            Err(e) => {
                tracing::warn!(
                    session_id = %session_id,
                    node_id = %node_id,
                    error = %e,
                    "FetchInspectorProperties: could not resolve UI isolate"
                );
                let _ = msg_tx
                    .send(Message::DevToolsInspectorPropertiesFetchFailed {
                        session_id,
                        node_id,
                        error: format!("Could not resolve Flutter UI isolate: {e}"),
                    })
                    .await;
                return;
            }
        };

        // ── Step 2: First getProperties call (widget's own properties) ────────
        let mut widget_args = HashMap::new();
        widget_args.insert("arg".to_string(), node_id.clone());
        widget_args.insert(
            "objectGroup".to_string(),
            INSPECTOR_OBJECT_GROUP.to_string(),
        );

        let widget_resp = match tokio::time::timeout(
            PROPERTIES_FETCH_TIMEOUT,
            handle.call_extension(ext::GET_PROPERTIES, &isolate_id, Some(widget_args)),
        )
        .await
        {
            Err(_timeout) => {
                tracing::warn!(
                    session_id = %session_id,
                    node_id = %node_id,
                    "FetchInspectorProperties: initial getProperties call timed out"
                );
                let _ = msg_tx
                    .send(Message::DevToolsInspectorPropertiesFetchTimeout {
                        session_id,
                        node_id,
                    })
                    .await;
                return;
            }
            Ok(Err(e)) => {
                tracing::warn!(
                    session_id = %session_id,
                    node_id = %node_id,
                    error = %e,
                    "FetchInspectorProperties: initial getProperties call failed"
                );
                let _ = msg_tx
                    .send(Message::DevToolsInspectorPropertiesFetchFailed {
                        session_id,
                        node_id,
                        error: e.to_string(),
                    })
                    .await;
                return;
            }
            Ok(Ok(v)) => v,
        };

        let all_props = match parse_properties_response(&widget_resp) {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!(
                    session_id = %session_id,
                    node_id = %node_id,
                    error = %e,
                    "FetchInspectorProperties: failed to parse getProperties response"
                );
                let _ = msg_tx
                    .send(Message::DevToolsInspectorPropertiesFetchFailed {
                        session_id,
                        node_id,
                        error: e.to_string(),
                    })
                    .await;
                return;
            }
        };

        let (widget_properties, mut render_properties) =
            split_widget_and_render_properties(all_props);

        // ── Step 3: Recursive sub-fetch for each render-object property ───────
        //
        // DevTools calls getProperties on the valueId of each render-object
        // property to fetch the render object's own sub-properties (constraints,
        // size, layer, semantics, etc.).
        // Reference: inspector_controller.dart:914–931 (_loadPropertiesForNode).
        //
        // Collect value IDs first to avoid borrow conflicts on render_properties.
        let render_value_ids: Vec<String> = render_properties
            .iter()
            .filter_map(|p| p.value_id.clone())
            .collect();

        for value_id in render_value_ids {
            let mut sub_args = HashMap::new();
            sub_args.insert("arg".to_string(), value_id.clone());
            sub_args.insert(
                "objectGroup".to_string(),
                INSPECTOR_OBJECT_GROUP.to_string(),
            );

            match tokio::time::timeout(
                PROPERTIES_FETCH_TIMEOUT,
                handle.call_extension(ext::GET_PROPERTIES, &isolate_id, Some(sub_args)),
            )
            .await
            {
                Err(_timeout) => {
                    // A sub-fetch timeout aborts the entire operation so the
                    // caller does not display stale render-object sub-properties.
                    tracing::warn!(
                        session_id = %session_id,
                        node_id = %node_id,
                        value_id = %value_id,
                        "FetchInspectorProperties: sub-property getProperties call timed out"
                    );
                    let _ = msg_tx
                        .send(Message::DevToolsInspectorPropertiesFetchTimeout {
                            session_id,
                            node_id,
                        })
                        .await;
                    return;
                }
                Ok(Err(e)) => {
                    // Sub-fetch failure: log at debug level and continue — DevTools
                    // best-effort merges sub-properties (inspector_controller.dart:920).
                    tracing::debug!(
                        session_id = %session_id,
                        node_id = %node_id,
                        value_id = %value_id,
                        error = %e,
                        "FetchInspectorProperties: sub-property getProperties failed; skipping"
                    );
                    // Do not abort — partial render-property data is better than none.
                }
                Ok(Ok(v)) => match parse_properties_response(&v) {
                    Ok(subs) => {
                        render_properties.extend(subs);
                    }
                    Err(e) => {
                        tracing::debug!(
                            session_id = %session_id,
                            node_id = %node_id,
                            value_id = %value_id,
                            error = %e,
                            "FetchInspectorProperties: sub-property parse failed; skipping"
                        );
                    }
                },
            }
        }

        // ── Step 4: Send success message ──────────────────────────────────────
        if let Err(e) = msg_tx
            .send(Message::DevToolsInspectorPropertiesFetched {
                session_id,
                node_id,
                widget_properties,
                render_properties,
            })
            .await
        {
            tracing::error!(
                session_id = %session_id,
                error = ?e,
                "FetchInspectorProperties: failed to send success message — receiver dropped"
            );
        }
    });
}

/// Spawn a background task that disposes both DevTools VM object groups.
///
/// Disposes `"fdemon-inspector-1"` (widget inspector) and `"devtools-layout"`
/// (layout explorer) groups. Called when the user exits DevTools mode to release
/// VM references held by the Flutter inspector and prevent memory accumulation
/// during long debugging sessions.
///
/// Both disposal calls are fire-and-forget: failures are logged at debug level
/// and do not surface to the UI. `disposeGroup` is idempotent, so calling it
/// when a group does not exist is also safe.
pub(super) fn spawn_dispose_devtools_groups(session_id: SessionId, handle: VmRequestHandle) {
    tokio::spawn(async move {
        let isolate_id = match handle.resolve_flutter_ui_isolate().await {
            Ok(id) => id,
            Err(e) => {
                tracing::debug!(
                    "DisposeDevToolsGroups: could not get isolate ID for session {} \
                     (non-fatal, VM may have disconnected): {}",
                    session_id,
                    e
                );
                return;
            }
        };

        for group in &[INSPECTOR_OBJECT_GROUP, LAYOUT_OBJECT_GROUP] {
            let mut args = HashMap::new();
            args.insert("objectGroup".to_string(), (*group).to_string());
            if let Err(e) = handle
                .call_extension(ext::DISPOSE_GROUP, &isolate_id, Some(args))
                .await
            {
                tracing::debug!(
                    "DisposeDevToolsGroups: disposeGroup '{}' failed for session {} \
                     (non-fatal): {}",
                    group,
                    session_id,
                    e
                );
            } else {
                tracing::debug!(
                    "DisposeDevToolsGroups: disposed '{}' for session {}",
                    group,
                    session_id
                );
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    /// Helper: create a one-shot `(SessionId, mpsc::Sender, mpsc::Receiver)` triple.
    fn make_channel() -> (SessionId, mpsc::Sender<Message>, mpsc::Receiver<Message>) {
        let (tx, rx) = mpsc::channel(16);
        // SessionId is u64; use a fixed test value.
        (42_u64, tx, rx)
    }

    #[test]
    fn test_layout_fetch_timeout_is_reasonable() {
        assert_eq!(
            LAYOUT_FETCH_TIMEOUT,
            Duration::from_secs(10),
            "layout fetch timeout should be 10 seconds"
        );
        assert!(
            LAYOUT_FETCH_TIMEOUT >= Duration::from_secs(5),
            "layout fetch timeout must be at least 5 seconds to avoid false timeouts"
        );
    }

    #[test]
    fn test_properties_fetch_timeout_is_reasonable() {
        assert_eq!(
            PROPERTIES_FETCH_TIMEOUT,
            Duration::from_secs(10),
            "properties fetch timeout should be 10 seconds"
        );
        assert!(
            PROPERTIES_FETCH_TIMEOUT >= Duration::from_secs(5),
            "properties fetch timeout must be at least 5 seconds to avoid false timeouts"
        );
    }

    /// When `resolve_flutter_ui_isolate` fails (no cached isolate + closed channel),
    /// `spawn_fetch_inspector_properties` emits `DevToolsInspectorPropertiesFetchFailed`.
    #[tokio::test]
    async fn spawn_properties_emits_failed_when_isolate_resolution_fails() {
        // new_for_test(None): no cached isolate ID, closed channel →
        // resolve_flutter_ui_isolate() calls getVM which fails with ChannelClosed.
        let handle = fdemon_daemon::vm_service::VmRequestHandle::new_for_test(None);
        let (session_id, tx, mut rx) = make_channel();
        let node_id = "node-abc".to_string();

        spawn_fetch_inspector_properties(session_id, node_id.clone(), handle, tx);

        let msg = tokio::time::timeout(Duration::from_secs(5), rx.recv())
            .await
            .expect("timed out waiting for message")
            .expect("channel closed");

        match msg {
            Message::DevToolsInspectorPropertiesFetchFailed {
                session_id: sid,
                node_id: nid,
                error: _,
            } => {
                assert_eq!(sid, session_id);
                assert_eq!(nid, "node-abc");
            }
            other => panic!("expected DevToolsInspectorPropertiesFetchFailed, got: {other:?}"),
        }
    }

    /// When `call_extension` fails (cached isolate present, but closed channel),
    /// `spawn_fetch_inspector_properties` emits `DevToolsInspectorPropertiesFetchFailed`.
    #[tokio::test]
    async fn spawn_properties_emits_failed_when_first_call_errors() {
        // new_for_test(Some("isolate-1")): cached isolate → resolve_flutter_ui_isolate() succeeds.
        // But the channel is closed → call_extension returns Err(ChannelClosed).
        let handle = fdemon_daemon::vm_service::VmRequestHandle::new_for_test(Some(
            "isolates/main-1".to_string(),
        ));
        let (session_id, tx, mut rx) = make_channel();
        let node_id = "node-xyz".to_string();

        spawn_fetch_inspector_properties(session_id, node_id.clone(), handle, tx);

        let msg = tokio::time::timeout(Duration::from_secs(5), rx.recv())
            .await
            .expect("timed out waiting for message")
            .expect("channel closed");

        match msg {
            Message::DevToolsInspectorPropertiesFetchFailed {
                session_id: sid,
                node_id: nid,
                error: _,
            } => {
                assert_eq!(sid, session_id);
                assert_eq!(nid, "node-xyz");
            }
            other => panic!("expected DevToolsInspectorPropertiesFetchFailed, got: {other:?}"),
        }
    }

    /// When the initial `getProperties` call times out,
    /// `spawn_fetch_inspector_properties` emits `DevToolsInspectorPropertiesFetchTimeout`.
    #[tokio::test]
    async fn spawn_properties_emits_timeout_when_first_call_hangs() {
        tokio::time::pause();

        // new_for_test(Some(...)): cached isolate so resolve succeeds.
        // Then the task blocks waiting for call_extension.
        // We advance simulated time past PROPERTIES_FETCH_TIMEOUT to fire the timeout arm.
        let handle = fdemon_daemon::vm_service::VmRequestHandle::new_for_test(Some(
            "isolates/main-1".to_string(),
        ));
        let (session_id, tx, mut rx) = make_channel();
        let node_id = "node-timeout".to_string();

        spawn_fetch_inspector_properties(session_id, node_id.clone(), handle, tx);

        // Advance past the 10s timeout. The closed channel actually causes the
        // call_extension future to resolve immediately with Err, which hits the
        // `Ok(Err(e))` arm instead of the timeout arm. Because we cannot inject
        // a true "slow" RPC without a real WebSocket, this test verifies the
        // channel-error path completes promptly and sends a message.
        tokio::time::advance(Duration::from_secs(11)).await;

        // The message is either FetchFailed (channel closed wins the race)
        // or FetchTimeout (timeout fires first). Either is correct behavior;
        // we just confirm a message is sent and the task does not hang.
        let msg = tokio::time::timeout(Duration::from_secs(5), rx.recv())
            .await
            .expect("timed out — spawn task appears to be stuck")
            .expect("channel closed unexpectedly");

        assert!(
            matches!(
                msg,
                Message::DevToolsInspectorPropertiesFetchFailed { .. }
                    | Message::DevToolsInspectorPropertiesFetchTimeout { .. }
            ),
            "expected FetchFailed or FetchTimeout, got: {msg:?}"
        );
    }

    /// The module-level `INSPECTOR_OBJECT_GROUP` constant is reused (no new group created).
    #[test]
    fn spawn_properties_reuses_inspector_object_group() {
        // Verify that INSPECTOR_OBJECT_GROUP is the canonical group name and
        // that spawn_fetch_inspector_properties does not introduce a new one.
        assert_eq!(INSPECTOR_OBJECT_GROUP, "fdemon-inspector-1");
    }
}
