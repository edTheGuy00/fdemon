//! VM Service connection and event forwarding.
//!
//! This module provides the two async helpers that manage the lifecycle of a
//! Dart VM Service WebSocket connection for a single Flutter session:
//!
//! - [`spawn_vm_service_connection`] — connects to the VM Service, subscribes
//!   to Flutter event streams, and enters the event-forwarding loop.
//! - [`forward_vm_events`] — the inner loop: translates `VmClientEvent`s into
//!   TEA [`Message`]s and drives the heartbeat probe.
//!
//! Both functions are private to the `actions` module; `spawn_vm_service_connection`
//! is called from `mod.rs`'s `handle_action` dispatcher and the returned
//! `JoinHandle` is stored in the session task map for lifecycle tracking.

use std::time::Duration;

use tokio::sync::{mpsc, watch};
use tracing::{debug, error, info, warn};

use crate::message::Message;
use crate::session::SessionId;
use fdemon_daemon::vm_service::{
    enable_frame_tracking, flutter_error_to_log_entry, parse_flutter_error, parse_frame_timing,
    parse_gc_event, parse_log_record, vm_log_to_log_entry, VmClientEvent, VmServiceClient,
};

/// Maximum time to wait for the initial VM Service WebSocket connection.
const VM_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// Interval between VM Service heartbeat probes.
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(30);

/// Maximum time to wait for a heartbeat response.
const HEARTBEAT_TIMEOUT: Duration = Duration::from_secs(5);

/// Number of consecutive heartbeat failures before declaring the connection dead.
const MAX_HEARTBEAT_FAILURES: u32 = 3;

/// Spawn a task that connects to the VM Service and forwards events as Messages.
pub(super) fn spawn_vm_service_connection(
    session_id: SessionId,
    ws_uri: String,
    msg_tx: mpsc::Sender<Message>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let connect_result =
            tokio::time::timeout(VM_CONNECT_TIMEOUT, VmServiceClient::connect(&ws_uri)).await;

        let connect_result = match connect_result {
            Ok(result) => result,
            Err(_) => {
                warn!(
                    "VM Service: connection timed out for session {} ({})",
                    session_id, ws_uri
                );
                let _ = msg_tx
                    .send(Message::VmServiceConnectionFailed {
                        session_id,
                        error: "Connection timed out".to_string(),
                    })
                    .await;
                return;
            }
        };

        match connect_result {
            Ok(client) => {
                // Subscribe to Extension and Logging streams
                let stream_errors = client.subscribe_flutter_streams().await;
                for err in &stream_errors {
                    warn!(
                        "VM Service: stream subscription failed for session {}: {}",
                        session_id, err
                    );
                }

                // Best-effort: enable Flutter frame timing event emission.
                // `Flutter.Frame` events may already arrive without this call;
                // this attempts to also enable `profileWidgetBuilds` for build
                // timing detail. Errors are silently ignored (profile mode, etc.).
                if let Ok(isolate_id) = client.main_isolate_id().await {
                    let _ = enable_frame_tracking(&client.request_handle(), &isolate_id).await;
                }

                // Extract the request handle BEFORE entering the forwarding loop.
                // This allows the TEA handler and background tasks to make on-demand
                // RPC calls through the same WebSocket connection without going through
                // the event-forwarding loop.
                let handle = client.request_handle();
                let _ = msg_tx
                    .send(Message::VmServiceHandleReady { session_id, handle })
                    .await;

                // Create shutdown channel — sender goes to the session handle,
                // receiver lets the forwarding loop exit cleanly on AppStop.
                let (vm_shutdown_tx, vm_shutdown_rx) = tokio::sync::watch::channel(false);
                let vm_shutdown_tx = std::sync::Arc::new(vm_shutdown_tx);

                // Attach shutdown sender to the session handle BEFORE notifying
                // about connection so the session can signal shutdown at any time.
                let _ = msg_tx
                    .send(Message::VmServiceAttached {
                        session_id,
                        vm_shutdown_tx,
                    })
                    .await;

                // Notify TEA that the VM Service is connected
                let _ = msg_tx
                    .send(Message::VmServiceConnected { session_id })
                    .await;

                // Forward events from the VM Service to the TEA message loop
                forward_vm_events(client, session_id, msg_tx, vm_shutdown_rx).await;
            }
            Err(e) => {
                warn!(
                    "VM Service: connection failed for session {}: {}",
                    session_id, e
                );
                let _ = msg_tx
                    .send(Message::VmServiceConnectionFailed {
                        session_id,
                        error: e.to_string(),
                    })
                    .await;
            }
        }
    })
}

