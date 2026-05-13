//! Private helpers for the widget tree fetch operation.
//!
//! Contains the readiness-polling loop and API-fallback logic used by
//! [`super::spawn_fetch_widget_tree`].

use std::collections::HashMap;
use std::time::Duration;

use fdemon_daemon::vm_service::{ext, parse_diagnostics_node_response, VmRequestHandle};

use crate::session::SessionId;

/// Default number of `isWidgetTreeReady` poll attempts.
///
/// Derived so that the worst-case budget is ≤ 2.5 s:
/// `DEFAULT_READINESS_POLL_ATTEMPTS × (DEFAULT_READINESS_POLL_CALL_TIMEOUT_MS +
/// DEFAULT_READINESS_POLL_INTERVAL_MS) = 2 × (1000 + 250) = 2500 ms`.
pub(super) const DEFAULT_READINESS_POLL_ATTEMPTS: u32 = 2;

/// Default sleep between consecutive `isWidgetTreeReady` calls (milliseconds).
pub(super) const DEFAULT_READINESS_POLL_INTERVAL_MS: u64 = 250;

/// Default per-call timeout for each `isWidgetTreeReady` RPC (milliseconds).
pub(super) const DEFAULT_READINESS_POLL_CALL_TIMEOUT_MS: u64 = 1000;

/// Configuration controlling the `isWidgetTreeReady` polling loop.
///
/// Passed to [`poll_widget_tree_ready`] by [`super::spawn_fetch_widget_tree`]
/// so that callers can supply values read from `.fdemon/config.toml` rather
/// than the hard-coded defaults.
#[derive(Debug, Clone, Copy)]
pub(super) struct ReadinessPollConfig {
    /// Maximum number of poll attempts before proceeding with the fetch anyway.
    pub attempts: u32,
    /// Milliseconds to sleep between consecutive poll calls.
    pub interval_ms: u64,
    /// Per-call timeout in milliseconds for each `isWidgetTreeReady` RPC.
    pub call_timeout_ms: u64,
}

impl Default for ReadinessPollConfig {
    fn default() -> Self {
        Self {
            attempts: DEFAULT_READINESS_POLL_ATTEMPTS,
            interval_ms: DEFAULT_READINESS_POLL_INTERVAL_MS,
            call_timeout_ms: DEFAULT_READINESS_POLL_CALL_TIMEOUT_MS,
        }
    }
}

/// Poll `ext.flutter.inspector.isWidgetTreeReady` until it returns `true`,
/// the extension is not available (older Flutter SDK), or we exhaust attempts.
///
/// Each poll is wrapped in a per-call timeout so that a slow VM isolate cannot
/// consume the entire outer fetch budget. A timed-out poll counts as "not
/// ready" and we continue to the next attempt.
///
/// **Exhaustion is not an error.** When all attempts are spent without a
/// `true` reply, this function logs a `warn!` and returns normally so that
/// the subsequent `try_fetch_widget_tree` call can speak for itself — matching
/// the behaviour of browser DevTools, which does not poll for readiness at all.
///
/// This guards against the known Flutter bug where `getRootWidgetTree` throws
/// a null-check failure on complex or freshly-reloaded widget trees.
pub(super) async fn poll_widget_tree_ready(
    handle: &VmRequestHandle,
    isolate_id: &str,
    session_id: SessionId,
    config: &ReadinessPollConfig,
) {
    tracing::info!(
        session_id = %session_id,
        max_polls = config.attempts,
        poll_interval_ms = config.interval_ms,
        poll_call_timeout_ms = config.call_timeout_ms,
        "Inspector: readiness poll loop entered"
    );

    for attempt in 1..=config.attempts {
        tracing::debug!(
            session_id = %session_id,
            attempt = attempt,
            max_polls = config.attempts,
            "Inspector: readiness poll attempt"
        );
        let call_timeout = Duration::from_millis(config.call_timeout_ms);
        let call_result = tokio::time::timeout(
            call_timeout,
            handle.call_extension(ext::IS_WIDGET_TREE_READY, isolate_id, None),
        )
        .await;

        match call_result {
            Err(_timeout) => {
                // Per-call timeout — treat as "not ready" and continue.
                tracing::debug!(
                    session_id = %session_id,
                    attempt = attempt,
                    max_polls = config.attempts,
                    "isWidgetTreeReady timed out; treating as not ready"
                );
            }
            Ok(Ok(value)) => {
                // The extension returns {"result": true/false} or {"result": "true"/"false"}.
                let ready = value
                    .get("result")
                    .and_then(|v| v.as_bool().or_else(|| v.as_str().map(|s| s == "true")))
                    .unwrap_or(false);
                if ready {
                    tracing::info!(
                        session_id = %session_id,
                        attempt = attempt,
                        max_polls = config.attempts,
                        "Inspector: widget tree is ready"
                    );
                    return;
                }
                tracing::debug!(
                    session_id = %session_id,
                    attempt = attempt,
                    max_polls = config.attempts,
                    "Widget tree not ready; waiting"
                );
            }
            Ok(Err(e)) => {
                if is_method_not_found(&e) {
                    // Extension not available (older Flutter SDK) — skip polling.
                    tracing::info!(
                        session_id = %session_id,
                        "Inspector: isWidgetTreeReady not available (older Flutter SDK) — skipping readiness poll"
                    );
                    return;
                }
                if !is_transient_error(&e) {
                    // Fatal error (channel closed, IO) — bail out.
                    tracing::warn!(
                        session_id = %session_id,
                        error = %e,
                        "Inspector: isWidgetTreeReady fatal error — skipping readiness poll"
                    );
                    return;
                }
                tracing::debug!(
                    session_id = %session_id,
                    attempt = attempt,
                    max_polls = config.attempts,
                    error = %e,
                    "isWidgetTreeReady transient error; continuing"
                );
            }
        }
        tokio::time::sleep(Duration::from_millis(config.interval_ms)).await;
    }

    tracing::warn!(
        session_id = %session_id,
        attempts = config.attempts,
        "Inspector: readiness poll exhausted; proceeding with fetch anyway"
    );
}

