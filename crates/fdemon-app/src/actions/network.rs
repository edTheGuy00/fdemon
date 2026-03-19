//! Network monitoring and HTTP profile actions for Flutter sessions.
//!
//! This module provides background tasks for:
//! - Periodic HTTP profile polling via the VM Service (`spawn_network_monitoring`)
//! - One-shot HTTP request detail fetching (`spawn_fetch_http_request_detail`)
//! - One-shot HTTP profile clearing (`spawn_clear_http_profile`)
//! - Cross-platform browser launch for opening DevTools (`open_url_in_browser`)
//!
//! All four entry points are called from `mod.rs`'s `handle_action` dispatcher
//! for the corresponding `UpdateAction` variants.
//!
//! **Polling strategy for `spawn_network_monitoring`:**
//! - Sends `VmServiceNetworkMonitoringStarted` with lifecycle handles on startup.
//! - Calls `ext.dart.io.httpEnableTimelineLogging(enabled: true)`. If the
//!   extension is absent (release mode), emits `VmServiceNetworkExtensionsUnavailable`
//!   and exits.
//! - Best-effort: enables socket profiling via `ext.dart.io.socketProfilingEnabled`.
//! - Polls `ext.dart.io.getHttpProfile` at `poll_interval_ms`
//!   (min [`NETWORK_POLL_MIN_MS`]), passing the previous response's `timestamp`
//!   as `updatedSince` for incremental updates.
//! - Exits when the shutdown channel receives `true` or `msg_tx` is closed.
//!
//! **Mode-aware scaling:**
//! In profile/release mode, the poll interval is scaled by
//! [`PROFILE_MODE_MULTIPLIER`] and clamped to [`PROFILE_NETWORK_POLL_MIN_MS`].
//! This reduces VM Service round-trip frequency while keeping network data
//! reasonably fresh. In debug mode, [`NETWORK_POLL_MIN_MS`] applies.

use std::process::Command;

use tokio::sync::mpsc;

use crate::config::FlutterMode;
use crate::message::Message;
use crate::session::SessionId;
use fdemon_daemon::vm_service::{network, VmRequestHandle};

/// Minimum network polling interval (500 ms) to avoid excessive VM Service calls.
pub(super) const NETWORK_POLL_MIN_MS: u64 = 500;

/// Multiplier applied to network poll interval in profile/release mode.
///
/// Network polling is less expensive than memory/alloc polling, but still adds
/// VM Service round-trip latency. A 3x multiplier is consistent with the
/// performance module's scaling.
///
/// Note: this could be made configurable via a `profile_polling_multiplier`
/// config key as a future follow-up. Hardcoded for now.
const PROFILE_MODE_MULTIPLIER: u64 = 3;

/// Minimum network poll interval in profile/release mode (ms).
///
/// Network polling is less expensive than memory/alloc polling,
/// but still adds VM Service round-trip latency.
const PROFILE_NETWORK_POLL_MIN_MS: u64 = 3000;

/// Compute the effective network poll interval for a given base value, considering
/// the current Flutter run mode.
///
/// In debug mode the interval is clamped to [`NETWORK_POLL_MIN_MS`] only.
/// In profile/release mode the interval is first clamped, then multiplied by
/// [`PROFILE_MODE_MULTIPLIER`], and finally clamped to [`PROFILE_NETWORK_POLL_MIN_MS`].
///
/// # Examples
///
/// ```text
/// // Debug: base_ms=1000  → 1000ms (above base minimum, no change)
/// // Profile: base_ms=1000 → max(1000*3, 3000) = 3000ms
/// // Profile: base_ms=500  → max(500*3, 3000)  = 3000ms  (profile minimum wins)
/// ```
fn effective_network_interval(base_ms: u64, mode: FlutterMode) -> u64 {
    let clamped = base_ms.max(NETWORK_POLL_MIN_MS);
    match mode {
        FlutterMode::Profile | FlutterMode::Release => {
            (clamped.saturating_mul(PROFILE_MODE_MULTIPLIER)).max(PROFILE_NETWORK_POLL_MIN_MS)
        }
        FlutterMode::Debug => clamped,
    }
}

