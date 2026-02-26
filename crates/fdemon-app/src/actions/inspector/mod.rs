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

use crate::message::{DebugOverlayKind, Message};
use crate::session::SessionId;
use fdemon_daemon::vm_service::{
    ext, extract_layout_info, parse_bool_extension_response, VmRequestHandle,
};

/// Timeout for a single `getLayoutExplorerNode` RPC call.
const LAYOUT_FETCH_TIMEOUT: Duration = Duration::from_secs(10);

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
///    up to 8 times (500ms apart, 2s per-call timeout) before attempting the
///    tree fetch. A timed-out poll is treated as "not ready".
/// 2. **API fallback** — tries `getRootWidgetTree` first, falls back to
///    `getRootWidgetSummaryTree` on "method not found".
/// 3. **Configurable outer timeout** — `fetch_timeout_secs` (min 5s) wraps the
///    entire operation.
///
/// Sends `Message::WidgetTreeFetched` on success,
/// `Message::WidgetTreeFetchFailed` on error, or
/// `Message::WidgetTreeFetchTimeout` on timeout.
pub(super) fn spawn_fetch_widget_tree(
    session_id: SessionId,
    handle: VmRequestHandle,
    msg_tx: mpsc::Sender<Message>,
    tree_max_depth: u32,
    fetch_timeout_secs: u64,
) {
    tokio::spawn(async move {
        let timeout_dur = Duration::from_secs(fetch_timeout_secs.max(5));

        let fetch_result = tokio::time::timeout(timeout_dur, async {
            // Step 1: Get isolate ID.
            let isolate_id = handle
                .main_isolate_id()
                .await
                .map_err(|e| format!("Could not get isolate ID: {e}"))?;

            // Step 2: Poll widget tree readiness.
            widget_tree::poll_widget_tree_ready(&handle, &isolate_id, session_id).await;

            // Step 3: Dispose previous object group.
            let object_group = "fdemon-inspector-1";
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
            widget_tree::try_fetch_widget_tree(
                &handle,
                &isolate_id,
                object_group,
                tree_max_depth,
                session_id,
            )
            .await
            .map_err(|e| e.to_string())
        })
        .await;

        let msg = match fetch_result {
            Err(_timeout) => {
                tracing::warn!(
                    "FetchWidgetTree timed out after {}s for session {}",
                    fetch_timeout_secs.max(5),
                    session_id
                );
                Message::WidgetTreeFetchTimeout { session_id }
            }
            Ok(Ok(root)) => Message::WidgetTreeFetched {
                session_id,
                root: Box::new(root),
            },
            Ok(Err(error)) => {
                tracing::warn!(
                    "FetchWidgetTree failed for session {}: {}",
                    session_id,
                    error
                );
                Message::WidgetTreeFetchFailed { session_id, error }
            }
        };
        let _ = msg_tx.send(msg).await;
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
        let isolate_id = match handle.main_isolate_id().await {
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

        let _ = msg_tx
            .send(Message::DebugOverlayToggled {
                extension,
                enabled: new_state,
            })
            .await;
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
        let isolate_id = match handle.main_isolate_id().await {
            Ok(id) => id,
            Err(e) => {
                tracing::warn!(
                    "FetchLayoutData: could not get isolate ID for session {}: {}",
                    session_id,
                    e
                );
                let _ = msg_tx
                    .send(Message::LayoutDataFetchFailed {
                        session_id,
                        error: format!("Could not get isolate ID: {e}"),
                    })
                    .await;
                return;
            }
        };

        // Use a dedicated object group for the layout explorer.
        let layout_group = "devtools-layout";

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
                let _ = msg_tx
                    .send(Message::LayoutDataFetchTimeout { session_id })
                    .await;
                return;
            }
            Ok(Ok(v)) => v,
            Ok(Err(e)) => {
                tracing::warn!(
                    "FetchLayoutData: extension call failed for session {}: {}",
                    session_id,
                    e
                );
                let _ = msg_tx
                    .send(Message::LayoutDataFetchFailed {
                        session_id,
                        error: e.to_string(),
                    })
                    .await;
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
                    let _ = msg_tx
                        .send(Message::LayoutDataFetchFailed {
                            session_id,
                            error: format!("Failed to parse layout data: {e}"),
                        })
                        .await;
                    return;
                }
            };

        let _ = msg_tx
            .send(Message::LayoutDataFetched {
                session_id,
                layout: Box::new(layout),
            })
            .await;
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
        let isolate_id = match handle.main_isolate_id().await {
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

        for group in &["fdemon-inspector-1", "devtools-layout"] {
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
}
