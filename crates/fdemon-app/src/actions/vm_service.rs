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
use fdemon_daemon::vm_service::protocol::stream_id;
use fdemon_daemon::vm_service::{
    enable_frame_tracking, flutter_error_to_log_entry, flutter_extension_kind, parse_debug_event,
    parse_flutter_error, parse_frame_timing, parse_gc_event, parse_isolate_event, parse_log_record,
    redact_vm_service_token, vm_log_to_log_entry, VmClientEvent, VmServiceClient,
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
///
/// `rebuilt_widgets_gate_rx` controls whether `Flutter.RebuiltWidgets` events
/// are forwarded. When `true` the forwarder parses and dispatches the event;
/// when `false` it skips parsing entirely. The receiver is updated by the
/// TEA handler (via `SessionHandle::rebuilt_widgets_gate_tx`) whenever the
/// active DevTools panel changes. `None` means no gate is installed — the
/// forwarder will always skip `Flutter.RebuiltWidgets` (safe default for
/// the rare case where hydration was skipped, e.g. in unit tests that don't
/// call `process_message`).
pub(super) fn spawn_vm_service_connection(
    session_id: SessionId,
    ws_uri: String,
    msg_tx: mpsc::Sender<Message>,
    rebuilt_widgets_gate_rx: Option<tokio::sync::watch::Receiver<bool>>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let connect_result =
            tokio::time::timeout(VM_CONNECT_TIMEOUT, VmServiceClient::connect(&ws_uri)).await;

        let connect_result = match connect_result {
            Ok(result) => result,
            Err(_) => {
                warn!(
                    "VM Service: connection timed out for session {} ({})",
                    session_id,
                    redact_vm_service_token(&ws_uri)
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
                forward_vm_events(
                    client,
                    session_id,
                    msg_tx,
                    vm_shutdown_rx,
                    rebuilt_widgets_gate_rx,
                )
                .await;
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
/// The `rebuilt_widgets_gate_rx` receiver gates `Flutter.RebuiltWidgets` event
/// forwarding. When the current value is `false` (gate closed), the branch
/// returns early without parsing or allocating. When `None`, events are always
/// skipped (conservative default).
///
/// Sends `VmServiceDisconnected` when the loop exits.
async fn forward_vm_events(
    mut client: VmServiceClient,
    session_id: SessionId,
    msg_tx: mpsc::Sender<Message>,
    mut vm_shutdown_rx: watch::Receiver<bool>,
    rebuilt_widgets_gate_rx: Option<tokio::sync::watch::Receiver<bool>>,
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

                        // Try parsing as Flutter.RebuiltWidgets (Phase 3 rebuild stats).
                        // Placed before Flutter.Frame because both share the Extension stream
                        // and rebuild events are more expensive to miss than frame timing.
                        if let Some("Flutter.RebuiltWidgets") =
                            flutter_extension_kind(&event.params.event)
                        {
                            // H1 — Panel gate: skip parsing entirely when Performance is not
                            // the active panel. The gate receiver is `true` when forwarding
                            // should proceed and `false` (or absent) when it should be skipped.
                            // This eliminates ~60 fps allocation and dispatch churn when the
                            // user is viewing Logs, Inspector, Memory, or Network.
                            let gate_open = rebuilt_widgets_gate_rx
                                .as_ref()
                                .map(|rx| *rx.borrow())
                                .unwrap_or(false);
                            if !gate_open {
                                continue;
                            }

                            if let Some(ext_data) =
                                event.params.event.data.get("extensionData")
                            {
                                match fdemon_core::rebuild_stats::parse_rebuilt_widgets_event(
                                    ext_data,
                                ) {
                                    Ok(payload) => {
                                        // L10 — Non-blocking send: replace .send().await with
                                        // try_send to avoid head-of-line blocking other events
                                        // (Flutter.Frame, errors) when the handler is slow.
                                        let frame_number = payload.frame_number;
                                        match msg_tx.try_send(Message::RebuildStatsEventReceived {
                                            session_id,
                                            payload,
                                        }) {
                                            Ok(()) => {}
                                            Err(tokio::sync::mpsc::error::TrySendError::Full(
                                                _,
                                            )) => {
                                                tracing::debug!(
                                                    "Flutter.RebuiltWidgets: channel full, \
                                                     dropping frame {} for session {}",
                                                    frame_number,
                                                    session_id
                                                );
                                            }
                                            Err(tokio::sync::mpsc::error::TrySendError::Closed(
                                                _,
                                            )) => {
                                                tracing::error!(
                                                    "Flutter.RebuiltWidgets: message channel \
                                                     closed for session {} — exiting forwarder",
                                                    session_id
                                                );
                                                break;
                                            }
                                        }
                                    }
                                    // L3 — Parse-error log level downgrade from warn! to debug!
                                    // to prevent log flooding at 60 fps in pathological cases.
                                    // The panel gate further bounds this: parse errors only occur
                                    // when the user is actively viewing the Performance panel.
                                    Err(e) => {
                                        tracing::debug!(
                                            "Failed to parse Flutter.RebuiltWidgets: {}",
                                            e
                                        );
                                    }
                                }
                            }
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

                        // Route Debug stream events (breakpoints, pause, resume, etc.).
                        // Checked by stream_id so we only attempt parsing on the correct stream.
                        if event.params.stream_id == stream_id::DEBUG {
                            if let Some(debug_event) = parse_debug_event(&event.params.event) {
                                let _ = msg_tx
                                    .send(Message::VmServiceDebugEvent {
                                        session_id,
                                        event: debug_event,
                                    })
                                    .await;
                            } else {
                                tracing::debug!(
                                    "Debug stream: unrecognized or malformed event kind '{}'",
                                    event.params.event.kind
                                );
                            }
                            continue;
                        }

                        // Route Isolate stream events (isolate lifecycle).
                        // Checked by stream_id so we only attempt parsing on the correct stream.
                        if event.params.stream_id == stream_id::ISOLATE {
                            if let Some(isolate_event) = parse_isolate_event(&event.params.event) {
                                let _ = msg_tx
                                    .send(Message::VmServiceIsolateEvent {
                                        session_id,
                                        event: isolate_event,
                                    })
                                    .await;
                            } else {
                                tracing::debug!(
                                    "Isolate stream: unrecognized or malformed event kind '{}'",
                                    event.params.event.kind
                                );
                            }
                            continue;
                        }

                        // Other event kinds (Timeline, etc.) are intentionally ignored
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

    // FIXME: see clippy-rust-191-cleanup — asserts constant invariant that
    // MAX_HEARTBEAT_FAILURES (3) is > 1, required for the counter-reset logic to be observable.
    #[allow(clippy::assertions_on_constants)]
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

    // ── Flutter.RebuiltWidgets routing ────────────────────────────────────────

    /// Verify that `flutter_extension_kind` correctly identifies `Flutter.RebuiltWidgets`
    /// from the event data, which is the discriminator used in `forward_vm_events`.
    #[test]
    fn forward_vm_events_routes_rebuilt_widgets_discriminator() {
        use fdemon_daemon::vm_service::protocol::StreamEvent;
        use serde_json::json;

        // Build a StreamEvent whose `data` field contains extensionKind.
        let rebuilt_widgets_event_data = json!({
            "kind": "Extension",
            "extensionKind": "Flutter.RebuiltWidgets",
            "extensionData": {
                "frameNumber": 42,
                "startTime": 12345,
                "events": [1, 1, 2, 3]
            }
        });

        // Deserialize as a StreamEvent (we use serde).
        let stream_event: StreamEvent =
            serde_json::from_value(rebuilt_widgets_event_data).expect("should parse StreamEvent");

        // Verify the discriminator matches what forward_vm_events checks.
        let kind = flutter_extension_kind(&stream_event);
        assert_eq!(kind, Some("Flutter.RebuiltWidgets"));

        // Verify that parse_rebuilt_widgets_event can parse the extensionData.
        let ext_data = stream_event
            .data
            .get("extensionData")
            .expect("extensionData should exist");
        let payload = fdemon_core::rebuild_stats::parse_rebuilt_widgets_event(ext_data)
            .expect("should parse RebuildEventPayload");
        assert_eq!(payload.frame_number, 42);
        assert_eq!(payload.start_time_micros, 12345);
        assert_eq!(payload.events, vec![(1, 1), (2, 3)]);
    }

    /// Verify that non-RebuiltWidgets extension events do NOT match the discriminator.
    #[test]
    fn forward_vm_events_does_not_route_frame_event_as_rebuilt_widgets() {
        use fdemon_daemon::vm_service::protocol::StreamEvent;
        use serde_json::json;

        let frame_event_data = json!({
            "kind": "Extension",
            "extensionKind": "Flutter.Frame",
            "extensionData": { "number": 10 }
        });

        let stream_event: StreamEvent =
            serde_json::from_value(frame_event_data).expect("should parse StreamEvent");

        let kind = flutter_extension_kind(&stream_event);
        assert_ne!(kind, Some("Flutter.RebuiltWidgets"));
        assert_eq!(kind, Some("Flutter.Frame"));
    }

    // ── Panel gate tests (H1) ─────────────────────────────────────────────────

    /// Assert that `Flutter.RebuiltWidgets` events are NOT dispatched when the
    /// panel gate is closed (false). This exercises the panel-gate branch that
    /// was introduced to eliminate ~60 fps parsing churn while the user is
    /// viewing Inspector, Memory, Network, or Logs.
    ///
    /// We test the gate contract in isolation: when the receiver holds `false`,
    /// the `gate_open` check must be `false` and no `RebuildStatsEventReceived`
    /// should be emitted.
    #[test]
    fn test_rebuilt_widgets_event_skipped_when_panel_not_performance() {
        // Simulate gate states for non-Performance panels: Inspector, Memory,
        // Network, and the "None" case (no receiver installed).
        let gate_states: &[(Option<bool>, &str)] = &[
            (Some(false), "Inspector"),
            (Some(false), "Memory"),
            (Some(false), "Network"),
            (None, "no receiver (default)"),
        ];

        for (gate_value, label) in gate_states {
            // Construct the receiver (or None) matching this panel state.
            let gate_rx: Option<tokio::sync::watch::Receiver<bool>> =
                gate_value.map(|v| tokio::sync::watch::channel(v).1);

            // Evaluate the same gate expression used in forward_vm_events.
            let gate_open = gate_rx.as_ref().map(|rx| *rx.borrow()).unwrap_or(false);

            assert!(
                !gate_open,
                "Gate should be CLOSED for panel '{}' — got gate_open = {}",
                label, gate_open
            );
        }
    }

    /// Assert that `Flutter.RebuiltWidgets` events ARE dispatched when the
    /// panel gate is open (true), i.e. when the Performance panel is active.
    #[test]
    fn test_rebuilt_widgets_event_dispatched_when_performance_active() {
        // Simulate gate state for Performance panel: gate is open (true).
        let (gate_tx, gate_rx) = tokio::sync::watch::channel(true);
        let _ = gate_tx; // keep sender alive to prevent channel close

        // Evaluate the same gate expression used in forward_vm_events.
        let gate_open = Some(&gate_rx).map(|rx| *rx.borrow()).unwrap_or(false);

        assert!(
            gate_open,
            "Gate should be OPEN for Performance panel — got gate_open = {}",
            gate_open
        );

        // Verify that the gate correctly transitions from open to closed when
        // a panel switch fires (analogous to handle_switch_panel sending false).
        gate_tx.send(false).expect("send should succeed");
        let gate_open_after = *gate_rx.borrow();
        assert!(
            !gate_open_after,
            "Gate should be CLOSED after panel-switch signal — got {}",
            gate_open_after
        );
    }

    /// Verify that `try_send` error variants behave correctly — specifically that
    /// `TrySendError::Closed` is distinct from `TrySendError::Full`.
    ///
    /// This test documents the expected branching behavior for the L10 fix.
    #[test]
    fn test_try_send_error_variant_discrimination() {
        use tokio::sync::mpsc::error::TrySendError;

        // Create a channel with capacity 1.
        let (tx, rx) = tokio::sync::mpsc::channel::<u32>(1);

        // First send fills the slot.
        assert!(tx.try_send(1).is_ok(), "first send should succeed");

        // Second send should be Full.
        match tx.try_send(2) {
            Err(TrySendError::Full(_)) => {} // expected
            other => panic!("expected TrySendError::Full, got {:?}", other),
        }

        // Drop receiver — now sends should be Closed.
        drop(rx);
        match tx.try_send(3) {
            Err(TrySendError::Closed(_)) => {} // expected
            other => panic!("expected TrySendError::Closed, got {:?}", other),
        }
    }
}