/// Spawn the periodic HTTP-profile polling task for a session.
///
/// Creates a `watch::channel(false)` shutdown channel outside the spawned task
/// so that both the sender and the `JoinHandle` are available to package into
/// `VmServiceNetworkMonitoringStarted`. The TEA layer can then:
/// - Signal the task to stop by sending `true` on the shutdown channel, and
/// - Abort the task directly via the `JoinHandle` if needed.
///
/// The polling loop:
/// 1. Sends `VmServiceNetworkMonitoringStarted` (carries lifecycle handles).
/// 2. Calls `ext.dart.io.httpEnableTimelineLogging(enabled: true)`.
///    - If the extension is unavailable (release mode), sends
///      `VmServiceNetworkExtensionsUnavailable` and exits.
/// 3. Best-effort: enables socket profiling via `ext.dart.io.socketProfilingEnabled`.
/// 4. Polls `ext.dart.io.getHttpProfile` at `poll_interval_ms` (min 500ms in
///    debug, min 3000ms in profile/release after 3× scaling), passing the
///    previous response's `timestamp` as `updatedSince` for incremental updates.
/// 5. Exits when the shutdown channel receives `true` or `msg_tx` is closed.
pub(super) fn spawn_network_monitoring(
    session_id: SessionId,
    handle: VmRequestHandle,
    msg_tx: mpsc::Sender<Message>,
    poll_interval_ms: u64,
    mode: FlutterMode,
) {
    let poll_interval_ms = effective_network_interval(poll_interval_ms, mode);

    // Create the shutdown channel outside the task so both ends are available
    // before the task starts running.
    let (network_shutdown_tx, mut network_shutdown_rx) = tokio::sync::watch::channel(false);
    // Arc is required because Message derives Clone and watch::Sender does not impl Clone.
    let network_shutdown_tx = std::sync::Arc::new(network_shutdown_tx);

    // Create the pause channel outside the task. Initial value is `false` (active)
    // because the network task only starts when the user is already on the Network
    // tab — polling should begin immediately without a separate unpause signal.
    // Unlike perf_pause/alloc_pause which start paused (true).
    let (network_pause_tx, mut network_pause_rx) = tokio::sync::watch::channel(false);
    let network_pause_tx = std::sync::Arc::new(network_pause_tx);
    let network_pause_tx_for_msg = network_pause_tx.clone();

    // The JoinHandle from `tokio::spawn` is only available after the call, but
    // the task will read it from the slot when sending the "started" message.
    // We use `Arc<Mutex<Option<>>>` as a rendezvous — the slot is filled
    // synchronously (before any await) after spawn returns.
    let task_handle_slot: std::sync::Arc<std::sync::Mutex<Option<tokio::task::JoinHandle<()>>>> =
        std::sync::Arc::new(std::sync::Mutex::new(None));
    let task_handle_slot_for_msg = task_handle_slot.clone();

    let join_handle = tokio::spawn(async move {
        // Notify the TEA layer that monitoring has started, passing the lifecycle
        // handles so the session can store them for later cleanup. The slot is
        // populated synchronously by the caller before this first `.await` runs.
        if msg_tx
            .send(Message::VmServiceNetworkMonitoringStarted {
                session_id,
                network_shutdown_tx,
                network_task_handle: task_handle_slot_for_msg,
                network_pause_tx: network_pause_tx_for_msg,
            })
            .await
            .is_err()
        {
            // Engine shutting down.
            return;
        }

        // Obtain the main isolate ID (cached after the first successful call).
        let isolate_id = match handle.main_isolate_id().await {
            Ok(id) => id,
            Err(e) => {
                tracing::warn!(
                    "Network monitoring: could not get isolate ID for session {}: {}",
                    session_id,
                    e
                );
                return;
            }
        };

        // Step 1: Enable HTTP timeline logging so the VM starts recording requests.
        // If the extension is not available (release mode), inform the TEA layer
        // and exit — no point polling for data that will never arrive.
        match network::enable_http_timeline_logging_handle(&handle, &isolate_id, true).await {
            Ok(_) => {
                tracing::info!(
                    "Network monitoring: HTTP timeline logging enabled for session {}",
                    session_id
                );
            }
            Err(e) => {
                // `Error::Protocol` is the variant returned when the VM Service
                // reports a JSON-RPC error (-32601 "Method not found"), which
                // indicates the extension is not registered in release/profile mode.
                if matches!(e, fdemon_core::Error::Protocol { .. }) {
                    tracing::info!(
                        "Network monitoring: ext.dart.io extensions not available for \
                         session {} (release mode?): {}",
                        session_id,
                        e
                    );
                    let _ = msg_tx
                        .send(Message::VmServiceNetworkExtensionsUnavailable { session_id })
                        .await;
                    return;
                }
                // Other errors (channel closed, transient) — log and continue.
                // The polling loop will fail gracefully if the VM is gone.
                tracing::warn!(
                    "Network monitoring: failed to enable HTTP timeline logging for \
                     session {}: {}",
                    session_id,
                    e
                );
            }
        }

        // Step 2: Best-effort — enable socket profiling. Failure is non-fatal.
        if let Err(e) =
            network::set_socket_profiling_enabled_handle(&handle, &isolate_id, true).await
        {
            tracing::debug!(
                "Network monitoring: socket profiling unavailable for session {} \
                 (non-fatal): {}",
                session_id,
                e
            );
        }

        // Step 3: Start incremental polling loop.
        let mut poll_tick =
            tokio::time::interval(tokio::time::Duration::from_millis(poll_interval_ms));
        poll_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        // Track the last profile timestamp for incremental `updatedSince` polling.
        let mut last_timestamp: Option<i64> = None;

        loop {
            tokio::select! {
                _ = poll_tick.tick() => {
                    // Skip if network monitoring is paused (user is not on Network tab).
                    if *network_pause_rx.borrow() {
                        continue;
                    }
                    match network::get_http_profile_handle(&handle, &isolate_id, last_timestamp).await {
                        Ok(profile) => {
                            // Always update the timestamp so the next poll only returns new data.
                            last_timestamp = Some(profile.timestamp);
                            if !profile.requests.is_empty()
                                && msg_tx
                                    .send(Message::VmServiceHttpProfileReceived {
                                        session_id,
                                        timestamp: profile.timestamp,
                                        entries: profile.requests,
                                    })
                                    .await
                                    .is_err()
                            {
                                // Engine shutting down.
                                break;
                            }
                        }
                        Err(e) => {
                            // Transient errors (isolate paused during reload, etc.)
                            // are expected — log at debug level and retry next tick.
                            tracing::debug!(
                                "Network monitoring: HTTP profile poll failed for \
                                 session {} (non-fatal): {}",
                                session_id,
                                e
                            );
                        }
                    }
                }
                Ok(()) = network_pause_rx.changed() => {
                    // When the pause state changes to `false` (unpaused), fire an
                    // immediate getHttpProfile fetch so the Network tab shows any
                    // requests that arrived while the tab was hidden. Network data
                    // accumulates on the VM side during the pause and is retrieved
                    // in full on the next fetch.
                    if !*network_pause_rx.borrow() {
                        tracing::debug!(
                            "Network monitoring: unpaused for session {}, firing immediate fetch",
                            session_id
                        );
                        match network::get_http_profile_handle(&handle, &isolate_id, last_timestamp).await {
                            Ok(profile) => {
                                last_timestamp = Some(profile.timestamp);
                                if !profile.requests.is_empty()
                                    && msg_tx
                                        .send(Message::VmServiceHttpProfileReceived {
                                            session_id,
                                            timestamp: profile.timestamp,
                                            entries: profile.requests,
                                        })
                                        .await
                                        .is_err()
                                {
                                    // Engine shutting down.
                                    break;
                                }
                            }
                            Err(e) => {
                                tracing::debug!(
                                    "Network monitoring: immediate fetch on unpause failed for \
                                     session {} (non-fatal): {}",
                                    session_id,
                                    e
                                );
                            }
                        }
                    }
                }
                _ = network_shutdown_rx.changed() => {
                    if *network_shutdown_rx.borrow() {
                        tracing::info!(
                            "Network monitoring: shutdown signal received for session {}",
                            session_id
                        );
                        break;
                    }
                }
            }
        }
    });

    // Synchronously store the JoinHandle in the slot. The task hasn't run yet
    // (tokio tasks don't run until the current thread yields to the runtime),
    // so the slot is populated before the first `.await` inside the task.
    let _ = task_handle_slot
        .lock()
        .map(|mut slot| *slot = Some(join_handle));
}

