//! Message processing with session event routing
//!
//! Handles TEA message processing and routes JSON-RPC responses
//! to the appropriate RequestTracker for multi-session mode.

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

use tokio::sync::{mpsc, watch};

use crate::handler::Task;
use crate::message::Message;
use crate::session::SessionId;
use crate::state::AppState;
use crate::{handler, UpdateAction};
use fdemon_core::{DaemonEvent, DaemonMessage};
use fdemon_daemon::{
    parse_daemon_message, parse_devtools_serve_response, vm_service::VmRequestHandle, CommandSender,
};
use fdemon_dap::{adapter::DebugEvent as DapDebugEvent, DapServerHandle};

use super::actions::handle_action;

/// Process a message through the TEA update function
#[allow(clippy::too_many_arguments)]
pub fn process_message(
    state: &mut AppState,
    message: Message,
    msg_tx: &mpsc::Sender<Message>,
    session_tasks: &Arc<std::sync::Mutex<HashMap<SessionId, tokio::task::JoinHandle<()>>>>,
    shutdown_rx: &watch::Receiver<bool>,
    project_path: &Path,
    dap_server_handle: Arc<Mutex<Option<DapServerHandle>>>,
    vm_handle_for_dap: Arc<Mutex<Option<VmRequestHandle>>>,
    dap_debug_senders: Arc<Mutex<Vec<tokio::sync::mpsc::Sender<DapDebugEvent>>>>,
) {
    // Route JSON-RPC responses from SessionDaemon events to RequestTracker
    route_session_daemon_response(&message, state, msg_tx);

    // Process message through TEA update loop
    let mut msg = Some(message);
    while let Some(m) = msg {
        let result = handler::update(state, m);

        // Handle any action
        if let Some(action) = result.action {
            // For ReloadAllSessions, collect cmd_senders for all sessions
            let session_senders = get_session_cmd_senders_for_action(&action, state);
            let session_cmd_sender = get_session_cmd_sender(&action, state);

            // Capture the pre-hydration action for defense-in-depth failure
            // reporting. If hydration discards a FetchWidgetTree or
            // FetchLayoutData action (e.g. VM disconnected between handler and
            // hydration), we send a failure message so the loading spinner is
            // cleared.
            let pre_hydration_action = action.clone();

            // Hydrate actions that carry an optional VmRequestHandle with the
            // actual handle from the session. The handlers only return session_id;
            // we need the handle from AppState here before dispatching.
            let action = hydrate_start_performance_monitoring(action, state);
            let action = action.and_then(|a| hydrate_fetch_widget_tree(a, state));
            let action = action.and_then(|a| hydrate_fetch_layout_data(a, state));
            let action = action.and_then(|a| hydrate_toggle_overlay(a, state));
            let action = action.and_then(|a| hydrate_dispose_devtools_groups(a, state));
            let action = action.and_then(|a| hydrate_start_network_monitoring(a, state));
            let action = action.and_then(|a| hydrate_fetch_http_request_detail(a, state));
            let action = action.and_then(|a| hydrate_clear_http_profile(a, state));
            let action = action.and_then(|a| hydrate_send_daemon_command(a, state));

            if let Some(action) = action {
                handle_action(
                    action,
                    msg_tx.clone(),
                    session_cmd_sender,
                    session_senders,
                    session_tasks.clone(),
                    shutdown_rx.clone(),
                    project_path,
                    state.tool_availability.clone(),
                    dap_server_handle.clone(),
                    vm_handle_for_dap.clone(),
                    dap_debug_senders.clone(),
                );
            } else {
                // Hydration discarded the action. Send a failure message for
                // fetch actions so the loading spinner is not stuck forever.
                match &pre_hydration_action {
                    UpdateAction::FetchWidgetTree { session_id, .. } => {
                        let _ = msg_tx.try_send(Message::WidgetTreeFetchFailed {
                            session_id: *session_id,
                            error: "VM Service handle unavailable".to_string(),
                        });
                    }
                    UpdateAction::FetchLayoutData { session_id, .. } => {
                        let _ = msg_tx.try_send(Message::LayoutDataFetchFailed {
                            session_id: *session_id,
                            error: "VM Service handle unavailable".to_string(),
                        });
                    }
                    UpdateAction::FetchHttpRequestDetail { session_id, .. } => {
                        let _ = msg_tx.try_send(Message::VmServiceHttpRequestDetailFailed {
                            session_id: *session_id,
                            error: "VM Service handle unavailable".to_string(),
                        });
                    }
                    _ => {}
                }
            }
        }

        // Continue with follow-up message
        msg = result.message;
    }
}

