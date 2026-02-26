//! Private helpers for the widget tree fetch operation.
//!
//! Contains the readiness-polling loop and API-fallback logic used by
//! [`super::spawn_fetch_widget_tree`].

use std::collections::HashMap;
use std::time::Duration;

use fdemon_daemon::vm_service::{ext, parse_diagnostics_node_response, VmRequestHandle};

use crate::session::SessionId;

/// Poll `ext.flutter.inspector.isWidgetTreeReady` until it returns `true`,
/// the extension is not available (older Flutter SDK), or we exhaust attempts.
///
/// Each poll is wrapped in a 2-second timeout so that a slow VM isolate cannot
/// consume the entire outer fetch budget. A timed-out poll counts as "not
/// ready" and we continue to the next attempt.
///
/// This guards against the known Flutter bug where `getRootWidgetTree` throws
/// a null-check failure on complex or freshly-reloaded widget trees.
pub(super) async fn poll_widget_tree_ready(
    handle: &VmRequestHandle,
    isolate_id: &str,
    session_id: SessionId,
) {
    const MAX_POLLS: u32 = 8;
    const POLL_INTERVAL: Duration = Duration::from_millis(500);
    const POLL_CALL_TIMEOUT: Duration = Duration::from_secs(2);

    for attempt in 1..=MAX_POLLS {
        let call_result = tokio::time::timeout(
            POLL_CALL_TIMEOUT,
            handle.call_extension(ext::IS_WIDGET_TREE_READY, isolate_id, None),
        )
        .await;

        match call_result {
            Err(_timeout) => {
                // Per-call timeout — treat as "not ready" and continue.
                tracing::debug!(
                    "isWidgetTreeReady timed out for session {} (poll {}/{}), treating as not ready",
                    session_id,
                    attempt,
                    MAX_POLLS,
                );
            }
            Ok(Ok(value)) => {
                // The extension returns {"result": true/false} or {"result": "true"/"false"}.
                let ready = value
                    .get("result")
                    .and_then(|v| v.as_bool().or_else(|| v.as_str().map(|s| s == "true")))
                    .unwrap_or(false);
                if ready {
                    tracing::debug!(
                        "Widget tree ready for session {} (poll {}/{})",
                        session_id,
                        attempt,
                        MAX_POLLS,
                    );
                    return;
                }
                tracing::debug!(
                    "Widget tree not ready for session {} (poll {}/{}), waiting…",
                    session_id,
                    attempt,
                    MAX_POLLS,
                );
            }
            Ok(Err(e)) => {
                if is_method_not_found(&e) {
                    // Extension not available (older Flutter SDK) — skip polling.
                    tracing::debug!(
                        "isWidgetTreeReady not available for session {} — skipping readiness poll",
                        session_id,
                    );
                    return;
                }
                if !is_transient_error(&e) {
                    // Fatal error (channel closed, IO) — bail out.
                    tracing::debug!(
                        "isWidgetTreeReady fatal error for session {}: {} — skipping readiness poll",
                        session_id,
                        e,
                    );
                    return;
                }
                tracing::debug!(
                    "isWidgetTreeReady transient error for session {} (poll {}/{}): {}",
                    session_id,
                    attempt,
                    MAX_POLLS,
                    e,
                );
            }
        }
        tokio::time::sleep(POLL_INTERVAL).await;
    }

    tracing::debug!(
        "Widget tree readiness polls exhausted for session {} — proceeding anyway",
        session_id,
    );
}

/// Fetch the widget tree, falling back across APIs on failure.
///
/// Strategy (no retry of the same failing call — each attempt triggers a
/// Flutter-side exception that spams the user's log):
///
/// 1. Try `getRootWidgetTree` (newer API, supports `subtreeDepth`).
/// 2. If "method not found" → permanent fallback to `getRootWidgetSummaryTree`.
/// 3. If transient error (e.g. null-check failure on large trees) → fall back
///    to `getRootWidgetSummaryTree` which uses a different code path
///    (`_getRootWidgetSummaryTree`) and avoids the known null-check bug.
/// 4. Fatal errors (ChannelClosed, Io) → fail immediately.
pub(super) async fn try_fetch_widget_tree(
    handle: &VmRequestHandle,
    isolate_id: &str,
    object_group: &str,
    tree_max_depth: u32,
    session_id: SessionId,
) -> fdemon_core::Result<fdemon_core::widget_tree::DiagnosticsNode> {
    // --- Attempt 1: newer getRootWidgetTree ---
    let mut newer_args = HashMap::new();
    newer_args.insert("groupName".to_string(), object_group.to_string());
    newer_args.insert("isSummaryTree".to_string(), "true".to_string());
    newer_args.insert("withPreviews".to_string(), "true".to_string());
    if tree_max_depth > 0 {
        newer_args.insert("subtreeDepth".to_string(), tree_max_depth.to_string());
    }

    match handle
        .call_extension(ext::GET_ROOT_WIDGET_TREE, isolate_id, Some(newer_args))
        .await
    {
        Ok(value) => return parse_diagnostics_node_response(&value),
        Err(e) => {
            if !is_transient_error(&e) {
                // Fatal error (ChannelClosed, Io) — no fallback will help.
                return Err(e);
            }

            // Transient error — fall back to summary tree (different code path).
            // This covers both "method not found" (older Flutter) and the
            // null-check bug in _getRootWidgetTree on complex trees.
            tracing::debug!(
                "getRootWidgetTree failed for session {}, \
                 falling back to getRootWidgetSummaryTree: {}",
                session_id,
                e,
            );
        }
    }

    // --- Attempt 2: older getRootWidgetSummaryTree ---
    let mut older_args = HashMap::new();
    older_args.insert("objectGroup".to_string(), object_group.to_string());

    match handle
        .call_extension(
            ext::GET_ROOT_WIDGET_SUMMARY_TREE,
            isolate_id,
            Some(older_args),
        )
        .await
    {
        Ok(value) => parse_diagnostics_node_response(&value),
        Err(e) => {
            tracing::debug!(
                "getRootWidgetSummaryTree also failed for session {}: {}",
                session_id,
                e,
            );
            Err(e)
        }
    }
}

/// Returns `true` if an error is transient and the operation should be retried.
///
/// Protocol errors (like the known Flutter null-check failure) and generic
/// VmService errors are considered transient. Connection-level errors
/// (ChannelClosed, Io, ChannelSend) are fatal and should not be retried.
pub(super) fn is_transient_error(error: &fdemon_core::Error) -> bool {
    matches!(
        error,
        fdemon_core::Error::Protocol { .. } | fdemon_core::Error::VmService(_)
    )
}

/// Returns `true` if the error indicates "method not found" (extension not
/// registered). The VM Service error code `-32601` is embedded in the
/// `Protocol` message by `vm_error_to_error`.
pub(super) fn is_method_not_found(error: &fdemon_core::Error) -> bool {
    match error {
        fdemon_core::Error::Protocol { message } => {
            message.contains("-32601") || message.to_lowercase().contains("method not found")
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_transient_error_protocol() {
        let err = fdemon_core::Error::Protocol {
            message: "null check failure".into(),
        };
        assert!(is_transient_error(&err));
    }

    #[test]
    fn test_is_method_not_found_by_code() {
        let err = fdemon_core::Error::Protocol {
            message: "RPC error -32601: method not found".into(),
        };
        assert!(is_method_not_found(&err));
    }

    #[test]
    fn test_is_method_not_found_non_protocol() {
        let err = fdemon_core::Error::VmService("something".into());
        assert!(!is_method_not_found(&err));
    }
}