/// Spawn a one-shot task that fetches full detail for a single HTTP request.
///
/// Uses `ext.dart.io.getHttpProfileRequest` to retrieve request/response
/// headers, bodies, timeline events, and connection info.
///
/// Sends `Message::VmServiceHttpRequestDetailReceived` on success or
/// `Message::VmServiceHttpRequestDetailFailed` on failure.
pub(super) fn spawn_fetch_http_request_detail(
    session_id: SessionId,
    request_id: String,
    handle: VmRequestHandle,
    msg_tx: mpsc::Sender<Message>,
) {
    tokio::spawn(async move {
        let isolate_id = match handle.main_isolate_id().await {
            Ok(id) => id,
            Err(e) => {
                tracing::warn!(
                    "FetchHttpRequestDetail: could not get isolate ID for session {}: {}",
                    session_id,
                    e
                );
                let _ = msg_tx
                    .send(Message::VmServiceHttpRequestDetailFailed {
                        session_id,
                        error: format!("Could not get isolate ID: {e}"),
                    })
                    .await;
                return;
            }
        };

        match network::get_http_profile_request_handle(&handle, &isolate_id, &request_id).await {
            Ok(detail) => {
                let _ = msg_tx
                    .send(Message::VmServiceHttpRequestDetailReceived {
                        session_id,
                        detail: Box::new(detail),
                    })
                    .await;
            }
            Err(e) => {
                tracing::debug!(
                    "FetchHttpRequestDetail: request detail fetch failed for session {}: {}",
                    session_id,
                    e
                );
                let _ = msg_tx
                    .send(Message::VmServiceHttpRequestDetailFailed {
                        session_id,
                        error: e.to_string(),
                    })
                    .await;
            }
        }
    });
}