/// Hydrate `StartPerformanceMonitoring` with the `VmRequestHandle` from the
/// session, returning `None` if the handle is unavailable (e.g. the VM has not
/// yet connected or has already disconnected) — in that case the action is
/// silently discarded.
///
/// All other action variants are returned unchanged.
fn hydrate_start_performance_monitoring(
    action: UpdateAction,
    state: &AppState,
) -> Option<UpdateAction> {
    if let UpdateAction::StartPerformanceMonitoring {
        session_id,
        handle,
        performance_refresh_ms,
        allocation_profile_interval_ms,
        mode,
    } = action
    {
        if handle.is_some() {
            // Already hydrated (shouldn't happen in normal flow, but safe).
            return Some(UpdateAction::StartPerformanceMonitoring {
                session_id,
                handle,
                performance_refresh_ms,
                allocation_profile_interval_ms,
                mode,
            });
        }
        // Extract the VM request handle from the session. If unavailable,
        // discard the action — there is nothing to poll yet.
        let vm_handle = state
            .session_manager
            .get(session_id)
            .and_then(|h| h.vm_request_handle.clone())?;
        return Some(UpdateAction::StartPerformanceMonitoring {
            session_id,
            handle: Some(vm_handle),
            performance_refresh_ms,
            allocation_profile_interval_ms,
            mode,
        });
    }
    Some(action)
}

/// Hydrate `FetchWidgetTree` with the `VmRequestHandle` from the session.
///
/// Returns `None` (discards the action) if the session has no active VM
/// connection, since there is nothing to query without one.
/// All other action variants are returned unchanged.
fn hydrate_fetch_widget_tree(action: UpdateAction, state: &AppState) -> Option<UpdateAction> {
    if let UpdateAction::FetchWidgetTree {
        session_id,
        vm_handle,
        tree_max_depth,
        fetch_timeout_secs,
    } = action
    {
        if vm_handle.is_some() {
            return Some(UpdateAction::FetchWidgetTree {
                session_id,
                vm_handle,
                tree_max_depth,
                fetch_timeout_secs,
            });
        }
        let handle = state
            .session_manager
            .get(session_id)
            .and_then(|h| h.vm_request_handle.clone())?;
        return Some(UpdateAction::FetchWidgetTree {
            session_id,
            vm_handle: Some(handle),
            tree_max_depth,
            fetch_timeout_secs,
        });
    }
    Some(action)
}

/// Hydrate `FetchLayoutData` with the `VmRequestHandle` from the session.
///
/// Returns `None` (discards the action) if the session has no active VM
/// connection, since there is nothing to query without one.
/// All other action variants are returned unchanged.
fn hydrate_fetch_layout_data(action: UpdateAction, state: &AppState) -> Option<UpdateAction> {
    if let UpdateAction::FetchLayoutData {
        session_id,
        node_id,
        vm_handle,
    } = action
    {
        if vm_handle.is_some() {
            return Some(UpdateAction::FetchLayoutData {
                session_id,
                node_id,
                vm_handle,
            });
        }
        let handle = state
            .session_manager
            .get(session_id)
            .and_then(|h| h.vm_request_handle.clone())?;
        return Some(UpdateAction::FetchLayoutData {
            session_id,
            node_id,
            vm_handle: Some(handle),
        });
    }
    Some(action)
}

/// Hydrate `ToggleOverlay` with the `VmRequestHandle` from the session.
///
/// Returns `None` (discards the action) if the session has no active VM
/// connection. All other action variants are returned unchanged.
fn hydrate_toggle_overlay(action: UpdateAction, state: &AppState) -> Option<UpdateAction> {
    if let UpdateAction::ToggleOverlay {
        session_id,
        extension,
        vm_handle,
    } = action
    {
        if vm_handle.is_some() {
            return Some(UpdateAction::ToggleOverlay {
                session_id,
                extension,
                vm_handle,
            });
        }
        let handle = state
            .session_manager
            .get(session_id)
            .and_then(|h| h.vm_request_handle.clone())?;
        return Some(UpdateAction::ToggleOverlay {
            session_id,
            extension,
            vm_handle: Some(handle),
        });
    }
    Some(action)
}