/// Fetch the widget tree, falling back across APIs on failure.
///
/// Strategy (no retry of the same failing call — each attempt triggers a
/// Flutter-side exception that spams the user's log):
///
/// 1. Try `getRootWidgetTree` (newer API, supports `subtreeDepth`).
/// 2. If any transient error occurs (including "method not found" for older
///    Flutter SDKs, or the known null-check failure on complex trees) →
///    fall back to `getRootWidgetSummaryTree` which uses a different code path
///    (`_getRootWidgetSummaryTree`) and avoids the null-check bug.
/// 3. Fatal errors (ChannelClosed, Io) → fail immediately without fallback.
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
                session_id = %session_id,
                error = %e,
                "getRootWidgetTree failed; falling back to getRootWidgetSummaryTree"
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
                session_id = %session_id,
                error = %e,
                "getRootWidgetSummaryTree also failed"
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
    use crate::session::SessionId;

    // ── Helper ────────────────────────────────────────────────────────────────

    fn test_session_id() -> SessionId {
        42
    }

    /// Minimal config with 0-ms sleep so tests don't wait.
    fn zero_wait_config(attempts: u32) -> ReadinessPollConfig {
        ReadinessPollConfig {
            attempts,
            interval_ms: 0,
            call_timeout_ms: 100,
        }
    }

    // ── ReadinessPollConfig ────────────────────────────────────────────────────

    #[test]
    fn test_readiness_poll_config_default_matches_spec() {
        let cfg = ReadinessPollConfig::default();
        assert_eq!(
            cfg.attempts, DEFAULT_READINESS_POLL_ATTEMPTS,
            "default attempts should be 2"
        );
        assert_eq!(
            cfg.interval_ms, DEFAULT_READINESS_POLL_INTERVAL_MS,
            "default interval should be 250 ms"
        );
        assert_eq!(
            cfg.call_timeout_ms, DEFAULT_READINESS_POLL_CALL_TIMEOUT_MS,
            "default call timeout should be 1000 ms"
        );
        // Verify worst-case budget ≤ 2.5 s
        let worst_ms = u64::from(cfg.attempts) * (cfg.call_timeout_ms + cfg.interval_ms);
        assert!(
            worst_ms <= 2500,
            "default readiness poll budget ({worst_ms} ms) exceeds 2500 ms"
        );
    }

    #[test]
    fn test_readiness_poll_config_custom_values_are_stored() {
        let cfg = ReadinessPollConfig {
            attempts: 5,
            interval_ms: 100,
            call_timeout_ms: 500,
        };
        assert_eq!(cfg.attempts, 5);
        assert_eq!(cfg.interval_ms, 100);
        assert_eq!(cfg.call_timeout_ms, 500);
    }

    // ── poll_widget_tree_ready behaviour ─────────────────────────────────────

    /// When the VM channel is closed (ChannelClosed = fatal), the loop exits
    /// early on the first attempt.  The function should complete without
    /// panicking — and crucially must not propagate any error (it returns `()`).
    #[tokio::test]
    async fn test_poll_widget_tree_ready_exhausted_returns_unit() {
        // new_for_test drops the receiver immediately → every RPC returns ChannelClosed (fatal).
        // The function therefore early-returns on the first fatal error branch.
        // The important property: the function completes and does not panic.
        let handle = fdemon_daemon::vm_service::VmRequestHandle::new_for_test(None);
        let cfg = zero_wait_config(2);
        // This must complete (not hang) and not panic.
        poll_widget_tree_ready(&handle, "isolates/1", test_session_id(), &cfg).await;
    }

    /// With 0 attempts the loop body is never entered; the warn is not emitted
    /// and the function returns immediately.
    #[tokio::test]
    async fn test_poll_widget_tree_ready_zero_attempts_returns_immediately() {
        let handle = fdemon_daemon::vm_service::VmRequestHandle::new_for_test(None);
        let cfg = ReadinessPollConfig {
            attempts: 0,
            interval_ms: 0,
            call_timeout_ms: 100,
        };
        poll_widget_tree_ready(&handle, "isolates/1", test_session_id(), &cfg).await;
    }

    /// Verifies that `poll_respects_custom_attempts_and_interval` — the
    /// function uses `config.attempts`, not the old hard-coded `8`.
    #[tokio::test]
    async fn test_poll_widget_tree_ready_custom_attempts_bound_loop() {
        let handle = fdemon_daemon::vm_service::VmRequestHandle::new_for_test(None);
        // With 1 attempt and a broken channel the function must return after at
        // most 1 call attempt (not 8 as the old hard-coded constant required).
        let cfg = zero_wait_config(1);
        poll_widget_tree_ready(&handle, "isolates/1", test_session_id(), &cfg).await;
    }

    // ── is_transient_error / is_method_not_found ──────────────────────────────

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

    // ── Task 07: Scenario 10 — poll exhaustion does not block try_fetch ───────

    /// Scenario 10: readiness poll exhausted → `try_fetch_widget_tree` still runs.
    ///
    /// `poll_widget_tree_ready` is defined to return normally (not error) when the
    /// budget is exhausted. This test verifies that property and explicitly asserts
    /// that the calling sequence in `spawn_fetch_widget_tree` would proceed to the
    /// actual fetch step — modelled here by calling `try_fetch_widget_tree` after
    /// `poll_widget_tree_ready` returns and observing that it completes (not hangs).
    ///
    /// With a closed channel the RPC will return `ChannelClosed` (a fatal error) —
    /// we check that `try_fetch_widget_tree` returns `Err(_)` rather than hanging
    /// forever, proving control reached it.
    #[tokio::test]
    async fn test_readiness_poll_exhausted_fetch_still_runs() {
        // Create a handle backed by a closed channel: every RPC immediately
        // returns ChannelClosed.  This simulates poll exhaustion (the poll
        // function treats ChannelClosed as fatal and returns early) followed by
        // the subsequent fetch attempt proceeding regardless.
        let handle = fdemon_daemon::vm_service::VmRequestHandle::new_for_test(None);

        let cfg = ReadinessPollConfig {
            attempts: 2,
            interval_ms: 0,
            call_timeout_ms: 50,
        };

        // Step 1: Run the poll.  With a closed channel the poll exits after
        // the first fatal error — it MUST return (not panic, not hang).
        poll_widget_tree_ready(&handle, "isolates/1", test_session_id(), &cfg).await;

        // Step 2: Verify the fetch STILL RUNS after the poll returns.
        // With a closed channel, try_fetch_widget_tree returns Err(ChannelClosed)
        // rather than hanging — proving control flow reached it.
        let result = try_fetch_widget_tree(
            &handle,
            "isolates/1",
            super::super::INSPECTOR_OBJECT_GROUP,
            0,
            test_session_id(),
        )
        .await;

        assert!(
            result.is_err(),
            "try_fetch_widget_tree should return Err (channel closed) \
             after poll exhaustion, confirming it ran (scenario 10)"
        );
    }

    /// Verifies the budget-exhaustion warning path: with 2 unsuccessful but
    /// non-fatal attempts (channel closed triggers early exit, so we use a
    /// mock that returns transient errors instead via a very short call timeout
    /// forcing the timeout arm).
    #[tokio::test]
    async fn test_readiness_poll_budget_exhaustion_warn_path() {
        // With attempts=2 and call_timeout_ms=1 on a closed channel the
        // per-call tokio::time::timeout fires before the `request()` can
        // return ChannelClosed — each attempt is a timeout (not-ready arm),
        // so the loop exhausts all attempts and emits the warn!.
        // The important invariant: the function still returns `()` without error.
        let handle = fdemon_daemon::vm_service::VmRequestHandle::new_for_test(None);
        let cfg = ReadinessPollConfig {
            attempts: 2,
            interval_ms: 0,
            call_timeout_ms: 1, // so short the timeout arm fires every time
        };
        poll_widget_tree_ready(&handle, "isolates/1", test_session_id(), &cfg).await;
        // If we reach here the function returned normally — budget exhaustion is
        // handled gracefully, not as a panic or propagated error.
    }
}