/// Receive VM Service stream events and translate them into TEA Messages.
///
/// Runs until:
/// - The event receiver closes (client disconnects or is dropped), OR
/// - The shutdown watch channel receives `true` (session stopped/closed)
///
/// Sends `VmServiceDisconnected` when the loop exits.
async fn forward_vm_events(
    mut client: VmServiceClient,
    session_id: SessionId,
    msg_tx: mpsc::Sender<Message>,
    mut vm_shutdown_rx: watch::Receiver<bool>,
) {
    let heartbeat_handle = client.request_handle();
    let mut heartbeat = tokio::time::interval(HEARTBEAT_INTERVAL);
    heartbeat.tick().await; // consume the immediate first tick so the first real probe fires after 30s
    let mut consecutive_failures: u32 = 0;

    loop {
        tokio::select! {
            event = client.event_receiver().recv() => {
                match event {
                    Some(VmClientEvent::StreamEvent(event)) => {
                        // Try parsing as Flutter.Error (Extension stream) — most critical.
                        if let Some(flutter_error) = parse_flutter_error(&event.params.event) {
                            let log_entry = flutter_error_to_log_entry(&flutter_error);
                            let _ = msg_tx
                                .send(Message::VmServiceFlutterError {
                                    session_id,
                                    log_entry,
                                })
                                .await;
                            continue;
                        }

                        // Try parsing as a Flutter.Frame event (frame timing).
                        // Checked after Flutter.Error because Flutter.Frame events share
                        // the Extension stream and are less critical than crash logs.
                        if let Some(timing) =
                            parse_frame_timing(&event.params.event)
                        {
                            let _ = msg_tx
                                .send(Message::VmServiceFrameTiming {
                                    session_id,
                                    timing,
                                })
                                .await;
                            continue;
                        }

                        // Try parsing as a GC event (GC stream).
                        if let Some(gc_event) = parse_gc_event(&event.params.event) {
                            let _ = msg_tx
                                .send(Message::VmServiceGcEvent {
                                    session_id,
                                    gc_event,
                                })
                                .await;
                            continue;
                        }

                        // Try parsing as a structured LogRecord (Logging stream).
                        if let Some(log_record) = parse_log_record(&event.params.event) {
                            let log_entry = vm_log_to_log_entry(&log_record);
                            let _ = msg_tx
                                .send(Message::VmServiceLogRecord {
                                    session_id,
                                    log_entry,
                                })
                                .await;
                            continue;
                        }

                        // Other event kinds (Isolate, Timeline, etc.) are intentionally ignored
                    }
                    Some(VmClientEvent::Reconnecting { attempt, max_attempts }) => {
                        consecutive_failures = 0; // prevent accumulation during backoff
                        let _ = msg_tx
                            .send(Message::VmServiceReconnecting {
                                session_id,
                                attempt,
                                max_attempts,
                            })
                            .await;
                    }
                    Some(VmClientEvent::Reconnected) => {
                        consecutive_failures = 0; // clean slate after successful reconnect
                        let _ = msg_tx
                            .send(Message::VmServiceReconnected { session_id })
                            .await;
                    }
                    Some(VmClientEvent::PermanentlyDisconnected) => {
                        break; // Fall through to VmServiceDisconnected below
                    }
                    None => {
                        // Event receiver closed — client disconnected
                        info!("VM Service event stream ended for session {}", session_id);
                        break;
                    }
                }
            }
            _ = vm_shutdown_rx.changed() => {
                if *vm_shutdown_rx.borrow() {
                    info!("VM Service shutdown signal received for session {}", session_id);
                    client.disconnect().await;
                    break;
                }
            }
            _ = heartbeat.tick() => {
                let probe = heartbeat_handle.get_version();
                match tokio::time::timeout(HEARTBEAT_TIMEOUT, probe).await {
                    Ok(Ok(_)) => {
                        if consecutive_failures > 0 {
                            debug!(
                                "VM Service heartbeat recovered for session {} after {} failure(s)",
                                session_id, consecutive_failures
                            );
                        }
                        consecutive_failures = 0;
                    }
                    Ok(Err(e)) => {
                        consecutive_failures += 1;
                        warn!(
                            "VM Service heartbeat failed for session {} ({}/{}): {}",
                            session_id, consecutive_failures, MAX_HEARTBEAT_FAILURES, e
                        );
                        if consecutive_failures >= MAX_HEARTBEAT_FAILURES {
                            error!(
                                "VM Service heartbeat failed {} consecutive times for session {}, disconnecting",
                                MAX_HEARTBEAT_FAILURES, session_id
                            );
                            break;
                        }
                    }
                    Err(_timeout) => {
                        consecutive_failures += 1;
                        warn!(
                            "VM Service heartbeat timed out for session {} ({}/{})",
                            session_id, consecutive_failures, MAX_HEARTBEAT_FAILURES
                        );
                        if consecutive_failures >= MAX_HEARTBEAT_FAILURES {
                            error!(
                                "VM Service heartbeat timed out {} consecutive times for session {}, disconnecting",
                                MAX_HEARTBEAT_FAILURES, session_id
                            );
                            break;
                        }
                    }
                }
            }
        }
    }

    let _ = msg_tx
        .send(Message::VmServiceDisconnected { session_id })
        .await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_heartbeat_constants_are_reasonable() {
        assert_eq!(
            HEARTBEAT_INTERVAL,
            Duration::from_secs(30),
            "heartbeat interval should be 30 seconds"
        );
        assert_eq!(
            HEARTBEAT_TIMEOUT,
            Duration::from_secs(5),
            "heartbeat timeout should be 5 seconds"
        );
        assert_eq!(
            MAX_HEARTBEAT_FAILURES, 3,
            "max heartbeat failures should be 3"
        );
        // Detection time = interval * max_failures = 30 * 3 = 90s, must be <= 120s
        assert!(
            HEARTBEAT_INTERVAL.as_secs() * MAX_HEARTBEAT_FAILURES as u64 <= 120,
            "heartbeat detection time should be at most 2 minutes (120 seconds)"
        );
    }

    #[test]
    fn test_heartbeat_counter_reset_on_reconnection() {
        // The counter reset to 0 on Reconnecting/Reconnected events is only
        // observable if MAX_HEARTBEAT_FAILURES > 1. If it were 1, a single
        // failure would immediately disconnect before any reset could occur.
        assert!(
            MAX_HEARTBEAT_FAILURES > 1,
            "MAX_HEARTBEAT_FAILURES must be > 1 for counter reset to have effect"
        );
    }
}