/// Hydrate `DisposeDevToolsGroups` with the `VmRequestHandle` from the session.
///
/// Unlike the fetch hydration functions, this one does **not** return `None`
/// when the handle is unavailable. If the VM is not connected there is nothing
/// to dispose, so the action is silently discarded by returning `None`.
/// All other action variants are returned unchanged.
fn hydrate_dispose_devtools_groups(action: UpdateAction, state: &AppState) -> Option<UpdateAction> {
    if let UpdateAction::DisposeDevToolsGroups {
        session_id,
        vm_handle,
    } = action
    {
        if vm_handle.is_some() {
            // Already hydrated.
            return Some(UpdateAction::DisposeDevToolsGroups {
                session_id,
                vm_handle,
            });
        }
        // If no VM handle is available (VM disconnected or not yet connected),
        // silently discard — there is nothing to dispose.
        let handle = state
            .session_manager
            .get(session_id)
            .and_then(|h| h.vm_request_handle.clone())?;
        return Some(UpdateAction::DisposeDevToolsGroups {
            session_id,
            vm_handle: Some(handle),
        });
    }
    Some(action)
}

/// Hydrate `StartNetworkMonitoring` with the `VmRequestHandle` from the session.
///
/// Returns `None` (discards the action) if the session has no active VM
/// connection, since there is nothing to poll without one.
/// All other action variants are returned unchanged.
fn hydrate_start_network_monitoring(
    action: UpdateAction,
    state: &AppState,
) -> Option<UpdateAction> {
    if let UpdateAction::StartNetworkMonitoring {
        session_id,
        handle,
        poll_interval_ms,
        mode,
    } = action
    {
        if handle.is_some() {
            // Already hydrated.
            return Some(UpdateAction::StartNetworkMonitoring {
                session_id,
                handle,
                poll_interval_ms,
                mode,
            });
        }
        let vm_handle = state
            .session_manager
            .get(session_id)
            .and_then(|h| h.vm_request_handle.clone())?;
        return Some(UpdateAction::StartNetworkMonitoring {
            session_id,
            handle: Some(vm_handle),
            poll_interval_ms,
            mode,
        });
    }
    Some(action)
}

/// Hydrate `FetchHttpRequestDetail` with the `VmRequestHandle` from the session.
///
/// Returns `None` (discards the action) if the session has no active VM
/// connection. All other action variants are returned unchanged.
fn hydrate_fetch_http_request_detail(
    action: UpdateAction,
    state: &AppState,
) -> Option<UpdateAction> {
    if let UpdateAction::FetchHttpRequestDetail {
        session_id,
        request_id,
        vm_handle,
    } = action
    {
        if vm_handle.is_some() {
            // Already hydrated.
            return Some(UpdateAction::FetchHttpRequestDetail {
                session_id,
                request_id,
                vm_handle,
            });
        }
        let handle = state
            .session_manager
            .get(session_id)
            .and_then(|h| h.vm_request_handle.clone())?;
        return Some(UpdateAction::FetchHttpRequestDetail {
            session_id,
            request_id,
            vm_handle: Some(handle),
        });
    }
    Some(action)
}

/// Hydrate `ClearHttpProfile` with the `VmRequestHandle` from the session.
///
/// Returns `None` (discards the action) if the session has no active VM
/// connection. All other action variants are returned unchanged.
fn hydrate_clear_http_profile(action: UpdateAction, state: &AppState) -> Option<UpdateAction> {
    if let UpdateAction::ClearHttpProfile {
        session_id,
        vm_handle,
    } = action
    {
        if vm_handle.is_some() {
            // Already hydrated.
            return Some(UpdateAction::ClearHttpProfile {
                session_id,
                vm_handle,
            });
        }
        // Silently discard if VM is not connected — nothing to clear on VM side.
        let handle = state
            .session_manager
            .get(session_id)
            .and_then(|h| h.vm_request_handle.clone())?;
        return Some(UpdateAction::ClearHttpProfile {
            session_id,
            vm_handle: Some(handle),
        });
    }
    Some(action)
}