/// Spawn a one-shot task that clears the VM-side HTTP profile.
///
/// Calls `ext.dart.io.clearHttpProfile`. The local `NetworkState` is cleared
/// immediately by the TEA handler; this action resets the VM's request history.
/// Fire-and-forget: errors are logged at warn level but do not propagate.
pub(super) fn spawn_clear_http_profile(session_id: SessionId, handle: VmRequestHandle) {
    tokio::spawn(async move {
        let isolate_id = match handle.main_isolate_id().await {
            Ok(id) => id,
            Err(e) => {
                tracing::warn!(
                    "ClearHttpProfile: could not get isolate ID for session {} \
                     (non-fatal, VM may have disconnected): {}",
                    session_id,
                    e
                );
                return;
            }
        };

        if let Err(e) = network::clear_http_profile_handle(&handle, &isolate_id).await {
            tracing::warn!(
                "ClearHttpProfile: failed to clear HTTP profile for session {} \
                 (non-fatal): {}",
                session_id,
                e
            );
        }
        // Fire-and-forget: the local NetworkState is already cleared by the TEA
        // handler that produced the ClearHttpProfile action (handle_clear_network_profile).
        // No follow-up message is needed — sending ClearNetworkProfile back would
        // re-trigger the handler and create an infinite loop.
    });
}