/// Prefix used for `devtools.serve` JSON-RPC request IDs.
///
/// Outgoing requests carry `"devtools-serve-{session_id}"` as a string ID
/// (set in `handler::session::maybe_serve_devtools`). The daemon echoes the
/// same ID in its response, which we match here to route the response into
/// a `Message::DevToolsServed` / `Message::DevToolsServeFailed`.
pub(crate) const DEVTOOLS_SERVE_REQUEST_PREFIX: &str = "devtools-serve-";

/// Route JSON-RPC responses for multi-session daemon events.
///
/// Two routing strategies live here:
///
/// 1. **Numeric-ID responses** — registered via `RequestTracker` (the
///    standard `CommandSender::send`/`send_with_timeout` path). Routed to
///    the tracker so the awaiting future resolves.
/// 2. **String-ID `devtools.serve` responses** — sent via
///    `send_fire_and_forget` with an explicit string ID. The tracker has no
///    pending entry for these, so we parse the response payload here and
///    forward it as a synthetic `Message::DevToolsServed` /
///    `Message::DevToolsServeFailed` on `msg_tx`. Without this, the
///    response would be silently dropped and `devtools_serve_pending` would
///    remain `true` forever, blocking any future fallback dispatch.
fn route_session_daemon_response(
    message: &Message,
    state: &AppState,
    msg_tx: &mpsc::Sender<Message>,
) {
    if let Message::SessionDaemon {
        session_id,
        event: DaemonEvent::Stdout(ref line),
    } = message
    {
        if let Some(DaemonMessage::Response { id, result, error }) = parse_daemon_message(line) {
            // String-ID devtools.serve response → synthesize a Message.
            if let Some(id_str) = id.as_str() {
                if id_str.starts_with(DEVTOOLS_SERVE_REQUEST_PREFIX) {
                    if let Some(m) = synthesize_devtools_serve_message(
                        *session_id,
                        result.as_ref(),
                        error.as_ref(),
                    ) {
                        // Best-effort: a full channel means the runtime
                        // is overwhelmed; logging is enough.
                        if let Err(e) = msg_tx.try_send(m) {
                            tracing::warn!(
                                session_id = *session_id,
                                error = %e,
                                "Failed to forward devtools.serve response"
                            );
                        }
                    }
                    return;
                }
            }

            // Numeric-ID response → standard tracker path.
            if let Some(handle) = state.session_manager.get(*session_id) {
                if let Some(ref sender) = handle.cmd_sender {
                    if let Some(id_num) = id.as_u64() {
                        let tracker = sender.tracker().clone();
                        tokio::spawn(async move {
                            tracker.handle_response(id_num, result, error).await;
                        });
                    }
                }
            }
        }
    }
}

/// Convert a parsed `devtools.serve` response payload into a follow-up
/// `Message::DevToolsServed` or `Message::DevToolsServeFailed` for the given
/// session. Returns `None` when the response is malformed (no result + no
/// error).
///
/// Extracted as a pure function so it can be unit-tested without an async
/// runtime, channels, or a full `AppState`.
fn synthesize_devtools_serve_message(
    session_id: SessionId,
    result: Option<&serde_json::Value>,
    error: Option<&serde_json::Value>,
) -> Option<Message> {
    let parsed = parse_devtools_serve_response(result, error)?;
    match parsed {
        DaemonMessage::DevToolsServed { base_url, .. } => Some(Message::DevToolsServed {
            session_id,
            base_url,
        }),
        DaemonMessage::DevToolsServeFailed { reason } => {
            Some(Message::DevToolsServeFailed { session_id, reason })
        }
        _ => None,
    }
}

/// Get session-specific command sender for SpawnTask actions
fn get_session_cmd_sender(action: &UpdateAction, state: &AppState) -> Option<CommandSender> {
    if let UpdateAction::SpawnTask(task) = action {
        let session_id = match task {
            Task::Reload { session_id, .. } => *session_id,
            Task::Restart { session_id, .. } => *session_id,
            Task::Stop { session_id, .. } => *session_id,
        };
        return state
            .session_manager
            .get(session_id)
            .and_then(|h| h.cmd_sender.clone());
    }
    None
}

/// Hydrate `SendDaemonCommand` with the `CommandSender` from the session.
///
/// If the session has no attached `CommandSender` (process not yet spawned or
/// already exited), the action is silently discarded by returning `None`.
/// All other action variants are returned unchanged.
fn hydrate_send_daemon_command(action: UpdateAction, state: &AppState) -> Option<UpdateAction> {
    if let UpdateAction::SendDaemonCommand {
        session_id,
        command,
        cmd_sender,
    } = action
    {
        if cmd_sender.is_some() {
            // Already hydrated.
            return Some(UpdateAction::SendDaemonCommand {
                session_id,
                command,
                cmd_sender,
            });
        }
        // Fetch the cmd_sender from the session. If unavailable (process not yet
        // attached or already exited), discard the action silently.
        let sender = state
            .session_manager
            .get(session_id)
            .and_then(|h| h.cmd_sender.clone())?;
        return Some(UpdateAction::SendDaemonCommand {
            session_id,
            command,
            cmd_sender: Some(sender),
        });
    }
    Some(action)
}

/// Get command senders for all sessions in ReloadAllSessions action
fn get_session_cmd_senders_for_action(
    action: &UpdateAction,
    state: &AppState,
) -> Vec<(SessionId, String, CommandSender)> {
    if let UpdateAction::ReloadAllSessions { sessions } = action {
        sessions
            .iter()
            .filter_map(|(session_id, app_id)| {
                state
                    .session_manager
                    .get(*session_id)
                    .and_then(|h| h.cmd_sender.clone())
                    .map(|sender| (*session_id, app_id.clone(), sender))
            })
            .collect()
    } else {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const SID: SessionId = 7;

    #[test]
    fn synthesize_serves_message_on_success() {
        let result = json!({"host": "127.0.0.1", "port": 9100});
        let msg = synthesize_devtools_serve_message(SID, Some(&result), None).unwrap();
        match msg {
            Message::DevToolsServed {
                session_id,
                base_url,
            } => {
                assert_eq!(session_id, SID);
                assert_eq!(base_url, "http://127.0.0.1:9100");
            }
            other => panic!("expected DevToolsServed, got {other:?}"),
        }
    }

    #[test]
    fn synthesize_serves_failed_on_method_not_found() {
        let error = json!({"code": -32601, "message": "Method not found"});
        let msg = synthesize_devtools_serve_message(SID, None, Some(&error)).unwrap();
        match msg {
            Message::DevToolsServeFailed { session_id, reason } => {
                assert_eq!(session_id, SID);
                assert!(reason.contains("Method not supported"), "got: {reason}");
            }
            other => panic!("expected DevToolsServeFailed, got {other:?}"),
        }
    }

    #[test]
    fn synthesize_serves_failed_on_null_host_port() {
        let result = json!({"host": null, "port": null});
        let msg = synthesize_devtools_serve_message(SID, Some(&result), None).unwrap();
        assert!(matches!(msg, Message::DevToolsServeFailed { .. }));
    }

    #[test]
    fn synthesize_serves_failed_on_unsafe_host() {
        let result = json!({"host": "127.0.0.1@evil.com", "port": 9100});
        let msg = synthesize_devtools_serve_message(SID, Some(&result), None).unwrap();
        assert!(matches!(msg, Message::DevToolsServeFailed { .. }));
    }

    #[test]
    fn synthesize_returns_none_on_malformed_response() {
        // No result and no error → malformed.
        assert!(synthesize_devtools_serve_message(SID, None, None).is_none());
    }

    #[test]
    fn devtools_serve_request_prefix_matches_session_format() {
        // Guards against the prefix in process.rs drifting from the format used
        // by maybe_serve_devtools — a silent break would orphan the response
        // path again. If the prefix changes, both sites must change.
        let request_id = format!("{}{}", DEVTOOLS_SERVE_REQUEST_PREFIX, 42);
        assert!(request_id.starts_with(DEVTOOLS_SERVE_REQUEST_PREFIX));
        assert_eq!(request_id, "devtools-serve-42");
    }
}