/// Open a URL in the system browser (cross-platform, fire-and-forget).
///
/// If `browser` is non-empty, uses it as the browser command.
/// Otherwise uses the platform-default browser opener.
///
/// Called from the `handle_action` dispatch for
/// [`crate::UpdateAction::OpenBrowserDevTools`].
pub(super) fn open_url_in_browser(url: &str, browser: &str) -> std::io::Result<()> {
    if !browser.is_empty() {
        // Custom browser specified in settings.
        Command::new(browser).arg(url).spawn()?;
        return Ok(());
    }

    // Platform-default browser.
    #[cfg(target_os = "macos")]
    {
        Command::new("open").arg(url).spawn()?;
        return Ok(());
    }

    #[cfg(target_os = "linux")]
    {
        Command::new("xdg-open").arg(url).spawn()?;
        return Ok(());
    }

    #[cfg(target_os = "windows")]
    {
        Command::new("cmd").args(["/C", "start", "", url]).spawn()?;
        return Ok(());
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        return Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "no browser opener available for this platform",
        ));
    }

    #[allow(unreachable_code)]
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_network_poll_min_ms_is_reasonable() {
        assert_eq!(
            NETWORK_POLL_MIN_MS, 500,
            "network poll minimum should be 500ms"
        );
    }

    #[test]
    fn test_profile_network_constants_are_reasonable() {
        assert_eq!(
            PROFILE_MODE_MULTIPLIER, 3,
            "profile multiplier should be 3x"
        );
        assert_eq!(
            PROFILE_NETWORK_POLL_MIN_MS, 3000,
            "profile network minimum should be 3000ms"
        );
        assert!(
            PROFILE_NETWORK_POLL_MIN_MS > NETWORK_POLL_MIN_MS,
            "profile network minimum must exceed debug minimum"
        );
    }

    #[test]
    fn test_debug_mode_uses_base_interval() {
        // Given poll_interval_ms = 1000 and mode = Debug
        // Then effective interval = 1000ms (above base minimum, no multiplier)
        let result = effective_network_interval(1000, FlutterMode::Debug);
        assert_eq!(result, 1000, "debug mode should not scale the interval");
    }

    #[test]
    fn test_debug_mode_clamps_to_base_minimum() {
        // Given poll_interval_ms = 100 and mode = Debug
        // Then effective interval = 500ms (clamped to base minimum)
        let result = effective_network_interval(100, FlutterMode::Debug);
        assert_eq!(result, 500, "debug mode should clamp to base minimum");
    }

    #[test]
    fn test_profile_network_interval_scales() {
        // Given network_poll_interval_ms = 1000 and mode = Profile
        // Then effective interval = max(1000 * 3, 3000) = 3000ms
        let result = effective_network_interval(1000, FlutterMode::Profile);
        assert_eq!(
            result, 3000,
            "profile mode should scale 1000ms to 3000ms (profile minimum)"
        );
    }

    #[test]
    fn test_profile_mode_network_from_aggressive_settings() {
        // Given network_poll_interval_ms = 500 and mode = Profile
        // Then effective interval = max(500 * 3, 3000) = 3000ms (profile minimum wins)
        let result = effective_network_interval(500, FlutterMode::Profile);
        assert_eq!(
            result, 3000,
            "profile mode with 500ms base should reach 3000ms minimum"
        );
    }

    #[test]
    fn test_profile_mode_network_respects_user_higher_interval() {
        // Given network_poll_interval_ms = 5000 and mode = Profile
        // Then effective interval = max(5000 * 3, 3000) = 15000ms
        let result = effective_network_interval(5000, FlutterMode::Profile);
        assert_eq!(
            result, 15_000,
            "profile mode should apply multiplier to user's high interval"
        );
    }

    #[test]
    fn test_release_mode_network_uses_same_scaling_as_profile() {
        // Release mode must produce identical results to Profile mode
        let profile_result = effective_network_interval(1000, FlutterMode::Profile);
        let release_result = effective_network_interval(1000, FlutterMode::Release);
        assert_eq!(
            profile_result, release_result,
            "release and profile should produce the same network interval"
        );
    }

    #[test]
    fn test_network_multiplier_applied_after_base_clamp() {
        // Verifies: clamp first, then multiply
        // Given poll_interval_ms = 100 (below base_min=500), mode = Profile
        // Step 1: clamp(100, 500) = 500
        // Step 2: 500 * 3 = 1500, then max(1500, 3000) = 3000
        let result = effective_network_interval(100, FlutterMode::Profile);
        assert_eq!(
            result, 3000,
            "multiplier should be applied after base clamp"
        );
    }
}
